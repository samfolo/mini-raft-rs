use std::{collections::HashMap, time::Duration};

use tokio::{sync::mpsc, task::JoinHandle, time};

use crate::domain::node_id;

use super::{
    ServerRequest,
    handle::{self, ServerHandle},
};

type PeerList = HashMap<node_id::NodeId, ServerHandle>;

#[derive(Clone)]
pub struct ServerPeerList {
    peer_list: PeerList,
}

impl ServerPeerList {
    pub fn get(&self, id: &node_id::NodeId) -> Option<&ServerHandle> {
        self.peer_list.get(id)
    }

    pub fn insert(&mut self, id: node_id::NodeId, handle: ServerHandle) -> Option<ServerHandle> {
        self.peer_list.insert(id, handle)
    }

    pub fn remove(&mut self, id: &node_id::NodeId) -> Option<ServerHandle> {
        self.peer_list.remove(id)
    }

    pub fn random_peer(&self) -> (node_id::NodeId, ServerHandle) {
        let random_index = rand::random_range(..self.peer_list.len());
        let ids: Vec<_> = self.peer_list.keys().collect();
        let id = ids[random_index];
        (id.clone(), self.peer_list.get(id).unwrap().clone())
    }
}

impl From<PeerList> for ServerPeerList {
    fn from(peer_list: PeerList) -> Self {
        Self { peer_list }
    }
}
