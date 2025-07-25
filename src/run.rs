#[cfg(unix)]
use tokio::signal::unix::{self, SignalKind};
use {
    crate::{
        domain::solver::{self, Solver},
        infra::{cli, config, dex},
    },
    clap::Parser,
    std::net::SocketAddr,
    tokio::sync::oneshot,
};

pub async fn start(args: impl IntoIterator<Item = String>) {
    observe::panic_hook::install();
    let args = cli::Args::parse_from(args);
    run_with(args, None).await;
}

pub async fn run(
    args: impl IntoIterator<Item = String>,
    bind: Option<oneshot::Sender<SocketAddr>>,
) {
    let args = cli::Args::parse_from(args);
    run_with(args, bind).await;
}

async fn run_with(args: cli::Args, bind: Option<oneshot::Sender<SocketAddr>>) {
    let obs_config = observe::Config::new(
        &args.log,
        tracing::Level::ERROR.into(),
        args.use_json_logs,
        None,
    );
    observe::tracing::initialize_reentrant(&obs_config);
    tracing::info!("running solver engine with {args:#?}");

    let solver = match args.command {
        cli::Command::ZeroEx { config } => {
            let config = config::dex::zeroex::file::load(&config).await;
            Solver::Dex(solver::Dex::new(
                dex::Dex::ZeroEx(
                    dex::zeroex::ZeroEx::new(config.zeroex).expect("invalid 0x configuration"),
                ),
                config.base.clone(),
            ))
        }
        cli::Command::Balancer { config } => {
            let config = config::dex::balancer::file::load(&config).await;
            Solver::Dex(solver::Dex::new(
                dex::Dex::Balancer(Box::new(
                    dex::balancer::Sor::new(config.sor).expect("invalid Balancer configuration"),
                )),
                config.base.clone(),
            ))
        }
        cli::Command::OneInch { config } => {
            let config = config::dex::oneinch::file::load(&config).await;
            Solver::Dex(solver::Dex::new(
                dex::Dex::OneInch(dex::oneinch::OneInch::new(config.oneinch).await),
                config.base.clone(),
            ))
        }
        cli::Command::ParaSwap { config } => {
            let config = config::dex::paraswap::file::load(&config).await;
            Solver::Dex(solver::Dex::new(
                dex::Dex::ParaSwap(dex::paraswap::ParaSwap::new(config.paraswap)),
                config.base.clone(),
            ))
        }
        cli::Command::Okx { config } => {
            let config = config::dex::okx::file::load(&config).await;
            Solver::Dex(solver::Dex::new(
                dex::Dex::Okx(
                    dex::okx::Okx::try_new(config.okx).expect("invalid OKX configuration"),
                ),
                config.base.clone(),
            ))
        }
    };

    crate::api::Api {
        addr: args.addr,
        solver,
    }
    .serve(bind, shutdown_signal())
    .await
    .unwrap();
}

#[cfg(unix)]
async fn shutdown_signal() {
    // Intercept main signals for graceful shutdown.
    // Kubernetes sends sigterm, whereas locally sigint (ctrl-c) is most common.
    let mut interrupt = unix::signal(SignalKind::interrupt()).unwrap();
    let mut terminate = unix::signal(SignalKind::terminate()).unwrap();
    tokio::select! {
        _ = interrupt.recv() => (),
        _ = terminate.recv() => (),
    };
}

#[cfg(windows)]
async fn shutdown_signal() {
    // We don't support signal handling on Windows.
    std::future::pending().await
}
