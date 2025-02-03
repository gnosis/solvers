use {
    crate::{
        domain::eth,
        infra::{config::dex::file, contracts, dex::zeroex},
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
    /// Chain ID used to automatically determine the address of the settlement
    /// contract and for metrics.
    #[serde_as(as = "serialize::ChainId")]
    chain_id: eth::ChainId,

    /// The versioned URL endpoint for the 0x swap API.
    #[serde(default = "default_endpoint")]
    #[serde_as(as = "serde_with::DisplayFromStr")]
    endpoint: reqwest::Url,

    /// This is needed when configuring 0x to use
    /// the gated API for partners.
    api_key: String,

    /// The list of excluded liquidity sources. Liquidity from these sources
    /// will not be considered when solving.
    #[serde(default)]
    excluded_sources: Vec<String>,
}

fn default_endpoint() -> reqwest::Url {
    "https://api.0x.org/swap/permit2/".parse().unwrap()
}

/// Load the 0x solver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> super::Config {
    let (base, config) = file::load::<Config>(path).await;

    // Note that we just assume Mainnet here - this is because this is the
    // only chain that the 0x solver supports anyway.
    let settlement = contracts::Contracts::for_chain(config.chain_id).settlement;

    super::Config {
        zeroex: zeroex::Config {
            chain_id: config.chain_id,
            endpoint: config.endpoint,
            api_key: config.api_key,
            excluded_sources: config.excluded_sources,
            settlement,
            block_stream: base.block_stream.clone(),
        },
        base,
    }
}
