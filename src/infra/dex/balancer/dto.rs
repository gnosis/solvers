use {
    crate::{
        domain::{auction, dex, eth, order},
        infra::dex::balancer::Error,
        util::serialize,
    },
    bigdecimal::{num_bigint::BigInt, BigDecimal},
    ethereum_types::{H160, H256, U256},
    number::conversions::u256_to_big_decimal,
    serde::{Deserialize, Serialize, Serializer},
    serde_with::serde_as,
};

/// Get swap quote from the SOR v2 for the V2 vault.
const QUERY: &str = r#"
query sorGetSwapPaths($callDataInput: GqlSwapCallDataInput!, $chain: GqlChain!, $queryBatchSwap: Boolean!, $swapAmount: AmountHumanReadable!, $swapType: GqlSorSwapType!, $tokenIn: String!, $tokenOut: String!) {
    sorGetSwapPaths(
        callDataInput: $callDataInput,
        chain: $chain,
        queryBatchSwap: $queryBatchSwap,
        swapAmount: $swapAmount,
        swapType: $swapType,
        tokenIn: $tokenIn,
        tokenOut: $tokenOut,
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
        protocolVersion
        paths {
            inputAmountRaw
            isBuffer
            outputAmountRaw
            pools
            protocolVersion
            tokens {
              address
            }
        }
    }
}
"#;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Query<'a> {
    query: &'a str,
    variables: Variables,
}

impl Query<'_> {
    pub fn from_domain(
        order: &dex::Order,
        tokens: &auction::Tokens,
        slippage: &dex::Slippage,
        chain: Chain,
        contract_address: eth::ContractAddress,
        query_batch_swap: bool,
        swap_deadline: Option<u64>,
    ) -> Result<Self, Error> {
        let token_decimals = match order.side {
            order::Side::Buy => tokens
                .decimals(&order.buy)
                .ok_or(Error::MissingDecimals(order.buy)),
            order::Side::Sell => tokens
                .decimals(&order.sell)
                .ok_or(Error::MissingDecimals(order.sell)),
        }?;
        let variables = Variables {
            call_data_input: CallDataInput {
                deadline: swap_deadline,
                receiver: contract_address.0,
                sender: contract_address.0,
                slippage_percentage: slippage.as_factor().clone(),
            },
            chain,
            query_batch_swap,
            swap_amount: HumanReadableAmount::from_u256(&order.amount.get(), token_decimals),
            swap_type: SwapType::from_domain(order.side),
            token_in: order.sell.0,
            token_out: order.buy.0,
        };
        Ok(Self {
            query: QUERY,
            variables,
        })
    }
}

/// Refers to the SOR API V3's `AmountHumanReadable` type and represents a token
/// amount without decimals.
#[serde_as]
#[derive(Debug, Default, PartialEq)]
pub struct HumanReadableAmount {
    amount: U256,
    decimals: u8,
}

impl HumanReadableAmount {
    /// Convert a `U256` amount to a human form.
    pub fn from_u256(amount: &U256, decimals: u8) -> HumanReadableAmount {
        Self {
            amount: *amount,
            decimals,
        }
    }

    pub fn value(&self) -> BigDecimal {
        let decimals: BigDecimal = BigInt::from(10).pow(self.decimals as u32).into();
        u256_to_big_decimal(&self.amount) / decimals
    }

    /// Convert the human readable amount to a `U256` with the token's decimals.
    pub fn as_wei(&self) -> U256 {
        self.amount
    }
}

impl Serialize for HumanReadableAmount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.value().serialize(serializer)
    }
}

#[serde_as]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Variables {
    call_data_input: CallDataInput,
    /// The Chain to query.
    chain: Chain,
    /// Whether to run `queryBatchSwap` to update the return amount with most
    /// up-to-date on-chain values.
    query_batch_swap: bool,
    /// The amount to swap in human form.
    swap_amount: HumanReadableAmount,
    /// SwapType either exact_in or exact_out (also givenIn or givenOut).
    swap_type: SwapType,
    /// Token address of the tokenIn.
    token_in: H160,
    /// Token address of the tokenOut.
    token_out: H160,
}

/// Inputs for the call data to create the swap transaction. If this input is
/// given, call data is added to the response.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CallDataInput {
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
#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(super) enum Chain {
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
    pub(crate) fn from_domain(chain_id: eth::ChainId) -> Result<Self, Error> {
        match chain_id {
            eth::ChainId::Mainnet => Ok(Self::Mainnet),
            eth::ChainId::Gnosis => Ok(Self::Gnosis),
            eth::ChainId::ArbitrumOne => Ok(Self::Arbitrum),
            eth::ChainId::Base => Ok(Self::Base),
            eth::ChainId::Goerli => Err(Error::UnsupportedChainId(chain_id)),
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
    /// The swap route (different representation than `swaps` suitable for
    /// the v3 BatchRouter).
    pub paths: Vec<Path>,
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
    /// Which balancer version the trade needs to be routed through.
    pub protocol_version: ProtocolVersion,
}

#[derive(serde_repr::Deserialize_repr, PartialEq, Eq, Default, Debug)]
#[serde(untagged)]
#[repr(u8)]
pub enum ProtocolVersion {
    #[default]
    V2 = 2,
    V3 = 3,
}

impl Quote {
    /// Check for "empty" quotes - i.e. all 0's with no swaps. Balancer SOR API
    /// returns this in case it fails to find a route for whatever reason (not
    /// enough liquidity, no trading path, etc.). We don't consider this an
    /// error case.
    pub fn is_empty(&self) -> bool {
        self.return_amount_raw.is_zero() && self.swap_amount_raw.is_zero() && self.swaps.is_empty()
    }
}

/// A swap included in a larger batched swap.
#[serde_as]
#[derive(Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Swap {
    /// The ID of the pool swapping in this step.
    pub pool_id: PoolId,
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

#[serde_as]
#[derive(Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum PoolId {
    V2(H256),
    V3(H160),
}

impl PoolId {
    pub fn as_v2(&self) -> Result<H256, Error> {
        if let Self::V2(h256) = self {
            Ok(*h256)
        } else {
            Err(Error::InvalidPoolIdFormat)
        }
    }

    pub fn as_v3(&self) -> Result<H160, Error> {
        if let Self::V3(h160) = self {
            Ok(*h160)
        } else {
            Err(Error::InvalidPoolIdFormat)
        }
    }
}

impl Default for PoolId {
    fn default() -> Self {
        Self::V2(H256::default())
    }
}

/// Models a single swap path consisting of multiple steps.
#[serde_as]
#[derive(Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Path {
    #[serde_as(as = "serialize::U256")]
    pub input_amount_raw: U256,
    #[serde_as(as = "serialize::U256")]
    pub output_amount_raw: U256,
    pub is_buffer: Vec<bool>,
    pub pools: Vec<PoolId>,
    pub tokens: Vec<PathToken>,
}

#[serde_as]
#[derive(Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathToken {
    pub address: H160,
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
    use {
        super::*,
        maplit::hashmap,
        number::conversions::big_decimal_to_u256,
        serde_json::json,
        std::str::FromStr,
    };

    #[test]
    fn test_query_serialization() {
        let tokens = auction::Tokens(hashmap! {
            eth::TokenAddress(H160::from_str("0x2170ed0880ac9a755fd29b2688956bd959f933f8").unwrap()) => auction::Token {
                decimals: Some(18),
                symbol: Some("ETH".to_string()),
                reference_price: None,
                available_balance: U256::from(1000),
                trusted: true,
            },
            eth::TokenAddress(H160::from_str("0xdac17f958d2ee523a2206206994597c13d831ec7").unwrap()) => auction::Token {
                decimals: Some(24),
                symbol: Some("USDT".to_string()),
                reference_price: None,
                available_balance: U256::from(1000),
                trusted: true,
            },
        });
        let order = dex::Order {
            sell: H160::from_str("0x2170ed0880ac9a755fd29b2688956bd959f933f8")
                .unwrap()
                .into(),
            buy: H160::from_str("0xdac17f958d2ee523a2206206994597c13d831ec7")
                .unwrap()
                .into(),
            side: order::Side::Buy,
            amount: dex::Amount::new(U256::from(1000)),
            owner: H160::from_str("0x9008d19f58aabd9ed0d60971565aa8510560ab41").unwrap(),
        };
        let slippage = dex::Slippage::one_percent();
        let chain = Chain::Mainnet;
        let contract_address = eth::ContractAddress(
            H160::from_str("0x9008d19f58aabd9ed0d60971565aa8510560ab41").unwrap(),
        );
        let query = Query::from_domain(
            &order,
            &tokens,
            &slippage,
            chain,
            contract_address,
            false,
            Some(12345_u64),
        )
        .unwrap();

        let actual = serde_json::to_value(query).unwrap();
        let expected = json!({
            "query": QUERY,
            "variables": {
                "callDataInput": {
                    "deadline": 12345,
                    "receiver": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
                    "sender": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
                    "slippagePercentage": "0.01"
                },
                "chain": "MAINNET",
                "queryBatchSwap": false,
                "swapAmount": "0.000000000000000000001",
                "swapType": "EXACT_OUT",
                "tokenIn": "0x2170ed0880ac9a755fd29b2688956bd959f933f8",
                "tokenOut": "0xdac17f958d2ee523a2206206994597c13d831ec7",
            }
        });

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_human_readable_amount() {
        let amount =
            big_decimal_to_u256(&BigDecimal::from_str("1230000000000000000021").unwrap()).unwrap();
        let human_readable_amount = HumanReadableAmount::from_u256(&amount, 18);
        assert_eq!(
            human_readable_amount.value(),
            BigDecimal::from_str("1230.000000000000000021").unwrap()
        );
        assert_eq!(human_readable_amount.as_wei(), amount);

        let amount =
            big_decimal_to_u256(&BigDecimal::from_str("1230000000000021").unwrap()).unwrap();
        let human_readable_amount = HumanReadableAmount::from_u256(&amount, 18);
        assert_eq!(
            human_readable_amount.value(),
            BigDecimal::from_str("0.001230000000000021").unwrap()
        );
        assert_eq!(human_readable_amount.as_wei(), amount);
    }
}
