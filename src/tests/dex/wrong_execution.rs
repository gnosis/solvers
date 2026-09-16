use {
    crate::tests::{
        self,
        balancer::{self, SWAP_QUERY},
        mock,
    },
    serde_json::json,
};

struct Case {
    input_amount: &'static str,
    output_amount: &'static str,
    side: &'static str,
    /// Whether the swap satisfies the order's limit price and therefore gets
    /// simulated before being rejected.
    simulated: bool,
}

/// Test that verifies that attempting to settle an order when the DEX swap
/// whose amounts do not match the input order fails to produce a solution, even
/// if it can satisfy the order's limit price.
#[tokio::test]
async fn test() {
    for Case {
        input_amount,
        output_amount,
        side,
        simulated,
    } in [
        Case {
            input_amount: "1000000000000000001",
            output_amount: "227598784442065388110",
            side: "sell",
            simulated: false,
        },
        Case {
            input_amount: "999999999999999999",
            output_amount: "227598784442065388110",
            side: "sell",
            simulated: true,
        },
        Case {
            input_amount: "1000000000000000000",
            output_amount: "227598784442065388111",
            side: "buy",
            simulated: true,
        },
        Case {
            input_amount: "1000000000000000000",
            output_amount: "227598784442065388109",
            side: "buy",
            simulated: false,
        },
    ] {
        let api = mock::http::setup(vec![mock::http::Expectation::Post {
            path: mock::http::Path::Any,
            req: mock::http::RequestBody::Partial(
                json!({
                    "query": serde_json::to_value(SWAP_QUERY).unwrap(),
                    "variables": {
                        "chain": "MAINNET",
                        "swapAmount": if side == "sell" {
                            "1"
                        } else {
                            "227.59878444206538811"
                        },
                        "swapType": if side == "sell" {
                            "EXACT_IN"
                        } else {
                            "EXACT_OUT"
                        },
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
                                "amount": if side == "sell" { input_amount } else { output_amount },
                                "userData": "0x",
                            }
                        ],
                        // For buy orders the SOR reports the exact buy amount as the
                        // swap amount and the required sell amount as the return amount.
                        "swapAmountRaw": if side == "sell" { input_amount } else { output_amount },
                        "returnAmountRaw": if side == "sell" { output_amount } else { input_amount },
                        "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                        "tokenOut": "0xba100000625a3754423978a60c9317c58a424e3d",
                        "protocolVersion": 2,
                        "paths": [],
                    }
                }
            }),
        }])
        .await;

        // Balancer's on-chain amount query fails, so the SOR amounts are used.
        let mut node_calls = vec![mock::node::failing_call()];
        if simulated {
            node_calls.push(mock::node::gas_simulation(88_892));
        }
        let node = mock::http::setup(node_calls).await;

        let engine =
            tests::SolverEngine::new("balancer", balancer::config(&api.address, &node.address))
                .await;

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
                        "kind": side,
                        "partiallyFillable": false,
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
}
