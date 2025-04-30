use std::collections::HashMap;

use tokio::sync::mpsc;

use crate::domain::node_id;

use super::{ServerRequest, handle};

#[derive(Clone)]
pub struct PeerList {
    peers: HashMap<node_id::NodeId, handle::ServerHandle>,
}

impl PeerList {
    pub fn new() -> Self {
        PeerList {
            peers: HashMap::new(),
        }
    }

    pub fn insert(
        &mut self,
        id: node_id::NodeId,
        handle: handle::ServerHandle,
    ) -> Option<handle::ServerHandle> {
        self.peers.insert(id, handle)
    }

    pub fn remove(&mut self, id: &node_id::NodeId) -> Option<handle::ServerHandle> {
        self.peers.remove(id)
    }
}

impl Default for PeerList {
    fn default() -> Self {
        Self::new()
    }
}
