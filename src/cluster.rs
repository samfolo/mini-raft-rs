use std::time;

use tokio::sync::{broadcast, mpsc, watch};

use crate::{client, cluster_node, domain::listener, server, timeout};

/// A Raft cluster contains several servers
pub struct Cluster {
    nodes: cluster_node::ClusterNodeJoinSet,
    cluster_conn: mpsc::Sender<server::ServerRequest>,
    client_conn: broadcast::Sender<client::ClientRequest>,
    node_count: u64,
    heartbeat_interval: u64,
    min_election_timeout_ms: u64,
    max_election_timeout_ms: u64,
}

impl Cluster {
    const DEFAULT_NODE_COUNT: u64 = 5;
    const DEFAULT_MESSAGE_BUFFER_SIZE: usize = 32;
    const DEFAULT_HEARTBEAT_INTERVAL_MS: u64 = 50;
    const DEFAULT_MIN_ELECTION_TIMEOUT_MS: u64 = 150;
    const DEFAULT_MAX_ELECTION_TIMEOUT_MS: u64 = 300;

    pub fn new(buffer_size: usize) -> Self {
        let (cluster_conn, _) = mpsc::channel(buffer_size);
        let (client_conn, _) = broadcast::channel(buffer_size);

        Self {
            cluster_conn,
            client_conn,
            nodes: Default::default(),
            node_count: Self::DEFAULT_NODE_COUNT,
            heartbeat_interval: Self::DEFAULT_HEARTBEAT_INTERVAL_MS,
            min_election_timeout_ms: Self::DEFAULT_MIN_ELECTION_TIMEOUT_MS,
            max_election_timeout_ms: Self::DEFAULT_MAX_ELECTION_TIMEOUT_MS,
        }
    }

    pub fn with_node_count(mut self, node_count: u64) -> Self {
        assert!(node_count > 0);
        self.node_count = node_count;
        self
    }

    pub fn with_election_timeout_range(mut self, min: u64, max: u64) -> Self {
        assert!(min < max);
        self.min_election_timeout_ms = min;
        self.max_election_timeout_ms = max;
        self
    }

    pub fn with_heartbeat_interval(mut self, interval_ms: u64) -> Self {
        assert!(interval_ms > 0);
        self.heartbeat_interval = interval_ms;
        self
    }

    pub fn client_conn(&self) -> broadcast::Sender<client::ClientRequest> {
        self.client_conn.clone()
    }

    async fn register_node<N: cluster_node::ClusterNode + Sync>(
        &mut self,
        node_init: impl FnOnce(
            mpsc::Sender<server::ServerRequest>,
            broadcast::WeakSender<client::ClientRequest>,
        ) -> N,
    ) -> &mut Self {
        let cluster_conn = self.cluster_conn.clone();
        let client_conn = self.client_conn.downgrade();

        let node = node_init(cluster_conn, client_conn);
        self.nodes.spawn(async move { node.run().await });
        self
    }

    pub async fn start(mut self) -> Self {
        let (cluster_node_count_tx, _) = watch::channel::<u64>(self.node_count);

        for _ in 0..self.node_count {
            let cluster_node_count = cluster_node_count_tx.subscribe();
            let listener = listener::Listener::bind_random_local_port().await.unwrap();

            self.register_node(move |cluster_conn, client_conn| {
                server::Server::new(
                    cluster_conn,
                    client_conn,
                    time::Duration::from_millis(self.heartbeat_interval),
                    timeout::TimeoutRange::new(
                        self.min_election_timeout_ms,
                        self.max_election_timeout_ms,
                    ),
                    cluster_node_count,
                    listener,
                )
            })
            .await;
        }

        self
    }

    pub async fn run_until_ctrl_c(self) {
        match tokio::signal::ctrl_c().await {
            Ok(_) => self.shutdown().await,
            Err(err) => panic!("{err:?}"),
        }
    }

    async fn shutdown(mut self) {
        self.nodes.shutdown().await;
    }
}

impl Default for Cluster {
    fn default() -> Self {
        let (cluster_conn, _) = mpsc::channel(Self::DEFAULT_MESSAGE_BUFFER_SIZE);
        let (client_conn, _) = broadcast::channel(Self::DEFAULT_MESSAGE_BUFFER_SIZE);

        Self {
            nodes: Default::default(),
            cluster_conn,
            client_conn,
            node_count: Self::DEFAULT_NODE_COUNT,
            heartbeat_interval: Self::DEFAULT_HEARTBEAT_INTERVAL_MS,
            min_election_timeout_ms: Self::DEFAULT_MIN_ELECTION_TIMEOUT_MS,
            max_election_timeout_ms: Self::DEFAULT_MAX_ELECTION_TIMEOUT_MS,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::domain;

    use super::*;

    #[allow(unused)]
    struct MockServer {
        id: domain::node_id::NodeId,
        cluster_conn: mpsc::Sender<server::ServerRequest>,
        client_conn: broadcast::WeakSender<client::ClientRequest>,
    }

    impl MockServer {
        fn new(
            id: domain::node_id::NodeId,
            cluster_conn: mpsc::Sender<server::ServerRequest>,
            client_conn: broadcast::WeakSender<client::ClientRequest>,
        ) -> Self {
            Self {
                id,
                cluster_conn,
                client_conn,
            }
        }
    }

    impl cluster_node::ClusterNode for MockServer {
        async fn run(&self) -> cluster_node::Result<domain::node_id::NodeId> {
            Ok(self.id)
        }
    }

    #[tokio::test]
    async fn can_register_multiple_nodes() -> anyhow::Result<()> {
        let mut test_cluster = Cluster::default();

        let server_one_id = domain::node_id::NodeId::new();
        let server_two_id = domain::node_id::NodeId::new();

        test_cluster
            .register_node(Box::new(move |cluster_conn, client_conn| {
                MockServer::new(server_one_id.clone(), cluster_conn, client_conn)
            }))
            .await;

        assert_eq!(1, test_cluster.nodes.len());

        test_cluster
            .register_node(Box::new(move |cluster_conn, client_conn| {
                MockServer::new(server_two_id.clone(), cluster_conn, client_conn)
            }))
            .await;

        assert_eq!(2, test_cluster.nodes.len());

        let mut node_ids = HashSet::new();

        for task_result in test_cluster.nodes.join_all().await {
            let node_id = task_result?;
            node_ids.insert(node_id);
        }

        assert!(node_ids.contains(&server_one_id));
        assert!(node_ids.contains(&server_two_id));

        Ok(())
    }
}
