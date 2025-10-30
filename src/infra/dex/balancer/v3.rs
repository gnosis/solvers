//! This module contains logic for encoding swaps with the Balanver V3
//! BatchRouter. It serves as a thin wrapper around the `ethcontract` generated
//! bindings, defining structs with named fields instead of using tuples.

use {
    crate::domain::dex,
    alloy::{
        primitives::{aliases::U48, Address, Bytes, U160, U256},
        sol_types::SolCall,
    },
    contracts::alloy::{
        BalancerV3BatchRouter,
        BalancerV3BatchRouter::IBatchRouter::{SwapPathExactAmountIn, SwapPathExactAmountOut},
        Permit2 as Permit2Contract,
    },
};

pub struct Permit2(Address);

impl Permit2 {
    pub fn new(address: Address) -> Self {
        Self(address)
    }

    pub fn address(&self) -> Address {
        self.0
    }

    // Creates a interaction call to approve an address
    // Needed because in Balancer V3 transfers are done via Permit2, so we approve
    // the balancer v3 router to spend the input tokens
    pub fn create_approval_call(
        &self,
        spender: Address,
        token_in: Address,
        max_input: U256,
    ) -> dex::Call {
        let to = self.address();

        // expiration = 0 in permit2 means that the tokens are allowed to be spent on
        // the same block as the approval, this is enough for a settlement
        let expiration = U48::ZERO;

        // Transfers are done via Permit2, so we approve the balancer v3 router to spend
        // the input tokens
        let calldata = Permit2Contract::Permit2::approveCall {
            token: token_in,
            spender,
            amount: U160::from(max_input),
            expiration,
        }
        .abi_encode();

        dex::Call { to, calldata }
    }
}

pub struct Router(Address);

impl Router {
    pub fn new(address: Address) -> Self {
        Self(address)
    }

    pub fn address(&self) -> Address {
        self.0
    }

    pub fn swap_exact_amount_in(
        &self,
        paths: Vec<SwapPathExactAmountIn>,
        permit2: &Permit2,
        token_in: Address,
        max_input: U256,
    ) -> Vec<dex::Call> {
        let permit2_approval_call =
            permit2.create_approval_call(self.address(), token_in, max_input);

        let calldata = BalancerV3BatchRouter::BalancerV3BatchRouter::swapExactInCall {
            paths,
            deadline: Self::deadline(),
            wethIsEth: Self::weth_is_eth(),
            userData: Self::user_data(),
        }
        .abi_encode();

        vec![
            permit2_approval_call,
            dex::Call {
                to: self.address(),
                calldata,
            },
        ]
    }

    pub fn swap_exact_amount_out(
        &self,
        paths: Vec<SwapPathExactAmountOut>,
        permit2: &Permit2,
        token_in: Address,
        max_input: U256,
    ) -> Vec<dex::Call> {
        let permit2_approval_call =
            permit2.create_approval_call(self.address(), token_in, max_input);

        let calldata = BalancerV3BatchRouter::BalancerV3BatchRouter::swapExactOutCall {
            paths,
            deadline: Self::deadline(),
            wethIsEth: Self::weth_is_eth(),
            userData: Self::user_data(),
        }
        .abi_encode();

        vec![
            permit2_approval_call,
            dex::Call {
                to: self.address(),
                calldata,
            },
        ]
    }

    /// Returns a `deadline` value that is sufficiently large with as many 0's
    /// as possible for some small gas savings (i.e. b1000...0000).
    fn deadline() -> U256 {
        U256::ONE << 255
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
    fn user_data() -> Bytes {
        Default::default()
    }
}
