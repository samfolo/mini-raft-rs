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
    const DEFAULT_MESSAGE_BUFFER_SIZE: usize = 32;

    pub fn new(buffer_size: usize) -> Self {
        let (publisher, subscriber) = broadcast::channel(buffer_size);
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

impl Default for Cluster {
    fn default() -> Self {
        let (publisher, subscriber) = broadcast::channel(Self::DEFAULT_MESSAGE_BUFFER_SIZE);
        Self {
            nodes: Default::default(),
            publisher: Arc::new(Mutex::new(publisher)),
            subscriber: Arc::new(Mutex::new(subscriber)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    struct MockServer {
        id: uuid::Uuid,
        tx: Publisher,
        rx: Subscriber,
    }

    impl MockServer {
        fn new(id: uuid::Uuid, tx: Publisher, rx: Subscriber) -> Self {
            Self { id, tx, rx }
        }
    }

    impl cluster_node::ClusterNode for MockServer {
        async fn run(&mut self) -> cluster_node::Result<uuid::Uuid> {
            Ok(self.id)
        }
    }

    #[tokio::test]
    async fn can_register_multiple_nodes() -> anyhow::Result<()> {
        let mut test_cluster = Cluster::new(Cluster::DEFAULT_MESSAGE_BUFFER_SIZE);

        let server_one_id = uuid::Uuid::new_v4();
        let server_two_id = uuid::Uuid::new_v4();

        test_cluster
            .register_node(Box::new(move |tx, rx| {
                MockServer::new(server_one_id.clone(), tx, rx)
            }))
            .await;

        assert_eq!(1, test_cluster.nodes.len());

        test_cluster
            .register_node(Box::new(move |tx, rx| {
                MockServer::new(server_two_id.clone(), tx, rx)
            }))
            .await;

        assert_eq!(2, test_cluster.nodes.len());

        let mut node_ids = HashSet::new();

        for handle in test_cluster.nodes {
            let node_id = handle.await??;
            node_ids.insert(node_id);
        }

        assert!(node_ids.contains(&server_one_id));
        assert!(node_ids.contains(&server_two_id));

        Ok(())
    }
}
