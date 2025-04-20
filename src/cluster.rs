use tokio::sync::broadcast;

use crate::server;

/// A Raft cluster contains several servers
pub struct Cluster {
    servers: Vec<server::Server>,
    publisher: broadcast::Sender<server::ServerRequest>,
    subscriber: broadcast::Receiver<server::ServerRequest>,
}

impl Cluster {
    const MESSAGE_BUFFER_SIZE: usize = 32;

    pub fn new() -> Self {
        let (publisher, subscriber) = broadcast::channel(Self::MESSAGE_BUFFER_SIZE);

        Self {
            servers: Default::default(),
            publisher,
            subscriber,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init() -> anyhow::Result<()> {
        let _ = Cluster::new();
        Ok(())
    }
}
