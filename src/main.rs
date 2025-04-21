use std::num::NonZeroU64;

use mini_raft_rs::cluster;

#[tokio::main]
async fn main() {
    cluster::Cluster::new(1024)
        .run_with_nodes(NonZeroU64::new(5).unwrap())
        .await;
}
