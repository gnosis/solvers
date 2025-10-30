use {crate::domain::eth, alloy::primitives::Address, ethrpc::alloy::conversions::IntoAlloy};

#[derive(Clone, Debug)]
pub struct Contracts {
    pub settlement: Address,
    pub authenticator: Address,
    pub permit2: Address,
}

impl Contracts {
    pub fn for_chain(chain: eth::ChainId) -> Self {
        Self {
            settlement: contract_address_for_chain(
                chain,
                contracts::GPv2Settlement::raw_contract(),
            )
            .into_alloy(),
            authenticator: contract_address_for_chain(
                chain,
                contracts::GPv2AllowListAuthentication::raw_contract(),
            )
            .into_alloy(),
            permit2: contracts::alloy::Permit2::deployment_address(&(chain as u64))
                .expect("contract address for all supported chains"),
        }
    }
}

pub fn contract_address_for_chain(
    chain: eth::ChainId,
    contract: &contracts::ethcontract::Contract,
) -> eth::H160 {
    contract
        .networks
        .get(chain.network_id())
        .expect("contract address for all supported chains")
        .address
}
