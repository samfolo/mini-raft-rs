use std::fmt;
use tokio::net::TcpListener;

use anyhow::bail;

#[derive(Debug)]
pub struct Listener {
    _inner: TcpListener,
    port: u16,
}

impl PartialEq for Listener {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }
}

impl Listener {
    pub async fn bind_random_local_port() -> anyhow::Result<Self> {
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(l) => l,
            Err(err) => bail!("failed to bind to random port: {err:?}"),
        };

        let local_addr = match listener.local_addr() {
            Ok(addr) => addr,
            Err(err) => bail!("failed to resolve local address: {err:?}"),
        };

        Ok(Self {
            _inner: listener,
            port: local_addr.port(),
        })
    }
}

impl fmt::Display for Listener {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "localhost:{}", self.port)
    }
}
