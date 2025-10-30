//! This module contains logic for encoding swaps with the Balanver V2 Smart
//! Contract. It serves as a thin wrapper around the `ethcontract` generated
//! bindings, defining structs with named fields instead of using tuples.

use {
    crate::domain::{dex, eth},
    alloy::primitives::Address,
    anyhow::{Result, anyhow},
    contracts::{
        BalancerV2Vault,
        alloy::BalancerQueries::IVault::{BatchSwapStep, FundManagement},
        ethcontract::{Bytes, I256},
    },
    ethereum_types::{H160, H256, U256},
    ethrpc::alloy::conversions::IntoAlloy,
};

pub struct Vault(BalancerV2Vault);

#[repr(u8)]
pub enum SwapKind {
    GivenIn = 0,
    GivenOut = 1,
}

pub struct Swap {
    pub pool_id: H256,
    pub asset_in_index: U256,
    pub asset_out_index: U256,
    pub amount: U256,
    pub user_data: Vec<u8>,
}

pub struct Funds {
    pub sender: H160,
    pub from_internal_balance: bool,
    pub recipient: H160,
    pub to_internal_balance: bool,
}

impl Vault {
    pub fn new(address: eth::ContractAddress) -> Self {
        Self(contracts::dummy_contract!(BalancerV2Vault, address.0))
    }

    pub fn address(&self) -> eth::ContractAddress {
        eth::ContractAddress(self.0.address())
    }

    pub fn batch_swap(
        &self,
        kind: SwapKind,
        swaps: Vec<Swap>,
        assets: Vec<H160>,
        funds: Funds,
        limits: Vec<I256>,
    ) -> Vec<dex::Call> {
        vec![dex::Call {
            to: self.address(),
            calldata: self
                .0
                .methods()
                .batch_swap(
                    kind as _,
                    swaps
                        .into_iter()
                        .map(|swap| {
                            (
                                Bytes(swap.pool_id.0),
                                swap.asset_in_index,
                                swap.asset_out_index,
                                swap.amount,
                                Bytes(swap.user_data),
                            )
                        })
                        .collect(),
                    assets,
                    (
                        funds.sender,
                        funds.from_internal_balance,
                        funds.recipient,
                        funds.to_internal_balance,
                    ),
                    limits,
                    // `deadline`: Sufficiently large value with as many 0's as possible for some
                    // small gas savings.
                    U256::one() << 255,
                )
                .tx
                .data
                .expect("calldata")
                .0,
        }]
    }
}

/// BalancerQueries is a helper contract to provide quotes for common
/// interactions like swaps / joins / exits without submitting a transaction.
///
/// Deployed at 0xE39B5e3B6D74016b2F6A9673D7d7493B6DF549d5 on all chains.
///
/// Further documentation: https://docs-v2.balancer.fi/reference/contracts/query-functions.html
pub struct Queries(contracts::alloy::BalancerQueries::Instance);

impl Queries {
    /// Create a new BalancerQueries contract instance
    pub fn new(web3: &ethrpc::Web3, address: eth::ContractAddress) -> Self {
        Self(contracts::alloy::BalancerQueries::Instance::new(
            address.0.into_alloy(),
            web3.alloy.clone(),
        ))
    }

    /// Get the contract address
    pub fn address(&self) -> Address {
        *self.0.address()
    }

    /// Execute on-chain query and return the actual amounts (high-level
    /// contract call)
    pub async fn execute_query_batch_swap(
        &self,
        web3: &ethrpc::Web3,
        kind: SwapKind,
        swaps: Vec<Swap>,
        assets: Vec<H160>,
        funds: Funds,
    ) -> Result<Vec<alloy::primitives::I256>> {
        // Create a contract instance with the Web3 client
        let contract =
            contracts::alloy::BalancerQueries::Instance::new(self.address(), web3.alloy.clone());

        // Execute the query call directly
        let asset_deltas = contract
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
