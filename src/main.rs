use std::collections::HashMap;

use mini_raft_rs::{domain::node_id, server};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let id1 = node_id::NodeId::new();
    let id2 = node_id::NodeId::new();
    let id3 = node_id::NodeId::new();
    let id4 = node_id::NodeId::new();
    let id5 = node_id::NodeId::new();

    let (tx1, mut rx1) = mpsc::channel(8);
    let (tx2, mut rx2) = mpsc::channel(8);
    let (tx3, mut rx3) = mpsc::channel(8);
    let (tx4, mut rx4) = mpsc::channel(8);
    let (tx5, mut rx5) = mpsc::channel(8);

    let mut init = HashMap::new();
    init.insert(id1, server::ServerHandle::new(tx1));
    init.insert(id2, server::ServerHandle::new(tx2));
    init.insert(id3, server::ServerHandle::new(tx3));
    init.insert(id4, server::ServerHandle::new(tx4));
    init.insert(id5, server::ServerHandle::new(tx5));

    let broker = server::ServerPeerList::from(init);

    match tokio::signal::ctrl_c().await {
        Ok(_) => println!("done"),
        Err(err) => panic!("{err:?}"),
    }
}
