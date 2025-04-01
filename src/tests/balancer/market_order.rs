//! This test ensures that the Balancer SOR solver properly handles sell and buy
//! market orders, turning Balancer SOR responses into CoW Protocol solutions.

use {
    crate::tests::{
        self,
        balancer::{self, SWAP_QUERY},
        mock,
    },
    serde_json::json,
};

#[tokio::test]
async fn sell_v2() {
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
                "tokenOut": "0xba100000625a3754423978a60c9317c58a424e3d",
            }
        }), vec!["variables.callDataInput.deadline"]),
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

    let engine = tests::SolverEngine::new("balancer", balancer::config(&api.address)).await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {
                "0xba100000625a3754423978a60c9317c58a424e3D": {
                    "decimals": 18,
                    "symbol": "BAL",
                    "referencePrice": "4327903683155778",
                    "availableBalance": "1583034704488033979459",
                    "trusted": true
                },
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                    "decimals": 18,
                    "symbol": "WETH",
                    "referencePrice": "1000000000000000000",
                    "availableBalance": "482725140468789680",
                    "trusted": false
                },
            },
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                    "buyToken": "0xba100000625a3754423978a60c9317c58a424e3D",
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

    assert_eq!(
        solution,
        json!({
            "solutions": [{
                "id": 0,
                "prices": {
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "227598784442065388110",
                    "0xba100000625a3754423978a60c9317c58a424e3d": "1000000000000000000"
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a",
                        "executedAmount": "1000000000000000000"
                    }
                ],
                "preInteractions": [],
                "postInteractions": [],
                "interactions": [
                    {
                        "kind": "custom",
                        "internalize": false,
                        "target": "0xba12222222228d8ba445958a75a0704d566bf2c8",
                        "value": "0",
                        "callData": "0x945bcec9\
                                       0000000000000000000000000000000000000000000000000000000000000000\
                                       0000000000000000000000000000000000000000000000000000000000000120\
                                       0000000000000000000000000000000000000000000000000000000000000220\
                                       0000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab41\
                                       0000000000000000000000000000000000000000000000000000000000000000\
                                       0000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab41\
                                       0000000000000000000000000000000000000000000000000000000000000000\
                                       0000000000000000000000000000000000000000000000000000000000000280\
                                       8000000000000000000000000000000000000000000000000000000000000000\
                                       0000000000000000000000000000000000000000000000000000000000000001\
                                       0000000000000000000000000000000000000000000000000000000000000020\
                                       5c6ee304399dbdb9c8ef030ab642b10820db8f56000200000000000000000014\
                                       0000000000000000000000000000000000000000000000000000000000000000\
                                       0000000000000000000000000000000000000000000000000000000000000001\
                                       0000000000000000000000000000000000000000000000000de0b6b3a7640000\
                                       00000000000000000000000000000000000000000000000000000000000000a0\
                                       0000000000000000000000000000000000000000000000000000000000000000\
                                       0000000000000000000000000000000000000000000000000000000000000002\
                                       000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2\
                                       000000000000000000000000ba100000625a3754423978a60c9317c58a424e3d\
                                       0000000000000000000000000000000000000000000000000000000000000002\
                                       0000000000000000000000000000000000000000000000000de0b6b3a7640000\
                                       fffffffffffffffffffffffffffffffffffffffffffffff3c9049e4e47ca50ec",
                        "allowances": [
                            {
                                "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                                "spender": "0xba12222222228d8ba445958a75a0704d566bf2c8",
                                "amount": "1000000000000000000",
                            },
                        ],
                        "inputs": [
                            {
                                "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                                "amount": "1000000000000000000"
                            },
                        ],
                        "outputs": [
                            {
                                "token": "0xba100000625a3754423978a60c9317c58a424e3d",
                                "amount": "227598784442065388110"
                            },
                        ],
                    }
                ],
                "gas": 195283,
            }]
        }),
    );
}

#[tokio::test]
async fn sell_v3() {
    let api = mock::http::setup(vec![mock::http::Expectation::Post {
        path: mock::http::Path::exact("sor"),
        req: mock::http::RequestBody::Partial(
            json!({
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
                    "tokenOut": "0xb0415d55f2c87b7f99285848bd341c367feac1ea",
                }
            }),
            vec!["variables.callDataInput.deadline"],
        ),
        res: json!({
            "data": {
                "sorGetSwapPaths": {
                    "tokenAddresses": [
                        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "0xb0415d55f2c87b7f99285848bd341c367feac1ea"
                    ],
                    "swaps": [
                        {
                            "poolId": "0xecd2978447367ec0c944af58c3b8a7b52acfd7a4",
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
                    "tokenOut": "0xb0415d55f2c87b7f99285848bd341c367feac1ea",
                    "protocolVersion": 3,
                    "paths": [
                        {
                            "inputAmountRaw": "1000000000000000000",
                            "isBuffer": [
                                false
                            ],
                            "outputAmountRaw": "54226514002418090226166",
                            "pools": [
                                "0xb0415d55f2c87b7f99285848bd341c367feac1ea"
                            ],
                            "protocolVersion": 3,
                            "tokens": [
                                {
                                    "address": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                                },
                                {
                                    "address": "0xb0415d55f2c87b7f99285848bd341c367feac1ea"
                                }
                            ]

                        }
                    ],
                }
            }
        }),
    }])
    .await;

    let engine = tests::SolverEngine::new("balancer", balancer::config(&api.address)).await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {
                "0xb0415d55f2c87b7f99285848bd341c367feac1ea": {
                    "decimals": 18,
                    "symbol": "BAL",
                    "referencePrice": "4327903683155778",
                    "availableBalance": "1583034704488033979459",
                    "trusted": true
                },
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                    "decimals": 18,
                    "symbol": "WETH",
                    "referencePrice": "1000000000000000000",
                    "availableBalance": "482725140468789680",
                    "trusted": false
                },
            },
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                    "buyToken": "0xb0415d55f2c87b7f99285848bd341c367feac1ea",
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

    assert_eq!(
        solution,
        json!({
            "solutions": [{
                "id": 0,
                "prices": {
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "227598784442065388110",
                    "0xb0415d55f2c87b7f99285848bd341c367feac1ea": "1000000000000000000"
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a",
                        "executedAmount": "1000000000000000000"
                    }
                ],
                "preInteractions": [],
                "postInteractions": [],
                "interactions": [
                    {
                        "kind": "custom",
                        "internalize": false,
                        "target": "0xba12222222228d8ba445958a75a0704d566bf2c8",
                        "value": "0",
                        "callData": "0x286f580d00000000000000000000000000000000000000000000000000000000000000808000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001c000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000020000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000de0b6b3a7640000000000000000000000000000000000000000000000000b7ba00e0cd093fdb1f60000000000000000000000000000000000000000000000000000000000000001000000000000000000000000b0415d55f2c87b7f99285848bd341c367feac1ea000000000000000000000000b0415d55f2c87b7f99285848bd341c367feac1ea00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
                        "allowances": [
                            {
                                "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                                "spender": "0xba12222222228d8ba445958a75a0704d566bf2c8",
                                "amount": "1000000000000000000",
                            },
                        ],
                        "inputs": [
                            {
                                "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                                "amount": "1000000000000000000"
                            },
                        ],
                        "outputs": [
                            {
                                "token": "0xb0415d55f2c87b7f99285848bd341c367feac1ea",
                                "amount": "227598784442065388110"
                            },
                        ],
                    }
                ],
                "gas": 195283,
            }]
        }),
    );
}

#[tokio::test]
async fn buy_v2() {
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
                "swapAmount": "100",
                "swapType": "EXACT_OUT",
                "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                "tokenOut": "0xba100000625a3754423978a60c9317c58a424e3d",
              }
        }), vec!["variables.callDataInput.deadline"]),
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
                            "amount": "100000000000000000000",
                            "userData": "0x",
                        }
                    ],
                    "swapAmountRaw": "100000000000000000000",
                    "returnAmountRaw": "439470293178110675",
                    "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "tokenOut": "0xba100000625a3754423978a60c9317c58a424e3d",
                    "protocolVersion": 2,
                    "paths": [],
                }
            }
        }),
    }])
    .await;

    let engine = tests::SolverEngine::new("balancer", balancer::config(&api.address)).await;
    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {
                "0xba100000625a3754423978a60c9317c58a424e3D": {
                    "decimals": 18,
                    "symbol": "BAL",
                    "referencePrice": "4327903683155778",
                    "availableBalance": "1583034704488033979459",
                    "trusted": true
                },
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                    "decimals": 18,
                    "symbol": "WETH",
                    "referencePrice": "1000000000000000000",
                    "availableBalance": "482725140468789680",
                    "trusted": true
                },
            },
            "orders": [
                {
                    "uid": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                              2a2a2a2a",
                    "sellToken": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                    "buyToken": "0xba100000625a3754423978a60c9317c58a424e3D",
                    "sellAmount": "1000000000000000000",
                    "buyAmount": "100000000000000000000",
                    "fullSellAmount": "1000000000000000000",
                    "fullBuyAmount": "100000000000000000000",
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
            "solutions": [{
                "id": 0,
                "prices": {
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "100000000000000000000",
                    "0xba100000625a3754423978a60c9317c58a424e3d": "439470293178110675"
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a",
                        "executedAmount": "100000000000000000000"
                    }
                ],
                "preInteractions": [],
                "postInteractions": [],
                "interactions": [
                    {
                        "kind": "custom",
                        "internalize": true,
                        "target": "0xba12222222228d8ba445958a75a0704d566bf2c8",
                        "value": "0",
                        "callData": "0x945bcec9\
                                       0000000000000000000000000000000000000000000000000000000000000001\
                                       0000000000000000000000000000000000000000000000000000000000000120\
                                       0000000000000000000000000000000000000000000000000000000000000220\
                                       0000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab41\
                                       0000000000000000000000000000000000000000000000000000000000000000\
                                       0000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab41\
                                       0000000000000000000000000000000000000000000000000000000000000000\
                                       0000000000000000000000000000000000000000000000000000000000000280\
                                       8000000000000000000000000000000000000000000000000000000000000000\
                                       0000000000000000000000000000000000000000000000000000000000000001\
                                       0000000000000000000000000000000000000000000000000000000000000020\
                                       5c6ee304399dbdb9c8ef030ab642b10820db8f56000200000000000000000014\
                                       0000000000000000000000000000000000000000000000000000000000000000\
                                       0000000000000000000000000000000000000000000000000000000000000001\
                                       0000000000000000000000000000000000000000000000056bc75e2d63100000\
                                       00000000000000000000000000000000000000000000000000000000000000a0\
                                       0000000000000000000000000000000000000000000000000000000000000000\
                                       0000000000000000000000000000000000000000000000000000000000000002\
                                       000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2\
                                       000000000000000000000000ba100000625a3754423978a60c9317c58a424e3d\
                                       0000000000000000000000000000000000000000000000000000000000000002\
                                       0000000000000000000000000000000000000000000000000628ecdcbd5c38c6\
                                       fffffffffffffffffffffffffffffffffffffffffffffffa9438a1d29cf00000",
                        "allowances": [
                            {
                                "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                                "spender": "0xba12222222228d8ba445958a75a0704d566bf2c8",
                                "amount": "443864996109891782",
                            },
                        ],
                        "inputs": [
                            {
                                "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                                "amount": "439470293178110675"
                            },
                        ],
                        "outputs": [
                            {
                                "token": "0xba100000625a3754423978a60c9317c58a424e3d",
                                "amount": "100000000000000000000"
                            },
                        ],
                    }
                ],
                "gas": 195283,
            }]
        }),
    );
}
