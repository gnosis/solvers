use {
    crate::{
        domain::{dex, eth, order},
        util,
    },
    base64::prelude::*,
    chrono::SecondsFormat,
    ethrpc::block_stream::CurrentBlockWatcher,
    hmac::{Hmac, Mac},
    hyper::{header::HeaderValue, StatusCode},
    sha2::Sha256,
    std::sync::atomic::{self, AtomicU64},
    tracing::Instrument,
};

mod dto;

/// Bindings to the OKX swap API.
pub struct Okx {
    client: super::Client,
    endpoint: reqwest::Url,
    api_secret_key: String,
    defaults: dto::SwapRequest,
}

pub struct Config {
    /// The base URL for the 0KX swap API.
    pub endpoint: reqwest::Url,

    pub chain_id: eth::ChainId,

    /// OKX project ID to use. Instruction on how to create project:
    /// https://www.okx.com/en-au/web3/build/docs/waas/introduction-to-developer-portal-interface#create-project
    pub project_id: String,

    /// OKX API key. Instruction on how to generate API key:
    /// https://www.okx.com/en-au/web3/build/docs/waas/introduction-to-developer-portal-interface#generate-api-keys
    pub api_key: String,

    /// OKX API key additional security token. Instruction on how to get
    /// security token: https://www.okx.com/en-au/web3/build/docs/waas/introduction-to-developer-portal-interface#view-the-secret-key
    pub api_secret_key: String,

    /// OKX API key passphrase used to encrypt secrety key. Instruction on how
    /// to get passhprase: https://www.okx.com/en-au/web3/build/docs/waas/introduction-to-developer-portal-interface#generate-api-keys
    pub api_passphrase: String,

    /// The address of the settlement contract.
    pub settlement: eth::ContractAddress,

    /// The stream that yields every new block.
    pub block_stream: Option<CurrentBlockWatcher>,
}

impl Okx {
    pub fn try_new(config: Config) -> Result<Self, CreationError> {
        let client = {
            let mut api_key = reqwest::header::HeaderValue::from_str(&config.api_key)?;
            api_key.set_sensitive(true);
            let mut api_passphrase =
                reqwest::header::HeaderValue::from_str(&config.api_passphrase)?;
            api_passphrase.set_sensitive(true);

            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                "OK-ACCESS-PROJECT",
                reqwest::header::HeaderValue::from_str(&config.project_id)?,
            );
            headers.insert("OK-ACCESS-KEY", api_key);
            headers.insert("OK-ACCESS-PASSPHRASE", api_passphrase);

            let client = reqwest::Client::builder()
                .default_headers(headers)
                .build()?;
            super::Client::new(client, config.block_stream)
        };

        let defaults = dto::SwapRequest {
            chain_id: config.chain_id as u64,
            user_wallet_address: config.settlement.0,
            ..Default::default()
        };

        Ok(Self {
            client,
            endpoint: config.endpoint,
            api_secret_key: config.api_secret_key,
            defaults,
        })
    }

    fn sign_request(&self, request: &reqwest::Request, timestamp: &str) -> String {
        let mut data = String::new();
        data.push_str(timestamp);
        data.push_str(request.method().as_str());
        data.push_str(request.url().path());
        data.push('?');
        data.push_str(
            request
                .url()
                .query()
                .expect("Request query cannot be empty."),
        );

        type HmacSha256 = Hmac<Sha256>;
        // Safe to unwrap as HMAC can take key of any size
        let mut mac = HmacSha256::new_from_slice(self.api_secret_key.as_bytes()).unwrap();
        mac.update(data.as_bytes());
        let signature = mac.finalize().into_bytes();

        BASE64_STANDARD.encode(signature)
    }

    pub async fn swap(
        &self,
        order: &dex::Order,
        slippage: &dex::Slippage,
    ) -> Result<dex::Swap, Error> {
        let query = self.defaults.clone().with_domain(order, slippage);
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

        let quote_result = quote.data.first().ok_or(Error::NotFound)?;

        let max_sell_amount = match order.side {
            order::Side::Buy => slippage.add(quote_result.router_result.from_token_amount),
            order::Side::Sell => quote_result.router_result.from_token_amount,
        };

        Ok(dex::Swap {
            call: dex::Call {
                to: eth::ContractAddress(quote_result.tx.to),
                calldata: quote_result.tx.data.clone(),
            },
            input: eth::Asset {
                token: order.sell,
                amount: quote_result.router_result.from_token_amount,
            },
            output: eth::Asset {
                token: order.buy,
                amount: quote_result.router_result.to_token_amount,
            },
            allowance: dex::Allowance {
                spender: eth::ContractAddress(quote_result.tx.to),
                amount: dex::Amount::new(max_sell_amount),
            },
            gas: eth::Gas(quote_result.tx.gas), // todo ms: increase by 50% according to docs?
        })
    }

    async fn quote(&self, query: &dto::SwapRequest) -> Result<dto::SwapResponse, Error> {
        let request_builder = self
            .client
            .request(
                reqwest::Method::GET,
                util::url::join(&self.endpoint, "swap"),
            )
            .query(query);

        let quote = util::http::roundtrip!(
            <dto::SwapResponse, dto::Error>;
            request_builder,
            |request| {
                let timestamp = &chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true).to_string();
                let signature = self.sign_request(request, timestamp);
                request.headers_mut().insert(
                    "OK-ACCESS-TIMESTAMP",
                    // Safe to unwrap as timestamp in RFC3339 format is a valid HTTP header value.
                    reqwest::header::HeaderValue::from_str(&timestamp).unwrap(),
                );
                request.headers_mut().insert("OK-ACCESS-SIGN", HeaderValue::from_str(&signature)
                    .expect("Request sign header value has invalid characters: {signature}"));
            }
        )
        .await?;
        Ok(quote)
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
    #[error("unable to find a quote")]
    NotFound,
    #[error("quote does not specify an approval spender")]
    MissingSpender,
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
                if let util::http::Error::Status(code, _) = err {
                    match code {
                        StatusCode::TOO_MANY_REQUESTS => Self::RateLimited,
                        StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS => {
                            Self::UnavailableForLegalReasons
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
