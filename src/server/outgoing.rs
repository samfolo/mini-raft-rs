use std::{collections::HashMap, time::Duration};

use tokio::{sync::mpsc, task::JoinHandle, time};

use crate::domain::node_id;

use super::{
    ServerRequest,
    handle::{self, ServerHandle},
};

pub struct OutgoingMessageBroker {
    processes: HashMap<node_id::NodeId, JoinHandle<anyhow::Result<node_id::NodeId>>>,
}

impl OutgoingMessageBroker {
    pub fn from(initial_peers: HashMap<node_id::NodeId, ServerHandle>) -> Self {
        let mut peer_list = Self {
            processes: HashMap::new(),
        };

        for (id, handle) in initial_peers {
            let id_clone = id.clone();
            peer_list.insert(
                id,
                tokio::spawn(async move {
                    loop {
                        println!("SPAWNED PROCESS FOR ID: {id_clone}");
                        handle.append_entries(id_clone, 1, vec![]).await?;
                        time::sleep(Duration::from_millis(1000)).await;
                    }

                    Ok(id_clone)
                }),
            );
        }

        peer_list
    }

    pub fn insert(
        &mut self,
        id: node_id::NodeId,
        handle: JoinHandle<anyhow::Result<node_id::NodeId>>,
    ) -> Option<JoinHandle<anyhow::Result<node_id::NodeId>>> {
        self.processes.insert(id, handle)
    }

    pub fn remove(
        &mut self,
        id: &node_id::NodeId,
    ) -> Option<JoinHandle<anyhow::Result<node_id::NodeId>>> {
        self.processes.remove(id)
    }
}
