//! This module contains logic for encoding swaps with the Balanver V2 Smart
//! Contract. It serves as a thin wrapper around the `ethcontract` generated
//! bindings, defining structs with named fields instead of using tuples.

use {
    crate::domain::dex,
    alloy::{
        primitives::{Address, I256, U256},
        sol_types::SolInterface,
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
        let calldata = BalancerV2Vault::BalancerV2Vault::BalancerV2VaultCalls::batchSwap(
            BalancerV2Vault::BalancerV2Vault::batchSwapCall {
                kind: kind as _,
                swaps,
                assets,
                funds,
                limits,
                deadline: U256::ONE << 255,
            },
        )
        .abi_encode();

        vec![dex::Call {
            to: self.address(),
            calldata,
        }]
    }
}
