use mini_raft_rs::{
    client::{self, Client},
    cluster,
};
use tokio::time;

#[tokio::main]
async fn main() {
    let cluster = cluster::Cluster::new(1024)
        .with_node_count(5)
        .with_election_timeout_range(750, 1000)
        // Ideally the configured heartbeat interval should be
        // less than 0.5x the minimum election timeout range:
        .with_heartbeat_interval(374)
        .start()
        .await;

    time::sleep(time::Duration::from_millis(2000)).await;

    client::RandomDataClient::new()
        .with_request_interval_range(1000, 1500)
        .connect_to_cluster(&cluster)
        .make_random_requests()
        .await
        .unwrap();

    cluster.run_until_ctrl_c().await;
}
