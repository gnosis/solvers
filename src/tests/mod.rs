//! Solver engine end-to-end tests.
//!
//! Note that this is setup as a "unit test" in that it is part of the `src/`
//! directory. This is done intentionally as Cargo builds separate binaries for
//! each file in `tests/`, which makes `cargo test` slower.

use {
    anyhow::Context,
    reqwest::Url,
    std::io::Write,
    tokio::{sync::oneshot, task::JoinHandle},
};

mod balancer;
mod dex;
mod mock;
mod okx;
mod oneinch;
mod paraswap;
mod zeroex;

/// A solver engine handle for E2E testing.
pub struct SolverEngine {
    url: Url,
    #[allow(dead_code)] // only needed for Drop handling
    tempfile: Option<tempfile::TempPath>,
    handle: JoinHandle<()>,
}

/// Solver configuration.
pub enum Config {
    #[allow(dead_code)]
    None,
    String(String),
}

impl SolverEngine {
    /// Creates a new solver engine handle for the specified command
    /// configuration.
    pub async fn new(command: &str, config: Config) -> Self {
        let (bind, bind_receiver) = oneshot::channel();

        let mut args = vec![
            "/test/solvers/path".to_owned(),
            "--addr=0.0.0.0:0".to_owned(),
            "--log=solvers=trace".to_owned(),
            command.to_owned(),
        ];
        let tempfile = match config {
            Config::None => None,
            Config::String(config) => {
                let mut file = tempfile::NamedTempFile::new().unwrap();
                file.write_all(config.as_bytes()).unwrap();
                let path = file.into_temp_path();
                args.push(format!("--config={}", path.display()));
                Some(path)
            }
        };

        let handle = tokio::spawn(crate::run(args, Some(bind)));

        let addr = bind_receiver.await.unwrap();
        let url = format!("http://{addr}/").parse().unwrap();

        Self {
            url,
            tempfile,
            handle,
        }
    }

    /// Solves a raw JSON auction.
    pub async fn solve(&self, auction: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let client = reqwest::Client::new();
        let url = shared::url::join(&self.url, "solve");
        let response = client.post(url).json(&auction).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            anyhow::bail!("HTTP {}: {:?}", status, text);
        }

        response
            .json()
            .await
            .context("Failed to parse JSON response")
    }
}

impl Drop for SolverEngine {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
