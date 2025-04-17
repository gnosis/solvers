use {
    crate::{
        domain::{
            auction,
            dex,
            eth::{self, TokenAddress},
            order::{self, Side},
        },
        infra::dex::balancer::dto::Chain,
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
    v2_vault: v2::Vault,
    v3_batch_router: v3::Router,
    permit2: v3::Permit2,
    settlement: eth::ContractAddress,
    chain_id: Chain,
    query_batch_swap: bool,
}

pub struct Config {
    /// Stream that yields every new block.
    pub block_stream: Option<CurrentBlockWatcher>,

    /// The URL for the Balancer SOR API.
    pub endpoint: reqwest::Url,

    /// The address of the Balancer V2 Vault contract. For V2, it's used as both
    /// the spender and router.
    pub vault: eth::ContractAddress,

    /// The address of the Balancer V3 BatchRouter contract.
    pub v3_batch_router: eth::ContractAddress,

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
            v2_vault: v2::Vault::new(config.vault),
            v3_batch_router: v3::Router::new(config.v3_batch_router),
            permit2: v3::Permit2::new(config.permit2),
            settlement: config.settlement,
            chain_id: Chain::from_domain(config.chain_id)?,
            query_batch_swap: config.query_batch_swap,
        })
    }

    pub async fn swap(
        &self,
        order: &dex::Order,
        slippage: &dex::Slippage,
        tokens: &auction::Tokens,
    ) -> Result<dex::Swap, Error> {
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

        let (input, output) = match order.side {
            order::Side::Buy => (quote.return_amount_raw, quote.swap_amount_raw),
            order::Side::Sell => (quote.swap_amount_raw, quote.return_amount_raw),
        };

        let (max_input, min_output) = match order.side {
            order::Side::Buy => (slippage.add(input), output),
            order::Side::Sell => (input, slippage.sub(output)),
        };

        let gas = U256::from(quote.swaps.len()) * Self::GAS_PER_SWAP;
        let (spender, calls) = match quote.protocol_version {
            dto::ProtocolVersion::V2 => (
                self.v2_vault.address(),
                self.encode_v2_swap(order, &quote, max_input, min_output)?,
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
    ) -> Result<Vec<dex::Call>, Error> {
        let kind = match order.side {
            order::Side::Sell => v2::SwapKind::GivenIn,
            order::Side::Buy => v2::SwapKind::GivenOut,
        } as _;
        let swaps = quote
            .swaps
            .iter()
            .map(|swap| {
                Ok(v2::SwapV2 {
                    pool_id: swap.pool_id.as_v2()?,
                    asset_in_index: swap.asset_in_index.into(),
                    asset_out_index: swap.asset_out_index.into(),
                    amount: swap.amount,
                    user_data: swap.user_data.clone(),
                })
            })
            .collect::<Result<_, Error>>()?;
        let assets = quote.token_addresses.clone();
        let funds = v2::Funds {
            sender: self.settlement.0,
            from_internal_balance: false,
            recipient: self.settlement.0,
            to_internal_balance: false,
        };
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

        Ok(self
            .v2_vault
            .batch_swap_v2(kind, swaps, assets, funds, limits))
    }

    fn encode_v3_swap(
        &self,
        order: &dex::Order,
        quote: &dto::Quote,
        max_input: U256,
    ) -> Result<Vec<dex::Call>, Error> {
        let paths = quote
            .paths
            .iter()
            .map(|p| {
                Ok(v3::SwapPath {
                    token_in: p.tokens[0].address,
                    input_amount_raw: p.input_amount_raw,
                    output_amount_raw: p.output_amount_raw,
                    // A path step consists of 1 item of 3 different arrays at the correct
                    // index. `tokens` contains 1 item more where the first one needs
                    // to be skipped.
                    steps: p
                        .tokens
                        .iter()
                        .skip(1)
                        .zip(p.is_buffer.iter())
                        .zip(p.pools.iter())
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
            .collect::<Result<_, Error>>()?;

        Ok(match order.side {
            Side::Buy => self.v3_batch_router.swap_exact_amount_out(
                paths,
                &self.permit2,
                &self.v3_batch_router,
                quote.token_in,
                max_input,
            ),
            Side::Sell => self.v3_batch_router.swap_exact_amount_in(
                paths,
                &self.permit2,
                &self.v3_batch_router,
                quote.token_in,
                max_input,
            ),
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
    #[error("decimals are missing for the swapped token: {0:?}")]
    MissingDecimals(TokenAddress),
    #[error("invalid pool id format")]
    InvalidPoolIdFormat,
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
