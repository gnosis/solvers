//! This test ensures that the Balancer solver properly handles cases where no
//! swap was found for the specified quoted order.

use {
    crate::{
        infra::dex::balancer::dto,
        tests::{self, balancer, mock},
    },
    serde_json::json,
};

/// Tests that orders get marked as "mandatory" in `/quote` requests.
#[tokio::test]
async fn test() {
    let api = mock::http::setup(vec![mock::http::Expectation::Post {
        path: mock::http::Path::Any,
        req: mock::http::RequestBody::Exact(json!({
            "query": serde_json::to_value(dto::get_swap_paths_query::QUERY).unwrap(),
            "variables": {
                "callDataInput": {
                    "receiver": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
                    "sender": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
                    "slippagePercentage": "0.01"
                },
                "chain": "MAINNET",
                "queryBatchSwap": false,
                "swapAmount": "1",
                "swapType": "EXACT_IN",
                "tokenIn": "0x1111111111111111111111111111111111111111",
                "tokenOut": "0x2222222222222222222222222222222222222222",
                "useVaultVersion": 2
            }
        })),
        res: json!({
            "data": {
                "sorGetSwapPaths": {
                    "tokenAddresses": [],
                    "swaps": [],
                    "swapAmountRaw": "0",
                    "returnAmountRaw": "0",
                    "tokenIn": "",
                    "tokenOut": "",
                }
            }
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
                    "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "signingScheme": "presign",
                    "signature": "0x",
                },
            ],
            "liquidity": [],
            "effectiveGasPrice": "15000000000",
            "deadline": "2106-01-01T00:00:00.000Z",
            "surplusCapturingJitOrderOwners": []
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
