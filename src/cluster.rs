use tokio::sync::broadcast;

use crate::{cluster_node, server};

/// A Raft cluster contains several servers
pub struct Cluster {
    nodes: Vec<tokio::task::JoinHandle<cluster_node::Result<()>>>,
    publisher: broadcast::Sender<server::rpc::ServerRequest>,
    subscriber: broadcast::Receiver<server::rpc::ServerRequest>,
}

impl Cluster {
    const MESSAGE_BUFFER_SIZE: usize = 32;

    pub fn new() -> Self {
        let (publisher, subscriber) = broadcast::channel(Self::MESSAGE_BUFFER_SIZE);
        Self {
            nodes: Default::default(),
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
