//! Test to verify that the dependency injection refactoring works correctly.

use {
    crate::{
        domain::{auction, dex, eth, order},
        infra::dex::balancer::{self, dto, query_swap_provider::OnChainAmounts, QuerySwapProvider},
    },
    ethereum_types::{H160, U256},
    std::str::FromStr,
};

/// Mock query swap provider for testing
pub struct MockQuerySwapProvider {
    swap_amount: U256,
    return_amount: U256,
    should_error: bool,
    error: Option<crate::infra::dex::balancer::Error>,
}

impl MockQuerySwapProvider {
    /// Create a mock provider that returns a successful response
    pub fn success(swap_amount: U256, return_amount: U256) -> Self {
        Self {
            swap_amount,
            return_amount,
            should_error: false,
            error: None,
        }
    }

    /// Create a mock provider that returns an error
    pub fn error(error: crate::infra::dex::balancer::Error) -> Self {
        Self {
            swap_amount: U256::zero(),
            return_amount: U256::zero(),
            should_error: true,
            error: Some(error),
        }
    }
}

#[async_trait::async_trait]
impl QuerySwapProvider for MockQuerySwapProvider {
    async fn query_swap(
        &self,
        _order: &dex::Order,
        _quote: &dto::Quote,
    ) -> Result<OnChainAmounts, crate::infra::dex::balancer::Error> {
        if self.should_error {
            Err(crate::infra::dex::balancer::Error::InvalidPath) // Use a simple
                                                                 // error for testing
        } else {
            Ok(OnChainAmounts {
                swap_amount: self.swap_amount,
                return_amount: self.return_amount,
            })
        }
    }
}

#[tokio::test]
async fn test_sor_with_mock_query_swap_provider() {
    // Create a mock provider that returns successful on-chain amounts
    let mock_provider = MockQuerySwapProvider::success(
        U256::from(1000000000000000000u64), // swap_amount
        U256::from(2275987844420653881u64), // return_amount
    );

    // Create a minimal Sor config
    let config = balancer::Config {
        block_stream: None,
        endpoint: "http://localhost:8080".parse().unwrap(),
        rpc_url: "http://localhost:8545".parse().unwrap(),
        vault: Some(eth::ContractAddress(H160::from_low_u64_be(1))),
        v3_batch_router: None,
        queries: Some(eth::ContractAddress(H160::from_low_u64_be(2))),
        permit2: eth::ContractAddress(H160::from_low_u64_be(3)),
        settlement: eth::ContractAddress(H160::from_low_u64_be(4)),
        chain_id: eth::ChainId::Mainnet,
    };

    // Create Sor with the mock provider
    let _sor = balancer::Sor::new(config, Box::new(mock_provider))
        .expect("Failed to create Sor with mock provider");

    // Verify that the Sor was created successfully
    // This test mainly verifies that the dependency injection pattern works
    // and that the Sor can be constructed with a mock provider
    assert!(true); // If we get here, the construction succeeded
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
    let mock_provider = MockQuerySwapProvider::error(balancer::Error::InvalidPath);

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
    assert!(matches!(result.unwrap_err(), balancer::Error::InvalidPath));
}

#[tokio::test]
async fn test_mock_provider_affects_swap_result() {
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
        endpoint: "http://localhost:8545".parse().unwrap(),
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
