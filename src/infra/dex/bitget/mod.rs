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
pub const DEFAULT_ENDPOINT: &str = "https://bopenapi.bgwapi.io/bgw-pro/swapx/pro/";

/// Bitget API path for getting a swap quote.
const QUOTE_PATH: &str = "quote";

/// Bitget API path for getting swap calldata.
const SWAP_PATH: &str = "swap";

/// Bindings to the Bitget swap API.
pub struct Bitget {
    client: super::Client,
    endpoint: reqwest::Url,
    api_key: String,
    api_secret: String,
    partner_code: String,
    chain_name: dto::ChainName,
    settlement_contract: Address,
}

pub struct Config {
    /// The base URL for the Bitget swap API.
    pub endpoint: reqwest::Url,

    pub chain_id: eth::ChainId,

    pub settlement_contract: Address,

    /// Credentials used to access Bitget API.
    pub credentials: BitgetCredentialsConfig,

    /// Partner code sent in the `Partner-Code` header.
    pub partner_code: String,

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

        let chain_name = dto::ChainName::new(config.chain_id);

        Ok(Self {
            client,
            endpoint: config.endpoint,
            api_key: config.credentials.api_key,
            api_secret: config.credentials.api_secret,
            partner_code: config.partner_code,
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

        // Set up a tracing span to make debugging of API requests easier.
        // Historically, debugging API requests to external DEXs was a bit
        // of a headache.
        static ID: AtomicU64 = AtomicU64::new(0);
        let id = ID.fetch_add(1, atomic::Ordering::Relaxed);

        let (swap_response, quote_response, to_min_amount) = self
            .handle_sell_order(order, slippage)
            .instrument(tracing::trace_span!("swap", id = %id))
            .await?;

        let calldata = swap_response
            .decode_calldata()
            .map_err(|_| Error::InvalidCalldata)?;

        let contract = swap_response.contract;

        // Increase gas estimate by 50% for safety margin, similar to OKX.
        let gas_limit = U256::from(quote_response.gas_limit);
        let gas = gas_limit
            .checked_add(gas_limit / U256::from(2))
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
                amount: to_min_amount,
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
    ///
    /// To avoid a race condition between the quote and swap calls (where the
    /// quote returns one output amount but the swap calldata encodes a
    /// different one due to price movement), we:
    /// - Compute `toMinAmount` = quote's output minus slippage
    /// - Pass it explicitly to the swap endpoint so the calldata reverts
    ///   on-chain if output drops below this floor
    /// - Report `toMinAmount` as our output, guaranteeing consistency between
    ///   what we promise and what the calldata delivers
    async fn handle_sell_order(
        &self,
        order: &dex::Order,
        slippage: &dex::Slippage,
    ) -> Result<(dto::SwapResponse, dto::QuoteResponse, U256), Error> {
        // Step 1: Get quote
        let quote_request =
            dto::QuoteRequest::from_order(order, self.chain_name, self.settlement_contract);

        let quote_response: dto::QuoteResponse =
            self.send_post_request(QUOTE_PATH, &quote_request).await?;

        // Apply slippage to the quoted output to get the minimum we'll accept.
        // This becomes both the `toMinAmount` in the calldata and our reported
        // output, ensuring they're always consistent.
        let to_min_amount = slippage.sub(quote_response.to_amount);

        // Step 2: Get swap calldata
        let swap_request = dto::SwapRequest::from_order(
            order,
            slippage,
            self.chain_name,
            self.settlement_contract,
            quote_response.market.clone(),
            to_min_amount,
        );

        let swap_response: dto::SwapResponse =
            self.send_post_request(SWAP_PATH, &swap_request).await?;

        Ok((swap_response, quote_response, to_min_amount))
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

        let api_path = url.path();
        let signature = self.generate_signature(api_path, &body_str, &timestamp)?;

        let request_builder = self
            .client
            .request(reqwest::Method::POST, url)
            .header("Content-Type", "application/json")
            .header("Partner-Code", &self.partner_code)
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
