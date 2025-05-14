use {
    crate::{
        domain::eth,
        infra::{self, config::dex::file, dex},
        util::serialize,
    },
    contracts::BalancerV2Vault,
    ethereum_types::H160,
    serde::Deserialize,
    serde_with::serde_as,
    std::path::Path,
};

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Config {
    /// The URL of the Balancer SOR API.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    endpoint: reqwest::Url,

    /// Optional Balancer V2 Vault contract address. If not specified, the
    /// default Vault contract address will be used.
    vault: Option<H160>,

    /// Optional Balancer V3 BatchRouter contract address. If not specified, the
    /// default contract address will be used.
    v3_batch_router: Option<H160>,

    /// Optional Permit2 contract address. If not specified, the
    /// default contract address will be used.
    permit2: Option<H160>,

    /// Chain ID used to automatically determine contract addresses and send to
    /// the SOR API.
    #[serde_as(as = "serialize::ChainId")]
    chain_id: eth::ChainId,

    /// Whether to run `queryBatchSwap` to update the return amount with most
    /// up-to-date on-chain values.
    query_batch_swap: Option<bool>,
}

/// Load the driver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> super::Config {
    let (base, config) = file::load::<Config>(path).await;
    let contracts = infra::contracts::Contracts::for_chain(config.chain_id);
    let vault_contract = infra::contracts::contract_address_for_chain(
        config.chain_id,
        BalancerV2Vault::raw_contract(),
    );
    // Balancer V3 is not currently supported on Polygon.
    let batch_router = (config.chain_id != eth::ChainId::Polygon).then(|| {
        infra::contracts::contract_address_for_chain(
            config.chain_id,
            contracts::BalancerV3BatchRouter::raw_contract(),
        )
    });

    super::Config {
        sor: dex::balancer::Config {
            endpoint: config.endpoint,
            vault: config
                .vault
                .map(eth::ContractAddress)
                .unwrap_or(vault_contract),
            v3_batch_router: config
                .v3_batch_router
                .map(eth::ContractAddress)
                .or(batch_router),
            permit2: config
                .permit2
                .map(eth::ContractAddress)
                .unwrap_or(contracts.permit2),
            settlement: base.contracts.settlement,
            block_stream: base.block_stream.clone(),
            chain_id: config.chain_id,
            query_batch_swap: config.query_batch_swap.unwrap_or(false),
        },
        base,
    }
}
