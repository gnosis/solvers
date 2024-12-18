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

/// A OKX API swap query parameters.
///
/// See [API](https://www.okx.com/en-au/web3/build/docs/waas/dex-swap)
/// documentation for more detailed information on each parameter.
#[serde_as]
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {

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

    /// The percentage of from_token_address will be sent to the referrer's address, 
    /// the rest will be set as the input amount to be sold. 
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

    /// Used for transactions on the Solana network and similar to gas_price on Ethereum.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<serialize::U256>")]
    pub compute_unit_price: Option<U256>,

    /// Used for transactions on the Solana network and analogous to gas_limit on Ethereum.
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
    Slow
}



impl Query {
    pub fn with_domain(self, order: &dex::Order, slippage: &dex::Slippage) -> Self {
        let (from_token_address, to_token_address, amount) = match order.side {
            order::Side::Sell => (order.sell.0, order.buy.0, order.amount.get()),
            order::Side::Buy => (order.buy.0, order.sell.0, order.amount.get()),
        };

        Self {
            chain_id: 1, // todo ms: from config
            from_token_address,
            to_token_address,
            amount,
            slippage: Slippage(slippage.as_factor().clone()),
            ..self
        }
    }
}

/// A Ox API quote response.
#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    /// The address of the contract to call in order to execute the swap.
    pub to: H160,

    /// The swap calldata.
    #[serde_as(as = "serialize::Hex")]
    pub data: Vec<u8>,

    /// The estimate for the amount of gas that will actually be used in the
    /// transaction.
    #[serde_as(as = "serialize::U256")]
    pub estimated_gas: U256,

    /// The amount of sell token (in atoms) that would be sold in this swap.
    #[serde_as(as = "serialize::U256")]
    pub sell_amount: U256,

    /// The amount of buy token (in atoms) that would be bought in this swap.
    #[serde_as(as = "serialize::U256")]
    pub buy_amount: U256,

    /// The target contract address for which the user needs to have an
    /// allowance in order to be able to complete the swap.
    pub allowance_target: Option<H160>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Response {
    Ok(Quote),
    Err(Error),
}

impl Response {
    /// Turns the API response into a [`std::result::Result`].
    pub fn into_result(self) -> Result<Quote, Error> {
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
