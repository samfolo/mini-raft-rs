use std::num::NonZeroU64;

use mini_raft_rs::cluster;

#[tokio::main]
async fn main() {
    cluster::Cluster::new(1024)
        .with_node_count(NonZeroU64::new(5).unwrap())
        .with_election_timeout_range(200, 1000)
        .run()
        .await;
}
