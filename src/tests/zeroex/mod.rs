use {crate::tests, std::net::SocketAddr};

mod market_order;
mod not_found;
mod options;
mod out_of_price;

/// Creates a temporary file containing the config of the given solver and node.
pub fn config_with_node(solver_addr: &SocketAddr, node: &SocketAddr) -> tests::Config {
    tests::Config::String(format!(
        r"
node-url = 'http://{node}'
[dex]
endpoint = 'http://{solver_addr}/swap/v1/'
api-key = 'SUPER_SECRET_API_KEY'
        ",
    ))
}

/// Creates a temporary file containing the config of the given solver.
/// Does not have access to a node so only suitable for tests that not rely on
/// that.
pub fn config(solver_addr: &SocketAddr) -> tests::Config {
    tests::Config::String(format!(
        r"
node-url = 'http://localhost:8545'
[dex]
endpoint = 'http://{solver_addr}/swap/v1/'
api-key = 'SUPER_SECRET_API_KEY'
        ",
    ))
}
