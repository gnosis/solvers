//! DTOs for the ParaSwap swap API. Full documentation for the API can be found
//! [here](https://developers.paraswap.network/api/get-rate-for-a-token-pair).

use {
    crate::{
        domain::{auction, dex, order},
        util::serialize,
    },
    ethereum_types::{H160, U256},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

/// ParaSwap query parameters for the `/swap` endpoint.
///
/// This API is not public, so no docs are available.
#[serde_as]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapQuery {
    /// Source token address.
    pub src_token: H160,

    /// Destination token address.
    pub dest_token: H160,

    /// Source token decimals.
    pub src_decimals: u8,

    /// Destination token decimals.
    pub dest_decimals: u8,

    /// Source token amount when the side is "sell" or destination token amount
    /// when the side is "buy". The amount should be in atoms.
    #[serde_as(as = "serialize::U256")]
    pub amount: U256,

    /// Sell or buy?
    pub side: Side,

    /// The list of DEXs to exclude from the computed price route.
    #[serde(skip_serializing_if = "Vec::is_empty", rename = "excludeDEXS")]
    #[serde_as(as = "serialize::CommaSeparated")]
    pub exclude_dexs: Vec<String>,

    /// The network ID.
    pub network: String,

    /// The partner name
    pub partner: String,

    /// The maximum price impact accepted (in percentage, 0-100)
    pub max_impact: u8,

    /// The address of the signer.
    pub user_address: H160,

    /// A relative slippage tolerance denominated in bps.
    pub slippage: u16,

    /// The API version to use.
    pub version: String,

    /// Whether to throw an error if the USD price is not available.
    pub ignore_bad_usd_price: bool,
}

impl SwapQuery {
    pub fn new(
        config: &super::Config,
        order: &dex::Order,
        tokens: &auction::Tokens,
        slippage: &dex::Slippage,
    ) -> Result<Self, super::Error> {
        Ok(Self {
            src_token: order.sell.0,
            dest_token: order.buy.0,
            src_decimals: tokens
                .decimals(&order.sell)
                .ok_or(super::Error::MissingDecimals)?,
            dest_decimals: tokens
                .decimals(&order.buy)
                .ok_or(super::Error::MissingDecimals)?,
            side: match order.side {
                order::Side::Buy => Side::Buy,
                order::Side::Sell => Side::Sell,
            },
            amount: order.amount.get(),
            exclude_dexs: config.exclude_dexs.clone(),
            ignore_bad_usd_price: config.ignore_bad_usd_price,
            network: config.chain_id.network_id().to_string(),
            partner: config.partner.clone(),
            max_impact: 100,
            user_address: config.address,
            slippage: slippage
                .as_bps()
                .ok_or(super::Error::InvalidSlippage(slippage.clone()))?,
            version: "6.2".to_string(),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Side {
    Sell,
    Buy,
}

/// A ParaSwap swap API response.
#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Swap {
    pub price_route: PriceRoute,
    pub tx_params: TxParams,
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceRoute {
    /// The source token amount in atoms.
    #[serde_as(as = "serialize::U256")]
    pub src_amount: U256,
    /// The destination token amount in atoms.
    #[serde_as(as = "serialize::U256")]
    pub dest_amount: U256,
    /// The (very) approximate gas cost for the swap.
    #[serde_as(as = "serialize::U256")]
    pub gas_cost: U256,
    /// The token transfer proxy that requires an allowance.
    pub token_transfer_proxy: H160,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxParams {
    pub to: H160,

    #[serde_as(as = "serialize::Hex")]
    pub data: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub error: String,
}
