use std::num::NonZeroU64;

use tokio::{sync::broadcast, time};

use crate::{cluster_node, server};

type Publisher = broadcast::Sender<server::rpc::ServerRequest>;
type Subscriber = broadcast::Receiver<server::rpc::ServerRequest>;
type NodeInit<N> = Box<dyn FnOnce(Publisher, Subscriber) -> N>;

/// A Raft cluster contains several servers
pub struct Cluster {
    nodes: Vec<cluster_node::ClusterNodeHandle>,
    publisher: Publisher,
    subscriber: Subscriber,
    election_timeout: time::Duration,
}

impl Cluster {
    const DEFAULT_MESSAGE_BUFFER_SIZE: usize = 32;
    const DEFAULT_ELECTION_TIMEOUT_MS: u64 = 1000;

    pub fn new(buffer_size: usize, election_timeout: time::Duration) -> Self {
        let (publisher, subscriber) = broadcast::channel(buffer_size);
        Self {
            nodes: Default::default(),
            publisher,
            subscriber,
            election_timeout,
        }
    }

    async fn register_node<N: cluster_node::ClusterNode + Sync>(
        &mut self,
        node_init: NodeInit<N>,
    ) -> &mut Self {
        let publisher = self.publisher.clone();
        let subscriber = publisher.subscribe();
        let mut node = node_init(publisher, subscriber);
        let handle = tokio::spawn(async move { node.run().await });
        self.nodes.push(handle);
        self
    }

    pub async fn run_with_nodes(mut self, count: NonZeroU64) {
        for _ in 0..count.into() {
            self.register_node(Box::new(move |tx, rx| {
                server::Server::new(tx, rx, self.election_timeout)
            }))
            .await;
        }

        match tokio::signal::ctrl_c().await {
            Ok(_) => self.shutdown().await,
            Err(err) => panic!("{err:?}"),
        }
    }

    pub async fn shutdown(self) {
        for node in &self.nodes {
            node.abort();
        }

        for node in self.nodes {
            assert!(node.await.unwrap_err().is_cancelled());
        }
    }
}

impl Default for Cluster {
    fn default() -> Self {
        let (publisher, subscriber) = broadcast::channel(Self::DEFAULT_MESSAGE_BUFFER_SIZE);
        Self {
            nodes: Default::default(),
            publisher,
            subscriber,
            election_timeout: time::Duration::from_millis(Self::DEFAULT_ELECTION_TIMEOUT_MS),
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
        async fn run(self: &mut Self) -> cluster_node::Result<uuid::Uuid> {
            Ok(self.id)
        }
    }

    #[tokio::test]
    async fn can_register_multiple_nodes() -> anyhow::Result<()> {
        let mut test_cluster = Cluster::default();

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
