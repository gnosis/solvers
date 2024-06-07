use bigdecimal::BigDecimal;

use {
    crate::{
        domain::{dex, order},
        util::serialize,
    },
    ethereum_types::{H160, H256, U256},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

use crate::domain::eth;

mod pools_query {
    pub const QUERY: &str = r#"
        query sorGetSwapPaths($callDataInput: GqlSwapCallDataInput!, $chain: GqlChain!, $queryBatchSwap: Boolean!, $swapAmount: AmountHumanReadable!, $swapType: GqlSorSwapType!, $tokenIn: String!, $tokenOut: String!, $useVaultVersion: Int) {
            sorGetSwapPaths(
                callDataInput: $callDataInput,
                chain: $chain,
                queryBatchSwap: $queryBatchSwap,
                swapAmount: $swapAmount,
                swapType: $swapType,
                tokenIn: $tokenIn,
                tokenOut: $tokenOut,
                useVaultVersion: $useVaultVersion
            ) {
                tokenAddresses
                swaps {
                    poolId
                    assetInIndex
                    assetOutIndex
                    amount
                    userData
                }
                swapAmount
                returnAmount
                tokenIn
                tokenOut
            }
        }
    "#;
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Query<'a> {
    query: &'a str,
    variables: Variables,
}

impl Query<'_> {
    pub fn from_domain(order: &dex::Order, slippage: &dex::Slippage, chain_id: eth::ChainId, contract_address: eth::ContractAddress) -> Self {
        let swap_type = match order.side {
            order::Side::Buy => SwapType::ExactOut,
            order::Side::Sell => SwapType::ExactIn,
        };
        let chain = match chain_id {
            eth::ChainId::Mainnet => Chain::Mainnet,
            eth::ChainId::Gnosis => Chain::Gnosis,
            eth::ChainId::ArbitrumOne => Chain::Arbitrum,
            _ => panic!("Unsupported chain"),
        };
        let variables = Variables {
            call_data_input: CalDataInput {
                deadline: 999999999999999999,
                receiver: contract_address.0,
                sender: contract_address.0,
                slippage_percentage: slippage.as_factor().clone(),
            },
            chain,
            query_batch_swap: false,
            swap_amount: order.amount().amount,
            swap_type,
            token_in: order.sell.0,
            token_out: order.buy.0,
            use_vault_version: VaultVersion::V2 as u8,
        };
        Self {
            query: pools_query::QUERY,
            variables,
        }
    }
}

#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Variables {
    call_data_input: CalDataInput,
    chain: Chain,
    query_batch_swap: bool,
    #[serde_as(as = "serialize::U256")]
    swap_amount: U256,
    swap_type: SwapType,
    token_in: H160,
    token_out: H160,
    use_vault_version: u8,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CalDataInput {
    deadline: u64,
    receiver: H160,
    sender: H160,
    slippage_percentage: BigDecimal,
}

#[derive(Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Chain {
    Arbitrum,
    Avalanche,
    Base,
    Fantom,
    Fraxtal,
    Gnosis,
    Mainnet,
    Mode,
    Optimism,
    Polygon,
    Sepolia,
    ZkEvm,
}

#[derive(Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum SwapType {
    ExactIn,
    ExactOut,
}

enum VaultVersion {
    V2 = 2,
}

/// The swap route found by the Balancer SOR service.
#[serde_as]
#[derive(Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    /// The token addresses included in the swap route.
    pub token_addresses: Vec<H160>,
    /// The swap route.
    pub swaps: Vec<Swap>,
    /// The swapped token amount.
    ///
    /// In sell token for sell orders or buy token for buy orders.
    #[serde_as(as = "serialize::U256")]
    pub swap_amount: U256,
    /// The returned token amount.
    ///
    /// In buy token for sell orders or sell token for buy orders.
    #[serde_as(as = "serialize::U256")]
    pub return_amount: U256,
    /// The input (sell) token.
    #[serde(with = "address_default_when_empty")]
    pub token_in: H160,
    /// The output (buy) token.
    #[serde(with = "address_default_when_empty")]
    pub token_out: H160,
}

impl Quote {
    /// Check for "empty" quotes - i.e. all 0's with no swaps. Balancer SOR API
    /// returns this in case it fails to find a route for whatever reason (not
    /// enough liquidity, no trading path, etc.). We don't consider this an
    /// error case.
    pub fn is_empty(&self) -> bool {
        *self == Quote::default()
    }
}

/// A swap included in a larger batched swap.
#[serde_as]
#[derive(Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Swap {
    /// The ID of the pool swapping in this step.
    pub pool_id: H256,
    /// The index in `token_addresses` for the input token.
    #[serde(with = "value_or_string")]
    pub asset_in_index: usize,
    /// The index in `token_addresses` for the ouput token.
    #[serde(with = "value_or_string")]
    pub asset_out_index: usize,
    /// The amount to swap.
    #[serde_as(as = "serialize::U256")]
    pub amount: U256,
    /// Additional user data to pass to the pool.
    #[serde_as(as = "serialize::Hex")]
    pub user_data: Vec<u8>,
}

/// Balancer SOR responds with `address: ""` on error cases.
mod address_default_when_empty {
    use {
        ethereum_types::H160,
        serde::{de, Deserialize as _, Deserializer},
        std::borrow::Cow,
    };

    pub fn deserialize<'de, D>(deserializer: D) -> Result<H160, D::Error>
        where
            D: Deserializer<'de>,
    {
        let value = Cow::<str>::deserialize(deserializer)?;
        if value == "" {
            return Ok(H160::default());
        }
        value.parse().map_err(de::Error::custom)
    }
}

/// Tries to either parse the `T` directly or tries to convert the value in case
/// it's a string. This is intended for deserializing number/string but is
/// generic enough to be used for any value that can be converted from a string.
mod value_or_string {
    use {
        serde::{de, Deserialize, Deserializer},
        std::borrow::Cow,
    };

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
        where
            D: Deserializer<'de>,
            T: Deserialize<'de> + std::str::FromStr,
            <T as std::str::FromStr>::Err: std::fmt::Display,
    {
        #[derive(Debug, Deserialize)]
        #[serde(untagged)]
        enum Content<'a, T> {
            Value(T),
            String(Cow<'a, str>),
        }

        match <Content<T>>::deserialize(deserializer) {
            Ok(Content::Value(value)) => Ok(value),
            Ok(Content::String(s)) => s.parse().map_err(de::Error::custom),
            Err(err) => Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use serde_json::json;

    use super::*;

    #[test]
    fn test_query_serialization() {
        let order = dex::Order {
            sell: H160::from_str("0x2170ed0880ac9a755fd29b2688956bd959f933f8").unwrap().into(),
            buy: H160::from_str("0xdac17f958d2ee523a2206206994597c13d831ec7").unwrap().into(),
            side: order::Side::Buy,
            amount: dex::Amount::new(U256::from(1000)),
        };
        let slippage = dex::Slippage::one_percent();
        let chain_id = eth::ChainId::Mainnet;
        let contract_address = eth::ContractAddress(H160::from_str("0x9008d19f58aabd9ed0d60971565aa8510560ab41").unwrap());
        let query = Query::from_domain(&order, &slippage, chain_id, contract_address);

        let actual = serde_json::to_value(&query).unwrap();
        let deadline: u64 = 999999999999999999;
        let expected = json!({
            "query": pools_query::QUERY,
            "variables": {
                "callDataInput": {
                    "deadline": deadline,
                    "receiver": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
                    "sender": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
                    "slippagePercentage": "0.01"
                },
                "chain": "MAINNET",
                "queryBatchSwap": false,
                "swapAmount": "1000",
                "swapType": "EXACT_OUT",
                "tokenIn": "0x2170ed0880ac9a755fd29b2688956bd959f933f8",
                "tokenOut": "0xdac17f958d2ee523a2206206994597c13d831ec7",
                "useVaultVersion": 2
            }
        });

        assert_eq!(actual, expected);
    }
}
