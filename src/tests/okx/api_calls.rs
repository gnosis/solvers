use {
    crate::{
        domain::{dex::*, eth::*},
        infra::dex::okx as okx_dex,
    },
    ethereum_types::H160,
    std::{env, str::FromStr},
};

#[ignore]
#[tokio::test]
// To run this test set following environment variables accordingly to your OKX
// setup:  OKX_PROJECT_ID, OKX_API_KEY, OKX_SECRET_KEY, OKX_PASSPHRASE
async fn swap_sell() {
    let okx_config = okx_dex::Config {
        endpoint: reqwest::Url::parse("https://www.okx.com/api/v5/dex/aggregator/swap").unwrap(),
        chain_id: crate::domain::eth::ChainId::Mainnet,
        project_id: env::var("OKX_PROJECT_ID").unwrap(),
        api_key: env::var("OKX_API_KEY").unwrap(),
        api_secret_key: env::var("OKX_SECRET_KEY").unwrap(),
        api_passphrase: env::var("OKX_PASSPHRASE").unwrap(),
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
        amount: Amount::new(U256::from_dec_str("10000000000000").unwrap()),
        owner: H160::from_slice(&hex::decode("6f9ffea7370310cd0f890dfde5e0e061059dcfb8").unwrap()),
    };

    let slippage = Slippage::one_percent();

    let okx = crate::infra::dex::okx::Okx::try_new(okx_config).unwrap();
    let swap_response = okx.swap(&order, &slippage).await;
    let swap = swap_response.unwrap();

    assert_eq!(swap.input.token, order.amount().token);
    assert_eq!(swap.input.amount, order.amount().amount);
    assert_eq!(swap.output.token, order.buy);
}

#[tokio::test]
async fn swap_buy() {
    let okx_config = okx_dex::Config {
        endpoint: reqwest::Url::parse("https://www.okx.com/api/v5/dex/aggregator/swap").unwrap(),
        chain_id: crate::domain::eth::ChainId::Mainnet,
        project_id: String::new(),
        api_key: String::new(),
        api_secret_key: String::new(),
        api_passphrase: String::new(),
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
        amount: Amount::new(U256::from_dec_str("10000000000000").unwrap()),
        owner: H160::from_slice(&hex::decode("6f9ffea7370310cd0f890dfde5e0e061059dcfb8").unwrap()),
    };

    let slippage = Slippage::one_percent();

    let okx = crate::infra::dex::okx::Okx::try_new(okx_config).unwrap();
    let swap_response = okx.swap(&order, &slippage).await;
    assert!(matches!(
        swap_response.unwrap_err(),
        crate::infra::dex::okx::Error::OrderNotSupported
    ));
}

#[ignore]
#[tokio::test]
// To run this test set following environment variables accordingly to your OKX
// setup:  OKX_PROJECT_ID, OKX_API_KEY, OKX_SECRET_KEY, OKX_PASSPHRASE
async fn swap_api_error() {
    let okx_config = okx_dex::Config {
        endpoint: reqwest::Url::parse("https://www.okx.com/api/v5/dex/aggregator/swap").unwrap(),
        chain_id: crate::domain::eth::ChainId::Mainnet,
        project_id: env::var("OKX_PROJECT_ID").unwrap(),
        api_key: env::var("OKX_API_KEY").unwrap(),
        api_secret_key: env::var("OKX_SECRET_KEY").unwrap(),
        api_passphrase: env::var("OKX_PASSPHRASE").unwrap(),
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
        crate::infra::dex::okx::Error::Api { .. }
    ));
}
