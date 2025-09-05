//! Query swap provider implementations for on-chain query operations.

use {
    crate::{
        domain::{dex, eth, order},
        infra::{
            blockchain,
            dex::balancer::{dto, v2, v3},
        },
    },
    ethereum_types::U256,
};

/// Result from on-chain query containing updated swap amounts
#[derive(Debug, Clone)]
pub struct OnChainAmounts {
    pub swap_amount: U256,
    pub return_amount: U256,
}

/// Trait for providers that can execute on-chain queries to get updated swap
/// amounts
#[async_trait::async_trait]
pub trait QuerySwapProvider: Send + Sync {
    /// Execute on-chain query to get updated swap amounts for both V2 and V3
    async fn query_swap(
        &self,
        order: &dex::Order,
        quote: &dto::Quote,
    ) -> Result<OnChainAmounts, crate::infra::dex::balancer::Error>;
}

/// On-chain query swap provider that uses real blockchain calls
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
        rpc_url: reqwest::Url,
        settlement: eth::ContractAddress,
    ) -> Self {
        Self {
            queries: queries.map(v2::Queries::new),
            v3_batch_router: v3_batch_router.map(v3::Router::new),
            web3: blockchain::rpc(&rpc_url),
            settlement,
        }
    }
}

#[async_trait::async_trait]
impl QuerySwapProvider for OnChainQuerySwapProvider {
    async fn query_swap(
        &self,
        order: &dex::Order,
        quote: &dto::Quote,
    ) -> Result<OnChainAmounts, crate::infra::dex::balancer::Error> {
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
    ) -> Result<OnChainAmounts, crate::infra::dex::balancer::Error> {
        let (kind, swaps, funds) = self.build_v2_swap_data(order, quote)?;
        let assets = quote.token_addresses.clone();

        // Execute the on-chain query
        let asset_deltas = self
            .queries
            .as_ref()
            .ok_or(crate::infra::dex::balancer::Error::InvalidPath)?
            .execute_query_batch_swap(&self.web3, kind, swaps, assets, funds)
            .await
            .map_err(|_e| crate::infra::dex::balancer::Error::InvalidPath)?;

        // Parse the result - asset_deltas corresponds to the assets array
        // We need to find the indices for token_in and token_out in the quote's
        // token_addresses
        if asset_deltas.len() != quote.token_addresses.len() {
            return Err(crate::infra::dex::balancer::Error::InvalidPath);
        }

        let token_in_index = quote
            .token_addresses
            .iter()
            .position(|&addr| addr == order.sell.0)
            .ok_or(crate::infra::dex::balancer::Error::InvalidPath)?;
        let token_out_index = quote
            .token_addresses
            .iter()
            .position(|&addr| addr == order.buy.0)
            .ok_or(crate::infra::dex::balancer::Error::InvalidPath)?;

        // Get the deltas for token_in and token_out (convert to absolute values)
        let amount_in = U256::from_dec_str(&asset_deltas[token_in_index].abs().to_string())
            .map_err(|_| crate::infra::dex::balancer::Error::InvalidPath)?;
        let amount_out = U256::from_dec_str(&asset_deltas[token_out_index].abs().to_string())
            .map_err(|_| crate::infra::dex::balancer::Error::InvalidPath)?;

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
    ) -> Result<OnChainAmounts, crate::infra::dex::balancer::Error> {
        // Get the V3 batch router (it should be available for V3 quotes)
        let v3_batch_router = self
            .v3_batch_router
            .as_ref()
            .ok_or(crate::infra::dex::balancer::Error::InvalidPath)?;

        let paths = self.build_v3_swap_data(quote, order, &dex::Slippage::zero())?;

        // Execute the appropriate query based on order side
        let result = match order.side {
            order::Side::Sell => {
                // For sell orders, we know the input amount, query for output amount
                v3_batch_router
                    .query_swap_exact_amount_in(&self.web3, paths)
                    .await
                    .map_err(|_e| crate::infra::dex::balancer::Error::InvalidPath)?
            }
            order::Side::Buy => {
                // For buy orders, we know the output amount, query for input amount
                v3_batch_router
                    .query_swap_exact_amount_out(&self.web3, paths)
                    .await
                    .map_err(|_e| crate::infra::dex::balancer::Error::InvalidPath)?
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
    ) -> Result<(v2::SwapKind, Vec<v2::Swap>, v2::Funds), crate::infra::dex::balancer::Error> {
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
            .collect::<Result<_, crate::infra::dex::balancer::Error>>()?;

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
    ) -> Result<Vec<v3::SwapPath>, crate::infra::dex::balancer::Error> {
        quote
            .paths
            .iter()
            .map(|path| {
                Ok(v3::SwapPath {
                    token_in: path
                        .tokens
                        .first()
                        .map(|t| t.address)
                        .ok_or_else(|| crate::infra::dex::balancer::Error::InvalidPath)?,
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
                                pool: pool.as_v3()?,
                                token_out: token_out.address,
                                is_buffer: *is_buffer,
                            })
                        })
                        .collect::<Result<_, crate::infra::dex::balancer::Error>>()?,
                })
            })
            .collect::<Result<_, crate::infra::dex::balancer::Error>>()
    }
}
