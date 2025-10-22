//! This module contains logic for encoding swaps with the Balanver V2 Smart
//! Contract. It serves as a thin wrapper around the `ethcontract` generated
//! bindings, defining structs with named fields instead of using tuples.

use {
    crate::domain::{dex, eth},
    alloy::sol_types::SolCall,
    contracts::{alloy::BalancerV2Vault, ethcontract::I256},
    ethereum_types::{H160, H256, U256},
    ethrpc::alloy::conversions::IntoAlloy,
};

pub struct Vault(eth::ContractAddress);

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
        Self(address)
    }

    pub fn address(&self) -> eth::ContractAddress {
        self.0
    }

    pub fn batch_swap(
        &self,
        kind: SwapKind,
        swaps: Vec<Swap>,
        assets: Vec<H160>,
        funds: Funds,
        limits: Vec<I256>,
    ) -> Vec<dex::Call> {
        let calldata = BalancerV2Vault::BalancerV2Vault::batchSwapCall {
            kind: kind as _,
            swaps: swaps
                .into_iter()
                .map(|swap| BalancerV2Vault::IVault::BatchSwapStep {
                    poolId: swap.pool_id.into_alloy(),
                    assetInIndex: swap.asset_in_index.into_alloy(),
                    assetOutIndex: swap.asset_out_index.into_alloy(),
                    amount: swap.amount.into_alloy(),
                    userData: swap.user_data.into(),
                })
                .collect(),
            assets: assets.into_iter().map(|addr| addr.into_alloy()).collect(),
            funds: BalancerV2Vault::IVault::FundManagement {
                sender: funds.sender.into_alloy(),
                fromInternalBalance: funds.from_internal_balance,
                recipient: funds.recipient.into_alloy(),
                toInternalBalance: funds.to_internal_balance,
            },
            limits: limits.into_iter().map(|limit| limit.into_alloy()).collect(),
            //Sufficiently large value with as many 0's as possible for some small gas savings.
            deadline: alloy::primitives::U256::ONE.wrapping_shl(255),
        }
        .abi_encode();

        vec![dex::Call {
            to: self.address(),
            calldata,
        }]
    }
}
