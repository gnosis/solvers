//! CLI arguments for the `solvers` binary.

use {
    clap::{Parser, Subcommand},
    std::{net::SocketAddr, path::PathBuf},
};

/// Run a solver engine
#[derive(Parser, Debug)]
#[command(version)]
pub struct Args {
    /// The log filter.
    #[arg(
        long,
        env,
        default_value = "warn,solvers=debug,shared=debug,model=debug,solver=debug"
    )]
    pub log: String,

    /// Whether to use JSON format for the logs.
    #[clap(long, env, default_value = "false")]
    pub use_json_logs: bool,

    /// The socket address to bind to.
    #[arg(long, env, default_value = "127.0.0.1:7872")]
    pub addr: SocketAddr,

    #[command(subcommand)]
    pub command: Command,
}

/// The solver engine to run. The config field is a path to the solver
/// configuration file. This file should be in TOML format.
#[derive(Subcommand, Debug)]
#[clap(rename_all = "lowercase")]
pub enum Command {
    /// solve individual orders using Balancer API
    Balancer {
        #[clap(long, env)]
        config: PathBuf,
    },
    /// solve individual orders using 0x API
    ZeroEx {
        #[clap(long, env)]
        config: PathBuf,
    },
    /// solve individual orders using 1Inch API
    OneInch {
        #[clap(long, env)]
        config: PathBuf,
    },
    /// solve individual orders using Paraswap API
    ParaSwap {
        #[clap(long, env)]
        config: PathBuf,
    },
    /// solve individual orders using OKX API
    Okx {
        #[clap(long, env)]
        config: PathBuf,
    },
}
