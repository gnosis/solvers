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
endpoint = 'http://{solver_addr}'
exclude-dexs = ['UniswapV2']
address = '0xE0B3700e0aadcb18ed8d4BFF648Bc99896a18ad1'
partner = 'cow'
        ",
    ))
}

/// Creates a temporary file containing the config of the given solver and node.
pub fn config_with_node(solver_addr: &SocketAddr, node: &SocketAddr) -> tests::Config {
    tests::Config::String(format!(
        r"
node-url = 'http://{node}'
[dex]
endpoint = 'http://{solver_addr}'
exclude-dexs = ['UniswapV2']
address = '0xE0B3700e0aadcb18ed8d4BFF648Bc99896a18ad1'
partner = 'cow'
        ",
    ))
}
