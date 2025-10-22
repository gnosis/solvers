//! This module contains logic for encoding swaps with the Balanver V3
//! BatchRouter. It serves as a thin wrapper around the `ethcontract` generated
//! bindings, defining structs with named fields instead of using tuples.

use {
    crate::domain::{dex, eth},
    alloy::{primitives::ruint::UintTryFrom, sol_types::SolCall},
    contracts::alloy::{BalancerV3BatchRouter, Permit2 as Permit2Contract},
    ethereum_types::{H160, U256},
    ethrpc::alloy::conversions::IntoAlloy,
};

pub struct Permit2(eth::ContractAddress);

impl Permit2 {
    pub fn new(address: eth::ContractAddress) -> Self {
        Self(address)
    }

    pub fn address(&self) -> eth::ContractAddress {
        self.0
    }

    // Creates a interaction call to approve an addresss
    // Needed because in Balancer V3 transfers are done via Permit2, so we approve
    // the balancer v3 router to spend the input tokens
    pub fn create_approval_call(
        &self,
        spender: H160,
        token_in: H160,
        max_input: U256,
    ) -> dex::Call {
        // Transfers are done via Permit2, so we approve the balancer v3 router to spend
        // the input tokens
        let calldata = Permit2Contract::Permit2::approveCall {
            token: token_in.into_alloy(),
            spender: spender.into_alloy(),
            amount: UintTryFrom::uint_try_from(max_input.into_alloy()).unwrap(),
            // expiration = 0 in permit2 means that the tokens are allowed to be spent on
            // the same block as the approval, this is enough for a settlement
            expiration: Default::default(),
        }
        .abi_encode();

        dex::Call {
            to: self.address(),
            calldata,
        }
    }
}

pub struct Router(eth::ContractAddress);

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
        Self(address)
    }

    pub fn address(&self) -> eth::ContractAddress {
        self.0
    }

    pub fn swap_exact_amount_in(
        &self,
        paths: Vec<SwapPath>,
        permit2: &Permit2,
        token_in: H160,
        max_input: U256,
    ) -> Vec<dex::Call> {
        let permit2_approval_call =
            permit2.create_approval_call(self.address().0, token_in, max_input);

        let swap_call = BalancerV3BatchRouter::BalancerV3BatchRouter::swapExactInCall {
            paths: paths
                .into_iter()
                .map(
                    |path| BalancerV3BatchRouter::IBatchRouter::SwapPathExactAmountIn {
                        tokenIn: path.token_in.into_alloy(),
                        steps: path
                            .steps
                            .into_iter()
                            .map(|step| BalancerV3BatchRouter::IBatchRouter::SwapPathStep {
                                pool: step.pool.into_alloy(),
                                tokenOut: step.token_out.into_alloy(),
                                isBuffer: step.is_buffer,
                            })
                            .collect(),
                        exactAmountIn: path.input_amount_raw.into_alloy(),
                        minAmountOut: path.output_amount_raw.into_alloy(),
                    },
                )
                .collect(),
            deadline: Self::deadline(),
            wethIsEth: Self::weth_is_eth(),
            userData: Self::user_data(),
        }
        .abi_encode();

        vec![
            permit2_approval_call,
            dex::Call {
                to: self.address(),
                calldata: swap_call,
            },
        ]
    }

    pub fn swap_exact_amount_out(
        &self,
        paths: Vec<SwapPath>,
        permit2: &Permit2,
        token_in: H160,
        max_input: U256,
    ) -> Vec<dex::Call> {
        let permit2_approval_call =
            permit2.create_approval_call(self.address().0, token_in, max_input);

        let swap_call = BalancerV3BatchRouter::BalancerV3BatchRouter::swapExactOutCall {
            paths: paths
                .into_iter()
                .map(
                    |path| BalancerV3BatchRouter::IBatchRouter::SwapPathExactAmountOut {
                        tokenIn: path.token_in.into_alloy(),
                        steps: path
                            .steps
                            .into_iter()
                            .map(|step| BalancerV3BatchRouter::IBatchRouter::SwapPathStep {
                                pool: step.pool.into_alloy(),
                                tokenOut: step.token_out.into_alloy(),
                                isBuffer: step.is_buffer,
                            })
                            .collect(),
                        maxAmountIn: path.input_amount_raw.into_alloy(),
                        exactAmountOut: path.output_amount_raw.into_alloy(),
                    },
                )
                .collect(),
            deadline: Self::deadline(),
            wethIsEth: Self::weth_is_eth(),
            userData: Self::user_data(),
        }
        .abi_encode();

        vec![
            permit2_approval_call,
            dex::Call {
                to: self.address(),
                calldata: swap_call,
            },
        ]
    }

    /// Returns a `deadline` value that is sufficiently large with as many 0's
    /// as possible for some small gas savings (i.e. b1000...0000).
    fn deadline() -> alloy::primitives::U256 {
        alloy::primitives::U256::ONE.wrapping_shl(255)
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
    fn user_data() -> alloy::primitives::Bytes {
        Default::default()
    }
}
