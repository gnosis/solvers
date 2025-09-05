use {
    crate::{
        domain::{
            auction,
            dex,
            eth::{self, TokenAddress},
            order::{self, Side},
        },
        infra::{blockchain, config::dex::balancer::file::ApiVersion, dex::balancer::dto::Chain},
        util,
    },
    contracts::ethcontract::I256,
    ethereum_types::U256,
    ethrpc::block_stream::CurrentBlockWatcher,
    num::ToPrimitive,
    std::{
        ops::Add,
        sync::atomic::{self, AtomicU64},
        time::Duration,
    },
    tracing::Instrument,
};

mod dto;
mod v2;
mod v3;

/// Bindings to the Balancer Smart Order Router (SOR) API.
pub struct Sor {
    client: super::Client,
    endpoint: reqwest::Url,
    v2_vault: Option<v2::Vault>,
    v3_batch_router: Option<v3::Router>,
    queries: Option<v2::Queries>,
    permit2: v3::Permit2,
    settlement: eth::ContractAddress,
    chain_id: Chain,
    query_batch_swap: bool,
    web3: ethrpc::Web3,
}

pub struct Config {
    /// Stream that yields every new block.
    pub block_stream: Option<CurrentBlockWatcher>,

    /// The URL for the Balancer SOR API.
    pub endpoint: reqwest::Url,

    /// The RPC URL for on-chain queries.
    pub rpc_url: reqwest::Url,

    /// The address of the Balancer V2 Vault contract. For V2, it's used as both
    /// the spender and router.
    pub vault: Option<eth::ContractAddress>,

    /// The address of the Balancer V3 BatchRouter contract.
    /// Not supported on some chains.
    pub v3_batch_router: Option<eth::ContractAddress>,

    /// The address of the Balancer Queries contract for on-chain swap queries.
    pub queries: Option<eth::ContractAddress>,

    /// The address of the Permit2 contract.
    pub permit2: eth::ContractAddress,

    /// The address of the Settlement contract.
    pub settlement: eth::ContractAddress,

    /// For which chain the solver is configured.
    pub chain_id: eth::ChainId,

    /// Whether to run `queryBatchSwap` to update the return amount with most
    /// up-to-date on-chain values.
    pub query_batch_swap: bool,
}

impl Sor {
    /// An approximate gas an individual Balancer swap uses.
    ///
    /// This value was determined heuristically using a Dune query that has been
    /// lost to time... See <https://github.com/cowprotocol/services/pull/171>.
    const GAS_PER_SWAP: u64 = 88_892;

    pub fn new(config: Config) -> Result<Self, Error> {
        Ok(Self {
            client: super::Client::new(Default::default(), config.block_stream),
            endpoint: config.endpoint,
            v2_vault: config.vault.map(v2::Vault::new),
            v3_batch_router: config.v3_batch_router.map(v3::Router::new),
            queries: config.queries.map(v2::Queries::new),
            permit2: v3::Permit2::new(config.permit2),
            settlement: config.settlement,
            chain_id: Chain::from_domain(config.chain_id)?,
            query_batch_swap: config.query_batch_swap,
            web3: blockchain::rpc(&config.rpc_url),
        })
    }

    pub async fn swap(
        &self,
        order: &dex::Order,
        slippage: &dex::Slippage,
        tokens: &auction::Tokens,
    ) -> Result<dex::Swap, Error> {
        // Receiving this error indicates that V2 is now supported on the current chain.
        let Some(v2_vault) = &self.v2_vault else {
            return Err(Error::DisabledApiVersion(ApiVersion::V2));
        };
        let query = dto::Query::from_domain(
            order,
            tokens,
            slippage,
            self.chain_id,
            self.settlement,
            self.query_batch_swap,
            // 2 minutes from now
            chrono::Utc::now()
                .add(Duration::from_secs(120))
                .timestamp()
                .to_u64(),
        )?;
        let quote = {
            // Set up a tracing span to make debugging of API requests easier.
            // Historically, debugging API requests to external DEXs was a bit
            // of a headache.
            static ID: AtomicU64 = AtomicU64::new(0);
            let id = ID.fetch_add(1, atomic::Ordering::Relaxed);
            self.quote(&query)
                .instrument(tracing::trace_span!("quote", id = %id))
                .await?
        };

        if quote.is_empty() {
            return Err(Error::NotFound);
        }

        // Execute on-chain query if BalancerQueries contract is available to get
        // up-to-date amounts, otherwise use the SOR quote amounts
        let (updated_swap_amount, updated_return_amount) =
            match self.query_swap(order, &quote).await {
                Ok(on_chain_amounts) => {
                    tracing::debug!(
                        "Using on-chain amounts: swap={}, return={}",
                        on_chain_amounts.swap_amount,
                        on_chain_amounts.return_amount
                    );
                    (on_chain_amounts.swap_amount, on_chain_amounts.return_amount)
                }
                Err(e) => {
                    tracing::warn!("On-chain query failed: {:?}, using SOR quote amounts", e);
                    (quote.swap_amount_raw, quote.return_amount_raw)
                }
            };

        let (input, output) = match order.side {
            order::Side::Buy => (updated_return_amount, updated_swap_amount),
            order::Side::Sell => (updated_swap_amount, updated_return_amount),
        };

        let (max_input, min_output) = match order.side {
            order::Side::Buy => (slippage.add(input), output),
            order::Side::Sell => (input, slippage.sub(output)),
        };

        let gas = U256::from(quote.swaps.len()) * Self::GAS_PER_SWAP;
        let (spender, calls) = match quote.protocol_version {
            dto::ProtocolVersion::V2 => (
                v2_vault.address(),
                self.encode_v2_swap(order, &quote, max_input, min_output, v2_vault)?,
            ),
            dto::ProtocolVersion::V3 => {
                // In Balancer v3, the spender must be the Permit2 contract, as it's the one
                // doing the transfer of funds from the settlement
                (
                    self.permit2.address(),
                    self.encode_v3_swap(order, &quote, max_input)?,
                )
            }
        };

        Ok(dex::Swap {
            calls,
            input: eth::Asset {
                token: eth::TokenAddress(quote.token_in),
                amount: input,
            },
            output: eth::Asset {
                token: eth::TokenAddress(quote.token_out),
                amount: output,
            },
            allowance: dex::Allowance {
                spender,
                amount: dex::Amount::new(max_input),
            },
            gas: eth::Gas(gas),
        })
    }

    fn encode_v2_swap(
        &self,
        order: &dex::Order,
        quote: &dto::Quote,
        max_input: U256,
        min_output: U256,
        v2_vault: &v2::Vault,
    ) -> Result<Vec<dex::Call>, Error> {
        let (kind, swaps, funds) = self.build_v2_swap_data(order, quote)?;
        let assets = quote.token_addresses.clone();
        let limits = quote
            .token_addresses
            .iter()
            .map(|token| {
                if *token == quote.token_in {
                    // Use positive swap limit for sell amounts (that is, maximum
                    // amount that can be transferred in).
                    I256::try_from(max_input).unwrap_or_default()
                } else if *token == quote.token_out {
                    I256::try_from(min_output)
                        .unwrap_or_default()
                        .checked_neg()
                        .expect("positive integer can't overflow negation")
                } else {
                    I256::zero()
                }
            })
            .collect();

        Ok(v2_vault.batch_swap(kind, swaps, assets, funds, limits))
    }

    fn encode_v3_swap(
        &self,
        order: &dex::Order,
        quote: &dto::Quote,
        max_input: U256,
    ) -> Result<Vec<dex::Call>, Error> {
        // Receiving this error indicates that V3 is now supported on the current chain.
        let Some(v3_batch_router) = &self.v3_batch_router else {
            return Err(Error::DisabledApiVersion(ApiVersion::V3));
        };
        let paths = self.build_v3_swap_data(quote)?;

        Ok(match order.side {
            Side::Buy => v3_batch_router.swap_exact_amount_out(
                paths,
                &self.permit2,
                quote.token_in,
                max_input,
            ),
            Side::Sell => v3_batch_router.swap_exact_amount_in(
                paths,
                &self.permit2,
                quote.token_in,
                max_input,
            ),
        })
    }

    /// Build common V2 swap data (kind, swaps, funds) from order and quote
    fn build_v2_swap_data(
        &self,
        order: &dex::Order,
        quote: &dto::Quote,
    ) -> Result<(v2::SwapKind, Vec<v2::Swap>, v2::Funds), Error> {
        // Determine swap kind based on order side
        let kind = match order.side {
            order::Side::Sell => v2::SwapKind::GivenIn,
            order::Side::Buy => v2::SwapKind::GivenOut,
        };

        // Convert quote swaps to v2::Swap format
        let swaps = quote
            .swaps
            .iter()
            .map(|swap| {
                Ok(v2::Swap {
                    pool_id: swap.pool_id.as_v2()?,
                    asset_in_index: swap.asset_in_index.into(),
                    asset_out_index: swap.asset_out_index.into(),
                    amount: swap.amount,
                    user_data: swap.user_data.clone(),
                })
            })
            .collect::<Result<_, Error>>()?;

        // Create funds structure
        let funds = v2::Funds {
            sender: self.settlement.0,
            from_internal_balance: false,
            recipient: self.settlement.0,
            to_internal_balance: false,
        };

        Ok((kind, swaps, funds))
    }

    /// Build common V3 swap data (paths) from quote
    fn build_v3_swap_data(&self, quote: &dto::Quote) -> Result<Vec<v3::SwapPath>, Error> {
        quote
            .paths
            .iter()
            .map(|path| {
                Ok(v3::SwapPath {
                    token_in: path
                        .tokens
                        .first()
                        .map(|t| t.address)
                        .ok_or_else(|| Error::InvalidPath)?,
                    input_amount_raw: path.input_amount_raw,
                    output_amount_raw: path.output_amount_raw,
                    // A path step consists of 1 item of 3 different arrays at the correct
                    // index. `tokens` contains 1 item more where the first one needs
                    // to be skipped.
                    steps: path
                        .tokens
                        .iter()
                        .skip(1)
                        .zip(path.is_buffer.iter())
                        .zip(path.pools.iter())
                        .map(|((token_out, is_buffer), pool)| {
                            Ok(v3::SwapPathStep {
                                pool: pool.as_v3()?,
                                token_out: token_out.address,
                                is_buffer: *is_buffer,
                            })
                        })
                        .collect::<Result<_, Error>>()?,
                })
            })
            .collect::<Result<_, Error>>()
    }

    /// Execute on-chain query to get updated swap amounts for both V2 and V3
    async fn query_swap(
        &self,
        order: &dex::Order,
        quote: &dto::Quote,
    ) -> Result<OnChainAmounts, Error> {
        match quote.protocol_version {
            dto::ProtocolVersion::V2 => self.query_swap_v2(order, quote).await,
            dto::ProtocolVersion::V3 => self.query_swap_v3(order, quote).await,
        }
    }

    /// Execute on-chain query for V2 using BalancerQueries contract
    async fn query_swap_v2(
        &self,
        order: &dex::Order,
        quote: &dto::Quote,
    ) -> Result<OnChainAmounts, Error> {
        let (kind, swaps, funds) = self.build_v2_swap_data(order, quote)?;
        let assets = quote.token_addresses.clone();

        // Execute the on-chain query
        let asset_deltas = self
            .queries
            .as_ref()
            .ok_or(Error::InvalidPath)?
            .execute_query_batch_swap(&self.web3, kind, swaps, assets, funds)
            .await
            .map_err(|_e| Error::InvalidPath)?;

        // Parse the result - asset_deltas corresponds to the assets array
        // We need to find the indices for token_in and token_out in the quote's
        // token_addresses
        if asset_deltas.len() != quote.token_addresses.len() {
            return Err(Error::InvalidPath);
        }

        let token_in_index = quote
            .token_addresses
            .iter()
            .position(|&addr| addr == order.sell.0)
            .ok_or(Error::InvalidPath)?;
        let token_out_index = quote
            .token_addresses
            .iter()
            .position(|&addr| addr == order.buy.0)
            .ok_or(Error::InvalidPath)?;

        // Get the deltas for token_in and token_out (convert to absolute values)
        let amount_in = U256::from_dec_str(&asset_deltas[token_in_index].abs().to_string())
            .map_err(|_| Error::InvalidPath)?;
        let amount_out = U256::from_dec_str(&asset_deltas[token_out_index].abs().to_string())
            .map_err(|_| Error::InvalidPath)?;

        Ok(OnChainAmounts {
            swap_amount: amount_in,
            return_amount: amount_out,
        })
    }

    /// Execute on-chain query for V3 using BalancerV3BatchRouter contract
    async fn query_swap_v3(
        &self,
        order: &dex::Order,
        quote: &dto::Quote,
    ) -> Result<OnChainAmounts, Error> {
        // Get the V3 batch router (it should be available for V3 quotes)
        let v3_batch_router = self.v3_batch_router.as_ref().ok_or(Error::InvalidPath)?;

        let paths = self.build_v3_swap_data(quote)?;

        // Execute the appropriate query based on order side
        let result = match order.side {
            order::Side::Sell => {
                // For sell orders, we know the input amount, query for output amount
                v3_batch_router
                    .query_swap_exact_amount_in(&self.web3, paths)
                    .await
                    .map_err(|_e| Error::InvalidPath)?
            }
            order::Side::Buy => {
                // For buy orders, we know the output amount, query for input amount
                v3_batch_router
                    .query_swap_exact_amount_out(&self.web3, paths)
                    .await
                    .map_err(|_e| Error::InvalidPath)?
            }
        };

        // For V3, the result is a single amount
        // We need to determine which is the input and which is the output based on
        // order side
        let (swap_amount, return_amount) = match order.side {
            order::Side::Sell => {
                // For sell orders: swap_amount is the input (known), return_amount is the
                // output (queried)
                (quote.swap_amount_raw, result)
            }
            order::Side::Buy => {
                // For buy orders: swap_amount is the input (queried), return_amount is the
                // output (known)
                (result, quote.return_amount_raw)
            }
        };

        Ok(OnChainAmounts {
            swap_amount,
            return_amount,
        })
    }

    async fn quote(&self, query: &dto::Query<'_>) -> Result<dto::Quote, Error> {
        let response = util::http::roundtrip!(
            <dto::GetSwapPathsResponse, util::serialize::Never>;
            self.client
                .request(reqwest::Method::POST, self.endpoint.clone())
                .json(query)
        )
        .await?;
        Ok(response.data.sor_get_swap_paths)
    }
}

/// Result from on-chain query containing updated swap amounts
#[derive(Debug, Clone)]
struct OnChainAmounts {
    swap_amount: U256,
    return_amount: U256,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no valid swap interaction could be found")]
    NotFound,
    #[error("rate limited")]
    RateLimited,
    #[error(transparent)]
    Http(util::http::Error),
    #[error("unsupported chain: {0:?}")]
    UnsupportedChainId(eth::ChainId),
    #[error("disabled API version: {0:?}")]
    DisabledApiVersion(ApiVersion),
    #[error("decimals are missing for the swapped token: {0:?}")]
    MissingDecimals(TokenAddress),
    #[error("invalid pool id format")]
    InvalidPoolIdFormat,
    #[error("invalid path")]
    InvalidPath,
}

impl From<util::http::RoundtripError<util::serialize::Never>> for Error {
    fn from(err: util::http::RoundtripError<util::serialize::Never>) -> Self {
        match err {
            util::http::RoundtripError::Http(util::http::Error::Status(status_code, _))
                if status_code.as_u16() == 429 =>
            {
                Self::RateLimited
            }
            other_err => Self::Http(other_err.into()),
        }
    }
}
