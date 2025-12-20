use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse, Response};
use bytes::Bytes;
use hyper::{header, HeaderMap, StatusCode};

use serde::Deserialize;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, error, info, trace};

use crate::bit_torrent::info_hash::InfoHash;

use crate::AppState;

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

#[allow(clippy::module_name_repetitions)]
pub async fn get_metainfo_file_handler(
    State(app_state): State<Arc<AppState>>,
    Path(info_hash): Path<InfoHashParam>,
) -> Response {
    let Ok(info_hash) = InfoHash::from_str(&info_hash.lowercase()) else {
        return (StatusCode::BAD_REQUEST, "Invalid info hash").into_response();
    };

    info!("req: {}", info_hash.to_hex_string());

    if app_state.cache.contains(&info_hash) {
        if let Ok(bytes) = app_state.cache.get(&info_hash) {
            debug!("cached torrent: {}", app_state.cache.path(&info_hash));

            return torrent_file_response(
                bytes,
                &format!("{}.torrent", info_hash.to_hex_string()),
                &info_hash.to_hex_string(),
            );
        }
    }

    let magnet_link = format!("magnet:?xt=urn:btih:{}", info_hash.to_hex_string());

    let Ok((_info, bytes)) = app_state.client.resolve_magnet(magnet_link).await else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "BitTorrent client error").into_response();
    };

    match app_state.cache.add(&info_hash, &bytes) {
        Ok(()) => {
            trace!("added torrent to cache: {}", info_hash.to_hex_string());
        }
        Err(err) => {
            error!("error adding torrent to cache: {}", err);
        }
    }

    torrent_file_response(
        bytes,
        &format!("{}.torrent", info_hash.to_hex_string()),
        &info_hash.to_hex_string(),
    )
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

#[allow(clippy::module_name_repetitions)]
pub async fn health_check_handler() -> Response {
    (StatusCode::OK, "OK").into_response()
}

#[allow(clippy::module_name_repetitions)]
pub async fn entrypoint_handler() -> Html<&'static str> {
    let html = r#"
    <!DOCTYPE html>
    <html lang="en">

    <head>
        <meta charset="UTF-8">
        <meta http-equiv="X-UA-Compatible" content="IE=edge">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
        <title>Hash2Torrent</title>
        <style>
            body {
                font-family: Arial, sans-serif;
                display: flex;
                flex-direction: column;
                justify-content: center;
                align-items: center;
                height: 100vh;
                margin: 0;
                background-color: #f8f9fa;
            }

            h1 {
                font-size: 3em;
                margin-bottom: 20px;
                color: #333;
            }

            p {
                font-size: 1.1em;
                margin-bottom: 30px;
                color: #555;
                text-align: center;
            }

            .input-container {
                display: flex;
                flex-direction: column;
                align-items: center;
                margin-bottom: 50px;
            }

            input[type="text"] {
                width: 400px;
                padding: 10px;
                font-size: 1.1em;
                margin-bottom: 20px;
                border: 1px solid #ccc;
                border-radius: 4px;
                transition: border-color 0.3s;
            }

            input[type="text"]:focus {
                border-color: #007bff;
                outline: none;
            }

            button {
                padding: 10px 20px;
                font-size: 1.1em;
                color: #fff;
                background-color: #007bff;
                border: none;
                border-radius: 4px;
                cursor: pointer;
                transition: background-color 0.3s;
            }

            button:hover {
                background-color: #0056b3;
            }

            .github-link {
                margin-top: 20px;
                font-size: 1em;
                color: #007bff;
                text-decoration: none;
                transition: color 0.3s;
            }

            .github-link:hover {
                color: #0056b3;
            }
        </style>
    </head>

    <body>
        <h1>Hash2Torrent</h1>

        <div class="input-container">
            <input type="text" id="infohash" placeholder="">
            <button onclick="downloadTorrent()">Download metadata</button>
        </div>

        <p>Introduce a torrent v1 infohash like <a href="/torrents/443c7602b4fde83d1154d6d9da48808418b181b6">443c7602b4fde83d1154d6d9da48808418b181b6</a><br/> and download the bencoded torrent metadata (only info dictionary)</p>

        <a href="https://github.com/torrust/torrust-hash2torrent" class="github-link" target="_blank">Fork on GitHub</a>

        <script>
            function downloadTorrent() {
                var infohash = document.getElementById('infohash').value.trim();
                if (infohash) {
                    window.location.href = '/torrents/' + infohash;
                } else {
                    alert('Please enter a valid infohash like 443c7602b4fde83d1154d6d9da48808418b181b6');
                }
            }
        </script>
    </body>

    </html>
    "#;

    Html(html) // Wrap HTML content in Html response type
}
