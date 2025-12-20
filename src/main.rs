use std::{
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};
use torrust_hash2torrent::bit_torrent::client::Client;
use torrust_hash2torrent::config::{self, Config};
use torrust_hash2torrent::{
    api::{self, cache::Cache},
    AppState,
};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt().init();

    let session_output_dir = "/var/lib/torrust/hash2torrent/session";
    let torrents_cache_dir = "/var/lib/torrust/hash2torrent/torrents";
    let bind_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 3000);

    let config = Config {
        api: config::Api {
            bind_address,
            torrents_cache_dir: torrents_cache_dir.into(),
        },
        client: config::Client {
            listen_port_range: Some(51000..51010),
            session_output_dir: session_output_dir.into(),
        },
    };

    check_storage(&config)?;

    info!("creating BitTorrent client and starting the session ...");

    let mut client = Client::new(config.client.clone());
    client.start_session().await?;

    info!("starting API on: http://{bind_address} ..."); // DevSkim: ignore DS137138

    let app_state = AppState {
        config: Arc::new(config),
        client: Arc::new(client),
        cache: Arc::new(Cache::new(torrents_cache_dir.into())),
    };

    api::start(&bind_address, app_state).await;

    Ok(())
}

fn check_storage(config: &Config) -> Result<(), anyhow::Error> {
    // Check if the directories exist
    if fs::metadata(config.client.session_output_dir.clone()).is_err() {
        warn!(
            "Session output directory not found: {}",
            config.client.session_output_dir
        );
        return Err(anyhow::anyhow!(
            "Session output directory not found: {}",
            config.client.session_output_dir
        ));
    }

    if fs::metadata(config.api.torrents_cache_dir.clone()).is_err() {
        warn!(
            "Torrents cache directory not found: {}",
            config.api.torrents_cache_dir
        );
        return Err(anyhow::anyhow!(
            "Torrents cache directory not found: {}",
            config.api.torrents_cache_dir
        ));
    }

    Ok(())
}
