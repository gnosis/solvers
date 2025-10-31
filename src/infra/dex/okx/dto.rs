//! DTOs for the OKX swap API. Full documentation for the API can be found
//! [here](https://web3.okx.com/build/dev-docs/wallet-api/dex-swap).

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

/// A OKX API swap request parameters (only mandatory fields).
/// OKX v6 supports both sell orders (exactIn) and buy orders (exactOut).
///
/// See [API](https://web3.okx.com/build/dev-docs/wallet-api/dex-swap)
/// documentation for more detailed information on each parameter.
#[serde_as]
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapRequest {
    /// Chain ID
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub chain_index: u64,

    /// Input amount of a token to be sold or bought set in minimal divisible
    /// units.
    #[serde_as(as = "serialize::U256")]
    pub amount: U256,

    /// Contract address of a token to be sent
    pub from_token_address: H160,

    /// Contract address of a token to be received
    pub to_token_address: H160,

    /// Limit of price slippage you are willing to accept
    pub slippage_percent: Slippage,

    /// User's wallet address. Where the sell tokens will be taken from.
    pub user_wallet_address: H160,

    /// Where the buy tokens get sent to.
    pub swap_receiver_address: H160,

    /// Swap mode: "exactIn" for sell orders (default), "exactOut" for buy
    /// orders
    pub swap_mode: SwapMode,
}

/// A OKX slippage amount.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Slippage(BigDecimal);

/// A OKX swap mode.
#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SwapMode {
    #[default]
    ExactIn,
    #[expect(dead_code)] // Disabled for now
    ExactOut,
}

impl SwapRequest {
    pub fn try_with_domain(
        self,
        order: &dex::Order,
        slippage: &dex::Slippage,
    ) -> Result<Self, super::Error> {
        let swap_mode = match order.side {
            order::Side::Sell => SwapMode::ExactIn,
            // Buy orders are limited on OKX
            order::Side::Buy => return Err(super::Error::OrderNotSupported),
        };

        Ok(Self {
            from_token_address: order.sell.0,
            to_token_address: order.buy.0,
            amount: order.amount.get(),
            slippage_percent: Slippage(slippage.as_factor().clone()),
            swap_mode,
            ..self
        })
    }
}

/// A OKX API swap response.
///
/// See [API](https://web3.okx.com/build/dev-docs/wallet-api/dex-swap)
/// documentation for more detailed information on each parameter.
#[serde_as]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponse {
    /// Quote execution path.
    pub router_result: SwapResponseRouterResult,

    /// Contract related response.
    pub tx: SwapResponseTx,
}

/// A OKX API swap response - quote execution path.
/// Deserializing fields which are only used by the implementation.
/// For all possible fields look into the documentation:
/// [API](https://web3.okx.com/build/dev-docs/wallet-api/dex-swap)
#[serde_as]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponseRouterResult {
    /// The information of a token to be sold.
    pub from_token: SwapResponseFromToToken,

    /// The information of a token to be bought.
    pub to_token: SwapResponseFromToToken,

    /// The input amount of a token to be sold.
    #[serde_as(as = "serialize::U256")]
    pub from_token_amount: U256,

    /// The resulting amount of a token to be bought.
    #[serde_as(as = "serialize::U256")]
    pub to_token_amount: U256,
}

/// A OKX API swap response - token information.
/// Deserializing fields which are only used by the implementation.
/// For all possible fields look into the documentation:
/// [API](https://web3.okx.com/build/dev-docs/wallet-api/dex-swap)
#[serde_as]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponseFromToToken {
    /// Address of the token smart contract.
    pub token_contract_address: H160,
}

/// A OKX API swap response - contract related information.
/// Deserializing fields which are only used by the implementation.
/// For all possible fields look into the documentation:
/// [API](https://web3.okx.com/build/dev-docs/wallet-api/dex-swap)
#[serde_as]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponseTx {
    /// Estimated amount of the gas limit.
    #[serde_as(as = "serialize::U256")]
    pub gas: U256,

    /// The contract address of OKX DEX router.
    pub to: H160,

    /// Call data.
    #[serde_as(as = "serialize::Hex")]
    pub data: Vec<u8>,
}

/// A OKX API approve transaction request.
///
/// See [API](https://web3.okx.com/build/dev-docs/wallet-api/dex-approve-transaction)
/// documentation for more detailed information on each parameter.
#[serde_as]
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveTransactionRequest {
    /// Chain ID
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub chain_index: u64,

    /// Contract address of a token to be permitted.
    pub token_contract_address: H160,

    /// The amount of token that needs to be permitted (in minimal divisible
    /// units).
    #[serde_as(as = "serialize::U256")]
    pub approve_amount: U256,
}

impl ApproveTransactionRequest {
    pub fn with_domain(chain_index: u64, order: &dex::Order) -> Self {
        Self {
            chain_index,
            token_contract_address: order.sell.0,
            approve_amount: order.amount.get(),
        }
    }
}

/// A OKX API approve transaction response.
/// Deserializing fields which are only used by the implementation.
/// See [API](https://web3.okx.com/build/dev-docs/wallet-api/dex-approve-transaction)
/// documentation for more detailed information on each parameter.
#[serde_as]
#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveTransactionResponse {
    /// The contract address of OKX DEX approve.
    pub dex_contract_address: H160,
}

/// A OKX API response - generic wrapper for success and failure cases.
#[serde_as]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Response<T> {
    /// Error code, 0 for success, otherwise one of:
    /// [error codes](https://web3.okx.com/build/dev-docs/wallet-api/dex-error-code)
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub code: i64,

    /// Response data.
    pub data: Vec<T>,

    /// Error code text message.
    pub msg: String,
}

#[derive(Deserialize)]
pub struct Error {
    pub code: i64,
    pub reason: String,
}
