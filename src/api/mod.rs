//! Serve a solver engine API.

use {
    crate::domain::solver::Solver,
    axum::extract::DefaultBodyLimit,
    observe::tracing::distributed::axum::{make_span, record_trace_id},
    std::{future::Future, io, net::SocketAddr, sync::Arc},
    tokio::{net::TcpListener, sync::oneshot},
};

mod routes;

pub struct Api {
    pub addr: SocketAddr,
    pub solver: Solver,
}

impl Api {
    pub async fn serve(
        self,
        bind: Option<oneshot::Sender<SocketAddr>>,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), io::Error> {
        let app = axum::Router::new()
            .route("/metrics", axum::routing::get(routes::metrics))
            .route("/healthz", axum::routing::get(routes::healthz))
            .route("/solve", axum::routing::post(routes::solve))
            .layer(tower_http::trace::TraceLayer::new_for_http().make_span_with(make_span))
            .layer(axum::middleware::from_fn(|request: axum::extract::Request, next: axum::middleware::Next| async {
                next.run(record_trace_id(request)).await
            }))
            .layer(DefaultBodyLimit::disable())
            .with_state(Arc::new(self.solver));

        let listener = TcpListener::bind(self.addr).await?;
        if let Some(bind) = bind {
            let _ = bind.send(listener.local_addr()?);
        }

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await
    }
}
