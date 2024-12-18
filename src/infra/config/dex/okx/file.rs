use {
    crate::{
        domain::eth,
        infra::{config::dex::file, contracts, dex::okx},
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
    /// The versioned URL endpoint for the 0x swap API.
    #[serde(default = "default_endpoint")]
    #[serde_as(as = "serde_with::DisplayFromStr")]
    endpoint: reqwest::Url,

    /// Chain ID used to automatically determine contract addresses.
    #[serde_as(as = "serialize::ChainId")]
    chain_id: eth::ChainId,

    pub api_project_id: String,

    pub api_key: String,

    pub api_secret_key: String,

    pub api_passphrase: String,
}

fn default_endpoint() -> reqwest::Url {
    "https://api.0x.org/swap/v1/".parse().unwrap()
}

fn default_affiliate() -> H160 {
    contracts::Contracts::for_chain(eth::ChainId::Mainnet)
        .settlement
        .0
}

/// Load the 0x solver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> super::Config {
    let (base, config) = file::load::<Config>(path).await;

    let settlement = contracts::Contracts::for_chain(eth::ChainId::Mainnet).settlement;

    super::Config {
        okx: okx::Config {
            chain_id: config.chain_id,
            project_id: config.api_project_id,
            api_key: config.api_key,
            api_secret_key: config.api_secret_key,
            api_passphrase: config.api_passphrase,
            endpoint: config.endpoint,
            settlement,
            block_stream: base.block_stream.clone(),
        },
        base,
    }
}
