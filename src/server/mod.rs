pub mod slowloris;

use axum::error_handling::HandleErrorLayer;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{BoxError, Router};
use axum_server::Server;
use bytes::Bytes;
use hyper::{header, HeaderMap, StatusCode};
use hyper_util::rt::TokioTimer;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, ByteBufOwned, ListOnlyResponse, Session,
    TorrentMetaV1Info,
};
use serde::Deserialize;
use std::net::{SocketAddr, TcpListener};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tower::{timeout::TimeoutLayer, ServiceBuilder};
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::info_hash::InfoHash;
use crate::server::slowloris::TimeoutAcceptor;

const TIMEOUT: Duration = Duration::from_secs(10);

/// The info hash URL path parameter.
///
/// For example: ` http://127.0.0.1:3000/torrents/443c7602b4fde83d1154d6d9da48808418b181b6`.
///
/// The info hash represents the value collected from the URL path parameter.
/// It does not include validation as this is done by the API endpoint handler,
/// in order to provide a more specific error message.
#[derive(Deserialize)]
pub struct InfoHashParam(pub String);

impl InfoHashParam {
    fn lowercase(&self) -> String {
        self.0.to_lowercase()
    }
}

enum ResolveMagnetError {
    AddedForDownloading, // It should not be added for downloading.
    NotAdded,
}

/// It starts the web server.
///
/// # Panics
///
/// Will panic if it can get the local server address
pub async fn start(bind_to: &SocketAddr, session: Arc<Session>) {
    let socket =
        std::net::TcpListener::bind(bind_to).expect("Could not bind tcp_listener to address.");

    let server_address = socket
        .local_addr()
        .expect("Could not get local_addr from tcp_listener.");

    info!("server bound to address: http://{server_address}"); // DevSkim: ignore DS137138

    let server = from_tcp_with_timeouts(socket);

    let app = Router::new()
        .route("/torrents/:info_hash", get(get_metainfo))
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
        .with_state(session);

    server
        .acceptor(TimeoutAcceptor)
        .serve(app.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .await
        .expect("Axum server crashed.");
}

fn from_tcp_with_timeouts(socket: TcpListener) -> Server {
    let mut server = axum_server::from_tcp(socket);

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

async fn get_metainfo(
    State(session): State<Arc<Session>>,
    Path(info_hash): Path<InfoHashParam>,
) -> Response {
    let Ok(info_hash) = InfoHash::from_str(&info_hash.lowercase()) else {
        return (StatusCode::BAD_REQUEST, "Invalid info hash").into_response();
    };

    info!("req: {}", info_hash.to_hex_string());

    let magnet_link = format!("magnet:?xt=urn:btih:{}", info_hash.to_hex_string());

    match resolve_magnet(session, magnet_link).await {
        Ok((info, bytes)) => {
            // Resolve torrent name
            let name = if let Some(name) = info.name.as_ref() {
                if let Ok(name) = std::str::from_utf8(name) {
                    name
                } else {
                    &info_hash.to_hex_string()
                }
            } else {
                &info_hash.to_hex_string()
            };

            // Return the torrent file as the response
            torrent_file_response(
                bytes,
                &format!("{name}.torrent"),
                &info_hash.to_hex_string(),
            )
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "BitTorrent client error").into_response(),
    }
}

async fn resolve_magnet(
    session: Arc<Session>,
    url: String,
) -> Result<(TorrentMetaV1Info<ByteBufOwned>, Bytes), ResolveMagnetError> {
    let added = match session
        .add_torrent(
            AddTorrent::from_url(&url),
            Some(AddTorrentOptions {
                list_only: true,
                ..Default::default()
            }),
        )
        .await
    {
        Ok(add_torrent_response) => add_torrent_response,
        Err(_err) => return Err(ResolveMagnetError::NotAdded),
    };

    let (info, content) = match added {
        AddTorrentResponse::AlreadyManaged(_, handle) => (
            handle.shared().info.clone(),
            handle.shared().torrent_bytes.clone(),
        ),
        AddTorrentResponse::ListOnly(ListOnlyResponse {
            info,
            torrent_bytes,
            ..
        }) => (info, torrent_bytes),
        AddTorrentResponse::Added(_, _) => return Err(ResolveMagnetError::AddedForDownloading),
    };

    Ok((info, content))
}

/// Builds the binary response for a torrent file.
///
/// # Panics
///
/// Panics if the filename is not a valid header value for the `content-disposition`
/// header.
#[must_use]
pub fn torrent_file_response(bytes: Bytes, filename: &str, info_hash: &str) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "application/x-bittorrent"
            .parse()
            .expect("HTTP content type header should be valid"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename={filename}").parse().expect(
            "Torrent filename should be a valid header value for the content disposition header",
        ),
    );
    headers.insert(
        "x-torrust-torrent-infohash",
        info_hash.parse().expect(
            "Torrent infohash should be a valid header value for the content disposition header",
        ),
    );

    (StatusCode::OK, headers, bytes).into_response()
}
