//! This module contains logic for encoding swaps with the Balanver V2 Smart
//! Contract. It serves as a thin wrapper around the `ethcontract` generated
//! bindings, defining structs with named fields instead of using tuples.

use {
    crate::domain::dex,
    alloy::{
        primitives::{Address, I256, U256},
        sol_types::SolCall,
    },
    contracts::alloy::{
        BalancerV2Vault,
        BalancerV2Vault::IVault::{BatchSwapStep, FundManagement},
    },
};

pub struct Vault(Address);

// In solidity this is not represented as an enum, but rather as a wrapper of
// u8.
#[repr(u8)]
pub enum SwapKind {
    GivenIn = 0,
    GivenOut = 1,
}

impl Vault {
    pub fn new(address: Address) -> Self {
        Self(address)
    }

    pub fn address(&self) -> Address {
        self.0
    }

    pub fn batch_swap(
        &self,
        kind: SwapKind,
        swaps: Vec<BatchSwapStep>,
        assets: Vec<Address>,
        funds: FundManagement,
        limits: Vec<I256>,
    ) -> Vec<dex::Call> {
        let calldata = BalancerV2Vault::BalancerV2Vault::batchSwapCall {
            kind: kind as _,
            swaps,
            assets,
            funds,
            limits,
            deadline: U256::ONE << 255,
        }
        .abi_encode();

        vec![dex::Call {
            to: self.address(),
            calldata,
        }]
    }
}

/// Extension trait for BalancerQueries contract to provide quotes for common
/// interactions like swaps / joins / exits without submitting a transaction.
///
/// Deployed at 0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5 on all chains.
///
/// Further documentation: https://docs-v2.balancer.fi/reference/contracts/query-functions.html
pub trait BalancerQueriesExt {
    /// Execute on-chain query and return the actual amounts (high-level
    /// contract call)
    async fn execute_query_batch_swap(
        &self,
        kind: SwapKind,
        swaps: Vec<Swap>,
        assets: Vec<H160>,
        funds: Funds,
    ) -> Result<Vec<alloy::primitives::I256>>;
}

impl BalancerQueriesExt for contracts::alloy::BalancerQueries::Instance {
    async fn execute_query_batch_swap(
        &self,
        kind: SwapKind,
        swaps: Vec<Swap>,
        assets: Vec<H160>,
        funds: Funds,
    ) -> Result<Vec<alloy::primitives::I256>> {
        // Execute the query call directly
        let asset_deltas = self
            .queryBatchSwap(
                kind as _,
                swaps
                    .into_iter()
                    .map(|swap| BatchSwapStep {
                        poolId: alloy::primitives::FixedBytes(swap.pool_id.0),
                        assetInIndex: swap.asset_in_index.into_alloy(),
                        assetOutIndex: swap.asset_out_index.into_alloy(),
                        amount: swap.amount.into_alloy(),
                        userData: alloy::primitives::Bytes::copy_from_slice(&swap.user_data),
                    })
                    .collect(),
                assets.into_iter().map(IntoAlloy::into_alloy).collect(),
                FundManagement {
                    sender: funds.sender.into_alloy(),
                    fromInternalBalance: funds.from_internal_balance,
                    recipient: funds.recipient.into_alloy(),
                    toInternalBalance: funds.to_internal_balance,
                },
            )
            .call()
            .await
            .map_err(|e| anyhow!("V2 query_batch_swap RPC call failed: {e:?}"))?;

        Ok(asset_deltas)
    }
}
