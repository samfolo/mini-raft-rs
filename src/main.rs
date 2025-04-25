use mini_raft_rs::cluster;

#[tokio::main]
async fn main() {
    cluster::Cluster::new(1024)
        .with_node_count(5)
        .with_election_timeout_range(750, 1000)
        .with_heartbeat_interval(100)
        .run()
        .await;
}
