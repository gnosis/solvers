use {
    crate::{
        infra::dex::balancer::dto,
        tests::{self, balancer, mock},
    },
    serde_json::json,
};

struct Case {
    input_amount: &'static str,
    output_amount: &'static str,
    side: &'static str,
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
    } in [
        Case {
            input_amount: "1000000000000000001",
            output_amount: "227598784442065388110",
            side: "sell",
        },
        Case {
            input_amount: "999999999999999999",
            output_amount: "227598784442065388110",
            side: "sell",
        },
        Case {
            input_amount: "1000000000000000000",
            output_amount: "227598784442065388110",
            side: "buy",
        },
        Case {
            input_amount: "1000000000000000000",
            output_amount: "227598784442065388110",
            side: "buy",
        },
    ] {
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
                    "useVaultVersion": 2
                }
            })),
            res: json!({
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
                        "amount": input_amount,
                        "userData": "0x",
                    }
                ],
                "swapAmountRaw": input_amount,
                "returnAmountRaw": output_amount,
                "tokenIn": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                "tokenOut": "0xba100000625a3754423978a60c9317c58a424e3d",
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
                        "kind": side,
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
}
