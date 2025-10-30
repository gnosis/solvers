//! This module contains logic for encoding swaps with the Balanver V3
//! BatchRouter. It serves as a thin wrapper around the `ethcontract` generated
//! bindings, defining structs with named fields instead of using tuples.

use {
    crate::domain::dex,
    alloy::{
        primitives::{Address, Bytes, U160, U256, aliases::U48},
        sol_types::SolCall,
    },
    anyhow::{Result, anyhow, ensure},
    contracts::alloy::{
        BalancerV3BatchRouter,
        BalancerV3BatchRouter::IBatchRouter::{SwapPathExactAmountIn, SwapPathExactAmountOut},
        Permit2 as Permit2Contract,
    },
    ethrpc::AlloyProvider,
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
        let mut calldata = Permit2Contract::Permit2::approveCall {
            token: token_in,
            spender,
            amount: U160::from(max_input),
            expiration,
        }
        .abi_encode();

        // As alloy encodes the last argument (expiration) as a U48 (6 bytes),
        // we need to add 24 bytes to pad it into a U256 (32 bytes) (which is the
        // expected for EVM arguments)
        calldata.extend_from_slice(&[0u8; 24]);

        dex::Call { to, calldata }
    }
}

pub struct Router(BalancerV3BatchRouter::Instance);

impl Router {
    pub fn new(address: Address, alloy_provider: AlloyProvider) -> Self {
        Self(BalancerV3BatchRouter::Instance::new(
            address,
            alloy_provider,
        ))
    }

    pub fn address(&self) -> Address {
        *self.0.address()
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

    /// Execute on-chain query for V3 swap and return the actual amounts
    /// (high-level contract call)
    pub async fn query_swap_exact_amount_in(
        &self,
        paths: Vec<SwapPathExactAmountIn>,
    ) -> Result<U256> {
        // Execute the query call directly
        let swap_in_return = self
            .0
            .querySwapExactIn(paths, Address::ZERO, Self::user_data())
            .call()
            .await
            .map_err(|e| anyhow!("V3 query_swap_exact_amount_in RPC call failed: {e:?}"))?;

        // The result is (amounts_out, tokens_out, amounts_in)
        // For exact amount in, we want the output amount (first element of amounts_out)
        ensure!(
            !swap_in_return.amountsOut.is_empty(),
            "V3 query_swap_exact_in returned no output amounts"
        );
        Ok(swap_in_return.amountsOut[0])
    }

    /// Execute on-chain query for V3 swap and return the actual amounts
    /// (high-level contract call)
    pub async fn query_swap_exact_amount_out(
        &self,
        paths: Vec<SwapPathExactAmountOut>,
    ) -> Result<U256> {
        // Execute the query call directly
        let swap_out_return = self
            .0
            .querySwapExactOut(paths, Address::ZERO, Self::user_data())
            .call()
            .await
            .map_err(|e| anyhow!("V3 query_swap_exact_amount_out RPC call failed: {e:?}"))?;

        // The result is (amounts_out, tokens_out, amounts_in)
        // For exact amount out, we want the input amount (first element of amounts_in)
        ensure!(
            !swap_out_return.amountsIn.is_empty(),
            "V3 query_swap_exact_out returned no input amounts"
        );
        Ok(swap_out_return.amountsIn[0])
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
