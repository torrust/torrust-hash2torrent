pub mod cache;
pub mod handler;
pub mod slowloris;

use axum::error_handling::HandleErrorLayer;

use axum::routing::get;
use axum::{BoxError, Router};
use axum_server::Server;

use handler::{entrypoint_handler, get_metainfo_file_handler, health_check_handler};
use hyper::StatusCode;
use hyper_util::rt::TokioTimer;
use std::net::{SocketAddr, TcpListener};

use std::sync::Arc;
use std::time::Duration;

use tower::{timeout::TimeoutLayer, ServiceBuilder};
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::api::slowloris::TimeoutAcceptor;
use crate::AppState;

const TIMEOUT: Duration = Duration::from_secs(10);

/// It starts the web server.
///
/// # Panics
///
/// Will panic if it can get the local server address
pub async fn start(bind_to: &SocketAddr, state: AppState) {
    let socket =
        std::net::TcpListener::bind(bind_to).expect("Could not bind tcp_listener to address.");

    let server_address = socket
        .local_addr()
        .expect("Could not get local_addr from tcp_listener.");

    info!("API bound to address: http://{server_address}"); // DevSkim: ignore DS137138

    let server = from_tcp_with_timeouts(socket);

    let app = Router::new()
        .route("/", get(entrypoint_handler))
        .route("/health_check", get(health_check_handler))
        .route("/torrents/:info_hash", get(get_metainfo_file_handler))
        .layer(TraceLayer::new_for_http())
        .layer(
            ServiceBuilder::new()
                // this middleware goes above `TimeoutLayer` because it will receive
                // errors returned by `TimeoutLayer`
                .layer(HandleErrorLayer::new(|_: BoxError| async {
                    StatusCode::REQUEST_TIMEOUT
                }))
                .layer(TimeoutLayer::new(TIMEOUT)),
        )
        .with_state(Arc::new(state));

    server
        .acceptor(TimeoutAcceptor)
        .serve(app.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .await
        .expect("Axum server crashed.");
}

fn from_tcp_with_timeouts(socket: TcpListener) -> Server<std::net::SocketAddr> {
    let mut server = axum_server::from_tcp(socket).unwrap();

    server.http_builder().http1().timer(TokioTimer::new());
    server.http_builder().http2().timer(TokioTimer::new());

    server
        .http_builder()
        .http1()
        .header_read_timeout(Duration::from_secs(1));
    server
        .http_builder()
        .http2()
        .keep_alive_timeout(Duration::from_secs(1))
        .keep_alive_interval(Duration::from_secs(1));

    server
}
