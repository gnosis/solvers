use {
    crate::{
        domain::{dex, eth},
        infra::blockchain,
    },
    alloy::primitives::U256,
    contracts::{
        alloy::support::{
            AnyoneAuthenticator,
            Swapper::{
                self,
                Swapper::{Allowance, Asset, Interaction},
            },
        },
        ethcontract::{state_overrides::StateOverride, web3},
    },
    ethrpc::alloy::conversions::{IntoAlloy, IntoLegacy},
    std::collections::HashMap,
};

/// A DEX swap simulator.
#[derive(Debug, Clone)]
pub struct Simulator {
    web3: ethrpc::Web3,
    settlement: eth::ContractAddress,
    authenticator: eth::ContractAddress,
}

impl Simulator {
    /// Create a new simulator for computing DEX swap gas usage.
    pub fn new(
        url: &reqwest::Url,
        settlement: eth::ContractAddress,
        authenticator: eth::ContractAddress,
    ) -> Self {
        Self {
            web3: blockchain::rpc(url),
            settlement,
            authenticator,
        }
    }

    /// Simulate the gas needed by a single order DEX swap.
    ///
    /// This will return a `None` if the gas simulation is unavailable.
    pub async fn gas(
        &self,
        owner: ethereum_types::Address,
        swap: &dex::Swap,
    ) -> Result<eth::Gas, Error> {
        if owner == self.settlement.0 {
            // we can't have both the settlement and swapper contracts at the same address
            return Err(Error::SettlementContractIsOwner);
        }

        let swapper = Swapper::Instance::new(owner.into_alloy(), self.web3.alloy.clone());

        let overrides = HashMap::<_, _>::from_iter([
            // Setup up our trader code that actually executes the settlement
            (
                swapper.address().into_legacy(),
                StateOverride {
                    code: Some(Swapper::Swapper::DEPLOYED_BYTECODE.clone().into_legacy()),
                    ..Default::default()
                },
            ),
            // Override the CoW protocol solver authenticator with one that
            // allows any address to solve
            (
                self.authenticator.0,
                StateOverride {
                    code: Some(
                        AnyoneAuthenticator::AnyoneAuthenticator::DEPLOYED_BYTECODE
                            .clone()
                            .into_legacy(),
                    ),
                    ..Default::default()
                },
            ),
        ]);

        let swapper_calls_arg = swap
            .calls
            .iter()
            .map(|call| Interaction {
                target: call.to,
                value: U256::ZERO,
                callData: alloy::primitives::Bytes::copy_from_slice(&call.calldata),
            })
            .collect();
        let sell = Asset {
            token: swap.input.token.0.into_alloy(),
            amount: swap.input.amount.into_alloy(),
        };
        let buy = Asset {
            token: swap.output.token.0.into_alloy(),
            amount: swap.output.amount.into_alloy(),
        };
        let allowance = Allowance {
            spender: swap.allowance.spender,
            amount: swap.allowance.amount.get().into_alloy(),
        };
        let gas = swapper
            .swap(
                self.settlement.0.into_alloy(),
                sell,
                buy,
                allowance,
                swapper_calls_arg,
            )
            .call()
            .overrides(overrides.into_alloy())
            .await?;

        // `gas == 0` means that the simulation is not possible. See
        // `Swapper.sol` contract for more details. In this case, use the
        // heuristic gas amount from the swap.
        Ok(if gas.is_zero() {
            tracing::info!(
                gas = ?swap.gas,
                "could not simulate dex swap to get gas used; fall back to gas estimate provided \
                 by dex API"
            );
            swap.gas
        } else {
            eth::Gas(gas.into_legacy())
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("web3 error: {0:?}")]
    Web3(#[from] web3::error::Error),

    #[error("contract call error: {0:?}")]
    ContractCall(#[from] alloy::contract::Error),

    #[error("can't simulate gas for an order for which the settlement contract is the owner")]
    SettlementContractIsOwner,
}
