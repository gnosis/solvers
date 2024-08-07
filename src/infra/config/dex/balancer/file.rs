use {
    crate::{
        domain::eth,
        infra::{config::dex::file, contracts, dex},
        util::serialize,
    },
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
    let contracts = contracts::Contracts::for_chain(config.chain_id);

    super::Config {
        sor: dex::balancer::Config {
            endpoint: config.endpoint,
            vault: config
                .vault
                .map(eth::ContractAddress)
                .unwrap_or(contracts.balancer_vault),
            settlement: base.contracts.settlement,
            block_stream: base.block_stream.clone(),
            chain_id: config.chain_id,
            query_batch_swap: config.query_batch_swap.unwrap_or(false),
        },
        base,
    }
}
