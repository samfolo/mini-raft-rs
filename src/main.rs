use mini_raft_rs::cluster;

#[tokio::main]
async fn main() {
    cluster::Cluster::new(1024)
        .with_node_count(5)
        .with_election_timeout_range(750, 1000)
        // Ideally the configured heartbeat interval should be
        // less than 0.5x the minimum election timeout range:
        .with_heartbeat_interval(374)
        .run()
        .await;
}
