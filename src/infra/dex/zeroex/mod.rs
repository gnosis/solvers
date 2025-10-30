use {
    crate::{
        domain::{dex, eth},
        util,
    },
    alloy::primitives::{address, Address},
    ethrpc::{alloy::conversions::IntoAlloy, block_stream::CurrentBlockWatcher},
    hyper::StatusCode,
    std::sync::atomic::{self, AtomicU64},
    tracing::Instrument,
};

mod dto;

/// Bindings to the 0x swap API.
pub struct ZeroEx {
    client: super::Client,
    endpoint: reqwest::Url,
    defaults: dto::Query,
}

/// https://0x.org/docs/introduction/0x-cheat-sheet#0x-contracts
const DEFAULT_ALLOWANCE_TARGET: Address = address!("0x0000000000001fF3684f28c67538d4D072C22734");

pub struct Config {
    /// The chain ID identifying the network to use for all requests.
    pub chain_id: eth::ChainId,

    /// The base URL for the 0x swap API.
    pub endpoint: reqwest::Url,

    /// 0x provides a gated API for partners that requires authentication
    /// by specifying this as header in the HTTP request.
    pub api_key: String,

    /// The list of excluded liquidity sources. Liquidity from these sources
    /// will not be considered when solving.
    pub excluded_sources: Vec<String>,
    /// The address of the settlement contract.
    pub settlement: eth::ContractAddress,

    /// The stream that yields every new block.
    pub block_stream: Option<CurrentBlockWatcher>,
}

impl ZeroEx {
    pub fn new(config: Config) -> Result<Self, CreationError> {
        let client = {
            let mut key = reqwest::header::HeaderValue::from_str(&config.api_key)?;
            key.set_sensitive(true);

            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("0x-api-key", key);
            headers.insert(
                "0x-version",
                reqwest::header::HeaderValue::from_static("v2"),
            );

            let client = reqwest::Client::builder()
                .default_headers(headers)
                .build()?;
            super::Client::new(client, config.block_stream)
        };
        let defaults = dto::Query {
            taker: config.settlement.0,
            excluded_sources: config.excluded_sources,
            chain_id: config.chain_id.value().as_u64(),
            ..Default::default()
        };

        Ok(Self {
            client,
            endpoint: config.endpoint,
            defaults,
        })
    }

    pub async fn swap(
        &self,
        order: &dex::Order,
        slippage: &dex::Slippage,
    ) -> Result<dex::Swap, Error> {
        let query = self.defaults.clone().try_with_domain(order, slippage)?;
        let quote = {
            // Set up a tracing span to make debugging of API requests easier.
            // Historically, debugging API requests to external DEXs was a bit
            // of a headache.
            static ID: AtomicU64 = AtomicU64::new(0);
            let id = ID.fetch_add(1, atomic::Ordering::Relaxed);
            self.quote(&query)
                .instrument(tracing::trace_span!("quote", id = %id))
                .await?
        };

        Ok(dex::Swap {
            calls: vec![dex::Call {
                to: quote.transaction.to.into_alloy(),
                calldata: quote.transaction.data,
            }],
            input: eth::Asset {
                token: order.sell,
                amount: quote.sell_amount,
            },
            output: eth::Asset {
                token: order.buy,
                amount: quote.buy_amount,
            },
            allowance: dex::Allowance {
                spender: quote
                    .issues
                    .allowance
                    .map(|allowance| allowance.spender.into_alloy())
                    .unwrap_or(DEFAULT_ALLOWANCE_TARGET),
                amount: dex::Amount::new(quote.sell_amount),
            },
            gas: eth::Gas(quote.transaction.gas.ok_or(Error::MissingGasEstimate)?),
        })
    }

    async fn quote(&self, query: &dto::Query) -> Result<dto::ValidQuote, Error> {
        let quote = Into::<Option<dto::ValidQuote>>::into(
            util::http::roundtrip!(
                <dto::Quote, dto::Error>;
                self.client
                    .request(reqwest::Method::GET, util::url::join(&self.endpoint, "quote"))
                    .query(query)
            )
            .await?,
        );

        quote.ok_or(Error::NotFound)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreationError {
    #[error(transparent)]
    Header(#[from] reqwest::header::InvalidHeaderValue),
    #[error(transparent)]
    Client(#[from] reqwest::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("order type is not supported")]
    OrderNotSupported,
    #[error("gas estimate not available")]
    MissingGasEstimate,
    #[error("unable to find a quote")]
    NotFound,
    #[error("rate limited")]
    RateLimited,
    #[error("sell token or buy token are banned from trading")]
    UnavailableForLegalReasons,
    #[error("api error code {code}: {reason}")]
    Api { code: i64, reason: String },
    #[error(transparent)]
    Http(util::http::Error),
}

impl From<util::http::RoundtripError<dto::Error>> for Error {
    fn from(err: util::http::RoundtripError<dto::Error>) -> Self {
        match err {
            util::http::RoundtripError::Http(err) => {
                if let util::http::Error::Status(code, ref body) = err {
                    match code {
                        StatusCode::TOO_MANY_REQUESTS => Self::RateLimited,
                        StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS => {
                            Self::UnavailableForLegalReasons
                        }
                        StatusCode::UNPROCESSABLE_ENTITY => Self::UnavailableForLegalReasons,
                        // This mapping avoids false positives on some alarms that trigger due to
                        // log messages on the Http variant TODO: remove
                        // when the 0x team finds/fixes the root cause of this errors
                        StatusCode::BAD_REQUEST if body.contains("SWAP_VALIDATION_FAILED") => {
                            Self::NotFound
                        }
                        _ => Self::Http(err),
                    }
                } else {
                    Self::Http(err)
                }
            }
            util::http::RoundtripError::Api(err) => {
                // Unfortunately, AFAIK these codes aren't documented anywhere. These
                // based on empirical observations of what the API has returned in the
                // past.
                match err.code {
                    100 => Self::NotFound,
                    429 => Self::RateLimited,
                    422 => Self::UnavailableForLegalReasons,
                    451 => Self::UnavailableForLegalReasons,
                    _ => Self::Api {
                        code: err.code,
                        reason: err.reason,
                    },
                }
            }
        }
    }
}
