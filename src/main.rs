use std::collections::HashMap;

use mini_raft_rs::{client, domain::node_id, server};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let id1 = node_id::NodeId::new();
    let id2 = node_id::NodeId::new();
    let id3 = node_id::NodeId::new();
    let id4 = node_id::NodeId::new();
    let id5 = node_id::NodeId::new();

    let (tx1, rx1) = mpsc::channel(8);
    let (tx2, rx2) = mpsc::channel(8);
    let (tx3, rx3) = mpsc::channel(8);
    let (tx4, rx4) = mpsc::channel(8);
    let (tx5, rx5) = mpsc::channel(8);

    let mut init = HashMap::new();
    init.insert(id1.clone(), server::ServerHandle::new(tx1));
    init.insert(id2.clone(), server::ServerHandle::new(tx2));
    init.insert(id3.clone(), server::ServerHandle::new(tx3));
    init.insert(id4.clone(), server::ServerHandle::new(tx4));
    init.insert(id5.clone(), server::ServerHandle::new(tx5));

    let peer_list = server::ServerPeerList::from(init);

    let server1 = server::Server::new(id1, peer_list.clone());
    let server2 = server::Server::new(id2, peer_list.clone());
    let server3 = server::Server::new(id3, peer_list.clone());
    let server4 = server::Server::new(id4, peer_list.clone());
    let server5 = server::Server::new(id5, peer_list.clone());

    tokio::try_join!(
        server1.run(rx1),
        server2.run(rx2),
        server3.run(rx3),
        server4.run(rx4),
        server5.run(rx5),
    )?;

    match tokio::signal::ctrl_c().await {
        Ok(_) => println!("done"),
        Err(err) => panic!("{err:?}"),
    };

    Ok(())
}
