use mini_raft_rs::cluster;

#[tokio::main]
async fn main() {
    cluster::Cluster::new(1024)
        .with_node_count(5)
        .with_election_timeout_range(150, 300)
        .run()
        .await;
}
