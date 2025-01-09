//! DTOs for the OKX swap API. Full documentation for the API can be found
//! [here](https://www.okx.com/en-au/web3/build/docs/waas/dex-swap).

use {
    crate::{
        domain::{dex, order},
        util::serialize,
    },
    bigdecimal::BigDecimal,
    ethereum_types::{H160, U256},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

/// A OKX API swap request parameters.
/// Only sell orders are supported by OKX.
///
/// See [API](https://www.okx.com/en-au/web3/build/docs/waas/dex-swap)
/// documentation for more detailed information on each parameter.
#[serde_as]
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapRequest {
    /// Chain ID
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub chain_id: u64,

    /// Input amount of a token to be sold set in minimal divisible units.
    #[serde_as(as = "serialize::U256")]
    pub amount: U256,

    /// Contract address of a token to be sent
    pub from_token_address: H160,

    /// Contract address of a token to be received
    pub to_token_address: H160,

    /// Limit of price slippage you are willing to accept
    pub slippage: Slippage,

    /// User's wallet address. Where the sell tokens will be taken from.
    pub user_wallet_address: H160,
}

/// A OKX slippage amount.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Slippage(BigDecimal);

/// A OKX gas level.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GasLevel {
    #[default]
    Average,
    Fast,
    Slow,
}

impl SwapRequest {
    pub fn with_domain(self, order: &dex::Order, slippage: &dex::Slippage) -> Option<Self> {
        // Buy orders are not supported on OKX
        if order.side == order::Side::Buy {
            return None;
        };

        let (from_token_address, to_token_address, amount) = match order.side {
            order::Side::Sell => (order.sell.0, order.buy.0, order.amount.get()),
            order::Side::Buy => (order.buy.0, order.sell.0, order.amount.get()),
        };

        Some(Self {
            from_token_address,
            to_token_address,
            amount,
            slippage: Slippage(slippage.as_factor().clone()),
            user_wallet_address: order.owner,
            ..self
        })
    }
}

/// A OKX API quote response.
#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponse {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub code: i64,

    pub data: Vec<SwapResponseInner>,

    pub msg: String,
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponseInner {
    pub router_result: SwapResponseRouterResult,

    pub tx: SwapResponseTx,
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponseRouterResult {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub chain_id: u64,

    #[serde_as(as = "serialize::U256")]
    pub from_token_amount: U256,

    #[serde_as(as = "serialize::U256")]
    pub to_token_amount: U256,

    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub trade_fee: f64,

    #[serde_as(as = "serialize::U256")]
    pub estimate_gas_fee: U256,

    pub dex_router_list: Vec<SwapResponseDexRouterList>,

    pub quote_compare_list: Vec<SwapResponseQuoteCompareList>,

    pub to_token: SwapResponseFromToToken,

    pub from_token: SwapResponseFromToToken,
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponseDexRouterList {
    pub router: String,

    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub router_percent: f64,

    pub sub_router_list: Vec<SwapResponseDexSubRouterList>,
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponseDexSubRouterList {
    pub dex_protocol: Vec<SwapResponseDexProtocol>,

    pub from_token: SwapResponseFromToToken,

    pub to_token: SwapResponseFromToToken,
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponseDexProtocol {
    pub dex_name: String,

    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub percent: f64,
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponseFromToToken {
    pub token_contract_address: H160,

    pub token_symbol: String,

    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub token_unit_price: f64,

    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub decimal: u8,

    pub is_honey_pot: bool,

    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub tax_rate: f64,
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponseQuoteCompareList {
    pub dex_name: String,

    pub dex_logo: String,

    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub trade_fee: f64,

    // todo: missing in docs?
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub amount_out: f64,
    // todo: missing from response?
    //#[serde_as(as = "serialize::U256")]
    //pub receive_amount: U256,

    // todo: missing from response?
    //#[serde_as(as = "serde_with::DisplayFromStr")]
    //pub price_impact_percentage: f64,
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponseTx {
    pub signature_data: Vec<String>,

    pub from: H160,

    #[serde_as(as = "serialize::U256")]
    pub gas: U256,

    #[serde_as(as = "serialize::U256")]
    pub gas_price: U256,

    #[serde_as(as = "serialize::U256")]
    pub max_priority_fee_per_gas: U256,

    pub to: H160,

    #[serde_as(as = "serialize::U256")]
    pub value: U256,

    #[serde_as(as = "serialize::U256")]
    pub min_receive_amount: U256,

    #[serde_as(as = "serialize::Hex")]
    pub data: Vec<u8>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Response {
    Ok(SwapResponse),
    Err(Error),
}

impl Response {
    /// Turns the API response into a [`std::result::Result`].
    pub fn into_result(self) -> Result<SwapResponse, Error> {
        match self {
            Response::Ok(quote) => Ok(quote),
            Response::Err(err) => Err(err),
        }
    }
}

#[derive(Deserialize)]
pub struct Error {
    pub code: i64,
    pub reason: String,
}
