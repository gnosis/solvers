//! DTOs for the Bitget swap API.
//! Full documentation: https://web3.bitget.com/en/docs/swap/

use {
    crate::domain::{dex, eth},
    alloy::primitives::U256,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

/// Bitget chain name used in API requests.
pub fn chain_name(chain_id: eth::ChainId) -> &'static str {
    match chain_id {
        eth::ChainId::Mainnet => "eth",
        eth::ChainId::Bnb => "bsc",
        eth::ChainId::Base => "base",
        _ => panic!("unsupported Bitget chain: {chain_id:?}"),
    }
}

/// A Bitget API quote request.
///
/// See [API](https://web3.bitget.com/en/docs/swap/)
/// documentation for more detailed information on each parameter.
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRequest {
    /// Source token contract address (empty string for native token).
    pub from_contract: String,

    /// Input amount in minimal divisible units.
    pub from_amount: String,

    /// Source chain name (e.g., "eth", "bsc", "base").
    pub from_chain: String,

    /// Target token contract address (empty string for native token).
    pub to_contract: String,

    /// Target chain name (same as from_chain for same-chain swaps).
    pub to_chain: String,

    /// Debit address for gas estimation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_address: Option<String>,

    /// Whether to estimate gas.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimate_gas: Option<bool>,
}

impl QuoteRequest {
    pub fn from_order(
        order: &dex::Order,
        chain_name: &str,
        settlement_contract: eth::Address,
    ) -> Self {
        Self {
            from_contract: format!("{:?}", order.sell.0),
            from_amount: order.amount.get().to_string(),
            from_chain: chain_name.to_string(),
            to_contract: format!("{:?}", order.buy.0),
            to_chain: chain_name.to_string(),
            from_address: Some(format!("{:?}", settlement_contract)),
            estimate_gas: Some(true),
        }
    }
}

/// A Bitget API swap (calldata) request.
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapRequest {
    /// Source token contract address.
    pub from_contract: String,

    /// Input amount in minimal divisible units.
    pub from_amount: String,

    /// Source chain name.
    pub from_chain: String,

    /// Target token contract address.
    pub to_contract: String,

    /// Target chain name.
    pub to_chain: String,

    /// Debit address.
    pub from_address: String,

    /// Recipient address.
    pub to_address: String,

    /// Optimal channel from quote API.
    pub market: String,

    /// Slippage percentage (e.g., 1 = 1%).
    pub slippage: f64,

    /// Fee rate in per mille. 0 for no fee.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_rate: Option<f64>,
}

impl SwapRequest {
    pub fn from_order(
        order: &dex::Order,
        slippage: &dex::Slippage,
        chain_name: &str,
        settlement_contract: eth::Address,
        market: String,
    ) -> Self {
        // Convert slippage factor to percentage (0.01 -> 1.0)
        let slippage_percent: f64 = (slippage.as_factor() * bigdecimal::BigDecimal::from(100))
            .to_string()
            .parse()
            .unwrap_or(1.0);

        let settlement = format!("{:?}", settlement_contract);
        Self {
            from_contract: format!("{:?}", order.sell.0),
            from_amount: order.amount.get().to_string(),
            from_chain: chain_name.to_string(),
            to_contract: format!("{:?}", order.buy.0),
            to_chain: chain_name.to_string(),
            from_address: settlement.clone(),
            to_address: settlement,
            market,
            slippage: slippage_percent,
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
    pub to_amount: String,

    /// Channel name (e.g., "uniswap.v3").
    pub market: String,

    /// Estimated gas limit.
    #[serde(default)]
    pub gas_limit: u64,
}

/// A Bitget API swap response.
#[serde_as]
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponse {
    /// Contract address to interact with (EVM chains only).
    /// This is the router/spender address.
    #[serde(default)]
    pub contract: String,

    /// Base64-encoded calldata for the transaction.
    pub calldata: String,
}

impl SwapResponse {
    /// Decode the base64-encoded calldata to bytes.
    pub fn decode_calldata(&self) -> Result<Vec<u8>, base64::DecodeError> {
        use base64::prelude::*;
        BASE64_STANDARD.decode(&self.calldata)
    }

    /// Parse the contract address.
    pub fn parse_contract(&self) -> Result<eth::Address, String> {
        self.contract
            .parse()
            .map_err(|e| format!("invalid contract address '{}': {e}", self.contract))
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

/// Parsed output amounts from the quote response, converted to U256.
pub struct QuoteAmounts {
    pub to_amount: U256,
    pub gas_limit: U256,
    pub market: String,
}

impl QuoteResponse {
    /// Parse the quote response amounts into U256 values.
    pub fn parse_amounts(&self) -> Result<QuoteAmounts, ParseError> {
        let to_amount =
            U256::from_str_radix(&self.to_amount, 10).map_err(|_| ParseError::InvalidAmount)?;
        Ok(QuoteAmounts {
            to_amount,
            gas_limit: U256::from(self.gas_limit),
            market: self.market.clone(),
        })
    }
}

#[derive(Debug)]
pub enum ParseError {
    InvalidAmount,
}
