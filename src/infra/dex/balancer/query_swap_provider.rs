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
        domain::{dex, eth, order},
        infra::{
            blockchain,
            dex::balancer::{dto, v2, v3},
        },
    },
    anyhow::{anyhow, ensure, Context, Result},
    ethereum_types::U256,
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
    queries: Option<v2::Queries>,
    v3_batch_router: Option<v3::Router>,
    web3: ethrpc::Web3,
    settlement: eth::ContractAddress,
}

impl OnChainQuerySwapProvider {
    pub fn new(
        queries: Option<eth::ContractAddress>,
        v3_batch_router: Option<eth::ContractAddress>,
        node_url: reqwest::Url,
        settlement: eth::ContractAddress,
    ) -> Self {
        Self {
            queries: queries.map(v2::Queries::new),
            v3_batch_router: v3_batch_router.map(v3::Router::new),
            web3: blockchain::rpc(&node_url),
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
        let assets = quote.token_addresses.clone();

        // Execute the on-chain query
        let asset_deltas = self
            .queries
            .as_ref()
            .context("BalancerQueries not configured (required for V2 on-chain query)")?
            .execute_query_batch_swap(&self.web3, kind, swaps, assets, funds)
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
        let amount_in = U256::from_dec_str(&asset_deltas[token_in_index].abs().to_string())
            .map_err(|e| {
                anyhow!(
                    "failed to parse token_in delta '{}' into U256: {e:?}",
                    asset_deltas[token_in_index]
                )
            })?;
        let amount_out = U256::from_dec_str(&asset_deltas[token_out_index].abs().to_string())
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

        let paths = self
            .build_v3_swap_data(quote, order, &dex::Slippage::zero())
            .context("failed to build V3 swap paths from quote")?;

        // Execute the appropriate query based on order side
        let result = match order.side {
            order::Side::Sell => {
                // For sell orders, we know the input amount, query for output amount
                v3_batch_router
                    .query_swap_exact_amount_in(&self.web3, paths)
                    .await
                    .map_err(|e| {
                        anyhow!("RPC call failed: Router.query_swap_exact_amount_in: {e:?}")
                    })?
            }
            order::Side::Buy => {
                // For buy orders, we know the output amount, query for input amount
                v3_batch_router
                    .query_swap_exact_amount_out(&self.web3, paths)
                    .await
                    .map_err(|e| {
                        anyhow!("RPC call failed: Router.query_swap_exact_amount_out: {e:?}")
                    })?
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

    /// Build common V2 swap data (kind, swaps, funds) from order and quote
    fn build_v2_swap_data(
        &self,
        order: &dex::Order,
        quote: &dto::Quote,
    ) -> Result<(v2::SwapKind, Vec<v2::Swap>, v2::Funds)> {
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
                    pool_id: swap
                        .pool_id
                        .as_v2()
                        .context("invalid V2 pool id format in quote.swap")?,
                    asset_in_index: swap.asset_in_index.into(),
                    asset_out_index: swap.asset_out_index.into(),
                    amount: swap.amount,
                    user_data: swap.user_data.clone(),
                })
            })
            .collect::<Result<_>>()?;

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
    fn build_v3_swap_data(
        &self,
        quote: &dto::Quote,
        order: &dex::Order,
        slippage: &dex::Slippage,
    ) -> Result<Vec<v3::SwapPath>> {
        quote
            .paths
            .iter()
            .map(|path| {
                Ok(v3::SwapPath {
                    token_in: path
                        .tokens
                        .first()
                        .map(|t| t.address)
                        .ok_or_else(|| anyhow!("path.tokens is empty; token_in missing"))?,
                    input_amount_raw: match order.side {
                        order::Side::Buy => slippage.add(path.input_amount_raw),
                        order::Side::Sell => path.input_amount_raw,
                    },
                    output_amount_raw: match order.side {
                        order::Side::Buy => path.output_amount_raw,
                        order::Side::Sell => slippage.sub(path.output_amount_raw),
                    },

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
                                pool: pool
                                    .as_v3()
                                    .context("invalid V3 pool id format in path step")?,
                                token_out: token_out.address,
                                is_buffer: *is_buffer,
                            })
                        })
                        .collect::<Result<_>>()?,
                })
            })
            .collect::<Result<_>>()
    }
}
