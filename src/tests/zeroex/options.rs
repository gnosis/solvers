//! This tests the 0x solver's handling of optional configuration fields.

use {
    crate::tests::{self, mock},
    serde_json::json,
};

#[tokio::test]
async fn test() {
    let api = mock::http::setup(vec![mock::http::Expectation::Get {
        path: mock::http::Path::exact(
            "swap/permit2/quote?chainId=1&buyToken=0xe41d2489571d322189246dafa5ebde1f4699f498&\
             sellToken=0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2&sellAmount=1000000000000000000&\
             taker=0x9008d19f58aabd9ed0d60971565aa8510560ab41&slippageBps=1000&\
             excludedSources=Uniswap_V2%2CBalancer_V2",
        ),
        res: json!({
            "sellAmount": "1000000000000000000",
            "buyAmount": "5876422636675954000000",
            "transaction": {
                "to": "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
                "data": "0x6af479b2\
                       0000000000000000000000000000000000000000000000000000000000000080\
                       0000000000000000000000000000000000000000000000000de0b6b3a7640000\
                       00000000000000000000000000000000000000000000013b603a9ce6a341ab60\
                       0000000000000000000000000000000000000000000000000000000000000000\
                       000000000000000000000000000000000000000000000000000000000000002b\
                       c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb8e41d2489571d322189\
                       246dafa5ebde1f4699f498000000000000000000000000000000000000000000\
                       869584cd0000000000000000000000009008d19f58aabd9ed0d60971565aa851\
                       0560ab4100000000000000000000000000000000000000000000009c6fd65477\
                       63f8730a",
                "gas": "127886",
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

    let config = tests::Config::String(format!(
        r"
node-url = 'http://localhost:8545'
relative-slippage = '0.1'
[dex]
chain-id = '1'
endpoint = 'http://{}/swap/permit2/'
api-key = 'abc123'
excluded-sources = ['Uniswap_V2', 'Balancer_V2']
        ",
        api.address
    ));
    let engine = tests::SolverEngine::new("zeroex", config).await;

    let solution = engine
        .solve(json!({
            "id": "1",
            "tokens": {
                "0xe41d2489571d322189246dafa5ebde1f4699f498": {
                    "decimals": 18,
                    "symbol": "ZRX",
                    "referencePrice": "168664736580767",
                    "availableBalance": "297403065984541243067",
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
                    "sellToken": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                    "buyToken": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                    "sellAmount": "1000000000000000000",
                    "buyAmount": "5000000000000000000000",
                    "fullSellAmount": "1000000000000000000",
                    "fullBuyAmount": "5000000000000000000000",
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
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "5876422636675954000000",
                    "0xe41d2489571d322189246dafa5ebde1f4699f498": "1000000000000000000",
                },
                "trades": [
                    {
                        "kind": "fulfillment",
                        "order": "0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\
                                    2a2a2a2a",
                        "executedAmount": "1000000000000000000",
                    }
                ],
                "preInteractions": [],
                "postInteractions": [],
                "interactions": [
                    {
                        "kind": "custom",
                        "internalize": false,
                        "target": "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
                        "value": "0",
                        "callData": "0x6af479b2\
                                       0000000000000000000000000000000000000000000000000000000000000080\
                                       0000000000000000000000000000000000000000000000000de0b6b3a7640000\
                                       00000000000000000000000000000000000000000000013b603a9ce6a341ab60\
                                       0000000000000000000000000000000000000000000000000000000000000000\
                                       000000000000000000000000000000000000000000000000000000000000002b\
                                       c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000bb8e41d2489571d322189\
                                       246dafa5ebde1f4699f498000000000000000000000000000000000000000000\
                                       869584cd0000000000000000000000009008d19f58aabd9ed0d60971565aa851\
                                       0560ab4100000000000000000000000000000000000000000000009c6fd65477\
                                       63f8730a",
                        "allowances": [
                            {
                                "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                                "spender": "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
                                "amount": "1000000000000000000",
                            },
                        ],
                        "inputs": [
                            {
                                "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                                "amount": "1000000000000000000",
                            },
                        ],
                        "outputs": [
                            {
                                "token": "0xe41d2489571d322189246dafa5ebde1f4699f498",
                                "amount": "5876422636675954000000",
                            },
                        ],
                    },
                ],
                "gas": 234277,
            }]
        }),
    );
}
