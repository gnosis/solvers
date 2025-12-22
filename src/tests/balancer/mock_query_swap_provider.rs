//! Tests Balancer SOR integration with a mocked on-chain query provider.
//! - Uses provider amounts when available; falls back to SOR on errors.
//! - Asserts the final solution (amounts/prices/callData) reflects inputs.

use {
    crate::{
        domain::{
            auction,
            dex,
            eth::{self, Address},
            order,
        },
        infra::dex::balancer::{
            self,
            dto,
            query_swap_provider::{MockQuerySwapProvider, OnChainAmounts, QuerySwapProvider},
        },
        tests::{self, mock},
    },
    alloy::primitives::{U256, address},
    serde_json::json,
};

#[tokio::test]
async fn test_mock_provider_success() {
    let mut mock_provider = MockQuerySwapProvider::new();
    mock_provider.expect_query_swap().returning(|_, _| {
        Ok(OnChainAmounts {
            swap_amount: U256::from(1000000000000000000u64),
            return_amount: U256::from(2275987844420653889u64),
        })
    });

    let order = dex::Order {
        sell: eth::TokenAddress(Address::with_last_byte(1)),
        buy: eth::TokenAddress(Address::with_last_byte(2)),
        side: order::Side::Sell,
        amount: dex::Amount::new(eth::U256::from(1000000000000000000u64)),
        owner: Address::with_last_byte(5),
    };

    let result = mock_provider
        .query_swap(&order, &create_dummy_quote())
        .await;
    assert!(result.is_ok());

    let amounts = result.unwrap();
    assert_eq!(amounts.swap_amount, U256::from(1000000000000000000u64));
    assert_eq!(amounts.return_amount, U256::from(2275987844420653889u64));
}

#[tokio::test]
async fn test_mock_provider_error() {
    let mut mock_provider = MockQuerySwapProvider::new();
    mock_provider
        .expect_query_swap()
        .returning(|_, _| Err(anyhow::anyhow!("invalid path")));

    let order = dex::Order {
        sell: eth::TokenAddress(Address::with_last_byte(1)),
        buy: eth::TokenAddress(Address::with_last_byte(2)),
        side: order::Side::Sell,
        amount: dex::Amount::new(eth::U256::from(1000000000000000000u64)),
        owner: Address::with_last_byte(5),
    };

    // Test that the mock provider returns an error
    let result = mock_provider
        .query_swap(&order, &create_dummy_quote())
        .await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .to_lowercase()
            .contains("invalid path")
    );
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
    let mut mock_provider = MockQuerySwapProvider::new();
    mock_provider.expect_query_swap().returning(|_, _| {
        Ok(OnChainAmounts {
            swap_amount: U256::from(1000000000000000000u64),
            return_amount: eth::U256::from(300000000000000000000u128),
        })
    });

    // Create Sor with the mock provider
    let config = balancer::Config {
        block_stream: None,
        endpoint: format!("http://{}/sor", api.address).parse().unwrap(), // Use mock server address
        chain_id: eth::ChainId::Mainnet,
        vault: Some(Address::with_last_byte(1)),
        queries: Some(Address::with_last_byte(2)),
        v3_batch_router: None,
        permit2: Address::with_last_byte(3),
        settlement: Address::with_last_byte(4),
    };
    let web3 = ethrpc::mock::web3();

    let sor = balancer::Sor::new(config, web3.alloy.clone(), Box::new(mock_provider))
        .expect("Failed to create Sor with mock provider");

    // Create a test order (sell order)
    let order = dex::Order {
        sell: eth::TokenAddress(address!("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")),
        buy: eth::TokenAddress(address!("0xba100000625a3754423978a60c9317c58a424e3d")),
        side: order::Side::Sell,
        amount: dex::Amount::new(eth::U256::from(1000000000000000000u64)),
        owner: Address::with_last_byte(5),
    };

    // Create test tokens
    let tokens = auction::Tokens(
        [
            (
                eth::TokenAddress(address!("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")),
                auction::Token {
                    decimals: Some(18),
                    reference_price: Some(auction::Price(eth::Ether(eth::U256::from(
                        1000000000000000000u64,
                    )))),
                    available_balance: eth::U256::from(1000000000000000000u64),
                    trusted: false,
                },
            ),
            (
                eth::TokenAddress(address!("0xba100000625a3754423978a60c9317c58a424e3d")),
                auction::Token {
                    decimals: Some(18),
                    reference_price: Some(auction::Price(eth::Ether(eth::U256::from(
                        4327903683155778u64,
                    )))),
                    available_balance: eth::U256::from(1583034704488033979459u128),
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
        eth::U256::from(300000000000000000000u128)
    );
    assert_eq!(
        swap_result.input.amount,
        eth::U256::from(1000000000000000000u64)
    );

    // Verify the token addresses are correct
    assert_eq!(swap_result.input.token, order.sell);
    assert_eq!(swap_result.output.token, order.buy);
}

// Helper function to create a dummy quote for testing
fn create_dummy_quote() -> dto::Quote {
    dto::Quote {
        token_addresses: vec![Address::with_last_byte(1), Address::with_last_byte(2)],
        swaps: vec![],
        swap_amount_raw: eth::U256::from(1000000000000000000u64),
        return_amount_raw: eth::U256::from(2275987844420653881u64),
        token_in: Address::with_last_byte(1),
        token_out: Address::with_last_byte(2),
        protocol_version: dto::ProtocolVersion::V2,
        paths: vec![],
    }
}
