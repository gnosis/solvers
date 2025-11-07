use {
    crate::{
        domain::{dex, eth, order},
        util,
    },
    alloy::primitives::Address,
    base64::prelude::*,
    chrono::SecondsFormat,
    ethrpc::{
        alloy::conversions::{IntoAlloy, IntoLegacy},
        block_stream::CurrentBlockWatcher,
    },
    futures::TryFutureExt,
    hmac::{Hmac, Mac},
    hyper::{StatusCode, header::HeaderValue},
    moka::future::Cache,
    serde::{Serialize, de::DeserializeOwned},
    sha2::Sha256,
    std::sync::atomic::{self, AtomicU64},
    tracing::Instrument,
};

mod dto;

/// Default OKX v6 DEX aggregator API endpoint (for sell orders - exactIn).
pub const DEFAULT_SELL_ORDERS_ENDPOINT: &str = "https://web3.okx.com/api/v6/dex/aggregator/";

const DEFAULT_DEX_APPROVED_ADDRESSES_CACHE_SIZE: u64 = 100;

/// Bindings to the OKX swap API.
pub struct Okx {
    client: super::Client,
    sell_orders_endpoint: reqwest::Url,
    buy_orders_endpoint: Option<reqwest::Url>,
    api_secret_key: String,
    defaults: dto::SwapRequest,
    /// Cache which stores a map of Token Address to contract address of
    /// OKX DEX approve contract.
    dex_approved_addresses: Cache<eth::TokenAddress, eth::ContractAddress>,
}

pub struct Config {
    /// The URL for the OKX swap API for sell orders (exactIn mode).
    /// Uses V6 API by default.
    pub sell_orders_endpoint: reqwest::Url,

    /// The URL for the OKX swap API for buy orders (exactOut mode).
    /// If specified, the URL must point to the V5 API. Otherwise, buy orders
    /// will be ignored.
    pub buy_orders_endpoint: Option<reqwest::Url>,

    pub chain_id: eth::ChainId,

    pub settlement_contract: Address,

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
            chain_index: config.chain_id as u64,
            // Funds first get moved in and out of the settlement contract so we have use
            // that address here to generate the correct calldata.
            swap_receiver_address: config.settlement_contract.into_legacy(),
            user_wallet_address: config.settlement_contract.into_legacy(),
            ..Default::default()
        };

        Ok(Self {
            client,
            sell_orders_endpoint: config.sell_orders_endpoint,
            buy_orders_endpoint: config.buy_orders_endpoint,
            api_secret_key: config.okx_credentials.api_secret_key,
            defaults,
            dex_approved_addresses: Cache::new(DEFAULT_DEX_APPROVED_ADDRESSES_CACHE_SIZE),
        })
    }

    pub async fn swap(
        &self,
        order: &dex::Order,
        slippage: &dex::Slippage,
    ) -> Result<dex::Swap, Error> {
        // Set up a tracing span to make debugging of API requests easier.
        // Historically, debugging API requests to external DEXs was a bit
        // of a headache.
        static ID: AtomicU64 = AtomicU64::new(0);
        let id = ID.fetch_add(1, atomic::Ordering::Relaxed);

        let (swap_response, dex_contract_address) = self
            .handle_api_requests(order, slippage)
            .instrument(tracing::trace_span!("swap", id = %id))
            .await?;

        // Increasing returned gas by 50% according to the documentation:
        // https://web3.okx.com/build/dev-docs/wallet-api/dex-swap (gas field description in Response param)
        let gas = swap_response
            .tx
            .gas
            .checked_add(swap_response.tx.gas / 2)
            .ok_or(Error::GasCalculationFailed)?;

        // For buy orders (ExactOut mode), the slippage is on the input token,
        // so we need to use U256::MAX allowance to cover the maximum possible input.
        let allowance_amount =
            if self.buy_orders_endpoint.is_some() && order.side == order::Side::Buy {
                eth::U256::max_value()
            } else {
                swap_response.router_result.from_token_amount
            };

        Ok(dex::Swap {
            calls: vec![dex::Call {
                to: swap_response.tx.to.into_alloy(),
                calldata: swap_response.tx.data.clone(),
            }],
            input: eth::Asset {
                token: swap_response
                    .router_result
                    .from_token
                    .token_contract_address
                    .into(),
                amount: swap_response.router_result.from_token_amount,
            },
            output: eth::Asset {
                token: swap_response
                    .router_result
                    .to_token
                    .token_contract_address
                    .into(),
                amount: swap_response.router_result.to_token_amount,
            },
            allowance: dex::Allowance {
                spender: dex_contract_address.0.into_alloy(),
                amount: dex::Amount::new(allowance_amount),
            },
            gas: eth::Gas(gas),
        })
    }

    /// Invokes /swap and /approve-transaction API requests in parallel.
    ///
    /// Returns a tuple of the /swap API response and dex contract address for
    /// the sell token obtained from /approve-transaction API endpoint or an
    /// error.
    ///
    /// Routes to sell_orders_endpoint for sell orders (exactIn) and
    /// buy_orders_endpoint for buy orders (exactOut) if configured.
    async fn handle_api_requests(
        &self,
        order: &dex::Order,
        slippage: &dex::Slippage,
    ) -> Result<(dto::SwapResponse, eth::ContractAddress), Error> {
        let swap_request_future = async {
            match order.side {
                order::Side::Sell => {
                    // Use V6 API for sell orders
                    let swap_request = self.defaults.clone().with_domain(order, slippage);
                    self.send_get_request(&self.sell_orders_endpoint, "swap", &swap_request)
                        .await
                }
                order::Side::Buy => {
                    // Use V5 API for buy orders if configured
                    let endpoint = self
                        .buy_orders_endpoint
                        .as_ref()
                        .ok_or(Error::OrderNotSupported)?;
                    let swap_request_v6 = self.defaults.clone().with_domain(order, slippage);
                    let swap_request_v5: dto::SwapRequestV5 = (&swap_request_v6).into();

                    self.send_get_request(endpoint, "swap", &swap_request_v5)
                        .await
                }
            }
        };

        let approve_transaction_request_future = async {
            let approve_request =
                dto::ApproveTransactionRequest::with_domain(self.defaults.chain_index, order);

            let approve_tx: dto::ApproveTransactionResponse = match order.side {
                order::Side::Sell => {
                    // Use V6 API for sell orders
                    self.send_get_request(
                        &self.sell_orders_endpoint,
                        "approve-transaction",
                        &approve_request,
                    )
                    .await?
                }
                order::Side::Buy => {
                    // Use V5 API for buy orders if configured
                    let endpoint = self
                        .buy_orders_endpoint
                        .as_ref()
                        .ok_or(Error::OrderNotSupported)?;
                    let approve_request_v5: dto::ApproveTransactionRequestV5 =
                        (&approve_request).into();

                    self.send_get_request(endpoint, "approve-transaction", &approve_request_v5)
                        .await?
                }
            };

            Ok(eth::ContractAddress(approve_tx.dex_contract_address))
        };

        tokio::try_join!(
            swap_request_future,
            self.dex_approved_addresses
                .try_get_with(order.sell, approve_transaction_request_future)
                .map_err(
                    |_: std::sync::Arc<Error>| Error::ApproveTransactionRequestFailed(order.sell)
                )
        )
    }

    /// OKX requires signature of the request to be added as dedicated HTTP
    /// Header. More information on generating the signature can be found in
    /// OKX documentation: https://web3.okx.com/build/dev-docs/wallet-api/rest-authentication
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

    /// OKX Error codes: [link](https://web3.okx.com/build/dev-docs/wallet-api/dex-error-code)
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

    async fn send_get_request<T, U>(
        &self,
        base_url: &reqwest::Url,
        endpoint: &str,
        query: &T,
    ) -> Result<U, Error>
    where
        T: Serialize,
        U: DeserializeOwned + Clone,
    {
        let mut request_builder = self
            .client
            .request(
                reqwest::Method::GET,
                base_url
                    .join(endpoint)
                    .map_err(|_| Error::RequestBuildFailed)?,
            )
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

        let response = util::http::roundtrip!(
            <dto::Response<U>, dto::Error>;
            request_builder
        )
        .await?;

        Self::handle_api_error(response.code, &response.msg)?;
        response.data.first().cloned().ok_or(Error::NotFound)
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
    #[error("failed to get approve-transaction response for token address: {0:?}")]
    ApproveTransactionRequestFailed(eth::TokenAddress),
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
