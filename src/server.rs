use std::{
    cmp,
    sync::{Arc, Mutex},
};
use tokio::sync::oneshot;

/// At any given time each server is in one of three states:
/// leader, follower, or candidate.
pub enum ServerState {
    Leader,
    Follower,
    Candidate,
}

/// ServerRequest represents a request sent by a Server.
struct ServerRequest {
    term: usize,
    responder: Arc<Mutex<oneshot::Sender<ServerResponse>>>,
}

/// ServerResponse represents a response received from a Server.
struct ServerResponse {
    term: usize,
}

/// A Server handles requests from Clients. Within the same Cluster,
/// Only one server can be the Leader at any one time.
pub struct Server {
    state: ServerState,
    current_term: usize,
}

/// Raft servers communicate using remote procedure calls (RPCs), and the basic
/// consensus algorithm requires only two types of RPCs. `RequestVote` RPCs are
/// initiated by candidates during elections, and `AppendEntries` RPCs are initiated
/// by leaders to replicate log entries and to provide a form of heartbeat.
impl Server {
    pub fn new() -> Self {
        Self {
            state: ServerState::Follower,
            current_term: 1,
        }
    }

    /// Current terms are exchanged whenever Servers communicate; if
    /// one Server’s current term is smaller than the other’s, then it updates
    /// its current term to the larger value.
    fn sync_term(&self, request: &Server) {
        match self.current_term.cmp(&request.current_term) {
            cmp::Ordering::Greater => {}
            cmp::Ordering::Less => {}
            cmp::Ordering::Equal => {}
        };
    }

    /// If a candidate or leader discovers that its term is out of date, it
    /// immediately reverts to follower state.
    fn revert_to_follower(&mut self) {
        self.state = ServerState::Follower;
    }

    pub fn append_entries(&self) {
        ()
    }

    pub fn request_vote(&self) {
        ()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts() -> anyhow::Result<()> {
        let _ = Server::new();
        Ok(())
    }
}
