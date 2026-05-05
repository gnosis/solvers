//! Local sol! bindings for Balancer V2 query and V3 batch router functions.
//!
//! These functions used to be exposed by the `contracts` crate but were moved
//! into a `_disabled` ABI key upstream (see services PR #4324) on the basis
//! that nothing in `services` referenced them. The solvers repo does, so we
//! redeclare the minimum required interfaces locally.

#![allow(non_snake_case, non_camel_case_types)]

alloy::sol! {
    #[sol(rpc)]
    #[derive(Debug)]
    library IVault {
        struct BatchSwapStep {
            bytes32 poolId;
            uint256 assetInIndex;
            uint256 assetOutIndex;
            uint256 amount;
            bytes userData;
        }

        struct FundManagement {
            address sender;
            bool fromInternalBalance;
            address recipient;
            bool toInternalBalance;
        }
    }

    #[sol(rpc)]
    #[derive(Debug)]
    library IBatchRouter {
        struct SwapPathStep {
            address pool;
            address tokenOut;
            bool isBuffer;
        }

        struct SwapPathExactAmountIn {
            address tokenIn;
            SwapPathStep[] steps;
            uint256 exactAmountIn;
            uint256 minAmountOut;
        }

        struct SwapPathExactAmountOut {
            address tokenIn;
            SwapPathStep[] steps;
            uint256 maxAmountIn;
            uint256 exactAmountOut;
        }
    }

    #[sol(rpc)]
    #[derive(Debug)]
    interface BalancerQueries {
        function queryBatchSwap(
            uint8 kind,
            IVault.BatchSwapStep[] swaps,
            address[] assets,
            IVault.FundManagement funds
        ) external returns (int256[] assetDeltas);
    }

    #[sol(rpc)]
    #[derive(Debug)]
    interface BalancerV3BatchRouter {
        function swapExactIn(
            IBatchRouter.SwapPathExactAmountIn[] paths,
            uint256 deadline,
            bool wethIsEth,
            bytes userData
        ) external payable returns (
            uint256[] pathAmountsOut,
            address[] tokensOut,
            uint256[] amountsOut
        );

        function swapExactOut(
            IBatchRouter.SwapPathExactAmountOut[] paths,
            uint256 deadline,
            bool wethIsEth,
            bytes userData
        ) external payable returns (
            uint256[] pathAmountsIn,
            address[] tokensIn,
            uint256[] amountsIn
        );

        function querySwapExactIn(
            IBatchRouter.SwapPathExactAmountIn[] paths,
            address sender,
            bytes userData
        ) external returns (
            uint256[] pathAmountsOut,
            address[] tokensOut,
            uint256[] amountsOut
        );

        function querySwapExactOut(
            IBatchRouter.SwapPathExactAmountOut[] paths,
            address sender,
            bytes userData
        ) external returns (
            uint256[] pathAmountsIn,
            address[] tokensIn,
            uint256[] amountsIn
        );
    }
}

pub type BalancerQueriesInstance =
    BalancerQueries::BalancerQueriesInstance<alloy::providers::DynProvider>;
pub type BalancerV3BatchRouterInstance =
    BalancerV3BatchRouter::BalancerV3BatchRouterInstance<alloy::providers::DynProvider>;
