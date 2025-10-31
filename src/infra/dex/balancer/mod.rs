use {
    crate::{
        domain::{
            auction,
            dex,
            eth::{self, TokenAddress},
            order::{self, Side},
        },
        infra::{config::dex::balancer::file::ApiVersion, dex::balancer::dto::Chain},
        util,
    },
    alloy::primitives::{Address, Bytes, FixedBytes, I256, U256},
    contracts::alloy::{
        BalancerV2Vault::IVault::{BatchSwapStep, FundManagement},
        BalancerV3BatchRouter::IBatchRouter::{
            SwapPathExactAmountIn,
            SwapPathExactAmountOut,
            SwapPathStep,
        },
    },
    ethrpc::{
        AlloyProvider,
        alloy::conversions::{IntoAlloy, IntoLegacy},
        block_stream::CurrentBlockWatcher,
    },
    itertools::Itertools,
    std::sync::{atomic, atomic::AtomicU64},
    tracing::Instrument,
};

pub mod dto;
pub mod query_swap_provider;
mod v2;
mod v3;

// Re-export query swap provider types
pub use query_swap_provider::{OnChainQuerySwapProvider, QuerySwapProvider};

/// Bindings to the Balancer Smart Order Router (SOR) API.
pub struct Sor {
    client: super::Client,
    endpoint: reqwest::Url,
    v2_vault: Option<v2::Vault>,
    v3_batch_router: Option<v3::Router>,
    permit2: v3::Permit2,
    settlement: Address,
    chain_id: Chain,
    query_swap_provider: Box<dyn QuerySwapProvider>,
}

pub struct Config {
    /// Stream that yields every new block.
    pub block_stream: Option<CurrentBlockWatcher>,

    /// The URL for the Balancer SOR API.
    pub endpoint: reqwest::Url,

    /// The address of the Balancer V2 Vault contract. For V2, it's used as both
    /// the spender and router.
    pub vault: Option<Address>,

    /// The address of the Balancer V3 BatchRouter contract.
    /// Not supported on some chains.
    pub v3_batch_router: Option<Address>,

    /// The address of the Balancer Queries contract for on-chain swap queries.
    pub queries: Option<Address>,

    /// The address of the Permit2 contract.
    pub permit2: Address,

    /// The address of the Settlement contract.
    pub settlement: Address,

    /// For which chain the solver is configured.
    pub chain_id: eth::ChainId,
}

impl Sor {
    /// An approximate gas an individual Balancer swap uses.
    ///
    /// This value was determined heuristically using a Dune query that has been
    /// lost to time... See <https://github.com/cowprotocol/services/pull/171>.
    const GAS_PER_SWAP: u64 = 88_892;

    pub fn new(
        config: Config,
        alloy_provider: AlloyProvider,
        query_swap_provider: Box<dyn QuerySwapProvider>,
    ) -> Result<Self, Error> {
        Ok(Self {
            client: super::Client::new(Default::default(), config.block_stream),
            endpoint: config.endpoint,
            v2_vault: config.vault.map(v2::Vault::new),
            v3_batch_router: config
                .v3_batch_router
                .map(|addr| v3::Router::new(addr, alloy_provider)),
            permit2: v3::Permit2::new(config.permit2),
            settlement: config.settlement,
            chain_id: Chain::from_domain(config.chain_id)?,
            query_swap_provider,
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
        let query = dto::Query::from_domain(order, tokens, self.chain_id)?;
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
            match self.query_swap_provider.query_swap(order, &quote).await {
                Ok(on_chain_amounts) => {
                    tracing::debug!(
                        swap = ?on_chain_amounts.swap_amount,
                        return = ?on_chain_amounts.return_amount,
                        "Using on-chain amounts"
                    );
                    (
                        on_chain_amounts.swap_amount.into_legacy(),
                        on_chain_amounts.return_amount.into_legacy(),
                    )
                }
                Err(e) => {
                    tracing::warn!(
                        error = ?e,
                        "On-chain query failed, using SOR quote amounts"
                    );
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

        let gas = U256::from(quote.swaps.len()) * U256::from(Self::GAS_PER_SWAP);
        let (spender, calls) = match quote.protocol_version {
            dto::ProtocolVersion::V2 => (
                v2_vault.address(),
                self.encode_v2_swap(
                    order,
                    &quote,
                    max_input.into_alloy(),
                    min_output.into_alloy(),
                    v2_vault,
                )?,
            ),
            dto::ProtocolVersion::V3 => {
                // In Balancer v3, the spender must be the Permit2 contract, as it's the one
                // doing the transfer of funds from the settlement
                (
                    self.permit2.address(),
                    self.encode_v3_swap(order, &quote, max_input.into_alloy(), slippage)?,
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
            gas: eth::Gas(gas.into_legacy()),
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
        let assets: Vec<Address> = quote
            .token_addresses
            .iter()
            .cloned()
            .map(IntoAlloy::into_alloy)
            .collect();
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
                    I256::ZERO
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
        slippage: &dex::Slippage,
    ) -> Result<Vec<dex::Call>, Error> {
        // Receiving this error indicates that V3 is now supported on the current chain.
        let Some(v3_batch_router) = &self.v3_batch_router else {
            return Err(Error::DisabledApiVersion(ApiVersion::V3));
        };
        let paths_in = quote
            .paths
            .iter()
            .map(|path| path_to_exact_amount_in(path, order.side, slippage))
            .try_collect()?;
        let paths_out = quote
            .paths
            .iter()
            .map(|path| path_to_exact_amount_out(path, order.side, slippage))
            .try_collect()?;

        Ok(match order.side {
            Side::Buy => v3_batch_router.swap_exact_amount_out(
                paths_out,
                &self.permit2,
                quote.token_in.into_alloy(),
                max_input,
            ),
            Side::Sell => v3_batch_router.swap_exact_amount_in(
                paths_in,
                &self.permit2,
                quote.token_in.into_alloy(),
                max_input,
            ),
        })
    }

    /// Build common V2 swap data (kind, swaps, funds) from order and quote
    fn build_v2_swap_data(
        &self,
        order: &dex::Order,
        quote: &dto::Quote,
    ) -> Result<(v2::SwapKind, Vec<BatchSwapStep>, FundManagement), Error> {
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
                Ok(BatchSwapStep {
                    poolId: FixedBytes(swap.pool_id.as_v2()?.0),
                    assetInIndex: U256::from(swap.asset_in_index),
                    assetOutIndex: U256::from(swap.asset_out_index),
                    amount: swap.amount.into_alloy(),
                    userData: Bytes::copy_from_slice(&swap.user_data),
                })
            })
            .collect::<Result<_, Error>>()?;

        // Create funds structure
        let funds = FundManagement {
            sender: self.settlement,
            fromInternalBalance: false,
            recipient: self.settlement,
            toInternalBalance: false,
        };

        Ok((kind, swaps, funds))
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

/// Converts a Balancer API path into a `SwapPathExactAmountIn` struct for V3
/// batch swaps.
fn path_to_exact_amount_in(
    path: &dto::Path,
    side: Side,
    slippage: &dex::Slippage,
) -> Result<SwapPathExactAmountIn, Error> {
    Ok(SwapPathExactAmountIn {
        tokenIn: path
            .tokens
            .first()
            .map(|t| t.address.into_alloy())
            .ok_or(Error::InvalidPath)?,
        exactAmountIn: match side {
            Side::Buy => slippage.add(path.input_amount_raw).into_alloy(),
            Side::Sell => path.input_amount_raw.into_alloy(),
        },
        minAmountOut: match side {
            Side::Buy => path.output_amount_raw.into_alloy(),
            Side::Sell => slippage.sub(path.output_amount_raw).into_alloy(),
        },
        steps: convert_path_steps(path)?,
    })
}

/// Converts a Balancer API path into a `SwapPathExactAmountOut` struct for V3
/// batch swaps.
fn path_to_exact_amount_out(
    path: &dto::Path,
    side: Side,
    slippage: &dex::Slippage,
) -> Result<SwapPathExactAmountOut, Error> {
    Ok(SwapPathExactAmountOut {
        tokenIn: path
            .tokens
            .first()
            .map(|t| t.address.into_alloy())
            .ok_or(Error::InvalidPath)?,
        maxAmountIn: match side {
            Side::Buy => slippage.add(path.input_amount_raw).into_alloy(),
            Side::Sell => path.input_amount_raw.into_alloy(),
        },
        exactAmountOut: match side {
            Side::Buy => path.output_amount_raw.into_alloy(),
            Side::Sell => slippage.sub(path.output_amount_raw).into_alloy(),
        },
        steps: convert_path_steps(path)?,
    })
}

/// Converts the path steps from a Balancer API path into the format expected by
/// the V3 batch router. A path step consists of 1 item from 3 different arrays
/// at the correct index. `tokens` contains 1 item more where the first one
/// needs to be skipped.
fn convert_path_steps(path: &dto::Path) -> Result<Vec<SwapPathStep>, Error> {
    path.tokens
        .iter()
        .skip(1)
        .zip(path.is_buffer.iter())
        .zip(path.pools.iter())
        .map(|((token_out, is_buffer), pool)| {
            Ok(SwapPathStep {
                pool: pool.as_v3()?.into_alloy(),
                tokenOut: token_out.address.into_alloy(),
                isBuffer: *is_buffer,
            })
        })
        .collect()
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
