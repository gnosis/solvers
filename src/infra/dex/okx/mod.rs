use {
    crate::{
        domain::{dex, eth},
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
    /// The URL for the 0KX swap API.
    pub endpoint: reqwest::Url,

    pub chain_id: eth::ChainId,

    /// Credentials used to access OKX API.
    pub okx_credentials: OkxCredentialsConfig,

    /// The stream that yields every new block.
    pub block_stream: Option<CurrentBlockWatcher>,
}

pub struct OkxCredentialsConfig {
    /// OKX project ID to use.
    pub project_id: String,

    /// OKX API key.
    pub api_key: String,

    /// OKX API key additional security token.
    pub api_secret_key: String,

    /// OKX API key passphrase used to encrypt secret key.
    pub api_passphrase: String,
}

impl Okx {
    pub fn try_new(config: Config) -> Result<Self, CreationError> {
        let client = {
            let mut api_key =
                reqwest::header::HeaderValue::from_str(&config.okx_credentials.api_key)?;
            api_key.set_sensitive(true);
            let mut api_passphrase =
                reqwest::header::HeaderValue::from_str(&config.okx_credentials.api_passphrase)?;
            api_passphrase.set_sensitive(true);

            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                "OK-ACCESS-PROJECT",
                reqwest::header::HeaderValue::from_str(&config.okx_credentials.project_id)?,
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
            ..Default::default()
        };

        Ok(Self {
            client,
            endpoint: config.endpoint,
            api_secret_key: config.okx_credentials.api_secret_key,
            defaults,
        })
    }

    /// OKX requires signature of the request to be added as dedicated HTTP
    /// Header. More information on generating the signature can be found in
    /// OKX documentation: https://www.okx.com/en-au/web3/build/docs/waas/rest-authentication#signature
    fn generate_signature(
        &self,
        request: &reqwest::Request,
        timestamp: &str,
    ) -> Result<String, Error> {
        let mut data = String::new();
        data.push_str(timestamp);
        data.push_str(request.method().as_str());
        data.push_str(request.url().path());
        data.push('?');
        data.push_str(request.url().query().ok_or(Error::SignRequestFailed)?);

        let mut mac = Hmac::<Sha256>::new_from_slice(self.api_secret_key.as_bytes())
            .map_err(|_| Error::SignRequestFailed)?;
        mac.update(data.as_bytes());
        let signature = mac.finalize().into_bytes();

        Ok(BASE64_STANDARD.encode(signature))
    }

    /// OKX Error codes: [link](https://www.okx.com/en-au/web3/build/docs/waas/dex-error-code)
    fn handle_api_error(code: i64, message: &str) -> Result<(), Error> {
        Err(match code {
            0 => return Ok(()),
            82000 => Error::NotFound, // Insufficient liquidity
            82104 => Error::NotFound, // Token not supported
            50011 => Error::RateLimited,
            _ => Error::Api {
                code,
                reason: message.to_string(),
            },
        })
    }

    pub async fn swap(
        &self,
        order: &dex::Order,
        slippage: &dex::Slippage,
    ) -> Result<dex::Swap, Error> {
        let query = self
            .defaults
            .clone()
            .with_domain(order, slippage)
            .ok_or(Error::OrderNotSupported)?;
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

        Self::handle_api_error(quote.code, &quote.msg)?;
        let quote_result = quote.data.first().ok_or(Error::NotFound)?;

        // Increasing returned gas by 50% according to the documentation:
        // https://www.okx.com/en-au/web3/build/docs/waas/dex-swap (gas field description in Response param)
        let gas = quote_result
            .tx
            .gas
            .checked_add(quote_result.tx.gas / 2)
            .ok_or(Error::GasCalculationFailed)?;

        Ok(dex::Swap {
            call: dex::Call {
                to: eth::ContractAddress(quote_result.tx.to),
                calldata: quote_result.tx.data.clone(),
            },
            input: eth::Asset {
                token: quote_result
                    .router_result
                    .from_token
                    .token_contract_address
                    .into(),
                amount: quote_result.router_result.from_token_amount,
            },
            output: eth::Asset {
                token: quote_result
                    .router_result
                    .to_token
                    .token_contract_address
                    .into(),
                amount: quote_result.router_result.to_token_amount,
            },
            allowance: dex::Allowance {
                spender: eth::ContractAddress(quote_result.tx.to),
                amount: dex::Amount::new(quote_result.router_result.from_token_amount),
            },
            gas: eth::Gas(gas),
        })
    }

    async fn quote(&self, query: &dto::SwapRequest) -> Result<dto::SwapResponse, Error> {
        let mut request_builder = self
            .client
            .request(reqwest::Method::GET, self.endpoint.clone())
            .query(query);

        let request = request_builder
            .try_clone()
            .ok_or(Error::RequestBuildFailed)?
            .build()
            .map_err(|_| Error::RequestBuildFailed)?;

        let timestamp = &chrono::Utc::now()
            .to_rfc3339_opts(SecondsFormat::Millis, true)
            .to_string();
        let signature = self.generate_signature(&request, timestamp)?;

        request_builder = request_builder.header(
            "OK-ACCESS-TIMESTAMP",
            reqwest::header::HeaderValue::from_str(timestamp)
                .map_err(|_| Error::RequestBuildFailed)?,
        );
        request_builder = request_builder.header(
            "OK-ACCESS-SIGN",
            HeaderValue::from_str(&signature).map_err(|_| Error::RequestBuildFailed)?,
        );

        let quote = util::http::roundtrip!(
            <dto::SwapResponse, dto::Error>;
            request_builder
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
    #[error("failed to build the request")]
    RequestBuildFailed,
    #[error("failed to sign the request")]
    SignRequestFailed,
    #[error("calculating output gas failed")]
    GasCalculationFailed,
    #[error("unable to find a quote")]
    NotFound,
    #[error("order type is not supported")]
    OrderNotSupported,
    #[error("rate limited")]
    RateLimited,
    #[error("api error code {code}: {reason}")]
    Api { code: i64, reason: String },
    #[error(transparent)]
    Http(util::http::Error),
}

impl From<util::http::RoundtripError<dto::Error>> for Error {
    // This function is only called when swap response body is not a valid json.
    // OKX is returning valid json for 4xx HTTP codes, and the errors are handled in
    // dedicated function: handle_api_error().
    fn from(err: util::http::RoundtripError<dto::Error>) -> Self {
        match err {
            util::http::RoundtripError::Http(err) => {
                if let util::http::Error::Status(code, _) = err {
                    match code {
                        StatusCode::TOO_MANY_REQUESTS => Self::RateLimited,
                        _ => Self::Http(err),
                    }
                } else {
                    Self::Http(err)
                }
            }
            util::http::RoundtripError::Api(err) => match err.code {
                429 => Self::RateLimited,
                _ => Self::Api {
                    code: err.code,
                    reason: err.reason,
                },
            },
        }
    }
}
