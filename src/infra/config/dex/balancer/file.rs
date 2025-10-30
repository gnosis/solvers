use {
    crate::{
        domain::eth,
        infra::{self, config::dex::file, dex},
        util::serialize,
    },
    alloy::primitives::Address,
    contracts::alloy::{BalancerQueries, BalancerV2Vault, BalancerV3BatchRouter},
    ethrpc::alloy::conversions::IntoAlloy,
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
    vault: Option<Address>,

    /// Optional Balancer V3 BatchRouter contract address. If not specified, the
    /// default contract address will be used.
    v3_batch_router: Option<Address>,

    /// Optional Balancer Queries contract address. If not specified, the
    /// default contract address will be used.
    queries: Option<Address>,

    /// Optional Permit2 contract address. If not specified, the
    /// default contract address will be used.
    permit2: Option<Address>,

    /// Chain ID used to automatically determine contract addresses and send to
    /// the SOR API.
    #[serde_as(as = "serialize::ChainId")]
    chain_id: eth::ChainId,

    /// Controls which API versions are enabled.
    /// Absence of this config param means all versions are enabled.
    enabled_api_versions: Option<Vec<ApiVersion>>,
}

#[derive(Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum ApiVersion {
    V2,
    V3,
}

impl ApiVersion {
    fn all() -> Vec<Self> {
        vec![Self::V2, Self::V3]
    }
}

/// Load the driver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> super::Config {
    let (base, config) = file::load::<Config>(path).await;
    let contracts = infra::contracts::Contracts::for_chain(config.chain_id);
    let enabled_api_versions = config.enabled_api_versions.unwrap_or_else(ApiVersion::all);
    let vault_contract = enabled_api_versions
        .contains(&ApiVersion::V2)
        .then(|| BalancerV2Vault::deployment_address(&config.chain_id.value().as_u64()))
        .flatten();
    let batch_router = enabled_api_versions
        .contains(&ApiVersion::V3)
        .then(|| BalancerV3BatchRouter::deployment_address(&config.chain_id.value().as_u64()))
        .flatten();
    let queries_contract = enabled_api_versions.contains(&ApiVersion::V2).then(|| {
        BalancerQueries::deployment_address(&(config.chain_id as u64))
            .expect("Balancer Queries contract not found for chain")
    });

    super::Config {
        sor: dex::balancer::Config {
            endpoint: config.endpoint,
            vault: config.vault.or(vault_contract),
            v3_batch_router: config.v3_batch_router.or(batch_router),
            queries: config.queries.or(queries_contract),
            permit2: config.permit2.unwrap_or(contracts.permit2),
            settlement: base.contracts.settlement.0.into_alloy(),
            block_stream: base.block_stream.clone(),
            chain_id: config.chain_id,
        },
        base,
    }
}
