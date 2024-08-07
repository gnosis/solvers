use {
    crate::{
        domain::eth::{self, ChainId},
        infra::{config::dex::file, dex::paraswap},
    },
    serde::Deserialize,
    serde_with::serde_as,
    std::path::Path,
};

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Config {
    /// The base URL for the ParaSwap API.
    #[serde_as(as = "Option<serde_with::DisplayFromStr>")]
    pub endpoint: Option<reqwest::Url>,

    /// The DEXs to exclude when using ParaSwap.
    #[serde(default)]
    pub exclude_dexs: Vec<String>,

    /// Whether to throw an error if the USD price is not available.
    #[serde(default)]
    pub ignore_bad_usd_price: bool,

    /// The solver address.
    pub address: eth::H160,

    /// This is needed when configuring ParaSwap to use
    /// the gated API for partners.
    pub api_key: String,

    /// Which partner to identify as to the paraswap API.
    pub partner: String,

    /// Which chain the solver is serving.
    pub chain_id: u64,
}

/// Load the ParaSwap solver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> super::Config {
    let (base, config) = file::load::<Config>(path).await;

    super::Config {
        paraswap: paraswap::Config {
            endpoint: config
                .endpoint
                .unwrap_or_else(|| paraswap::DEFAULT_URL.parse().unwrap()),
            exclude_dexs: config.exclude_dexs,
            ignore_bad_usd_price: config.ignore_bad_usd_price,
            address: config.address,
            api_key: config.api_key,
            partner: config.partner,
            chain_id: ChainId::new(config.chain_id.into()).unwrap(),
            block_stream: base.block_stream.clone(),
        },
        base,
    }
}
