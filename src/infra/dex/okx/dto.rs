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

    /// Input amount of a token to be sold set in minimal divisible units
    #[serde_as(as = "serialize::U256")]
    pub amount: U256,

    /// Contract address of a token to be send
    pub from_token_address: H160,

    /// Contract address of a token to be received
    pub to_token_address: H160,

    /// Limit of price slippage you are willing to accept
    pub slippage: Slippage,

    /// User's wallet address
    pub user_wallet_address: H160,

    /// The fromToken address that receives the commission.
    /// Only for SOL or SPL-Token commissions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referrer_address: Option<H160>,

    /// Recipient address of a purchased token if not set,
    /// user_wallet_address will receive a purchased token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swap_receiver_address: Option<H160>,

    /// The percentage of from_token_address will be sent to the referrer's
    /// address, the rest will be set as the input amount to be sold.
    /// Min percentage：0
    /// Max percentage：3
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<serde_with::DisplayFromStr>")]
    pub fee_percent: Option<f64>,

    /// The gas limit (in wei) for the swap transaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<serialize::U256>")]
    pub gas_limit: Option<U256>,

    /// The target gas price level for the swap transaction.
    /// Default value: average
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_level: Option<GasLevel>,

    /// List of DexId of the liquidity pool for limited quotes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde_as(as = "serialize::CommaSeparated")]
    pub dex_ids: Vec<String>,

    /// The percentage of the price impact allowed.
    /// Min value: 0
    /// Max value：1 (100%)
    /// Default value: 0.9 (90%)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<serde_with::DisplayFromStr>")]
    pub price_impact_protection_percentage: Option<f64>,

    /// Customized parameters sent on the blockchain in callData.
    /// Hex encoded 128-characters string.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde_as(as = "serialize::Hex")]
    pub call_data_memo: Vec<u8>,

    /// Address that receives the commission.
    /// Only for SOL or SPL-Token commissions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_token_referrer_address: Option<H160>,

    /// Used for transactions on the Solana network and similar to gas_price on
    /// Ethereum.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<serialize::U256>")]
    pub compute_unit_price: Option<U256>,

    /// Used for transactions on the Solana network and analogous to gas_limit
    /// on Ethereum.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<serialize::U256>")]
    pub compute_unit_limit: Option<U256>,

    /// The wallet address to receive the commission fee from the from_token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_token_referrer_wallet_address: Option<H160>,

    /// The wallet address to receive the commission fee from the to_token
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_token_referrer_wallet_address: Option<H160>,
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
    pub fn with_domain(self, order: &dex::Order, slippage: &dex::Slippage) -> Self {
        let (from_token_address, to_token_address, amount) = match order.side {
            order::Side::Sell => (order.sell.0, order.buy.0, order.amount.get()),
            order::Side::Buy => (order.buy.0, order.sell.0, order.amount.get()),
        };

        Self {
            from_token_address,
            to_token_address,
            amount,
            slippage: Slippage(slippage.as_factor().clone()),
            ..self
        }
    }
}

/// A OKX API quote response.
#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponse {
    pub code: String,

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

    #[serde_as(as = "serialize::U256")]
    pub trade_fee: U256,

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

    #[serde_as(as = "serialize::U256")]
    pub receive_amount: U256,

    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub price_impact_percentage: f64,
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
