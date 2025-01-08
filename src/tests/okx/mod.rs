use {
    crate::{
        domain::{
            dex::*,
            eth::{self, *},
        },
        infra::dex::okx as okx_dex,
    },
    ethereum_types::H160,
    std::{env, str::FromStr},
};

#[ignore]
#[tokio::test]
// To run this test set following environment variables accordingly to your OKX
// setup:  OKX_PROJECT_ID, OKX_API_KEY, OKX_SECRET_KEY, OKX_PASSPHRASE
async fn simple_sell() {
    let okx_config = okx_dex::Config {
        endpoint: reqwest::Url::parse("https://www.okx.com/api/v5/dex/aggregator/swap").unwrap(),
        chain_id: crate::domain::eth::ChainId::Mainnet,
        project_id: env::var("OKX_PROJECT_ID").unwrap(),
        api_key: env::var("OKX_API_KEY").unwrap(),
        api_secret_key: env::var("OKX_SECRET_KEY").unwrap(),
        api_passphrase: env::var("OKX_PASSPHRASE").unwrap(),
        settlement: eth::ContractAddress(H160::from_slice(
            &hex::decode("6f9ffea7370310cd0f890dfde5e0e061059dcfb8").unwrap(),
        )),
        block_stream: None,
    };

    let order = Order {
        sell: TokenAddress::from(H160::from_slice(
            &hex::decode("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee").unwrap(),
        )),
        buy: TokenAddress::from(H160::from_slice(
            &hex::decode("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
        )),
        side: crate::domain::order::Side::Sell,
        amount: Amount::new(U256::from_str("10000000000000").unwrap()),
        owner: H160::from_slice(&hex::decode("6f9ffea7370310cd0f890dfde5e0e061059dcfb8").unwrap()),
    };

    let slippage = Slippage::one_percent();

    let okx = crate::infra::dex::okx::Okx::try_new(okx_config).unwrap();
    let swap_response = okx.swap(&order, &slippage).await;
    let swap = swap_response.unwrap();

    assert_eq!(swap.input.token, order.amount().token);
    assert_eq!(swap.input.amount, order.amount().amount);
    assert_eq!(swap.output.token, order.buy);
    assert_eq!(swap.allowance.spender.0, order.owner);
}

#[ignore]
#[tokio::test]
// To run this test set following environment variables accordingly to your OKX
// setup:  OKX_PROJECT_ID, OKX_API_KEY, OKX_SECRET_KEY, OKX_PASSPHRASE
async fn simple_buy() {
    let okx_config = okx_dex::Config {
        endpoint: reqwest::Url::parse("https://www.okx.com/api/v5/dex/aggregator/swap").unwrap(),
        chain_id: crate::domain::eth::ChainId::Mainnet,
        project_id: env::var("OKX_PROJECT_ID").unwrap(),
        api_key: env::var("OKX_API_KEY").unwrap(),
        api_secret_key: env::var("OKX_SECRET_KEY").unwrap(),
        api_passphrase: env::var("OKX_PASSPHRASE").unwrap(),
        settlement: eth::ContractAddress(H160::from_slice(
            &hex::decode("6f9ffea7370310cd0f890dfde5e0e061059dcfb8").unwrap(),
        )),
        block_stream: None,
    };

    let order = Order {
        buy: TokenAddress::from(H160::from_slice(
            &hex::decode("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee").unwrap(),
        )),
        sell: TokenAddress::from(H160::from_slice(
            &hex::decode("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
        )),
        side: crate::domain::order::Side::Buy,
        amount: Amount::new(U256::from_str("10000000000000").unwrap()),
        owner: H160::from_slice(&hex::decode("6f9ffea7370310cd0f890dfde5e0e061059dcfb8").unwrap()),
    };

    let slippage = Slippage::one_percent();

    let okx = crate::infra::dex::okx::Okx::try_new(okx_config).unwrap();
    let swap_response = okx.swap(&order, &slippage).await;
    let swap = swap_response.unwrap();

    assert_eq!(swap.input.token, order.amount().token);
    assert_eq!(swap.input.amount, order.amount().amount);
    assert_eq!(swap.output.token, order.sell);
    assert_eq!(swap.allowance.spender.0, order.owner);
}

#[ignore]
#[tokio::test]
// To run this test set following environment variables accordingly to your OKX
// setup:  OKX_PROJECT_ID, OKX_API_KEY, OKX_SECRET_KEY, OKX_PASSPHRASE
async fn simple_api_error() {
    init_logging();
    env::set_var("OKX_PROJECT_ID", "5d0b6cbaf8e9cedb7eb6836a0f35d961");
    env::set_var("OKX_API_KEY", "4b2ba8b8-4201-4f53-9587-74420a702be6");
    env::set_var("OKX_SECRET_KEY", "E5BFA3CC41B40A52BF78AA75D07DD148");
    env::set_var("OKX_PASSPHRASE", "xjmgdqY6ApuZdfQFCnvR$");

    let okx_config = okx_dex::Config {
        endpoint: reqwest::Url::parse("https://www.okx.com/api/v5/dex/aggregator/swap").unwrap(),
        chain_id: crate::domain::eth::ChainId::Mainnet,
        project_id: env::var("OKX_PROJECT_ID").unwrap(),
        api_key: env::var("OKX_API_KEY").unwrap(),
        api_secret_key: env::var("OKX_SECRET_KEY").unwrap(),
        api_passphrase: env::var("OKX_PASSPHRASE").unwrap(),
        settlement: eth::ContractAddress(H160::from_slice(
            &hex::decode("6f9ffea7370310cd0f890dfde5e0e061059dcfb8").unwrap(),
        )),
        block_stream: None,
    };

    let order = Order {
        sell: TokenAddress::from(H160::from_slice(
            &hex::decode("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee").unwrap(),
        )),
        buy: TokenAddress::from(H160::from_slice(
            &hex::decode("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
        )),
        side: crate::domain::order::Side::Sell,
        amount: Amount::new(U256::from_str("0").unwrap()),
        owner: H160::from_slice(&hex::decode("6f9ffea7370310cd0f890dfde5e0e061059dcfb8").unwrap()),
    };

    let slippage = Slippage::one_percent();

    let okx = crate::infra::dex::okx::Okx::try_new(okx_config).unwrap();
    let swap_response = okx.swap(&order, &slippage).await;

    assert!(matches!(
        swap_response.unwrap_err(),
        crate::tests::okx::okx_dex::Error::Api { .. }
    ));
}
