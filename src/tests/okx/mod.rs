#![allow(unreachable_code)]
#![allow(unused_imports)]
use {
    crate::domain::dex::*,
    crate::domain::eth,
    crate::domain::eth::*,
    crate::{
        infra::{config::dex::okx as okx_config, dex::okx as okx_dex},
        tests::{self, mock, okx},
    },
    bigdecimal::BigDecimal,
    ethereum_types::H160,
    serde_json::json,
    std::default,
    std::num::NonZeroUsize,
    std::str::FromStr,
    std::env,
};

#[ignore]
#[tokio::test]
// To run this test set following environment variables accordingly to your OKX setup:
//  OKX_PROJECT_ID, OKX_API_KEY, OKX_SECRET_KEY, OKX_PASSPHRASE
async fn simple() {
    let okx_config = okx_dex::Config {
        endpoint: reqwest::Url::parse("https://www.okx.com/api/v5/dex/aggregator/")
            .unwrap(),
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
        side: crate::domain::order::Side::Buy,
        amount: Amount::new(U256::from_str("10000000000000").unwrap()),
        owner: H160::from_slice(
            &hex::decode("6f9ffea7370310cd0f890dfde5e0e061059dcfb8").unwrap(),
        ),
    };

    let slippage = Slippage::one_percent();

    let okx = crate::infra::dex::okx::Okx::new(okx_config).unwrap();
    let swap_result = okx.swap(&order, &slippage).await;
    swap_result.unwrap();
}
