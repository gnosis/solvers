use crate::domain::eth;

#[derive(Clone, Debug)]
pub struct Contracts {
    pub weth: eth::WethAddress,
    pub settlement: eth::ContractAddress,
    pub authenticator: eth::ContractAddress,
    pub balancer_v2_vault: eth::ContractAddress,
    pub balancer_v3_batch_router: eth::ContractAddress,
    pub balancer_v3_vault: eth::ContractAddress,
}

impl Contracts {
    pub fn for_chain(chain: eth::ChainId) -> Self {
        let a = |contract: &contracts::ethcontract::Contract| {
            eth::ContractAddress(
                contract
                    .networks
                    .get(chain.network_id())
                    .expect("contract address for all supported chains")
                    .address,
            )
        };
        Self {
            weth: eth::WethAddress(a(contracts::WETH9::raw_contract()).0),
            settlement: a(contracts::GPv2Settlement::raw_contract()),
            authenticator: a(contracts::GPv2AllowListAuthentication::raw_contract()),
            balancer_v2_vault: a(contracts::BalancerV2Vault::raw_contract()),
            balancer_v3_batch_router: a(contracts::BalancerV3BatchRouter::raw_contract()),
            balancer_v3_vault: a(contracts::BalancerV3Vault::raw_contract()),
        }
    }
}
