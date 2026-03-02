//! DTOs for the Bitget swap API.
//! Full documentation: https://web3.bitget.com/en/docs/swap/

use {
    crate::{
        domain::{dex, eth},
        util::serialize,
    },
    alloy::primitives::U256,
    bigdecimal::BigDecimal,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

/// A Bitget slippage amount.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Slippage(BigDecimal);

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
#[serde_as]
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRequest {
    /// Source token contract address (empty string for native token).
    pub from_contract: eth::Address,

    /// Input amount in minimal divisible units.
    #[serde_as(as = "serialize::U256")]
    pub from_amount: U256,

    /// Source chain name (e.g., "eth", "bsc", "base").
    pub from_chain: ChainName,

    /// Target token contract address (empty string for native token).
    pub to_contract: eth::Address,

    /// Target chain name (same as from_chain for same-chain swaps).
    pub to_chain: ChainName,

    /// Debit address for gas estimation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_address: Option<eth::Address>,

    /// Whether to estimate gas.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimate_gas: Option<bool>,
}

impl QuoteRequest {
    pub fn from_order(
        order: &dex::Order,
        chain_name: ChainName,
        settlement_contract: eth::Address,
    ) -> Self {
        Self {
            from_contract: order.sell.0,
            from_amount: order.amount.get(),
            from_chain: chain_name,
            to_contract: order.buy.0,
            to_chain: chain_name,
            from_address: Some(settlement_contract),
            estimate_gas: Some(true),
        }
    }
}

/// A Bitget API swap (calldata) request.
#[serde_as]
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapRequest {
    /// Source token contract address.
    pub from_contract: eth::Address,

    /// Input amount in minimal divisible units.
    #[serde_as(as = "serialize::U256")]
    pub from_amount: U256,

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

    /// Minimum amount to receive. By setting this explicitly we ensure
    /// the generated calldata will revert on-chain if the output drops
    /// below this value — avoiding a race between quote and swap calls.
    #[serde_as(as = "serialize::U256")]
    pub to_min_amount: U256,

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
        to_min_amount: U256,
    ) -> Self {
        Self {
            from_contract: order.sell.0,
            from_amount: order.amount.get(),
            from_chain: chain_name,
            to_contract: order.buy.0,
            to_chain: chain_name,
            from_address: settlement_contract,
            to_address: settlement_contract,
            market,
            to_min_amount,
            slippage: Slippage(slippage.as_factor().clone()),
            fee_rate: Some(0.0),
        }
    }
}

/// A Bitget API quote response.
#[serde_as]
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QuoteResponse {
    /// Output amount in minimal divisible units.
    #[serde_as(as = "serialize::U256")]
    pub to_amount: U256,

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
    /// Decode the base64-encoded calldata to bytes.
    pub fn decode_calldata(&self) -> Result<Vec<u8>, base64::DecodeError> {
        use base64::prelude::*;
        BASE64_STANDARD.decode(&self.calldata)
    }
}

/// A Bitget API response wrapper.
#[derive(Deserialize, Clone, Debug)]
pub struct Response<T> {
    /// Response status code (0 = success).
    pub status: i64,

    /// Response data.
    pub data: T,
}

/// Bitget API error response used for roundtrip error parsing.
#[derive(Deserialize, Debug)]
pub struct Error {
    pub status: i64,
}
