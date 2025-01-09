//! This test ensures that the OKX solver properly handles cases where no swap
//! was found for the specified order.

use {
    crate::tests::{self, mock},
    serde_json::json,
};

#[tokio::test]
async fn sell() {
    let api = mock::http::setup(vec![mock::http::Expectation::Get {
        path: mock::http::Path::exact(
            "?chainId=1&amount=1000000000000000000&\
             fromTokenAddress=0xc8cd2be653759aed7b0996315821aae71e1feadf&\
             toTokenAddress=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2&slippage=0.01&\
             userWalletAddress=0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a",
        ),
        res: json!({"code":"82000","data":[],"msg":"Insufficient liquidity."}),
    }])
    .await;

    let engine = tests::SolverEngine::new("okx", super::config(&api.address)).await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {
                "0xC8CD2BE653759aed7B0996315821AAe71e1FEAdF": {
                    "decimals": 18,
                    "symbol": "TETH",
                    "referencePrice": "4327903683155778",
                    "availableBalance": "1583034704488033979459",
                    "trusted": true,
                },
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                    "decimals": 18,
                    "symbol": "WETH",
                    "referencePrice": "1000000000000000000",
                    "availableBalance": "482725140468789680",
                    "trusted": true,
                },
            },
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0xC8CD2BE653759aed7B0996315821AAe71e1FEAdF",
                    "buyToken": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                    "sellAmount": "1000000000000000000",
                    "buyAmount": "200000000000000000000",
                    "fullSellAmount": "1000000000000000000",
                    "fullBuyAmount": "200000000000000000000",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                    "sellTokenSource": "erc20",
                    "buyTokenDestination": "erc20",
                    "preInteractions": [],
                    "postInteractions": [],
                    "owner": "0x5b1e2c2762667331bc91648052f646d1b0d35984",
                    "validTo": 0,
                    "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "signingScheme": "presign",
                    "signature": "0x",
                }
            ],
            "liquidity": [],
            "effectiveGasPrice": "15000000000",
            "deadline": "2106-01-01T00:00:00.000Z",
            "surplusCapturingJitOrderOwners": []
        }))
        .await
        .unwrap();

    assert_eq!(solution, json!({ "solutions": [] }),);
}
