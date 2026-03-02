use {
    crate::{
        domain::{dex, eth, order},
        util,
    },
    alloy::primitives::{Address, U256},
    base64::prelude::*,
    ethrpc::block_stream::CurrentBlockWatcher,
    hmac::{Hmac, Mac},
    hyper::StatusCode,
    sha2::Sha256,
    std::{
        collections::BTreeMap,
        sync::atomic::{self, AtomicU64},
    },
    tracing::Instrument,
};

mod dto;

/// Default Bitget swap API base endpoint.
pub const DEFAULT_ENDPOINT: &str = "https://web3.bitget.com/";

/// Bindings to the Bitget swap API.
pub struct Bitget {
    client: super::Client,
    endpoint: reqwest::Url,
    api_key: String,
    api_secret: String,
    chain_name: String,
    settlement_contract: Address,
}

pub struct Config {
    /// The base URL for the Bitget swap API.
    pub endpoint: reqwest::Url,

    pub chain_id: eth::ChainId,

    pub settlement_contract: Address,

    /// Credentials used to access Bitget API.
    pub credentials: BitgetCredentialsConfig,

    /// The stream that yields every new block.
    pub block_stream: Option<CurrentBlockWatcher>,
}

pub struct BitgetCredentialsConfig {
    /// Bitget API key.
    pub api_key: String,

    /// Bitget API secret for signing requests.
    pub api_secret: String,
}

impl Bitget {
    pub fn try_new(config: Config) -> Result<Self, CreationError> {
        let client = {
            let client = reqwest::Client::builder().build()?;
            super::Client::new(client, config.block_stream)
        };

        let chain_name = dto::chain_name(config.chain_id).to_string();

        Ok(Self {
            client,
            endpoint: config.endpoint,
            api_key: config.credentials.api_key,
            api_secret: config.credentials.api_secret,
            chain_name,
            settlement_contract: config.settlement_contract,
        })
    }

    pub async fn swap(
        &self,
        order: &dex::Order,
        slippage: &dex::Slippage,
    ) -> Result<dex::Swap, Error> {
        // Bitget only supports sell orders (exactIn).
        if order.side == order::Side::Buy {
            return Err(Error::OrderNotSupported);
        }

        static ID: AtomicU64 = AtomicU64::new(0);
        let id = ID.fetch_add(1, atomic::Ordering::Relaxed);

        let (swap_response, quote_amounts) = self
            .handle_sell_order(order, slippage)
            .instrument(tracing::trace_span!("swap", id = %id))
            .await?;

        let calldata = swap_response
            .decode_calldata()
            .map_err(|_| Error::InvalidCalldata)?;

        let contract = swap_response
            .parse_contract()
            .map_err(|_| Error::InvalidContract)?;

        // Increase gas estimate by 50% for safety margin, similar to OKX.
        let gas = quote_amounts
            .gas_limit
            .checked_add(quote_amounts.gas_limit / U256::from(2))
            .ok_or(Error::GasCalculationFailed)?;

        Ok(dex::Swap {
            calls: vec![dex::Call {
                to: contract,
                calldata,
            }],
            input: eth::Asset {
                token: order.sell,
                amount: order.amount.get(),
            },
            output: eth::Asset {
                token: order.buy,
                amount: quote_amounts.to_amount,
            },
            allowance: dex::Allowance {
                spender: contract,
                amount: dex::Amount::new(order.amount.get()),
            },
            gas: eth::Gas(gas),
        })
    }

    /// Handle sell orders with sequential API requests.
    ///
    /// Step 1: Get a quote to obtain the `market` channel and output amount.
    /// Step 2: Get the swap calldata using the market from the quote.
    async fn handle_sell_order(
        &self,
        order: &dex::Order,
        slippage: &dex::Slippage,
    ) -> Result<(dto::SwapResponse, dto::QuoteAmounts), Error> {
        // Step 1: Get quote
        let quote_request =
            dto::QuoteRequest::from_order(order, &self.chain_name, self.settlement_contract);

        let quote_response: dto::QuoteResponse = self
            .send_post_request("bgw-pro/swapx/pro/quote", &quote_request)
            .await?;

        let quote_amounts = quote_response
            .parse_amounts()
            .map_err(|_| Error::InvalidQuoteResponse)?;

        // Step 2: Get swap calldata
        let swap_request = dto::SwapRequest::from_order(
            order,
            slippage,
            &self.chain_name,
            self.settlement_contract,
            quote_amounts.market.clone(),
        );

        let swap_response: dto::SwapResponse = self
            .send_post_request("bgw-pro/swapx/pro/swap", &swap_request)
            .await?;

        Ok((swap_response, quote_amounts))
    }

    /// Generate HMAC-SHA256 signature for the Bitget API.
    ///
    /// The signature is computed over a JSON object with alphabetically sorted
    /// keys containing: the API path, body, API key, and timestamp.
    fn generate_signature(
        &self,
        api_path: &str,
        body: &str,
        timestamp: &str,
    ) -> Result<String, Error> {
        let mut content = BTreeMap::new();
        content.insert("apiPath", api_path);
        content.insert("body", body);
        content.insert("x-api-key", &self.api_key);
        content.insert("x-api-timestamp", timestamp);

        let content_str = serde_json::to_string(&content).map_err(|_| Error::SignRequestFailed)?;

        let mut mac = Hmac::<Sha256>::new_from_slice(self.api_secret.as_bytes())
            .map_err(|_| Error::SignRequestFailed)?;
        mac.update(content_str.as_bytes());
        let signature = mac.finalize().into_bytes();

        Ok(BASE64_STANDARD.encode(signature))
    }

    /// Bitget error handling based on status codes.
    fn handle_api_error(status: i64) -> Result<(), Error> {
        Err(match status {
            0 => return Ok(()),
            429 => Error::RateLimited,
            // Treat unknown error codes as "not found" since the API doesn't
            // document specific error codes for insufficient liquidity.
            40004 => Error::NotFound,
            _ => Error::Api { code: status },
        })
    }

    async fn send_post_request<T, U>(&self, endpoint: &str, body: &T) -> Result<U, Error>
    where
        T: serde::Serialize,
        U: serde::de::DeserializeOwned,
    {
        let url = self
            .endpoint
            .join(endpoint)
            .map_err(|_| Error::RequestBuildFailed)?;

        let body_str = serde_json::to_string(body).map_err(|_| Error::RequestBuildFailed)?;

        let timestamp = chrono::Utc::now().timestamp_millis().to_string();

        let api_path = format!("/{endpoint}");
        let signature = self.generate_signature(&api_path, &body_str, &timestamp)?;

        let request_builder = self
            .client
            .request(reqwest::Method::POST, url)
            .header("Content-Type", "application/json")
            .header("x-api-key", &self.api_key)
            .header("x-api-timestamp", &timestamp)
            .header("x-api-signature", &signature)
            .body(body_str);

        let response = util::http::roundtrip!(
            <dto::Response<U>, dto::Error>;
            request_builder
        )
        .await?;

        Self::handle_api_error(response.status)?;
        Ok(response.data)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CreationError {
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
    #[error("invalid calldata in response")]
    InvalidCalldata,
    #[error("invalid contract address in response")]
    InvalidContract,
    #[error("invalid quote response")]
    InvalidQuoteResponse,
    #[error("api error code {code}")]
    Api { code: i64 },
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
                        _ => Self::Http(err),
                    }
                } else {
                    Self::Http(err)
                }
            }
            util::http::RoundtripError::Api(err) => match err.status {
                429 => Self::RateLimited,
                _ => Self::Api { code: err.status },
            },
        }
    }
}
