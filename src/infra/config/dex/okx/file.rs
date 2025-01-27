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
    /// The versioned URL endpoint for the OKX swap API.
    #[serde(default = "default_endpoint")]
    #[serde_as(as = "serde_with::DisplayFromStr")]
    endpoint: reqwest::Url,

    /// Chain ID used to automatically determine contract addresses.
    #[serde_as(as = "serialize::ChainId")]
    chain_id: eth::ChainId,

    /// OKX API credentials
    #[serde(flatten)]
    okx_credentials: OkxCredentialsConfig,
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

fn default_endpoint() -> reqwest::Url {
    "https://www.okx.com/api/v5/dex/aggregator/"
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
            endpoint: config.endpoint,
            chain_id: config.chain_id,
            okx_credentials: config.okx_credentials.into(),
            block_stream: base.block_stream.clone(),
        },
        base,
    }
}
