use {crate::tests, std::net::SocketAddr};

mod market_order;
mod not_found;
mod out_of_price;

/// Creates a temporary file containing the config of the given solver.
pub fn config(solver_addr: &SocketAddr) -> tests::Config {
    tests::Config::String(format!(
        r"
node-url = 'http://localhost:8545'
risk-parameters = [0,0,0,0]
[dex]
chain-id = '1'
endpoint = 'http://{solver_addr}'
exclude-liquidity = ['UNISWAP_V3', 'PMM4']
        ",
    ))
}

/// Creates a temporary file containing the config of the given solver and node.
pub fn config_with_node(solver_addr: &SocketAddr, node: &SocketAddr) -> tests::Config {
    tests::Config::String(format!(
        r"
node-url = 'http://{node}'
risk-parameters = [0,0,0,0]
[dex]
chain-id = '1'
endpoint = 'http://{solver_addr}'
exclude-liquidity = ['UNISWAP_V3', 'PMM4']
        ",
    ))
}
