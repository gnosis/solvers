use {crate::tests, std::net::SocketAddr};

mod limit_order;
mod limit_order_quoting;
mod minimum_surplus;
mod mock_query_swap_provider;
mod not_found;
mod out_of_price;

/// Creates a temporary file containing the config of the given solver.
pub fn config(solver_addr: &SocketAddr, node_addr: &SocketAddr) -> tests::Config {
    tests::Config::String(format!(
        r"
node-url = 'http://{node_addr}'
[dex]
endpoint = 'http://{solver_addr}/sor'
chain-id = '1'
        ",
    ))
}

/// Creates a temporary file containing the config of the given solver with
/// custom top-level settings.
pub fn config_with(
    solver_addr: &SocketAddr,
    node_addr: &SocketAddr,
    extra_config: &str,
) -> tests::Config {
    tests::Config::String(format!(
        r"
node-url = 'http://{node_addr}'
{extra_config}
[dex]
endpoint = 'http://{solver_addr}/sor'
chain-id = '1'
        ",
    ))
}

// Copy from src/infra/dex/balancer/dto.rs
pub const SWAP_QUERY: &str = r#"
query sorGetSwapPaths($chain: GqlChain!, $swapAmount: AmountHumanReadable!, $swapType: GqlSorSwapType!, $tokenIn: String!, $tokenOut: String!) {
    sorGetSwapPaths(
        chain: $chain,
        swapAmount: $swapAmount,
        swapType: $swapType,
        tokenIn: $tokenIn,
        tokenOut: $tokenOut,
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
        protocolVersion
        paths {
            inputAmountRaw
            isBuffer
            outputAmountRaw
            pools
            protocolVersion
            tokens {
              address
            }
        }
    }
}
"#;
