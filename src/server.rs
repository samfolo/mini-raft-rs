#![allow(unused)] // TODO: use unused... need to refactor something first.

mod handle;
mod log;
mod peer_list;
mod receiver;
mod request;

pub use handle::ServerHandle;
pub use log::ServerLogEntry;
pub use peer_list::ServerPeerList;
pub use request::{
    ServerRequest, ServerRequestBody, ServerRequestHeaders, ServerResponse, ServerResponseBody,
    ServerResponseHeaders,
};

use std::sync::RwLock;
use tokio::{
    sync::{broadcast, mpsc, watch},
    time,
};

use crate::{
    client, cluster_node, domain, naive_logging, state_machine,
    timeout::{self, TimeoutRange},
};
use crate::{cluster_node::error::ClusterNodeError, domain::listener};

/// At any given time each server is in one of three states:
/// leader, follower, or candidate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ServerState {
    Leader,
    Follower,
    Candidate,
}

/// A Server handles requests from Clients. Within the same Cluster,
/// Only one server can be the Leader at any one time.
pub struct Server {
    // Internal metadata:
    // -----------------------------------------------------
    id: domain::node_id::NodeId,
    state: watch::Receiver<ServerState>,
    state_tx: watch::Sender<ServerState>,
    state_machine: state_machine::InMemoryStateMachine,

    // Persistent state:
    // -----------------------------------------------------

    // Volatile state:
    // -----------------------------------------------------

    // Cluster configuration:
    // -----------------------------------------------------
    heartbeat_interval: time::Duration,
    election_timeout_range: timeout::TimeoutRange,
}

/// Raft servers communicate using remote procedure calls (RPCs), and the basic
/// consensus algorithm requires only two types of RPCs:
/// - `RequestVote`
/// - `AppendEntries`
impl Server {
    const DEFAULT_MESSAGE_BUFFER_SIZE: usize = 32;

    pub fn new(
        heartbeat_interval: time::Duration,
        election_timeout_range: timeout::TimeoutRange,
    ) -> Self {
        let id = domain::node_id::NodeId::new();
        naive_logging::log(&id, "initialised.");

        let (state_tx, state) = watch::channel(ServerState::Follower);

        Self {
            // Internal metadata:
            // -----------------------------------------------------
            id,
            state,
            state_tx,
            state_machine: state_machine::InMemoryStateMachine::new(),

            // Persistent state:
            // -----------------------------------------------------

            // Volatile state:
            // -----------------------------------------------------

            // Cluster configuration:
            // -----------------------------------------------------
            heartbeat_interval: time::Duration::from_millis(500),
            election_timeout_range: TimeoutRange::new(500, 1000),
        }
    }

    /// If a candidate or leader discovers that its term is out of date, it
    /// immediately reverts to follower state.
    fn downgrade_to_follower(&self) -> Result<(), watch::error::SendError<ServerState>> {
        if *self.state.borrow() != ServerState::Follower {
            naive_logging::log(&self.id, "downgrading to follower...");
            return self.state_tx.send(ServerState::Follower);
        }

        Ok(())
    }

    /// To begin an election, a follower increments its current term and
    /// transitions to candidate state
    fn upgrade_to_candidate(&self) -> Result<(), watch::error::SendError<ServerState>> {
        if *self.state.borrow() != ServerState::Candidate {
            naive_logging::log(&self.id, "upgrading to candidate...");
            return self.state_tx.send(ServerState::Candidate);
        }

        Ok(())
    }

    /// A candidate wins an election if it receives votes from a majority
    /// of the servers in the full cluster for the same term. Once a
    /// candidate wins an election, it becomes leader.
    fn upgrade_to_leader(&self) -> Result<(), watch::error::SendError<ServerState>> {
        if *self.state.borrow() != ServerState::Leader {
            naive_logging::log(&self.id, "upgrading to leader...");
            return self.state_tx.send(ServerState::Leader);
        }

        Ok(())
    }
}

impl cluster_node::ClusterNode for Server {
    async fn run(&self) -> Result<domain::node_id::NodeId, ClusterNodeError> {
        naive_logging::log(&self.id, "running...");

        Ok(self.id)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        naive_logging::log(&self.id, "Shutting down...");
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::*;

    const TEST_CHANNEL_CAPACITY: usize = 16;

    // Actor model:
    // Has a mailbox into which it can ALWAYS receive messages
    // Has an address book (i.e. is aware of peers)
    // Can be contacted by an external client

    // When in a leader state:
    // Sends heartbeat messages in parallel to its peers
    // Tracks volatile state per-peer, updating it per-heartbeat

    // When in a follower state:
    // Upgrades to a candidate state after a period of time passes without any messages received

    // When in a candidate state:
    // Increments its "term" and sends messages in parallel to its peers
    // Tallies votes each election and anticipates a majority of votes
    // Restarts election (incremented term, new random timeout) if it does not win the election
    // Reverts to follower if it receives indication another node has won
}
