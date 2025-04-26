use mini_raft_rs::{
    client::{self, Client},
    cluster,
};
use tokio::time;

#[tokio::main]
async fn main() {
    let client = client::RandomDataClient::new();

    let cluster = cluster::Cluster::new(1024)
        .with_node_count(5)
        .with_election_timeout_range(750, 1000)
        // Ideally the configured heartbeat interval should be
        // less than 0.5x the minimum election timeout range:
        .with_heartbeat_interval(374)
        .start()
        .await;

    time::sleep(time::Duration::from_millis(2000)).await;

    client.connect_to_cluster(&cluster);
    cluster.run_until_ctrl_c().await;
}
