mod actors;
mod handle;
mod log;
mod peer_list;
mod receiver;
mod request;

pub use handle::ServerHandle;
pub use log::ServerLogEntry;
pub use peer_list::ServerPeerList;
pub use request::{
    Message, ServerRequest, ServerRequestBody, ServerRequestHeaders, ServerResponse,
    ServerResponseBody, ServerResponseHeaders,
};

use std::sync::RwLock;
use tokio::{
    sync::{mpsc, watch},
    time,
};
use tokio_util::sync::CancellationToken;

use crate::domain::node_id;
use crate::{
    domain, message, naive_logging, state_machine,
    timeout::{self, TimeoutRange},
};

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
    #[allow(unused)]
    state_machine: state_machine::InMemoryStateMachine,

    // Persistent state:
    // -----------------------------------------------------
    /// Each server stores a current term number, which increases
    /// monotonically over time.
    current_term: RwLock<usize>,
    voted_for: RwLock<Option<node_id::NodeId>>,

    // Volatile state:
    // -----------------------------------------------------

    // Cluster configuration:
    // -----------------------------------------------------
    heartbeat_interval: time::Duration,
    election_timeout_range: timeout::TimeoutRange,
    peer_list: peer_list::ServerPeerList,
}

/// Raft servers communicate using remote procedure calls (RPCs), and the basic
/// consensus algorithm requires only two types of RPCs:
/// - `RequestVote`
/// - `AppendEntries`
impl Server {
    const DEFAULT_MESSAGE_BUFFER_SIZE: usize = 32;
    const DEFAULT_HEARTBEAT_INTERVAL: time::Duration = time::Duration::from_millis(374);
    const DEFAULT_ELECTION_TIMEOUT_RANGE: TimeoutRange = TimeoutRange::new(750, 1000);

    pub fn new(id: node_id::NodeId, mut peer_list: peer_list::ServerPeerList) -> Self {
        naive_logging::log(&id, "initialised.");

        peer_list.remove(&id);

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
            current_term: RwLock::new(0),
            voted_for: RwLock::new(None),

            // Volatile state:
            // -----------------------------------------------------

            // Cluster configuration:
            // -----------------------------------------------------
            heartbeat_interval: Self::DEFAULT_HEARTBEAT_INTERVAL,
            election_timeout_range: Self::DEFAULT_ELECTION_TIMEOUT_RANGE,
            peer_list,
        }
    }

    pub fn heartbeat_interval_ms(mut self, interval_ms: u64) -> Self {
        self.heartbeat_interval = time::Duration::from_millis(interval_ms);
        self
    }

    pub fn election_timeout_range(mut self, min: u64, max: u64) -> Self {
        assert!(min < max);
        self.election_timeout_range = TimeoutRange::new(min, max);
        self
    }

    pub fn generate_random_timeout(&self) -> time::Duration {
        self.election_timeout_range.random()
    }

    pub fn current_term(&self) -> usize {
        match self.current_term.read() {
            Ok(val) => *val,
            Err(err) => panic!("failed to read current_term: {err:?}"),
        }
    }

    pub fn set_current_term(&self, setter: impl FnOnce(usize) -> usize) {
        match self.current_term.write() {
            Ok(mut val) => *val = setter(*val),
            Err(err) => panic!("failed to modify current_term: {err:?}"),
        }
    }

    pub fn voted_for(&self) -> Option<node_id::NodeId> {
        match self.voted_for.read() {
            Ok(val) => *val,
            Err(err) => panic!("failed to read voted_for: {err:?}"),
        }
    }

    pub fn set_voted_for(&self, candidate_id: Option<node_id::NodeId>) {
        match self.voted_for.write() {
            Ok(mut val) => *val = candidate_id,
            Err(err) => panic!("failed to modify voted_for: {err:?}"),
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

impl Server {
    pub async fn run(
        &self,
        rx: mpsc::Receiver<message::Message>,
    ) -> anyhow::Result<domain::node_id::NodeId> {
        naive_logging::log(&self.id, "running...");

        let cancel_tok = CancellationToken::new();

        let (follower_tx, follower_rx) = mpsc::channel(Self::DEFAULT_MESSAGE_BUFFER_SIZE);
        let (candidate_tx, candidate_rx) = mpsc::channel(Self::DEFAULT_MESSAGE_BUFFER_SIZE);
        let (leader_tx, leader_rx) = mpsc::channel(Self::DEFAULT_MESSAGE_BUFFER_SIZE);
        let (client_request_tx, client_request_rx) =
            mpsc::channel(Self::DEFAULT_MESSAGE_BUFFER_SIZE);

        tokio::try_join!(
            actors::run_root_actor(
                self,
                rx,
                follower_tx,
                candidate_tx,
                leader_tx,
                client_request_tx
            ),
            actors::run_follower_actor(self, follower_rx, cancel_tok.clone()),
            actors::run_candidate_actor(self, candidate_rx, cancel_tok.clone()),
            actors::run_leader_actor(self, leader_rx, cancel_tok.clone()),
            actors::run_client_request_actor(self, client_request_rx, cancel_tok.clone())
        )?;

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
    // use tokio::sync::mpsc;

    // use super::*;

    // const TEST_CHANNEL_CAPACITY: usize = 16;

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
