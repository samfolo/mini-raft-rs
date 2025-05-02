#![allow(unused)] // TODO: use unused... need to refactor something first.

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

use futures_util::StreamExt;
use std::sync::RwLock;
use tokio::{
    sync::{broadcast, mpsc, watch},
    time,
};
use tokio_util::sync::CancellationToken;

use crate::domain::listener;
use crate::{
    client, domain, message, naive_logging, state_machine,
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
    state_machine: state_machine::InMemoryStateMachine,

    // Persistent state:
    // -----------------------------------------------------
    current_term: usize,

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

    pub fn new(peer_list: peer_list::ServerPeerList) -> Self {
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
            current_term: 0,

            // Volatile state:
            // -----------------------------------------------------

            // Cluster configuration:
            // -----------------------------------------------------
            heartbeat_interval: time::Duration::from_millis(500),
            election_timeout_range: TimeoutRange::new(500, 1000),
            peer_list,
        }
    }

    pub fn generate_random_timeout(&self) -> time::Duration {
        self.election_timeout_range.random()
    }

    pub fn current_term(&self) -> usize {
        self.current_term
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

        let mut receiver = receiver::ServerReceiver::new(rx);

        let stream = async_stream::stream! {
            while let Some(item) = receiver.recv().await {
                yield item;
            }
        };
        futures_util::pin_mut!(stream);

        let cancellation_token = CancellationToken::new();
        let follower_cancellation_token = cancellation_token.clone();

        // TEST THIS
        cancellation_token.drop_guard();

        let (follower_tx, follower_rx) = mpsc::channel(32);
        tokio::join!(actors::run_follower_actor(
            self,
            follower_rx,
            follower_cancellation_token
        ));

        while let Some(message) = stream.next().await {
            println!("{message:?}");
        }

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
