//! DTOs for the Bitget swap API.
//! Full documentation: https://web3.bitget.com/en/docs/swap/

use {
    crate::domain::{dex, eth},
    bigdecimal::{BigDecimal, ToPrimitive},
    serde::{Deserialize, Serialize},
};

/// A Bitget slippage percentage (e.g. 1.0 = 1%).
/// Must serialize as a JSON number, not a string.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Slippage(f64);

/// Bitget chain name used in API requests.
#[derive(Clone, Copy, Serialize)]
pub enum ChainName {
    #[serde(rename = "eth")]
    Mainnet,
    #[serde(rename = "bsc")]
    Bnb,
    #[serde(rename = "base")]
    Base,
}

impl ChainName {
    pub fn new(chain_id: eth::ChainId) -> Self {
        match chain_id {
            eth::ChainId::Mainnet => Self::Mainnet,
            eth::ChainId::Bnb => Self::Bnb,
            eth::ChainId::Base => Self::Base,
            _ => panic!("unsupported Bitget chain: {chain_id:?}"),
        }
    }
}

/// A Bitget API quote request.
///
/// See [API](https://web3.bitget.com/en/docs/swap/)
/// documentation for more detailed information on each parameter.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRequest {
    /// Source token contract address (empty string for native token).
    pub from_contract: eth::Address,

    /// Input amount in human-readable decimal units (e.g. "1" for 1 WETH).
    pub from_amount: String,

    /// Source chain name (e.g., "eth", "bsc", "base").
    pub from_chain: ChainName,

    /// Debit address for gas estimation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_address: Option<eth::Address>,

    /// Target token contract address (empty string for native token).
    pub to_contract: eth::Address,

    /// Target chain name (same as from_chain for same-chain swaps).
    pub to_chain: ChainName,

    /// Whether to estimate gas.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimate_gas: Option<bool>,
}

impl QuoteRequest {
    pub fn from_order(
        order: &dex::Order,
        chain_name: ChainName,
        settlement_contract: eth::Address,
        sell_decimals: u8,
    ) -> Self {
        Self {
            from_contract: order.sell.0,
            from_amount: super::wei_to_decimal(order.amount.get(), sell_decimals).to_string(),
            from_chain: chain_name,
            to_contract: order.buy.0,
            to_chain: chain_name,
            from_address: Some(settlement_contract),
            estimate_gas: Some(true),
        }
    }
}

/// A Bitget API swap (calldata) request.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapRequest {
    /// Source token contract address.
    pub from_contract: eth::Address,

    /// Input amount in human-readable decimal units (e.g. "1" for 1 WETH).
    pub from_amount: String,

    /// Source chain name.
    pub from_chain: ChainName,

    /// Target token contract address.
    pub to_contract: eth::Address,

    /// Target chain name.
    pub to_chain: ChainName,

    /// Debit address.
    pub from_address: eth::Address,

    /// Recipient address.
    pub to_address: eth::Address,

    /// Optimal channel from quote API.
    pub market: String,

    /// Minimum amount to receive in decimal units. By setting this explicitly
    /// we ensure the generated calldata will revert on-chain if the output
    /// drops below this value — avoiding a race between quote and swap calls.
    pub to_min_amount: String,

    /// Slippage as a factor (e.g., 0.01 = 1%). The real slippage protection
    /// is enforced by `to_min_amount`; this field is only informational.
    pub slippage: Slippage,

    /// Fee rate in per mille. 0 for no fee.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_rate: Option<f64>,
}

impl SwapRequest {
    pub fn from_order(
        order: &dex::Order,
        slippage: &dex::Slippage,
        chain_name: ChainName,
        settlement_contract: eth::Address,
        market: String,
        to_min_amount: String,
        sell_decimals: u8,
    ) -> Self {
        Self {
            from_contract: order.sell.0,
            from_amount: super::wei_to_decimal(order.amount.get(), sell_decimals).to_string(),
            from_chain: chain_name,
            to_contract: order.buy.0,
            to_chain: chain_name,
            from_address: settlement_contract,
            to_address: settlement_contract,
            market,
            to_min_amount,
            slippage: Slippage(
                (slippage.as_factor() * BigDecimal::from(100))
                    .to_f64()
                    .unwrap_or_default(),
            ),
            fee_rate: Some(0.0),
        }
    }
}

/// A Bitget API quote response.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QuoteResponse {
    /// Output amount in decimal units (e.g. "1964.365496").
    pub to_amount: String,

    /// Channel name (e.g., "uniswap.v3").
    pub market: String,

    /// Estimated gas limit.
    #[serde(default)]
    pub gas_limit: u64,
}

/// A Bitget API swap response.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponse {
    /// Contract address to interact with (EVM chains only).
    /// This is the router/spender address.
    pub contract: eth::Address,

    /// Base64-encoded calldata for the transaction.
    pub calldata: String,
}

impl SwapResponse {
    /// Decode the hex-encoded calldata (with "0x" prefix) to bytes.
    pub fn decode_calldata(&self) -> Result<Vec<u8>, hex::FromHexError> {
        let hex_str = self.calldata.strip_prefix("0x").unwrap_or(&self.calldata);
        hex::decode(hex_str)
    }
}

/// A Bitget API response wrapper.
///
/// On success `status` is 0 and `data` contains the result.
/// On error `status` is non-zero and `data` is null.
#[derive(Deserialize, Clone, Debug)]
pub struct Response<T> {
    /// Response status code (0 = success).
    pub status: i64,

    /// Response data — `None` when the API returns an error.
    pub data: Option<T>,
}
