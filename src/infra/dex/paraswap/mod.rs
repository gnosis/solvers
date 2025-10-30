use {
    crate::{
        domain::{auction, dex, eth},
        util,
    },
    ethereum_types::Address,
    ethrpc::{alloy::conversions::IntoAlloy, block_stream::CurrentBlockWatcher},
};

mod dto;

pub const DEFAULT_URL: &str = "https://apiv5.paraswap.io";

/// Bindings to the ParaSwap API.
pub struct ParaSwap {
    client: super::Client,
    config: Config,
}

#[derive(Debug)]
pub struct Config {
    /// The base URL for the ParaSwap API.
    pub endpoint: reqwest::Url,

    /// The DEXs to exclude when using ParaSwap.
    pub exclude_dexs: Vec<String>,

    /// Whether to throw an error if the USD price is not available.
    pub ignore_bad_usd_price: bool,

    /// The solver address.
    pub address: Address,

    /// ParaSwap provides a gated API for partners that requires authentication
    /// by specifying this as header in the HTTP request.
    pub api_key: String,

    /// Our partner name.
    pub partner: String,

    /// For which chain the solver is configured.
    pub chain_id: eth::ChainId,

    /// A stream that yields every new block.
    pub block_stream: Option<CurrentBlockWatcher>,
}

impl ParaSwap {
    /// Tries to initialize a new solver instance. Panics if it fails.
    pub fn new(config: Config) -> Self {
        let mut key = reqwest::header::HeaderValue::from_str(&config.api_key).unwrap();
        key.set_sensitive(true);

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-api-key", key);

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();

        Self {
            client: super::Client::new(client, config.block_stream.clone()),
            config,
        }
    }

    /// Make a request to the `/swap` endpoint.
    pub async fn swap(
        &self,
        order: &dex::Order,
        slippage: &dex::Slippage,
        tokens: &auction::Tokens,
    ) -> Result<dex::Swap, Error> {
        let query = dto::SwapQuery::new(&self.config, order, tokens, slippage)?;
        let swap = util::http::roundtrip!(
            <dto::Swap, dto::Error>;
            self.client.request(reqwest::Method::GET, util::url::join(&self.config.endpoint, "swap"))
                .query(&query)
        )
        .await?;
        Ok(dex::Swap {
            calls: vec![dex::Call {
                to: swap.tx_params.to.into_alloy(),
                calldata: swap.tx_params.data,
            }],
            input: eth::Asset {
                token: order.sell,
                amount: swap.price_route.src_amount,
            },
            output: eth::Asset {
                token: order.buy,
                amount: swap.price_route.dest_amount,
            },
            allowance: dex::Allowance {
                spender: swap.price_route.token_transfer_proxy.into_alloy(),
                amount: dex::Amount::new(swap.price_route.src_amount),
            },
            gas: eth::Gas(swap.price_route.gas_cost),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no swap could be found")]
    NotFound,
    #[error("decimals are missing for the swapped tokens")]
    MissingDecimals,
    #[error("rate limited")]
    RateLimited,
    #[error("api error {0}")]
    Api(String),
    #[error(transparent)]
    Http(util::http::Error),
    #[error("unable to convert slippage to bps: {0:?}")]
    InvalidSlippage(dex::Slippage),
}

impl From<util::http::RoundtripError<dto::Error>> for Error {
    fn from(err: util::http::RoundtripError<dto::Error>) -> Self {
        match err {
            util::http::RoundtripError::Http(http_err) => match http_err {
                util::http::Error::Status(status_code, _) if status_code.as_u16() == 429 => {
                    Self::RateLimited
                }
                other_err => Self::Http(other_err),
            },
            util::http::RoundtripError::Api(err) => match err.error.as_str() {
                "ESTIMATED_LOSS_GREATER_THAN_MAX_IMPACT"
                | "No routes found with enough liquidity"
                | "Too much slippage on quote, please try again" => Self::NotFound,
                "Rate limited" | "Rate limit pricing" | "Rate limit reached" => Self::RateLimited,
                _ => Self::Api(err.error),
            },
        }
    }
}
