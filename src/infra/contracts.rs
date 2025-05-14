use crate::domain::eth;

#[derive(Clone, Debug)]
pub struct Contracts {
    pub weth: eth::WethAddress,
    pub settlement: eth::ContractAddress,
    pub authenticator: eth::ContractAddress,
    pub permit2: eth::ContractAddress,
}

impl Contracts {
    pub fn for_chain(chain: eth::ChainId) -> Self {
        Self {
            weth: eth::WethAddress(
                contract_address_for_chain(chain, contracts::WETH9::raw_contract()).0,
            ),
            settlement: contract_address_for_chain(
                chain,
                contracts::GPv2Settlement::raw_contract(),
            ),
            authenticator: contract_address_for_chain(
                chain,
                contracts::GPv2AllowListAuthentication::raw_contract(),
            ),
            permit2: contract_address_for_chain(chain, contracts::Permit2::raw_contract()),
        }
    }
}

pub fn contract_address_for_chain(
    chain: eth::ChainId,
    contract: &contracts::ethcontract::Contract,
) -> eth::ContractAddress {
    eth::ContractAddress(
        contract
            .networks
            .get(chain.network_id())
            .expect("contract address for all supported chains")
            .address,
    )
}
