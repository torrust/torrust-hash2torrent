# Torrust Hash2Torrent

[![Testing](https://github.com/torrust/torrust-hash2torrent/actions/workflows/testing.yaml/badge.svg)](https://github.com/torrust/torrust-hash2torrent/actions/workflows/testing.yaml) [![Container](https://github.com/torrust/torrust-hash2torrent/actions/workflows/container.yaml/badge.svg)](https://github.com/torrust/torrust-hash2torrent/actions/workflows/container.yaml)

A web service to get torrents' metadata from the infohashes.

The API is based on the Rust BitTorrent client [rqbit](https://github.com/ikatson/rqbit). The client uses [BEP 9](https://www.bittorrent.org/beps/bep_0009.html) to get the Metadata Files from other peers.

> NOTICE: DHT must be enabled because the client needs to find peers first.

Live demo: <https://hash2torrent.com/>

## Setup

```console
sudo ./contrib/dev-tools/init/install.sh $(id -u)
cargo run
```

### With Docker

Building the image from sources:

```console
sudo ./contrib/dev-tools/init/install.sh $(id -u)
./contrib/dev-tools/containers/docker-build.sh
./contrib/dev-tools/containers/docker-run.sh
```

## Usage

Download the torrent with curl:

```console
curl -o ./ubuntu-23.04-desktop-amd64.iso.torrent http://127.0.0.1:3000/torrents/443c7602b4fde83d1154d6d9da48808418b181b6
```

Or with the browser:

<http://127.0.0.1:3000/torrents/443c7602b4fde83d1154d6d9da48808418b181b6>

> NOTICE: The BitTorrent client may not find the torrent and the HTTP could return a 408 (timeout) error after 10

You can check the API with the health_check endpoint: <http://127.0.0.1:3000/health_check>

## Troubleshooting

### Tokio Runtime Error

If you encounter an error like:

```text
Registering a blocking socket with the tokio runtime is unsupported. If you wish to do anyways, please add `--cfg tokio_allow_from_blocking_fd` to your RUSTFLAGS.
```

You need to set the RUSTFLAGS environment variable when building and running the application:

```console
RUSTFLAGS="--cfg tokio_allow_from_blocking_fd" cargo run
```

This is a known issue with the tokio runtime on certain systems. The flag allows the application to register blocking file descriptors with the async runtime.

### Session Directory Not Found

If you see an error about the session output directory not being found:

```text
Error: Session output directory not found: /var/lib/torrust/hash2torrent/session
```

Make sure you've run the setup script first:

```console
sudo ./contrib/dev-tools/init/install.sh $(id -u)
```

This script creates the necessary directories with proper permissions for the BitTorrent client session data.

### Torrent Not Found (408 Timeout)

The BitTorrent client needs to find peers through DHT before it can download the metadata. This process can take time, and the request may timeout (10 seconds default). If this happens:

- DHT needs time to bootstrap and find peers
- Try the request again after a few moments
- Some torrents may have very few or no active peers
- Check that your network allows DHT traffic on the configured port

## Acknowledgments

[ikatson](https://github.com/ikatson) main contributor to [rqbit](https://github.com/ikatson/rqbit).
