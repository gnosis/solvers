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
    /// The URL endpoint for the OKX swap API for sell orders (exactIn mode).
    /// Uses V6 API by default.
    #[serde(default = "default_sell_orders_endpoint")]
    #[serde_as(as = "serde_with::DisplayFromStr")]
    sell_orders_endpoint: reqwest::Url,

    /// The URL endpoint for the OKX swap API for buy orders (exactOut mode).
    /// If specified, the URL must point to the V5 API. Otherwise, buy orders
    /// will be ignored.
    #[serde(default)]
    #[serde_as(as = "Option<serde_with::DisplayFromStr>")]
    buy_orders_endpoint: Option<reqwest::Url>,

    /// Optional base URL to use for signature generation for sell orders.
    /// This is useful when requests go through a proxy but signatures must be
    /// generated using the original OKX API URL path.
    /// If not specified, uses sell_orders_endpoint for signature generation.
    #[serde(default)]
    #[serde_as(as = "Option<serde_with::DisplayFromStr>")]
    sell_orders_signature_base_url: Option<reqwest::Url>,

    /// Optional base URL to use for signature generation for buy orders.
    /// This is useful when requests go through a proxy but signatures must be
    /// generated using the original OKX API URL path.
    /// If not specified, uses buy_orders_endpoint for signature generation.
    #[serde(default)]
    #[serde_as(as = "Option<serde_with::DisplayFromStr>")]
    buy_orders_signature_base_url: Option<reqwest::Url>,

    /// Chain ID used to automatically determine contract addresses.
    #[serde_as(as = "serialize::ChainId")]
    chain_id: eth::ChainId,

    /// OKX API credentials
    #[serde(flatten)]
    okx_credentials: OkxCredentialsConfig,

    /// The percentage (between 0.0 - 1.0) of the price impact allowed.
    /// When set to 1.0 (100%), the feature is disabled (default).
    /// Note: OKX API default is 0.9 (90%) if this parameter is NOT sent,
    /// but we default to 1.0 to disable the feature by default.
    #[serde(default = "default_price_impact_protection_percent")]
    price_impact_protection_percent: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct OkxCredentialsConfig {
    /// OKX Project ID. Instruction on how to create a project:
    /// https://www.okx.com/en-au/web3/build/docs/waas/introduction-to-developer-portal-interface#create-project
    api_project_id: String,

    /// OKX API Key. Instruction on how to generate an API key:
    /// https://www.okx.com/en-au/web3/build/docs/waas/introduction-to-developer-portal-interface#generate-api-keys
    api_key: String,

    /// OKX Secret key used for signing request. Instruction on how to get a
    /// security token: https://www.okx.com/en-au/web3/build/docs/waas/introduction-to-developer-portal-interface#view-the-secret-key
    api_secret_key: String,

    /// OKX Secret key passphrase. Instruction on how to get a passphrase:
    /// https://www.okx.com/en-au/web3/build/docs/waas/introduction-to-developer-portal-interface#generate-api-keys
    api_passphrase: String,
}

#[allow(clippy::from_over_into)]
impl Into<okx::OkxCredentialsConfig> for OkxCredentialsConfig {
    fn into(self) -> okx::OkxCredentialsConfig {
        okx::OkxCredentialsConfig {
            project_id: self.api_project_id,
            api_key: self.api_key,
            api_secret_key: self.api_secret_key,
            api_passphrase: self.api_passphrase,
        }
    }
}

fn default_sell_orders_endpoint() -> reqwest::Url {
    okx::DEFAULT_SELL_ORDERS_ENDPOINT.parse().unwrap()
}

fn default_price_impact_protection_percent() -> f64 {
    1.0 // 100% - feature disabled by default
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
            sell_orders_endpoint: config.sell_orders_endpoint,
            buy_orders_endpoint: config.buy_orders_endpoint,
            sell_orders_signature_base_url: config.sell_orders_signature_base_url,
            buy_orders_signature_base_url: config.buy_orders_signature_base_url,
            chain_id: config.chain_id,
            okx_credentials: config.okx_credentials.into(),
            block_stream: base.block_stream.clone(),
            settlement_contract: base.contracts.settlement,
            price_impact_protection_percent: config.price_impact_protection_percent,
        },
        base,
    }
}
