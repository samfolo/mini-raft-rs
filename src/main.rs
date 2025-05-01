use std::collections::HashMap;

use mini_raft_rs::{
    client::{self, Client},
    cluster,
    domain::node_id,
    server,
};
use tokio::{sync::mpsc, time};

#[tokio::main]
async fn main() {
    // let cluster = cluster::Cluster::new(1024)
    //     .with_node_count(5)
    //     .with_election_timeout_range(750, 1000)
    //     // Ideally the configured heartbeat interval should be
    //     // less than 0.5x the minimum election timeout range:
    //     .with_heartbeat_interval(374)
    //     .start()
    //     .await;

    // time::sleep(time::Duration::from_millis(2000)).await;

    // client::RandomDataClient::init()
    //     .with_request_interval_range(550, 887)
    //     .connect_to_cluster(&cluster)
    //     .make_random_requests()
    //     .await
    //     .unwrap();

    // cluster.run_until_ctrl_c().await;

    let id1 = node_id::NodeId::new();
    let id2 = node_id::NodeId::new();
    let id3 = node_id::NodeId::new();
    let id4 = node_id::NodeId::new();

    let (tx1, mut rx1) = mpsc::channel(8);
    let (tx2, mut rx2) = mpsc::channel(8);
    let (tx3, mut rx3) = mpsc::channel(8);
    let (tx4, mut rx4) = mpsc::channel(8);

    let mut init = HashMap::new();
    init.insert(id1, server::ServerHandle::new(tx1));
    init.insert(id2, server::ServerHandle::new(tx2));
    init.insert(id3, server::ServerHandle::new(tx3));
    init.insert(id4, server::ServerHandle::new(tx4));

    let broker = server::OutgoingMessageBroker::from(init);

    match tokio::signal::ctrl_c().await {
        Ok(_) => println!("done"),
        Err(err) => panic!("{err:?}"),
    }
}
