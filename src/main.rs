use std::num::NonZeroU64;

use mini_raft_rs::cluster;
use tokio::time;

#[tokio::main]
async fn main() {
    cluster::Cluster::new(1024, time::Duration::from_millis(1000))
        .run_with_nodes(NonZeroU64::new(5).unwrap())
        .await;
}
