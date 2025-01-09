use {
    crate::{
        domain::eth,
        infra::{config::dex::file, dex::okx},
        util::serialize,
    },
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

    /// OKX Project ID.
    api_project_id: String,

    /// OKX API Key.
    api_key: String,

    /// OKX Secret key used for signing request.
    api_secret_key: String,

    /// OKX Secret key passphrase.
    api_passphrase: String,
}

fn default_endpoint() -> reqwest::Url {
    "https://www.okx.com/api/v5/dex/aggregator/swap"
        .parse()
        .unwrap()
}

/// Load the OKX solver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> super::Config {
    let (base, config) = file::load::<Config>(path).await;

    super::Config {
        okx: okx::Config {
            chain_id: config.chain_id,
            project_id: config.api_project_id,
            api_key: config.api_key,
            api_secret_key: config.api_secret_key,
            api_passphrase: config.api_passphrase,
            endpoint: config.endpoint,
            block_stream: base.block_stream.clone(),
        },
        base,
    }
}
