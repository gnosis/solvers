//! This test verifies that the Balancer solver does not generate solutions when
//! the swap returned from the API does not meet the minimum surplus
//! requirement.
//!
//! This includes comprehensive testing of both buy and sell orders.

use {
    crate::tests::{
        self,
        balancer::{self, SWAP_QUERY},
        mock,
    },
    serde_json::json,
};

#[tokio::test]
async fn buy_order_insufficient_surplus() {
    let api = mock::http::setup(vec![mock::http::Expectation::Post {
        path: mock::http::Path::exact("sor"),
        req: mock::http::RequestBody::Partial(json!({
            "query": serde_json::to_value(SWAP_QUERY).unwrap(),
            "variables": {
                "callDataInput": {
                  "receiver": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
                  "sender": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
                  "slippagePercentage": "0.01"
                },
                "chain": "MAINNET",
                "queryBatchSwap": false,
                "swapAmount": "230",
                "swapType": "EXACT_OUT",
                "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                "tokenOut": "0xe41d2489571d322189246dafa5ebde1f4699f498",
            }
        }), vec!["variables.callDataInput.deadline"]),
        res: json!({
            "data": {
                "sorGetSwapPaths": {
                    "tokenAddresses": [
                        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "0xe41d2489571d322189246dafa5ebde1f4699f498"
                    ],
                    "swaps": [
                        {
                            "poolId": "0x5c6ee304399dbdb9c8ef030ab642b10820db8f56000200000000000000000014",
                            "assetInIndex": 0,
                            "assetOutIndex": 1,
                            // This exactly meets the order's limit price but doesn't provide 1% surplus
                            // Order wants 230 ZRX, with 1% surplus needs 230 * 1.01 = 232.3 ZRX
                            // But swap only provides 230 ZRX (no surplus)
                            "amount": "1000000000000000000",
                            "userData": "0x",
                        }
                    ],
                    "swapAmountRaw": "1000000000000000000",
                    "returnAmountRaw": "230000000000000000000",
                    "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "tokenOut": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                    "protocolVersion": 2,
                    "paths": [
                        {
                            "inputAmountRaw": "1000000000000000000",
                            "outputAmountRaw": "230000000000000000000",
                            "protocolVersion": 2,
                            "isBuffer": [false],
                            "pools": [
                                "0x5c6ee304399dbdb9c8ef030ab642b10820db8f56000200000000000000000014"
                            ],
                            "tokens": [
                                {
                                    "address": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                                },
                                {
                                    "address": "0xe41d2489571d322189246dafa5ebde1f4699f498"
                                }
                            ]
                        }
                    ]
                }
            }
        }),
    }])
    .await;

    let engine = tests::SolverEngine::new(
        "balancer",
        balancer::config_with(&api.address, "relative-minimum-surplus = '0.01'"),
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
                    "sellAmount": "1000000000000000000", // Willing to pay up to 1 WETH
                    "buyAmount": "230000000000000000000", // Want exactly 230 ZRX
                    "fullSellAmount": "1000000000000000000",
                    "fullBuyAmount": "230000000000000000000",
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
async fn buy_order_with_sufficient_surplus() {
    let api = mock::http::setup(vec![mock::http::Expectation::Post {
        path: mock::http::Path::exact("sor"),
        req: mock::http::RequestBody::Partial(json!({
            "query": serde_json::to_value(SWAP_QUERY).unwrap(),
            "variables": {
                "callDataInput": {
                  "receiver": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
                  "sender": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
                  "slippagePercentage": "0.01"
                },
                "chain": "MAINNET",
                "queryBatchSwap": false,
                "swapAmount": "230",
                "swapType": "EXACT_OUT",
                "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                "tokenOut": "0xe41d2489571d322189246dafa5ebde1f4699f498",
            }
        }), vec!["variables.callDataInput.deadline"]),
        res: json!({
            "data": {
                "sorGetSwapPaths": {
                    "tokenAddresses": [
                        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "0xe41d2489571d322189246dafa5ebde1f4699f498"
                    ],
                    "swaps": [
                        {
                            "poolId": "0x5c6ee304399dbdb9c8ef030ab642b10820db8f56000200000000000000000014",
                            "assetInIndex": 0,
                            "assetOutIndex": 1,
                            // This provides sufficient surplus: provides 235 ZRX for 230 ZRX order
                            // With 1% surplus requirement, needs 232.3 ZRX, gets 235 ZRX (sufficient)
                            "amount": "990000000000000000",
                            "userData": "0x",
                        }
                    ],
                    "returnAmountRaw": "990000000000000000",
                    "swapAmountRaw": "230000000000000000000",
                    "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "tokenOut": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                    "protocolVersion": 2,
                    "paths": [
                        {
                            "inputAmountRaw": "990000000000000000",
                            "outputAmountRaw": "230000000000000000000",
                            "protocolVersion": 2,
                            "isBuffer": [false],
                            "pools": [
                                "0x5c6ee304399dbdb9c8ef030ab642b10820db8f56000200000000000000000014"
                            ],
                            "tokens": [
                                {
                                    "address": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                                },
                                {
                                    "address": "0xe41d2489571d322189246dafa5ebde1f4699f498"
                                }
                            ]
                        }
                    ]
                }
            }
        }),
    }])
    .await;

    let engine = tests::SolverEngine::new(
        "balancer",
        balancer::config_with(&api.address, "relative-minimum-surplus = '0.005'"),
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
                    "sellAmount": "1000000000000000000", // Willing to pay up to 1 WETH
                    "buyAmount": "230000000000000000000", // Want exactly 230 ZRX
                    "fullSellAmount": "1000000000000000000",
                    "fullBuyAmount": "230000000000000000000",
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

    // This should produce a solution since the swap provides sufficient surplus
    assert_ne!(solution["solutions"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn sell_order_insufficient_surplus() {
    let api = mock::http::setup(vec![mock::http::Expectation::Post {
        path: mock::http::Path::exact("sor"),
        req: mock::http::RequestBody::Partial(json!({
            "query": serde_json::to_value(SWAP_QUERY).unwrap(),
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
                "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                "tokenOut": "0xe41d2489571d322189246dafa5ebde1f4699f498",
            }
        }), vec!["variables.callDataInput.deadline"]),
        res: json!({
            "data": {
                "sorGetSwapPaths": {
                    "tokenAddresses": [
                        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "0xe41d2489571d322189246dafa5ebde1f4699f498"
                    ],
                    "swaps": [
                        {
                            "poolId": "0x5c6ee304399dbdb9c8ef030ab642b10820db8f56000200000000000000000014",
                            "assetInIndex": 0,
                            "assetOutIndex": 1,
                            // This exactly meets the order's limit price but doesn't provide 1% surplus
                            // Order expects at least 230 ZRX, swap returns exactly 230 (no surplus)
                            "amount": "1000000000000000000",
                            "userData": "0x",
                        }
                    ],
                    "swapAmountRaw": "1000000000000000000",
                    "returnAmountRaw": "230000000000000000000",
                    "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "tokenOut": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                    "protocolVersion": 2,
                    "paths": [
                        {
                            "inputAmountRaw": "1000000000000000000",
                            "outputAmountRaw": "230000000000000000000",
                            "protocolVersion": 2,
                            "isBuffer": [false],
                            "pools": [
                                "0x5c6ee304399dbdb9c8ef030ab642b10820db8f56000200000000000000000014"
                            ],
                            "tokens": [
                                {
                                    "address": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                                },
                                {
                                    "address": "0xe41d2489571d322189246dafa5ebde1f4699f498"
                                }
                            ]
                        }
                    ]
                }
            }
        }),
    }])
    .await;

    let engine = tests::SolverEngine::new(
        "balancer",
        balancer::config_with(&api.address, "relative-minimum-surplus = '0.01'"),
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

    assert_eq!(
        solution,
        json!({
            "solutions": []
        }),
    );
}

#[tokio::test]
async fn sell_order_with_sufficient_surplus() {
    let api = mock::http::setup(vec![mock::http::Expectation::Post {
        path: mock::http::Path::exact("sor"),
        req: mock::http::RequestBody::Partial(json!({
            "query": serde_json::to_value(SWAP_QUERY).unwrap(),
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
                "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                "tokenOut": "0xe41d2489571d322189246dafa5ebde1f4699f498",
            }
        }), vec!["variables.callDataInput.deadline"]),
        res: json!({
            "data": {
                "sorGetSwapPaths": {
                    "tokenAddresses": [
                        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "0xe41d2489571d322189246dafa5ebde1f4699f498"
                    ],
                    "swaps": [
                        {
                            "poolId": "0x5c6ee304399dbdb9c8ef030ab642b10820db8f56000200000000000000000014",
                            "assetInIndex": 0,
                            "assetOutIndex": 1,
                            // This provides sufficient surplus: gives 235 ZRX for 230 ZRX order
                            // With 1% surplus requirement, needs 232.3 ZRX, gets 235 ZRX (sufficient)
                            "amount": "1000000000000000000",
                            "userData": "0x",
                        }
                    ],
                    "swapAmountRaw": "1000000000000000000",
                    "returnAmountRaw": "235000000000000000000",
                    "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "tokenOut": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                    "protocolVersion": 2,
                    "paths": [
                        {
                            "inputAmountRaw": "1000000000000000000",
                            "outputAmountRaw": "235000000000000000000",
                            "protocolVersion": 2,
                            "isBuffer": [false],
                            "pools": [
                                "0x5c6ee304399dbdb9c8ef030ab642b10820db8f56000200000000000000000014"
                            ],
                            "tokens": [
                                {
                                    "address": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                                },
                                {
                                    "address": "0xe41d2489571d322189246dafa5ebde1f4699f498"
                                }
                            ]
                        }
                    ]
                }
            }
        }),
    }])
    .await;

    let engine = tests::SolverEngine::new(
        "balancer",
        balancer::config_with(&api.address, "relative-minimum-surplus = '0.01'"),
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
    assert_ne!(solution["solutions"].as_array().unwrap().len(), 0);
}
