//! This module contains logic for encoding swaps with the Balanver V3
//! BatchRouter. It serves as a thin wrapper around the `ethcontract` generated
//! bindings, defining structs with named fields instead of using tuples.

use {
    crate::domain::{dex, eth},
    contracts::{ethcontract::Bytes, BalancerV3BatchRouter},
    ethereum_types::{H160, U256},
};

pub struct Router(BalancerV3BatchRouter);

pub struct SwapPathStep {
    pub pool: H160,
    pub token_out: H160,
    /// If true, the "pool" is an ERC4626 Buffer. Used to wrap/unwrap tokens if
    /// pool doesn't have enough liquidity.
    pub is_buffer: bool,
}

pub struct SwapPath {
    pub token_in: H160,
    pub steps: Vec<SwapPathStep>,
    pub input_amount_raw: U256,
    pub output_amount_raw: U256,
}

impl Router {
    pub fn new(address: eth::ContractAddress) -> Self {
        Self(contracts::dummy_contract!(BalancerV3BatchRouter, address.0))
    }

    pub fn address(&self) -> eth::ContractAddress {
        eth::ContractAddress(self.0.address())
    }

    pub fn swap_exact_amount_in(&self, paths: Vec<SwapPath>) -> dex::Call {
        let call = self.0.swap_exact_in(
            Self::encode_paths(paths),
            Self::deadline(),
            Self::weth_is_eth(),
            Self::user_data(),
        );

        dex::Call {
            to: self.address(),
            calldata: call.tx.data.unwrap().0,
        }
    }

    pub fn swap_exact_amount_out(&self, paths: Vec<SwapPath>) -> dex::Call {
        let call = self.0.swap_exact_out(
            Self::encode_paths(paths),
            Self::deadline(),
            Self::weth_is_eth(),
            Self::user_data(),
        );

        dex::Call {
            to: self.address(),
            calldata: call.tx.data.unwrap().0,
        }
    }

    /// Converts rust struct with readable fields into tuple arguments used by
    /// the smart contract bindings.
    #[allow(clippy::type_complexity)]
    fn encode_paths(paths: Vec<SwapPath>) -> Vec<(H160, Vec<(H160, H160, bool)>, U256, U256)> {
        paths
            .into_iter()
            .map(|p| {
                (
                    p.token_in,
                    p.steps
                        .into_iter()
                        .map(|s| (s.pool, s.token_out, s.is_buffer))
                        .collect(),
                    p.input_amount_raw,
                    p.output_amount_raw,
                )
            })
            .collect()
    }

    /// Returns a `deadline` value that is sufficiently large with as many 0's
    /// as possible for some small gas savings.
    fn deadline() -> U256 {
        U256::one() << 255
    }

    /// Returns value for the `wethIsEth` argument. If that is true, incoming
    /// ETH will be wrapped to WETH and outgoing WETH will be unwrapped to
    /// ETH. Since the settlement contract only works with WETH we don't
    /// have to think about wrapping.
    fn weth_is_eth() -> bool {
        false
    }

    /// Returns a value for the `userData` argument. The balancer SDK populates
    /// that with an empty value so we are doing that as well.
    fn user_data() -> Bytes<Vec<u8>> {
        Default::default()
    }
}
