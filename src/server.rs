use std::{
    cmp,
    sync::{Arc, Mutex},
};
use tokio::sync::{broadcast, mpsc};

/// At any given time each server is in one of three states:
/// leader, follower, or candidate.
pub enum ServerState {
    Leader,
    Follower,
    Candidate,
}

/// ServerRequest represents a request sent by a Server.
#[derive(Clone)]
pub struct ServerRequest {
    term: usize,
}

/// ServerResponse represents a response received from a Server.
pub struct ServerResponse {
    term: usize,
}

/// A Server handles requests from Clients. Within the same Cluster,
/// Only one server can be the Leader at any one time.
pub struct Server {
    state: ServerState,
    current_term: usize,
    publisher: broadcast::Sender<ServerRequest>,
    subscriber: broadcast::Receiver<ServerRequest>,
}

/// Raft servers communicate using remote procedure calls (RPCs), and the basic
/// consensus algorithm requires only two types of RPCs. `RequestVote` RPCs are
/// initiated by candidates during elections, and `AppendEntries` RPCs are initiated
/// by leaders to replicate log entries and to provide a form of heartbeat.
impl Server {
    pub fn new(
        publisher: broadcast::Sender<ServerRequest>,
        subscriber: broadcast::Receiver<ServerRequest>,
    ) -> Self {
        Self {
            state: ServerState::Follower,
            current_term: 1,
            publisher,
            subscriber,
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

    const TEST_CANNEL_CAPACITY: usize = 16;

    #[test]
    fn starts() -> anyhow::Result<()> {
        let (publisher, subscriber) = broadcast::channel(TEST_CANNEL_CAPACITY);
        let _ = Server::new(publisher, subscriber);
        Ok(())
    }
}
