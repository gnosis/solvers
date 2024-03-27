use crate::{
    domain::eth,
    tests::mock::http::{setup, Expectation, Path, RequestBody, ServerHandle},
};

/// Returns a node that will always return the given number as a U256
/// which will internally be used as a gas estimate for the proposed
/// solution.
pub async fn constant_gas_estimate(gas: u64) -> ServerHandle {
    setup(vec![Expectation::Post {
        path: Path::Any,
        req: RequestBody::Any,
        res: serde_json::json!({
            "result": format!("{:#066X}", eth::U256::from(gas)),
            "id": 0,
        }),
    }])
    .await
}
