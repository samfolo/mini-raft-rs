mod routines;
mod rpc;

pub use rpc::ServerRequest;

use std::sync::RwLock;
use tokio::{
    sync::{broadcast, watch},
    time,
};

use crate::cluster_node::error::ClusterNodeError;
use crate::{cluster_node, domain, naive_logging, timeout};

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

    // Persistent state:
    // -----------------------------------------------------
    current_term: RwLock<usize>,
    voted_for: RwLock<Option<domain::node_id::NodeId>>,

    // Cluster configuration:
    // -----------------------------------------------------
    cluster_node_count: watch::Receiver<u64>,
    publisher: broadcast::Sender<rpc::ServerRequest>,
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
        publisher: broadcast::Sender<rpc::ServerRequest>,
        heartbeat_interval: time::Duration,
        election_timeout_range: timeout::TimeoutRange,
        cluster_node_count: watch::Receiver<u64>,
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

            // Persistent state:
            // -----------------------------------------------------
            current_term: RwLock::new(0),
            voted_for: RwLock::new(None),

            // Cluster configuration:
            // -----------------------------------------------------
            cluster_node_count,
            publisher,
            heartbeat_interval,
            election_timeout_range,
        }
    }

    fn current_term(&self) -> usize {
        match self.current_term.read() {
            Ok(res) => *res,
            Err(err) => panic!("failed to read current_term: {err:?}"),
        }
    }

    fn set_current_term(&self, candidate_id_setter: impl Fn(usize) -> usize) {
        match self.current_term.write() {
            Ok(mut res) => {
                *res = candidate_id_setter(*res);
            }
            Err(err) => panic!("failed to modify current_term: {err:?}"),
        };
    }

    /// Current terms are exchanged whenever Servers communicate; if
    /// one Server’s current term is smaller than the other’s, then it updates
    /// its current term to the larger value.
    async fn sync_term(
        &self,
        other_server_term: usize,
    ) -> Result<(), watch::error::SendError<ServerState>> {
        if self.current_term() < other_server_term {
            self.set_current_term(|_| other_server_term);
            return self.downgrade_to_follower();
        }

        Ok(())
    }

    fn cluster_node_count(&self) -> u64 {
        *self.cluster_node_count.borrow()
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

    fn voted_for(&self) -> Option<domain::node_id::NodeId> {
        match self.voted_for.read() {
            Ok(res) => *res,
            Err(err) => panic!("failed to read voted_for: {err:?}"),
        }
    }

    fn vote(&self, candidate_id: Option<domain::node_id::NodeId>) {
        match self.voted_for.write() {
            Ok(mut res) => {
                *res = candidate_id;
            }
            Err(err) => panic!("failed to modify voted_for: {err:?}"),
        };
    }

    /// The leader handles all client requests; if a client contacts a follower, the
    /// follower redirects it to the leader.
    #[allow(unused)]
    async fn handle_client_request(&self, _: ()) -> anyhow::Result<()> {
        todo!("unimplemented")
    }
}

impl cluster_node::ClusterNode for Server {
    async fn run(&self) -> Result<domain::node_id::NodeId, ClusterNodeError> {
        naive_logging::log(&self.id, "running...");

        match tokio::try_join!(
            self.run_election_routine(),
            self.run_heartbeat_routine(),
            self.run_base_routine()
        ) {
            Ok(res) => {
                println!("{res:#?}");
            }
            Err(err) => {
                println!("{err:#?}");
            }
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
    use super::*;

    const TEST_CHANNEL_CAPACITY: usize = 16;

    #[test]
    fn starts() -> anyhow::Result<()> {
        let (publisher, _subscriber) = broadcast::channel(TEST_CHANNEL_CAPACITY);
        let (_, cluster_node_count) = watch::channel(1);

        let _ = Server::new(
            publisher,
            time::Duration::from_millis(5),
            timeout::TimeoutRange::new(10, 20),
            cluster_node_count,
        );

        Ok(())
    }
}
