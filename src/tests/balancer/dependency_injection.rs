//! Test to verify that the dependency injection refactoring works correctly.

use {
    crate::{
        domain::{auction, dex, eth, order},
        infra::dex::balancer::{self, dto, query_swap_provider::OnChainAmounts, QuerySwapProvider},
        tests::{self, mock},
    },
    anyhow::{anyhow, Result},
    ethereum_types::{H160, U256},
    serde_json::json,
    std::str::FromStr,
};

/// Mock query swap provider for testing
pub struct MockQuerySwapProvider {
    swap_amount: U256,
    return_amount: U256,
    should_error: bool,
    error_message: Option<String>,
}

impl MockQuerySwapProvider {
    /// Create a mock provider that returns a successful response
    pub fn success(swap_amount: U256, return_amount: U256) -> Self {
        Self {
            swap_amount,
            return_amount,
            should_error: false,
            error_message: None,
        }
    }

    /// Create a mock provider that returns an error
    pub fn error(msg: &str) -> Self {
        Self {
            swap_amount: U256::zero(),
            return_amount: U256::zero(),
            should_error: true,
            error_message: Some(msg.to_string()),
        }
    }
}

#[async_trait::async_trait]
impl QuerySwapProvider for MockQuerySwapProvider {
    async fn query_swap(&self, _order: &dex::Order, _quote: &dto::Quote) -> Result<OnChainAmounts> {
        if self.should_error {
            let msg: String = self
                .error_message
                .clone()
                .unwrap_or_else(|| "mock provider error".to_string());
            Err(anyhow!(msg))
        } else {
            Ok(OnChainAmounts {
                swap_amount: self.swap_amount,
                return_amount: self.return_amount,
            })
        }
    }
}

#[tokio::test]
async fn test_mock_provider_success() {
    let mock_provider = MockQuerySwapProvider::success(
        U256::from(1000000000000000000u64),
        U256::from(2275987844420653889u64), // return_amount (updated by user)
    );

    let order = dex::Order {
        sell: eth::TokenAddress(H160::from_low_u64_be(1)),
        buy: eth::TokenAddress(H160::from_low_u64_be(2)),
        side: order::Side::Sell,
        amount: dex::Amount::new(U256::from(1000000000000000000u64)),
        owner: H160::from_low_u64_be(5),
    };

    let result = mock_provider
        .query_swap(&order, &create_dummy_quote())
        .await; // Formatted by user
    assert!(result.is_ok());

    let amounts = result.unwrap();
    assert_eq!(amounts.swap_amount, U256::from(1000000000000000000u64));
    assert_eq!(amounts.return_amount, U256::from(2275987844420653889u64));
}

#[tokio::test]
async fn test_mock_provider_error() {
    let mock_provider = MockQuerySwapProvider::error("invalid path");

    let order = dex::Order {
        sell: eth::TokenAddress(H160::from_low_u64_be(1)),
        buy: eth::TokenAddress(H160::from_low_u64_be(2)),
        side: order::Side::Sell,
        amount: dex::Amount::new(U256::from(1000000000000000000u64)),
        owner: H160::from_low_u64_be(5),
    };

    // Test that the mock provider returns an error
    let result = mock_provider
        .query_swap(&order, &create_dummy_quote())
        .await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .to_lowercase()
        .contains("invalid path"));
}

#[tokio::test]
async fn test_mock_provider_affects_swap_result() {
    // Set up a mock HTTP server that returns a standard SOR response
    let api = mock::http::setup(vec![mock::http::Expectation::Post {
        path: mock::http::Path::exact("sor"),
        req: mock::http::RequestBody::Partial(
            json!({
                "query": serde_json::to_value(tests::balancer::SWAP_QUERY).unwrap(),
                "variables": {
                    "chain": "MAINNET",
                    "swapAmount": "1",
                    "swapType": "EXACT_IN",
                    "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "tokenOut": "0xba100000625a3754423978a60c9317c58a424e3d",
                }
            }),
            vec!["variables.callDataInput.deadline"],
        ),
        res: json!({
            "data": {
                "sorGetSwapPaths": {
                    "tokenAddresses": [
                        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "0xba100000625a3754423978a60c9317c58a424e3d"
                    ],
                    "swaps": [
                        {
                            "poolId": "0x5c6ee304399dbdb9c8ef030ab642b10820db8f56000200000000000000000014",
                            "assetInIndex": 0,
                            "assetOutIndex": 1,
                            "amount": "1000000000000000000",
                            "userData": "0x",
                            "returnAmount": "227598784442065388110"
                        }
                    ],
                    "swapAmountRaw": "1000000000000000000",
                    "returnAmountRaw": "227598784442065388110",
                    "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "tokenOut": "0xba100000625a3754423978a60c9317c58a424e3d",
                    "protocolVersion": 2,
                    "paths": [],
                }
            }
        }),
    }])
    .await;

    // Create a mock provider that returns different amounts than the SOR response
    // This will help us verify that the mock provider is actually being used
    let mock_provider = MockQuerySwapProvider::success(
        U256::from(1000000000000000000u64), // swap_amount (same as SOR)
        U256::from_dec_str("300000000000000000000").unwrap(), /* return_amount (different from SOR's
                                             * 227598784442065388110) */
    );

    // Create Sor with the mock provider
    let config = balancer::Config {
        block_stream: None,
        endpoint: format!("http://{}/sor", api.address).parse().unwrap(), // Use mock server address
        rpc_url: "http://localhost:8545".parse().unwrap(),
        chain_id: eth::ChainId::Mainnet,
        vault: Some(eth::ContractAddress(H160::from_low_u64_be(1))),
        queries: Some(eth::ContractAddress(H160::from_low_u64_be(2))),
        v3_batch_router: None,
        permit2: eth::ContractAddress(H160::from_low_u64_be(3)),
        settlement: eth::ContractAddress(H160::from_low_u64_be(4)),
    };

    let sor = balancer::Sor::new(config, Box::new(mock_provider))
        .expect("Failed to create Sor with mock provider");

    // Create a test order (sell order)
    let order = dex::Order {
        sell: eth::TokenAddress(
            H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
        ),
        buy: eth::TokenAddress(
            H160::from_str("0xba100000625a3754423978a60c9317c58a424e3d").unwrap(),
        ),
        side: order::Side::Sell,
        amount: dex::Amount::new(U256::from(1000000000000000000u64)),
        owner: H160::from_low_u64_be(5),
    };

    // Create test tokens
    let tokens = auction::Tokens(
        [
            (
                eth::TokenAddress(
                    H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
                ),
                auction::Token {
                    decimals: Some(18),
                    reference_price: Some(auction::Price(eth::Ether(U256::from(
                        1000000000000000000u64,
                    )))),
                    available_balance: U256::from(1000000000000000000u64),
                    trusted: false,
                },
            ),
            (
                eth::TokenAddress(
                    H160::from_str("0xba100000625a3754423978a60c9317c58a424e3d").unwrap(),
                ),
                auction::Token {
                    decimals: Some(18),
                    reference_price: Some(auction::Price(eth::Ether(U256::from(
                        4327903683155778u64,
                    )))),
                    available_balance: U256::from_dec_str("1583034704488033979459").unwrap(),
                    trusted: true,
                },
            ),
        ]
        .into_iter()
        .collect(),
    );

    // Test the swap method with zero slippage
    let slippage = dex::Slippage::zero();
    let swap_result = sor.swap(&order, &slippage, &tokens).await.unwrap();

    // Verify that the swap result uses the mock provider's return amount
    // (300000000000000000000) instead of the SOR response amount
    // (227598784442065388110) For a sell order, the output should be the
    // return_amount from our mock provider
    assert_eq!(
        swap_result.output.amount,
        U256::from_dec_str("300000000000000000000").unwrap()
    );
    assert_eq!(swap_result.input.amount, U256::from(1000000000000000000u64));

    // Verify the token addresses are correct
    assert_eq!(swap_result.input.token, order.sell);
    assert_eq!(swap_result.output.token, order.buy);
}

// Helper function to create a dummy quote for testing
fn create_dummy_quote() -> dto::Quote {
    dto::Quote {
        token_addresses: vec![H160::from_low_u64_be(1), H160::from_low_u64_be(2)],
        swaps: vec![],
        swap_amount_raw: U256::from(1000000000000000000u64),
        return_amount_raw: U256::from(2275987844420653881u64),
        token_in: H160::from_low_u64_be(1),
        token_out: H160::from_low_u64_be(2),
        protocol_version: dto::ProtocolVersion::V2,
        paths: vec![],
    }
}
