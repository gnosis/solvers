//! This module provides a trait-based abstraction for executing on-chain
//! queries to get real-time swap amounts from Balancer V2 and V3 contracts. It
//! serves as a bridge between the SOR (Smart Order Router) API quotes and
//! actual on-chain contract calls to ensure accurate pricing.
//!
//! ## Architecture Overview
//!
//! The module follows a trait-based design pattern:
//! - `QuerySwapProvider`: Trait defining the interface for on-chain queries
//! - `OnChainQuerySwapProvider`: Concrete implementation that makes real
//!   blockchain calls

use {
    crate::{
        domain::{
            dex,
            eth,
            order::{self, Side},
        },
        infra::{
            blockchain,
            dex::balancer::{
                Error,
                convert_path_steps,
                dto,
                v2::{self, BalancerQueriesExt},
                v3,
            },
        },
    },
    alloy::primitives::{Address, Bytes, FixedBytes, U256},
    anyhow::{Context, Result, anyhow, ensure},
    contracts::alloy::{
        BalancerQueries::IVault::{BatchSwapStep, FundManagement},
        BalancerV3BatchRouter::IBatchRouter::{SwapPathExactAmountIn, SwapPathExactAmountOut},
    },
    itertools::Itertools,
};

/// Result from on-chain query containing updated swap amounts.
///
/// The amounts represent the real-time values from on-chain contract calls:
/// - `swap_amount`: Always the given/exact amount (what the user specifies)
/// - `return_amount`: Always the calculated amount (what the user receives)
///
/// ## Order Side Mapping
///
/// For **sell orders** (exact input):
/// - `swap_amount` = amount in (what user wants to sell)
/// - `return_amount` = amount out (what user will receive in return)
///
/// For **buy orders** (exact output):
/// - `swap_amount` = amount out (what user wants to buy)
/// - `return_amount` = amount in (what user needs to pay)
#[derive(Debug, Clone)]
pub struct OnChainAmounts {
    /// The given/exact amount (amount in for sell orders, amount out for buy
    /// orders)
    pub swap_amount: U256,
    /// The calculated amount (amount out for sell orders, amount in for buy
    /// orders)
    pub return_amount: U256,
}

/// Defines the contract for providers that can execute on-chain queries to get
/// updated swap amounts. This abstraction allows for different implementations
/// (real blockchain calls, mocked responses, etc.).
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait QuerySwapProvider: Send + Sync {
    /// Execute on-chain query to get updated swap amounts for both V2 and V3
    async fn query_swap(&self, order: &dex::Order, quote: &dto::Quote) -> Result<OnChainAmounts>;
}

/// On-chain query swap provider that uses real blockchain calls
///
/// The main implementation that:
/// - Uses BalancerQueries contract for V2 swaps
/// - Uses BalancerV3BatchRouter contract for V3 swaps
/// - Handles both sell orders (exact input) and buy orders (exact output)
/// - Returns updated amounts that reflect current on-chain state
pub struct OnChainQuerySwapProvider {
    queries: Option<contracts::alloy::BalancerQueries::Instance>,
    v3_batch_router: Option<v3::Router>,
    settlement: Address,
}

impl OnChainQuerySwapProvider {
    pub fn new(
        queries: Option<Address>,
        v3_batch_router: Option<Address>,
        node_url: reqwest::Url,
        settlement: Address,
    ) -> Self {
        let web3 = blockchain::rpc(&node_url);
        Self {
            queries: queries.map(|addr| {
                contracts::alloy::BalancerQueries::Instance::new(addr, web3.alloy.clone())
            }),
            v3_batch_router: v3_batch_router.map(|addr| v3::Router::new(addr, web3.alloy.clone())),
            settlement,
        }
    }
}

#[async_trait::async_trait]
impl QuerySwapProvider for OnChainQuerySwapProvider {
    async fn query_swap(&self, order: &dex::Order, quote: &dto::Quote) -> Result<OnChainAmounts> {
        match quote.protocol_version {
            dto::ProtocolVersion::V2 => self.query_swap_v2(order, quote).await,
            dto::ProtocolVersion::V3 => self.query_swap_v3(order, quote).await,
        }
    }
}

impl OnChainQuerySwapProvider {
    /// Execute on-chain query for V2 using BalancerQueries contract
    async fn query_swap_v2(
        &self,
        order: &dex::Order,
        quote: &dto::Quote,
    ) -> Result<OnChainAmounts> {
        let (kind, swaps, funds) = self.build_v2_swap_data(order, quote)?;
        let assets: Vec<Address> = quote.token_addresses.to_vec();

        // Execute the on-chain query
        let asset_deltas = self
            .queries
            .as_ref()
            .context("BalancerQueries not configured (required for V2 on-chain query)")?
            .execute_query_batch_swap(kind, swaps, assets, funds)
            .await
            .map_err(|e| anyhow!("RPC call failed: Queries.execute_query_batch_swap: {e:?}"))?;

        // Parse the result - asset_deltas corresponds to the assets array
        // We need to find the indices for token_in and token_out in the quote's
        // token_addresses
        ensure!(
            asset_deltas.len() == quote.token_addresses.len(),
            "mismatched asset_deltas length: got {}, expected {}",
            asset_deltas.len(),
            quote.token_addresses.len()
        );

        let token_in_index = quote
            .token_addresses
            .iter()
            .position(|&addr| addr == order.sell.0)
            .ok_or_else(|| {
                anyhow!(
                    "token_in index not found in quote.token_addresses (sell token {:?})",
                    order.sell.0
                )
            })?;
        let token_out_index = quote
            .token_addresses
            .iter()
            .position(|&addr| addr == order.buy.0)
            .ok_or_else(|| {
                anyhow!(
                    "token_out index not found in quote.token_addresses (buy token {:?})",
                    order.buy.0
                )
            })?;

        // Get the deltas for token_in and token_out (convert to absolute values)
        let amount_in =
            eth::U256::from_str_radix(&asset_deltas[token_in_index].abs().to_string(), 10)
                .map_err(|e| {
                    anyhow!(
                        "failed to parse token_in delta '{}' into U256: {e:?}",
                        asset_deltas[token_in_index]
                    )
                })?;
        let amount_out =
            eth::U256::from_str_radix(&asset_deltas[token_out_index].abs().to_string(), 10)
                .map_err(|e| {
                    anyhow!(
                        "failed to parse token_out delta '{}' into U256: {e:?}",
                        asset_deltas[token_out_index]
                    )
                })?;

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
    ) -> Result<OnChainAmounts> {
        // Get the V3 batch router (it should be available for V3 quotes)
        let v3_batch_router = self
            .v3_batch_router
            .as_ref()
            .context("V3 batch router not configured (required for V3 on-chain query)")?;

        let paths_in = quote
            .paths
            .iter()
            .map(|path| Self::path_to_exact_amount_in(path, order.side, &dex::Slippage::zero()))
            .try_collect()
            .context("failed to build V3 exact amount in paths from quote")?;
        let paths_out = quote
            .paths
            .iter()
            .map(|path| Self::path_to_exact_amount_out(path, order.side, &dex::Slippage::zero()))
            .try_collect()
            .context("failed to build V3 exact amount out paths from quote")?;

        // Execute the appropriate query based on order side
        let result = match order.side {
            order::Side::Sell => {
                // For sell orders, we know the input amount, query for output amount
                v3_batch_router
                    .query_swap_exact_amount_in(paths_in)
                    .await
                    .map_err(|e| {
                        anyhow!("RPC call failed: Router.query_swap_exact_amount_in: {e:?}")
                    })?
            }
            order::Side::Buy => {
                // For buy orders, we know the output amount, query for input amount
                v3_batch_router
                    .query_swap_exact_amount_out(paths_out)
                    .await
                    .map_err(|e| {
                        anyhow!("RPC call failed: Router.query_swap_exact_amount_out: {e:?}")
                    })?
            }
        };

        // swap_amount: the given/exact amount from SOR
        //
        // The on-chain query result is always the "calculated" amount:
        // - For sell orders: result = output amount (what user receives)
        // - For buy orders: result = input amount (what user needs to pay)
        //
        // return_amount: the calculated amount from on-chain query
        Ok(OnChainAmounts {
            swap_amount: quote.swap_amount_raw,
            return_amount: result,
        })
    }

    /// Build common V2 swap data (kind, swaps, funds) from order and quote
    fn build_v2_swap_data(
        &self,
        order: &dex::Order,
        quote: &dto::Quote,
    ) -> Result<(v2::SwapKind, Vec<BatchSwapStep>, FundManagement)> {
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
                    poolId: FixedBytes(
                        swap.pool_id
                            .as_v2()
                            .context("invalid V2 pool id format in quote.swap")?
                            .0,
                    ),
                    assetInIndex: U256::from(swap.asset_in_index),
                    assetOutIndex: U256::from(swap.asset_out_index),
                    amount: swap.amount,
                    userData: Bytes::copy_from_slice(&swap.user_data),
                })
            })
            .collect::<Result<_>>()?;

        // Create funds structure
        let funds = FundManagement {
            sender: self.settlement,
            fromInternalBalance: false,
            recipient: self.settlement,
            toInternalBalance: false,
        };

        Ok((kind, swaps, funds))
    }

    /// Converts a Balancer API path into a `SwapPathExactAmountIn` struct for
    /// V3 batch swaps.
    fn path_to_exact_amount_in(
        path: &dto::Path,
        side: Side,
        slippage: &dex::Slippage,
    ) -> Result<SwapPathExactAmountIn, Error> {
        Ok(SwapPathExactAmountIn {
            tokenIn: path
                .tokens
                .first()
                .map(|t| t.address)
                .ok_or(Error::InvalidPath)?,
            exactAmountIn: match side {
                Side::Buy => slippage.add(path.input_amount_raw),
                Side::Sell => path.input_amount_raw,
            },
            minAmountOut: match side {
                Side::Buy => path.output_amount_raw,
                Side::Sell => slippage.sub(path.output_amount_raw),
            },
            steps: convert_path_steps(path)?,
        })
    }

    /// Converts a Balancer API path into a `SwapPathExactAmountOut` struct for
    /// V3 batch swaps.
    fn path_to_exact_amount_out(
        path: &dto::Path,
        side: Side,
        slippage: &dex::Slippage,
    ) -> Result<SwapPathExactAmountOut, Error> {
        Ok(SwapPathExactAmountOut {
            tokenIn: path
                .tokens
                .first()
                .map(|t| t.address)
                .ok_or(Error::InvalidPath)?,
            maxAmountIn: match side {
                Side::Buy => slippage.add(path.input_amount_raw),
                Side::Sell => path.input_amount_raw,
            },
            exactAmountOut: match side {
                Side::Buy => path.output_amount_raw,
                Side::Sell => slippage.sub(path.output_amount_raw),
            },
            steps: convert_path_steps(path)?,
        })
    }
}
