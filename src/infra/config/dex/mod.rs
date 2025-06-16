pub mod balancer;
mod file;
pub mod okx;
pub mod oneinch;
pub mod paraswap;
pub mod zeroex;

use {
    crate::domain::{dex::{minimum_surplus, slippage}, eth},
    ethrpc::block_stream::CurrentBlockWatcher,
    std::num::NonZeroUsize,
};

#[derive(Clone)]
pub struct Contracts {
    pub settlement: eth::ContractAddress,
    pub authenticator: eth::ContractAddress,
}

#[derive(Clone)]
pub struct Config {
    pub node_url: reqwest::Url,
    pub contracts: Contracts,
    pub slippage: slippage::Limits,
    pub minimum_surplus: minimum_surplus::Limits,
    pub concurrent_requests: NonZeroUsize,
    pub smallest_partial_fill: eth::Ether,
    pub rate_limiting_strategy: rate_limit::Strategy,
    pub gas_offset: eth::Gas,
    pub block_stream: Option<CurrentBlockWatcher>,
    pub internalize_interactions: bool,
}
