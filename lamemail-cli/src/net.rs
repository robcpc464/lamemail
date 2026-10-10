use std::fmt;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug)]
pub struct Refused(pub String);

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refused {}

fn tls_config() -> Arc<ClientConfig> {
    static CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let mut roots = RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            Arc::new(
                ClientConfig::builder()
                    .with_root_certificates(roots)
                    .with_no_client_auth(),
            )
        })
        .clone()
}

pub async fn timed<T>(what: &str, fut: impl Future<Output = T>) -> Result<T> {
    tokio::time::timeout(RESPONSE_TIMEOUT, fut)
        .await
        .with_context(|| format!("{what}: no response from the server"))
}

pub async fn tcp(host: &str, port: u16) -> Result<TcpStream> {
    tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect((host, port)))
        .await
        .with_context(|| format!("connecting to {host}:{port} timed out"))?
        .with_context(|| format!("connecting to {host}:{port}"))
}

pub async fn tls(host: &str, tcp: TcpStream) -> Result<TlsStream<TcpStream>> {
    let server_name = ServerName::try_from(host.to_string())
        .with_context(|| format!("invalid host name {host}"))?;
    tokio::time::timeout(
        CONNECT_TIMEOUT,
        TlsConnector::from(tls_config()).connect(server_name, tcp),
    )
    .await
    .context("TLS handshake timed out")?
    .context("TLS handshake")
}
