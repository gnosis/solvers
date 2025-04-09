//! DTOs for the 0x swap API. Full documentation for the API can be found
//! [here](https://0x.org/docs/api#tag/Swap/operation/swap::allowanceHolder::getQuote).

use {
    crate::{
        domain::{dex, order},
        util::serialize,
    },
    ethereum_types::{H160, U256},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

/// A 0x API quote query parameters.
///
/// See [API](https://0x.org/docs/api#tag/Swap/operation/swap::allowanceHolder::getQuote)
/// documentation for more detailed information on each parameter.
#[serde_as]
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    /// The chain ID of the network the query is prepared for.
    pub chain_id: u64,

    /// Contract address of a token to buy.
    pub buy_token: H160,

    /// Contract address of a token to sell.
    pub sell_token: H160,

    /// Amount of a token to sell, set in atoms.
    #[serde_as(as = "serialize::U256")]
    pub sell_amount: U256,

    /// The address which will fill the quote.
    pub taker: H160,

    /// Limit of price slippage you are willing to accept. Values are in basis
    /// points [ 0 .. 10000 ].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slippage_bps: Option<Slippage>,

    /// The target gas price for the swap transaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<serialize::U256>")]
    pub gas_price: Option<U256>,

    /// List of sources to exclude.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde_as(as = "serialize::CommaSeparated")]
    pub excluded_sources: Vec<String>,
}

/// A 0x slippage amount.
#[derive(Clone, Debug, Serialize)]
pub struct Slippage(u16);

impl Query {
    pub fn try_with_domain(
        self,
        order: &dex::Order,
        slippage: &dex::Slippage,
    ) -> Result<Self, super::Error> {
        // Buy orders are not supported on 0x
        if order.side == order::Side::Buy {
            return Err(super::Error::OrderNotSupported);
        };

        Ok(Self {
            sell_token: order.sell.0,
            buy_token: order.buy.0,
            sell_amount: order.amount.get(),
            slippage_bps: slippage.as_bps().map(Slippage),
            ..self
        })
    }
}

/// A Ox API quote response.
#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    /// Whether the quote is valid. If not valid, all other fields are missing.
    pub liquidity_available: bool,
    /// The amount of sell token (in atoms) that would be sold in this swap.
    #[serde_as(as = "Option<serialize::U256>")]
    pub sell_amount: Option<U256>,
    /// The amount of buy token (in atoms) that would be bought in this swap.
    #[serde_as(as = "Option<serialize::U256>")]
    pub buy_amount: Option<U256>,
    pub transaction: Option<QuoteTransaction>,
    pub issues: Option<Issues>,
}

/// A valid quote response, with liquidity available.
pub struct ValidQuote {
    /// The amount of sell token (in atoms) that would be sold in this swap.
    pub sell_amount: U256,
    /// The amount of buy token (in atoms) that would be bought in this swap.
    pub buy_amount: U256,
    pub transaction: QuoteTransaction,
    pub issues: Issues,
}

impl From<Quote> for Option<ValidQuote> {
    fn from(raw: Quote) -> Self {
        if !raw.liquidity_available {
            return None;
        }

        Some(ValidQuote {
            sell_amount: raw.sell_amount?,
            buy_amount: raw.buy_amount?,
            transaction: raw.transaction?,
            issues: raw.issues?,
        })
    }
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteTransaction {
    /// The address of the contract to call in order to execute the swap.
    pub to: H160,

    /// The swap calldata.
    #[serde_as(as = "serialize::Hex")]
    pub data: Vec<u8>,

    /// The estimate for the amount of gas that will actually be used in the
    /// transaction.
    #[serde_as(as = "Option<serialize::U256>")]
    pub gas: Option<U256>,
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issues {
    /// Allowance data for the sell token.
    pub allowance: Option<Allowance>,
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Allowance {
    /// The taker's current allowance of the spender
    #[serde_as(as = "serialize::U256")]
    pub actual: U256,
    /// The address to set the allowance on
    pub spender: H160,
}

#[derive(Deserialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
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

mod tests {
    #[test]
    fn test_quote_deserialization() {
        let json = r#"{
            "liquidityAvailable": true,
            "sellAmount": "1000000000000000000",
            "buyAmount": "1000000000000000000",
            "transaction": {
                "to": "0x1234567890123456789012345678901234567890",
                "data": "0xabcdef",
                "gas": "21000"
            },
            "issues": {
                "allowance": {
                    "actual": "1000000000000000000",
                    "spender": "0x1234567890123456789012345678901234567890"
                }
            }
        }"#;

        let quote: super::Quote = serde_json::from_str(json).unwrap();
        assert!(quote.liquidity_available);
    }

    #[test]
    fn test_quote_no_liquidity_deserialization() {
        let json = r#"{
            "liquidityAvailable": false
        }"#;

        let quote: super::Quote = serde_json::from_str(json).unwrap();
        assert!(!quote.liquidity_available);
    }
}
