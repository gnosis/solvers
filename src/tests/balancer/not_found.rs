//! This test ensures that the Balancer solver properly handles cases where no
//! swap was found for the specified quoted order.

use {
    crate::tests::{self, balancer, mock},
    serde_json::json,
};

/// Tests that orders get marked as "mandatory" in `/quote` requests.
#[tokio::test]
async fn test() {
    let api = mock::http::setup(vec![mock::http::Expectation::Post {
        path: mock::http::Path::Any,
        req: mock::http::RequestBody::Exact(json!({
            "sellToken": "0x1111111111111111111111111111111111111111",
            "buyToken": "0x2222222222222222222222222222222222222222",
            "orderKind": "sell",
            "amount": "1000000000000000000",
            "gasPrice": "15000000000",
        })),
        res: json!({
            "tokenAddresses": [],
            "swaps": [],
            "swapAmount": "0",
            "swapAmountForSwaps": "0",
            "returnAmount": "0",
            "returnAmountFromSwaps": "0",
            "returnAmountConsideringFees": "0",
            "tokenIn": "",
            "tokenOut": "",
            "marketSp": "0",
        }),
    }])
    .await;

    let engine = tests::SolverEngine::new("balancer", balancer::config(&api.address)).await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {},
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0x1111111111111111111111111111111111111111",
                    "buyToken": "0x2222222222222222222222222222222222222222",
                    "sellAmount": "1000000000000000000",
                    "buyAmount": "1000000000000000000",
                    "fullSellAmount": "1000000000000000000",
                    "fullBuyAmount": "100000000000000000000",
                    "kind": "sell",
                    "partiallyFillable": false,
                    "class": "market",
                    "sellTokenSource": "erc20",
                    "buyTokenDestination": "erc20",
                    "preInteractions": [],
                    "postInteractions": [],
                    "owner": "0x5b1e2c2762667331bc91648052f646d1b0d35984",
                    "validTo": 0,
                    "appData": "0x6000000000000000000000000000000000000000000000000000000000000007",
                    "signingScheme": "presign",
                    "signature": "0x",
                },
            ],
            "liquidity": [],
            "effectiveGasPrice": "15000000000",
            "deadline": "2106-01-01T00:00:00.000Z",
        }))
        .await
        .unwrap();

    assert_eq!(
        solution,
        json!({
            "solutions": []
        }),
    );
}
