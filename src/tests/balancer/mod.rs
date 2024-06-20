use {crate::tests, std::net::SocketAddr};

mod market_order;
mod not_found;
mod out_of_price;

/// Creates a temporary file containing the config of the given solver.
pub fn config(solver_addr: &SocketAddr) -> tests::Config {
    tests::Config::String(format!(
        r"
node-url = 'http://localhost:8545'
[dex]
endpoint = 'http://{solver_addr}/sor'
chain-id = '1'
        ",
    ))
}

// Copy from src/infra/dex/balancer/dto.rs
pub const SWAP_QUERY: &str = r#"
query sorGetSwapPaths($callDataInput: GqlSwapCallDataInput!, $chain: GqlChain!, $queryBatchSwap: Boolean!, $swapAmount: AmountHumanReadable!, $swapType: GqlSorSwapType!, $tokenIn: String!, $tokenOut: String!, $useProtocolVersion: Int) {
    sorGetSwapPaths(
        callDataInput: $callDataInput,
        chain: $chain,
        queryBatchSwap: $queryBatchSwap,
        swapAmount: $swapAmount,
        swapType: $swapType,
        tokenIn: $tokenIn,
        tokenOut: $tokenOut,
        useProtocolVersion: $useProtocolVersion
    ) {
        tokenAddresses
        swaps {
            poolId
            assetInIndex
            assetOutIndex
            amount
            userData
        }
        swapAmountRaw
        returnAmountRaw
        tokenIn
        tokenOut
    }
}
"#;
