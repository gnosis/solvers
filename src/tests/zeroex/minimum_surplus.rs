//! This test verifies that the 0x solver does not generate solutions when the
//! swap returned from the API does not meet the minimum surplus requirement.
//!
//! The actual test case is a modified version of the [`super::market_order`]
//! test cases where the swap barely satisfies the order but doesn't provide
//! the required surplus.

use {
    crate::tests::{self, mock, zeroex},
    serde_json::json,
};

#[tokio::test]
async fn sell_order_insufficient_surplus() {
    let api = mock::http::setup(vec![mock::http::Expectation::Get {
        path: mock::http::Path::exact(
            "swap/allowance-holder/quote?chainId=1&\
             buyToken=0xe41d2489571d322189246dafa5ebde1f4699f498&\
             sellToken=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2&sellAmount=1000000000000000000&\
             taker=0x9008d19f58aabd9ed0d60971565aa8510560ab41&slippageBps=100",
        ),
        res: json!({
            "liquidityAvailable": true,
            "sellAmount": "1000000000000000000",
            // This exactly meets the order's limit price but doesn't provide 1% surplus
            "buyAmount": "230000000000000000000",
            "transaction": {
                "to": "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
                "data": "0x6af479b2\
                       0000000000000000000000000000000000000000000000000000000000000080\
                       0000000000000000000000000000000000000000000000000de0b6b3a7640000\
                       00000000000000000000000000000000000000000000000c9f2c9cd04674edd8\
                       0000000000000000000000000000000000000000000000000000000000000000\
                       000000000000000000000000000000000000000000000000000000000000002b\
                       c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb8e41d2489571d322189\
                       246dafa5ebde1f4699f498000000000000000000000000000000000000000000\
                       869584cd0000000000000000000000009008d19f58aabd9ed0d60971565aa851\
                       0560ab4100000000000000000000000000000000000000000000009c6fd65477\
                       63f8730a",
                "gas": "111000",
            },
            "issues": {
                "allowance": {
                    "spender": "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
                    "actual": "1000000000000000000",
                },
            },
        }),
    }])
    .await;

    let engine = tests::SolverEngine::new(
        "zeroex",
        zeroex::config_with(
            &api.address.to_string(),
            "relative-minimum-surplus = '0.01'"
        ),
    )
    .await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {
                "0xe41d2489571d322189246dafa5ebde1f4699f498": {
                    "decimals": 18,
                    "symbol": "ZRX",
                    "referencePrice": "4327903683155778",
                    "availableBalance": "1583034704488033979459",
                    "trusted": true,
                },
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                    "decimals": 18,
                    "symbol": "WETH",
                    "referencePrice": "1000000000000000000",
                    "availableBalance": "1000000000000000000",
                    "trusted": true,
                },
            },
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                    "buyToken": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                    "sellAmount": "1000000000000000000",
                    "buyAmount": "230000000000000000000", // Expecting 230 ZRX
                    "fullSellAmount": "1000000000000000000",
                    "fullBuyAmount": "230000000000000000000",
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

    assert_eq!(
        solution,
        json!({
            "solutions": []
        }),
    );
}

#[tokio::test]
async fn buy_order_insufficient_surplus() {
    let api = mock::http::setup(vec![mock::http::Expectation::Get {
        path: mock::http::Path::exact(
            "swap/allowance-holder/quote?chainId=1&\
             buyToken=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2&\
             sellToken=0xe41d2489571d322189246dafa5ebde1f4699f498&buyAmount=1000000000000000000&\
             taker=0x9008d19f58aabd9ed0d60971565aa8510560ab41&slippageBps=100",
        ),
        res: json!({
            "liquidityAvailable": true,
            // This exactly meets the order's limit price but doesn't provide 1% surplus
            "sellAmount": "230000000000000000000",
            "buyAmount": "1000000000000000000",
            "transaction": {
                "to": "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
                "data": "0x6af479b2\
                       0000000000000000000000000000000000000000000000000000000000000080\
                       00000000000000000000000000000000000000000000000c9f2c9cd04674edd8\
                       0000000000000000000000000000000000000000000000000de0b6b3a7640000\
                       0000000000000000000000000000000000000000000000000000000000000001\
                       000000000000000000000000000000000000000000000000000000000000002b\
                       e41d2489571d322189246dafa5ebde1f4699f498000bb8c02aaa39b223fe8d0a\
                       0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000\
                       869584cd0000000000000000000000009008d19f58aabd9ed0d60971565aa851\
                       0560ab4100000000000000000000000000000000000000000000009c6fd65477\
                       63f8730a",
                "gas": "111000",
            },
            "issues": {
                "allowance": {
                    "spender": "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
                    "actual": "230000000000000000000",
                },
            },
        }),
    }])
    .await;

    let engine = tests::SolverEngine::new(
        "zeroex",
        zeroex::config_with(
            &api.address.to_string(),
            "relative-minimum-surplus = '0.01'"
        ),
    )
    .await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {
                "0xe41d2489571d322189246dafa5ebde1f4699f498": {
                    "decimals": 18,
                    "symbol": "ZRX",
                    "referencePrice": "4327903683155778",
                    "availableBalance": "1583034704488033979459",
                    "trusted": true,
                },
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                    "decimals": 18,
                    "symbol": "WETH",
                    "referencePrice": "1000000000000000000",
                    "availableBalance": "1000000000000000000",
                    "trusted": true,
                },
            },
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                    "buyToken": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                    "sellAmount": "230000000000000000000", // Expecting to sell 230 ZRX
                    "buyAmount": "1000000000000000000", // Expecting 1 WETH
                    "fullSellAmount": "230000000000000000000",
                    "fullBuyAmount": "1000000000000000000",
                    "kind": "buy",
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

    assert_eq!(
        solution,
        json!({
            "solutions": []
        }),
    );
}

#[tokio::test]
async fn sell_order_with_sufficient_surplus() {
    let api = mock::http::setup(vec![mock::http::Expectation::Get {
        path: mock::http::Path::exact(
            "swap/allowance-holder/quote?chainId=1&\
             buyToken=0xe41d2489571d322189246dafa5ebde1f4699f498&\
             sellToken=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2&sellAmount=1000000000000000000&\
             taker=0x9008d19f58aabd9ed0d60971565aa8510560ab41&slippageBps=100",
        ),
        res: json!({
            "liquidityAvailable": true,
            "sellAmount": "1000000000000000000",
            // This provides more than 1% surplus over the order's limit price
            "buyAmount": "235000000000000000000",
            "transaction": {
                "to": "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
                "data": "0x6af479b2\
                       0000000000000000000000000000000000000000000000000000000000000080\
                       0000000000000000000000000000000000000000000000000de0b6b3a7640000\
                       00000000000000000000000000000000000000000000000cbcacaaef1dd4d88\
                       0000000000000000000000000000000000000000000000000000000000000000\
                       000000000000000000000000000000000000000000000000000000000000002b\
                       c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb8e41d2489571d322189\
                       246dafa5ebde1f4699f498000000000000000000000000000000000000000000\
                       869584cd0000000000000000000000009008d19f58aabd9ed0d60971565aa851\
                       0560ab4100000000000000000000000000000000000000000000009c6fd65477\
                       63f8730a",
                "gas": "111000",
            },
            "issues": {
                "allowance": {
                    "spender": "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
                    "actual": "1000000000000000000",
                },
            },
        }),
    }])
    .await;

    let engine = tests::SolverEngine::new(
        "zeroex",
        zeroex::config_with(
            &api.address.to_string(),
            "relative-minimum-surplus = '0.01'"
        ),
    )
    .await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {
                "0xe41d2489571d322189246dafa5ebde1f4699f498": {
                    "decimals": 18,
                    "symbol": "ZRX",
                    "referencePrice": "4327903683155778",
                    "availableBalance": "1583034704488033979459",
                    "trusted": true,
                },
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                    "decimals": 18,
                    "symbol": "WETH",
                    "referencePrice": "1000000000000000000",
                    "availableBalance": "1000000000000000000",
                    "trusted": true,
                },
            },
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                    "buyToken": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                    "sellAmount": "1000000000000000000",
                    "buyAmount": "230000000000000000000", // Expecting at least 230 ZRX
                    "fullSellAmount": "1000000000000000000",
                    "fullBuyAmount": "230000000000000000000",
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

    // This should produce a solution since the swap provides sufficient surplus
    assert_ne!(
        solution["solutions"].as_array().unwrap().len(),
        0
    );
}