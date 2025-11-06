use {crate::domain::eth, alloy::primitives::Address};

#[derive(Clone, Debug)]
pub struct Contracts {
    pub settlement: Address,
    pub authenticator: Address,
    pub permit2: Address,
}

impl Contracts {
    pub fn for_chain(chain: eth::ChainId) -> Self {
        Self {
            settlement: contracts::alloy::GPv2Settlement::deployment_address(&(chain as u64))
                .expect("contract address for all supported chains"),
            authenticator: contracts::alloy::GPv2AllowListAuthentication::deployment_address(
                &(chain as u64),
            )
            .expect("contract address for all supported chains"),
            permit2: contracts::alloy::Permit2::deployment_address(&(chain as u64))
                .expect("contract address for all supported chains"),
        }
    }
}
