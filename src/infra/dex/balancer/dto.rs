use {
    crate::{
        domain::{dex, eth, order},
        infra::dex::balancer::Error,
        util::serialize,
    },
    bigdecimal::BigDecimal,
    ethereum_types::{H160, H256, U256},
    number::conversions::{big_decimal_to_u256, u256_to_big_decimal},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

pub mod get_swap_paths_query {
    /// Get swap quote from the SOR v2 for the V2 vault.
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
        swapAmountRaw
        returnAmountRaw
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
    pub fn from_domain(
        order: &dex::Order,
        slippage: &dex::Slippage,
        chain_id: eth::ChainId,
        contract_address: eth::ContractAddress,
        query_batch_swap: bool,
    ) -> Result<Self, Error> {
        let variables = Variables {
            call_data_input: CalDataInput {
                deadline: None,
                receiver: contract_address.0,
                sender: contract_address.0,
                slippage_percentage: slippage.as_factor().clone(),
            },
            chain: Chain::from_domain(chain_id)?,
            query_batch_swap,
            swap_amount: EtherAmount::from_wei(&order.amount.get()),
            swap_type: SwapType::from_domain(order.side),
            token_in: order.sell.0,
            token_out: order.buy.0,
            use_vault_version: VaultVersion::V2 as u8,
        };
        Ok(Self {
            query: get_swap_paths_query::QUERY,
            variables,
        })
    }
}

/// Ether amount refers to the SOR API V3's `AmountHumanReadable` type.
#[serde_as]
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EtherAmount(BigDecimal);

impl EtherAmount {
    pub fn from_wei(wei: &U256) -> EtherAmount {
        Self(u256_to_big_decimal(wei) / BigDecimal::from(10_u64.pow(18)))
    }

    pub fn value(&self) -> BigDecimal {
        self.0.clone()
    }

    pub fn to_wei(&self) -> Option<U256> {
        big_decimal_to_u256(&(&self.0 * BigDecimal::from(10_u64.pow(18))))
    }
}

#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Variables {
    call_data_input: CalDataInput,
    /// The Chain to query.
    chain: Chain,
    /// Whether to run `queryBatchSwap` to update the return amount with most
    /// up-to-date on-chain values.
    query_batch_swap: bool,
    /// The amount to swap in human form.
    swap_amount: EtherAmount,
    /// SwapType either exact_in or exact_out (also givenIn or givenOut).
    swap_type: SwapType,
    /// Token address of the tokenIn.
    token_in: H160,
    /// Token address of the tokenOut.
    token_out: H160,
    /// Which vault version to use. If none provided, will chose the better
    /// return from either version.
    use_vault_version: u8,
}

/// Inputs for the call data to create the swap transaction. If this input is
/// given, call data is added to the response.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CalDataInput {
    /// How long the swap should be valid, provide a timestamp. `999999999` for
    /// infinite. Default: infinite.
    #[serde(skip_serializing_if = "Option::is_none")]
    deadline: Option<u64>,
    /// Who receives the output amount.
    receiver: H160,
    /// Who sends the input amount.
    sender: H160,
    /// The max slippage in percent 0.01 -> 0.01%.
    slippage_percentage: BigDecimal,
}

/// Balancer SOR API supported chains.
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

impl Chain {
    fn from_domain(chain_id: eth::ChainId) -> Result<Self, Error> {
        match chain_id {
            eth::ChainId::Mainnet => Ok(Self::Mainnet),
            eth::ChainId::Gnosis => Ok(Self::Gnosis),
            eth::ChainId::ArbitrumOne => Ok(Self::Arbitrum),
            unsupported => Err(Error::UnsupportedChainId(unsupported)),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum SwapType {
    ExactIn,
    ExactOut,
}

impl SwapType {
    fn from_domain(side: order::Side) -> Self {
        match side {
            order::Side::Buy => Self::ExactOut,
            order::Side::Sell => Self::ExactIn,
        }
    }
}

enum VaultVersion {
    V2 = 2,
}

/// The response from the Balancer SOR service.
#[serde_as]
#[derive(Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSwapPathsResponse {
    pub data: Data,
}

/// The data field in the Balancer SOR response.
#[serde_as]
#[derive(Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub sor_get_swap_paths: Quote,
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
    pub swap_amount_raw: U256,
    /// The returned token amount.
    ///
    /// In buy token for sell orders or sell token for buy orders.
    #[serde_as(as = "serialize::U256")]
    pub return_amount_raw: U256,
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

mod wei_from_human_form {
    use {
        crate::domain::eth,
        bigdecimal::BigDecimal,
        number::conversions::big_decimal_to_u256,
        serde::{de::Error, Deserialize as _, Deserializer},
    };

    pub fn deserialize<'de, D>(deserializer: D) -> Result<eth::U256, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = BigDecimal::deserialize(deserializer)?;
        big_decimal_to_u256(&value)
            .and_then(|eth| eth.checked_mul(eth::U256::exp10(18)))
            .ok_or_else(|| D::Error::custom("Invalid value for U256: out of range or format error"))
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
    use {super::*, serde_json::json, std::str::FromStr};

    #[test]
    fn test_query_serialization() {
        let order = dex::Order {
            sell: H160::from_str("0x2170ed0880ac9a755fd29b2688956bd959f933f8")
                .unwrap()
                .into(),
            buy: H160::from_str("0xdac17f958d2ee523a2206206994597c13d831ec7")
                .unwrap()
                .into(),
            side: order::Side::Buy,
            amount: dex::Amount::new(U256::from(1000)),
        };
        let slippage = dex::Slippage::one_percent();
        let chain_id = eth::ChainId::Mainnet;
        let contract_address = eth::ContractAddress(
            H160::from_str("0x9008d19f58aabd9ed0d60971565aa8510560ab41").unwrap(),
        );
        let query =
            Query::from_domain(&order, &slippage, chain_id, contract_address, false).unwrap();

        let actual = serde_json::to_value(query).unwrap();
        let expected = json!({
            "query": get_swap_paths_query::QUERY,
            "variables": {
                "callDataInput": {
                    "receiver": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
                    "sender": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
                    "slippagePercentage": "0.01"
                },
                "chain": "MAINNET",
                "queryBatchSwap": false,
                "swapAmount": "0.000000000000001",
                "swapType": "EXACT_OUT",
                "tokenIn": "0x2170ed0880ac9a755fd29b2688956bd959f933f8",
                "tokenOut": "0xdac17f958d2ee523a2206206994597c13d831ec7",
                "useVaultVersion": 2
            }
        });

        assert_eq!(actual, expected);
    }
}
