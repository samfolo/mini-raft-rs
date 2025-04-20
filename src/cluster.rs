use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;

use crate::{cluster_node, server};

type Publisher = Arc<Mutex<broadcast::Sender<server::rpc::ServerRequest>>>;
type Subscriber = Arc<Mutex<broadcast::Receiver<server::rpc::ServerRequest>>>;

type NodeInit<N> = Box<dyn FnOnce(Publisher, Subscriber) -> N>;

/// A Raft cluster contains several servers
pub struct Cluster {
    nodes: Vec<cluster_node::ClusterNodeHandle>,
    publisher: Publisher,
    subscriber: Subscriber,
}

impl Cluster {
    const MESSAGE_BUFFER_SIZE: usize = 32;

    pub fn new() -> Self {
        let (publisher, subscriber) = broadcast::channel(Self::MESSAGE_BUFFER_SIZE);
        Self {
            nodes: Default::default(),
            publisher: Arc::new(Mutex::new(publisher)),
            subscriber: Arc::new(Mutex::new(subscriber)),
        }
    }

    pub async fn register_node<N: cluster_node::ClusterNode>(
        &mut self,
        node_init: NodeInit<N>,
    ) -> &mut Self {
        let mut node = node_init(Arc::clone(&self.publisher), Arc::clone(&self.subscriber));
        let handle = tokio::spawn(async move { node.run().await });
        self.nodes.push(handle);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockServer {
        id: &'static str,
        tx: Publisher,
        rx: Subscriber,
    }

    impl MockServer {
        fn new(id: &'static str, tx: Publisher, rx: Subscriber) -> Self {
            Self { id, tx, rx }
        }
    }

    impl cluster_node::ClusterNode for MockServer {
        async fn run(&mut self) -> cluster_node::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn can_register_node() -> anyhow::Result<()> {
        let mut test_cluster = Cluster::new();

        test_cluster
            .register_node(Box::new(|tx, rx| {
                MockServer::new("mock-server-1-id", tx, rx)
            }))
            .await;

        assert_eq!(1, test_cluster.nodes.len());

        Ok(())
    }
}
