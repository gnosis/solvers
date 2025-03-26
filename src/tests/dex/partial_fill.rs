use {
    crate::tests::{
        self,
        balancer::{self, SWAP_QUERY},
        mock,
    },
    serde_json::json,
};

/// Tests that dex solvers consecutively decrease the amounts they try to fill
/// partially fillable orders with across `/solve` requests to eventually find a
/// fillable amount that works.
/// If a fillable amount was found the solver tries to solve a bigger amount
/// next time in case some juicy liquidity appeared on chain which makes big
/// fills possible.
#[tokio::test]
async fn tested_amounts_adjust_depending_on_response() {
    // observe::tracing::initialize_reentrant("solvers=trace");
    let inner_request = |ether_amount| {
        mock::http::RequestBody::Partial(
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
                    "swapAmount": ether_amount,
                    "swapType": "EXACT_IN",
                    "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "tokenOut": "0xba100000625a3754423978a60c9317c58a424e3d",
                }
            }),
            vec!["variables.callDataInput.deadline"],
        )
    };

    let no_swap_found_response = json!({
        "data": {
            "sorGetSwapPaths": {
                "tokenAddresses": [],
                "swaps": [],
                "swapAmountRaw": "0",
                "returnAmountRaw": "0",
                "tokenIn": "0x0000000000000000000000000000000000000000",
                "tokenOut": "0x0000000000000000000000000000000000000000",
            }
        }
    });

    let limit_price_violation_response = |in_wei_amount| {
        json!({
            "data": {
                "sorGetSwapPaths": {
                    "tokenAddresses": [
                        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "0xba100000625a3754423978a60c9317c58a424e3d"
                    ],
                    "swaps": [
                        {
                            "poolId": "0x5c6ee304399dbdb9c8ef030ab642b10820\
                                db8f56000200000000000000000014",
                            "assetInIndex": 0,
                            "assetOutIndex": 1,
                            "amount": in_wei_amount,
                            "userData": "0x",
                        }
                    ],
                    "swapAmountRaw": in_wei_amount,
                    "returnAmountRaw": "1",
                    "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "tokenOut": "0xba100000625a3754423978a60c9317c58a424e3d",
                }
            }
        })
    };

    let api = mock::http::setup(vec![
        mock::http::Expectation::Post {
            path: mock::http::Path::Any,
            req: inner_request("16"),
            res: no_swap_found_response.clone(),
        },
        mock::http::Expectation::Post {
            path: mock::http::Path::Any,
            req: inner_request("8"),
            res: no_swap_found_response.clone(),
        },
        mock::http::Expectation::Post {
            path: mock::http::Path::Any,
            req: inner_request("4"),
            res: limit_price_violation_response("4000000000000000000").clone(),
        },
        mock::http::Expectation::Post {
            path: mock::http::Path::Any,
            req: inner_request("2"),
            res: limit_price_violation_response("2000000000000000000").clone(),
        },
        mock::http::Expectation::Post {
            path: mock::http::Path::Any,
            req: inner_request("1"),
            res: json!({
                "data": {
                    "sorGetSwapPaths": {
                        "tokenAddresses": [
                            "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                            "0xba100000625a3754423978a60c9317c58a424e3d"
                        ],
                        "swaps": [
                            {
                                "poolId": "0x5c6ee304399dbdb9c8ef030ab642b10820\
                                    db8f56000200000000000000000014",
                                "assetInIndex": 0,
                                "assetOutIndex": 1,
                                "amount": "1000000000000000000",
                                "userData": "0x",
                            }
                        ],
                        "swapAmountRaw": "1000000000000000000",
                        "returnAmountRaw": "227598784442065388110",
                        "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "tokenOut": "0xba100000625a3754423978a60c9317c58a424e3d",
                    }
                }
            }),
        },
        // After a successful response we try the next time with a bigger amount.
        mock::http::Expectation::Post {
            path: mock::http::Path::Any,
            req: inner_request("2"),
            res: no_swap_found_response.clone(),
        },
    ])
    .await;

    let simulation_node = mock::http::setup(vec![mock::http::Expectation::Post {
        path: mock::http::Path::Any,
        req: mock::http::RequestBody::Any,
        res: {
            json!({
                "id": 1,
                "jsonrpc": "2.0",
                "result": "0x0000000000000000000000000000000000000000000000000000000000015B3C"
            })
        },
    }])
    .await;

    let config = tests::Config::String(format!(
        r"
node-url = 'http://{}'
[dex]
endpoint = 'http://{}/sor'
chain-id = '1'
        ",
        simulation_node.address, api.address,
    ));

    let engine = tests::SolverEngine::new("balancer", config).await;

    let auction = json!({
        "id": "1",
        "tokens": {
            "0xba100000625a3754423978a60c9317c58a424e3D": {
                "decimals": 18,
                "symbol": "BAL",
                "referencePrice": "4327903683155778",
                "availableBalance": "0",
                "trusted": true
            },
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                "decimals": 18,
                "symbol": "WETH",
                "referencePrice": "1000000000000000000",
                "availableBalance": "0",
                "trusted": true
            },
            "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": {
                "decimals": 18,
                "symbol": "ETH",
                "referencePrice": "1000000000000000000",
                "availableBalance": "0",
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
                "sellAmount": "16000000000000000000",
                "buyAmount": "3630944624685908136768",
                "fullSellAmount": "16000000000000000000",
                "fullBuyAmount": "3630944624685908136768",
                "kind": "sell",
                "partiallyFillable": true,
                "class": "limit",
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
    });

    let empty_solution = json!({
        "solutions": [],
    });

    for _ in 0..4 {
        let solution = engine.solve(auction.clone()).await.unwrap();
        assert_eq!(solution, empty_solution);
    }

    let solution = engine.solve(auction.clone()).await.unwrap();

    // Solver finally found a solution after 5 tries.
    assert_eq!(
        solution,
        json!({
            "solutions": [{
                "id": 0,
                "preInteractions": [],
                "postInteractions": [],
                "interactions": [
                    {
                        "allowances": [
                            {
                                "amount": "1000000000000000000",
                                "spender": "0xba12222222228d8ba445958a75a0704d566bf2c8",
                                "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                            }
                        ],
                        "callData": "0x945bcec90000000000000000000000000000000000000000000\
                            00000000000000000000000000000000000000000000000000000000000000\
                            00000000000000000000120000000000000000000000000000000000000000\
                            00000000000000000000002200000000000000000000000009008d19f58aab\
                            d9ed0d60971565aa8510560ab4100000000000000000000000000000000000\
                            000000000000000000000000000000000000000000000000000009008d19f5\
                            8aabd9ed0d60971565aa8510560ab410000000000000000000000000000000\
                            00000000000000000000000000000000000000000000000000000000000000\
                            00000000000000000000000000000000280800000000000000000000000000\
                            00000000000000000000000000000000000000000000000000000000000000\
                            00000000000000000000000000000000000000100000000000000000000000\
                            000000000000000000000000000000000000000205c6ee304399dbdb9c8ef0\
                            30ab642b10820db8f560002000000000000000000140000000000000000000\
                            00000000000000000000000000000000000000000000000000000000000000\
                            00000000000000000000000000000000000000000000001000000000000000\
                            0000000000000000000000000000000000de0b6b3a76400000000000000000\
                            0000000000000000000000000000000000000000000000000a000000000000\
                            00000000000000000000000000000000000000000000000000000000000000\
                            00000000000000000000000000000000000000000000000000000020000000\
                            00000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000\
                            0000000000000000000ba100000625a3754423978a60c9317c58a424e3d000\
                            00000000000000000000000000000000000000000000000000000000000020\
                            000000000000000000000000000000000000000000000000de0b6b3a764000\
                            0fffffffffffffffffffffffffffffffffffffffffffffff3c9049e4e47ca5\
                            0ec",
                        "inputs": [
                            {
                                "amount": "1000000000000000000",
                                "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                            }
                        ],
                        "internalize": false,
                        "kind": "custom",
                        "outputs": [
                            {
                                "amount": "227598784442065388110",
                                "token": "0xba100000625a3754423978a60c9317c58a424e3d"
                            }
                        ],
                        "target": "0xba12222222228d8ba445958a75a0704d566bf2c8",
                        "value": "0"
                    }
                ],
                "prices": {
                    "0xba100000625a3754423978a60c9317c58a424e3d": "1000000000000000000",
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "227598784442065388110"
                },
                "trades": [
                    {
                        "executedAmount": "1000000000000000000",
                        "fee": "2929245000000000",
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a"
                    }
                ],
                "gas": 195283,
            }]
        })
    );

    // Solver tried a bigger fill after the last success but that failed again.
    let solution = engine.solve(auction.clone()).await.unwrap();
    assert_eq!(solution, empty_solution);
}

/// Tests that we don't converge to 0 with the amounts we try to fill. Instead
/// we start over when our tried amount would be worth less than 0.01 ETH.
#[tokio::test]
async fn tested_amounts_wrap_around() {
    // Test is set up such that 2.5 BAL or exactly 0.01 ETH.
    // And the lowest amount we are willing to fill is 0.01 ETH.
    let fill_attempts = [
        ("16", "16000000000000000000"), // 16 BAL == 0.064 ETH
        ("8", "8000000000000000000"),   // 8  BAL == 0.032 ETH
        ("4", "4000000000000000000"),   // 4  BAL == 0.016 ETH
        ("16", "16000000000000000000"), // 16 BAL == 0.064 ETH
    ]
    .into_iter()
    .map(|(amount_in, amount_in_wei)| mock::http::Expectation::Post {
        path: mock::http::Path::Any,
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
                    "swapAmount": amount_in,
                    "swapType": "EXACT_OUT",
                    "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "tokenOut": "0xba100000625a3754423978a60c9317c58a424e3d",
                }
            }),
            vec!["variables.callDataInput.deadline"],
        ),
        res: json!({
            "data": {
                "sorGetSwapPaths": {
                    "tokenAddresses": [
                        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "0xba100000625a3754423978a60c9317c58a424e3d"
                    ],
                    "swaps": [
                        {
                            "poolId": "0x5c6ee304399dbdb9c8ef030ab642b10820\
                                db8f56000200000000000000000014",
                            "assetInIndex": 0,
                            "assetOutIndex": 1,
                            "amount": amount_in_wei,
                            "userData": "0x",
                        }
                    ],
                    "swapAmountRaw": amount_in_wei,
                    // Does not satisfy limit price of any chunk...
                    "returnAmountRaw": "700000000000000000",
                    "returnAmountConsideringFees": "1",
                    "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "tokenOut": "0xba100000625a3754423978a60c9317c58a424e3d",
                }
            }
        }),
    })
    .collect();

    let api = mock::http::setup(fill_attempts).await;

    let engine = tests::SolverEngine::new("balancer", balancer::config(&api.address)).await;

    let auction = json!({
        "id": "1",
        "tokens": {
            "0xba100000625a3754423978a60c9317c58a424e3D": {
                "decimals": 18,
                "symbol": "BAL",
                "referencePrice": "4000000000000000",
                "availableBalance": "0",
                "trusted": true
            },
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                "decimals": 18,
                "symbol": "WETH",
                "referencePrice": "1000000000000000000",
                "availableBalance": "0",
                "trusted": true
            },
            "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": {
                "decimals": 18,
                "symbol": "ETH",
                "referencePrice": "1000000000000000000",
                "availableBalance": "0",
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
                "sellAmount": "60000000000000000",
                "buyAmount": "16000000000000000000",
                "fullSellAmount": "60000000000000000",
                "fullBuyAmount": "16000000000000000000",
                "kind": "buy",
                "partiallyFillable": true,
                "class": "limit",
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
    });

    for _ in 0..4 {
        let solution = engine.solve(auction.clone()).await.unwrap();
        assert_eq!(
            solution,
            json!({
                "solutions": []
            }),
        );
    }
}

/// Test that matches a partially fillable in such a way that there isn't enough
/// sell amount left to extract the user's surplus fee. The expectation here is
/// that we shift part of the fee into the buy token (i.e. transfer out less
/// than we receive from the swap).
#[tokio::test]
async fn moves_surplus_fee_to_buy_token() {
    // observe::tracing::initialize_reentrant("solvers=trace");
    let api = mock::http::setup(vec![
        mock::http::Expectation::Post {
            path: mock::http::Path::Any,
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
                        "swapAmount": "2",
                        "swapType": "EXACT_IN",
                        "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "tokenOut": "0xba100000625a3754423978a60c9317c58a424e3d",
                    }
                }),
                vec!["variables.callDataInput.deadline"],
            ),
            res: json!({
                "data": {
                    "sorGetSwapPaths": {
                        "tokenAddresses": [],
                        "swaps": [],
                        "swapAmountRaw": "0",
                        "returnAmountRaw": "0",
                        "tokenIn": "0x0000000000000000000000000000000000000000",
                        "tokenOut": "0x0000000000000000000000000000000000000000",
                    }
                }
            }),
        },
        mock::http::Expectation::Post {
            path: mock::http::Path::Any,
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
                        "tokenOut": "0xba100000625a3754423978a60c9317c58a424e3d",
                    }
                }),
                vec!["variables.callDataInput.deadline"],
            ),
            res: json!({
                "data": {
                    "sorGetSwapPaths": {
                        "tokenAddresses": [
                            "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                            "0xba100000625a3754423978a60c9317c58a424e3d"
                        ],
                        "swaps": [
                            {
                                "poolId": "0x5c6ee304399dbdb9c8ef030ab642b10820\
                                    db8f56000200000000000000000014",
                                "assetInIndex": 0,
                                "assetOutIndex": 1,
                                "amount": "1000000000000000000",
                                "userData": "0x",
                            }
                        ],
                        "swapAmountRaw": "1000000000000000000",
                        "returnAmountRaw": "227598784442065388110",
                        "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "tokenOut": "0xba100000625a3754423978a60c9317c58a424e3d",
                    }
                }
            }),
        },
    ])
    .await;

    let simulation_node = mock::http::setup(vec![mock::http::Expectation::Post {
        path: mock::http::Path::Any,
        req: mock::http::RequestBody::Any,
        res: {
            json!({
                "id": 1,
                "jsonrpc": "2.0",
                // If the simulation logic returns 0 it means that the user did not have the
                // required balance. This could be caused by a pre-interaction that acquires the
                // necessary sell_token before the trade which is currently not supported by the
                // simulation loic.
                // In that case we fall back to the heuristic gas price we had in the past.
                "result": "0x0000000000000000000000000000000000000000000000000000000000000000"
            })
        },
    }])
    .await;

    let config = tests::Config::String(format!(
        r"
node-url = 'http://{}'
[dex]
endpoint = 'http://{}/sor'
chain-id = '1'
        ",
        simulation_node.address, api.address,
    ));

    let engine = tests::SolverEngine::new("balancer", config).await;

    let auction = json!({
        "id": "1",
        "tokens": {
            "0xba100000625a3754423978a60c9317c58a424e3D": {
                "decimals": 18,
                "symbol": "BAL",
                "referencePrice": "4000000000000000",
                "availableBalance": "0",
                "trusted": true
            },
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                "decimals": 18,
                "symbol": "WETH",
                "referencePrice": "1000000000000000000",
                "availableBalance": "0",
                "trusted": true
            },
            "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": {
                "decimals": 18,
                "symbol": "ETH",
                "referencePrice": "1000000000000000000",
                "availableBalance": "0",
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
                "sellAmount": "2000000000000000000",
                "buyAmount": "1",
                "fullSellAmount": "2000000000000000000",
                "fullBuyAmount": "1",
                "kind": "sell",
                "partiallyFillable": true,
                "class": "limit",
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
        "effectiveGasPrice": "6000000000000",
        "deadline": "2106-01-01T00:00:00.000Z",
        "surplusCapturingJitOrderOwners": []
    });

    // The first try doesn't match.
    let solution = engine.solve(auction.clone()).await.unwrap();
    assert_eq!(
        solution,
        json!({
            "solutions": []
        })
    );

    let solution = engine.solve(auction.clone()).await.unwrap();
    assert_eq!(
        solution,
        json!({
            "solutions": [{
                "id": 0,
                "preInteractions": [],
                "postInteractions": [],
                "interactions": [
                    {
                        "allowances": [
                            {
                                "amount": "1000000000000000000",
                                "spender": "0xba12222222228d8ba445958a75a0704d566bf2c8",
                                "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                            }
                        ],
                        "callData": "0x945bcec90000000000000000000000000000000000000000000\
                            00000000000000000000000000000000000000000000000000000000000000\
                            00000000000000000000120000000000000000000000000000000000000000\
                            00000000000000000000002200000000000000000000000009008d19f58aab\
                            d9ed0d60971565aa8510560ab4100000000000000000000000000000000000\
                            000000000000000000000000000000000000000000000000000009008d19f5\
                            8aabd9ed0d60971565aa8510560ab410000000000000000000000000000000\
                            00000000000000000000000000000000000000000000000000000000000000\
                            00000000000000000000000000000000280800000000000000000000000000\
                            00000000000000000000000000000000000000000000000000000000000000\
                            00000000000000000000000000000000000000100000000000000000000000\
                            000000000000000000000000000000000000000205c6ee304399dbdb9c8ef0\
                            30ab642b10820db8f560002000000000000000000140000000000000000000\
                            00000000000000000000000000000000000000000000000000000000000000\
                            00000000000000000000000000000000000000000000001000000000000000\
                            0000000000000000000000000000000000de0b6b3a76400000000000000000\
                            0000000000000000000000000000000000000000000000000a000000000000\
                            00000000000000000000000000000000000000000000000000000000000000\
                            00000000000000000000000000000000000000000000000000000020000000\
                            00000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000\
                            0000000000000000000ba100000625a3754423978a60c9317c58a424e3d000\
                            00000000000000000000000000000000000000000000000000000000000020\
                            000000000000000000000000000000000000000000000000de0b6b3a764000\
                            0fffffffffffffffffffffffffffffffffffffffffffffff3c9049e4e47ca5\
                            0ec",
                        "inputs": [
                            {
                                "amount": "1000000000000000000",
                                "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                            }
                        ],
                        "internalize": false,
                        "kind": "custom",
                        "outputs": [
                            {
                                "amount": "227598784442065388110",
                                "token": "0xba100000625a3754423978a60c9317c58a424e3d"
                            }
                        ],
                        "target": "0xba12222222228d8ba445958a75a0704d566bf2c8",
                        "value": "0"
                    }
                ],
                "prices": {
                    "0xba100000625a3754423978a60c9317c58a424e3d": "828302000000000000",
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "188520528350931645103"
                },
                "trades": [
                    {
                        "executedAmount": "828302000000000000",
                        "fee": "1171698000000000000",
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a"
                    }
                ],
                "gas": 195283,
            }]
        })
    );
}

/// Test that verifies that no solution is proposed when a partially fillable
/// order is matched, but that there is insufficient surplus to charge the fee.
#[tokio::test]
async fn insufficient_room_for_surplus_fee() {
    let api = mock::http::setup(vec![mock::http::Expectation::Post {
        path: mock::http::Path::Any,
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
                    "tokenOut": "0xba100000625a3754423978a60c9317c58a424e3d",
                }
            }),
            vec!["variables.callDataInput.deadline"],
        ),
        res: json!({
            "data": {
                "sorGetSwapPaths": {
                    "tokenAddresses": [
                        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "0xba100000625a3754423978a60c9317c58a424e3d"
                    ],
                    "swaps": [
                        {
                            "poolId": "0x5c6ee304399dbdb9c8ef030ab642b10820\
                                db8f56000200000000000000000014",
                            "assetInIndex": 0,
                            "assetOutIndex": 1,
                            "amount": "1000000000000000000",
                            "userData": "0x",
                        }
                    ],
                    "swapAmountRaw": "1000000000000000000",
                    "returnAmountRaw": "227598784442065388110",
                    "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "tokenOut": "0xba100000625a3754423978a60c9317c58a424e3d",
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
                    "availableBalance": "0",
                    "trusted": true
                },
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                    "decimals": 18,
                    "symbol": "WETH",
                    "referencePrice": "1000000000000000000",
                    "availableBalance": "0",
                    "trusted": true
                },
                "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": {
                    "decimals": 18,
                    "symbol": "ETH",
                    "referencePrice": "1000000000000000000",
                    "availableBalance": "0",
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
                    "buyAmount": "227598784442065388110",
                    "fullSellAmount": "1000000000000000000",
                    "fullBuyAmount": "227598784442065388110",
                    "kind": "sell",
                    "partiallyFillable": true,
                    "class": "limit",
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

/// Test that documents how we deal with partially fillable market orders. In
/// particular, we assume that there is no solver fee to compute and that the
/// pre-agreed upon "feeAmount" is sufficient. In practice, this isn't expected
/// to happen, and this test is mostly included to document expected behaviour
/// in the case of these orders.
#[tokio::test]
async fn market() {
    let api = mock::http::setup(vec![mock::http::Expectation::Post {
        path: mock::http::Path::Any,
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
                    "tokenOut": "0xba100000625a3754423978a60c9317c58a424e3d",
                }
            }),
            vec!["variables.callDataInput.deadline"],
        ),
        res: json!({
            "data": {
                "sorGetSwapPaths": {
                    "tokenAddresses": [
                        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "0xba100000625a3754423978a60c9317c58a424e3d"
                    ],
                    "swaps": [
                        {
                            "poolId": "0x5c6ee304399dbdb9c8ef030ab642b10820\
                                db8f56000200000000000000000014",
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
                    "availableBalance": "0",
                    "trusted": true
                },
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2": {
                    "decimals": 18,
                    "symbol": "WETH",
                    "referencePrice": "1000000000000000000",
                    "availableBalance": "0",
                    "trusted": true
                },
                "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": {
                    "decimals": 18,
                    "symbol": "ETH",
                    "referencePrice": "1000000000000000000",
                    "availableBalance": "0",
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
                    "buyAmount": "227598784442065388110",
                    "fullSellAmount": "1000000000000000000",
                    "fullBuyAmount": "227598784442065388110",
                    "kind": "sell",
                    "partiallyFillable": true,
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
                "preInteractions": [],
                "postInteractions": [],
                "interactions": [
                    {
                        "allowances": [
                            {
                                "amount": "1000000000000000000",
                                "spender": "0xba12222222228d8ba445958a75a0704d566bf2c8",
                                "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                            }
                        ],
                        "callData": "0x945bcec90000000000000000000000000000000000000000000\
                            00000000000000000000000000000000000000000000000000000000000000\
                            00000000000000000000120000000000000000000000000000000000000000\
                            00000000000000000000002200000000000000000000000009008d19f58aab\
                            d9ed0d60971565aa8510560ab4100000000000000000000000000000000000\
                            000000000000000000000000000000000000000000000000000009008d19f5\
                            8aabd9ed0d60971565aa8510560ab410000000000000000000000000000000\
                            00000000000000000000000000000000000000000000000000000000000000\
                            00000000000000000000000000000000280800000000000000000000000000\
                            00000000000000000000000000000000000000000000000000000000000000\
                            00000000000000000000000000000000000000100000000000000000000000\
                            000000000000000000000000000000000000000205c6ee304399dbdb9c8ef0\
                            30ab642b10820db8f560002000000000000000000140000000000000000000\
                            00000000000000000000000000000000000000000000000000000000000000\
                            00000000000000000000000000000000000000000000001000000000000000\
                            0000000000000000000000000000000000de0b6b3a76400000000000000000\
                            0000000000000000000000000000000000000000000000000a000000000000\
                            00000000000000000000000000000000000000000000000000000000000000\
                            00000000000000000000000000000000000000000000000000000020000000\
                            00000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc200000\
                            0000000000000000000ba100000625a3754423978a60c9317c58a424e3d000\
                            00000000000000000000000000000000000000000000000000000000000020\
                            000000000000000000000000000000000000000000000000de0b6b3a764000\
                            0fffffffffffffffffffffffffffffffffffffffffffffff3c9049e4e47ca5\
                            0ec",
                        "inputs": [
                            {
                                "amount": "1000000000000000000",
                                "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                            }
                        ],
                        "internalize": false,
                        "kind": "custom",
                        "outputs": [
                            {
                                "amount": "227598784442065388110",
                                "token": "0xba100000625a3754423978a60c9317c58a424e3d"
                            }
                        ],
                        "target": "0xba12222222228d8ba445958a75a0704d566bf2c8",
                        "value": "0"
                    }
                ],
                "prices": {
                    "0xba100000625a3754423978a60c9317c58a424e3d": "1000000000000000000",
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "227598784442065388110"
                },
                "trades": [
                    {
                        "executedAmount": "1000000000000000000",
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a"
                    }
                ],
                "gas": 195283,
            }]
        })
    );
}
