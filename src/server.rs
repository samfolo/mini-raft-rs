pub mod rpc;

use rpc::ServerResponse;
use std::pin;
use std::sync::RwLock;
use tokio::sync::mpsc;
use tokio::time::error::Elapsed;
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
        naive_logging::log(id, "initialised.");

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
            naive_logging::log(self.id, "downgrading to follower...");
            return self.state_tx.send(ServerState::Follower);
        }

        Ok(())
    }

    /// To begin an election, a follower increments its current term and
    /// transitions to candidate state
    fn upgrade_to_candidate(&self) -> Result<(), watch::error::SendError<ServerState>> {
        if *self.state.borrow() != ServerState::Candidate {
            naive_logging::log(self.id, "upgrading to candidate...");
            return self.state_tx.send(ServerState::Candidate);
        }

        Ok(())
    }

    /// A candidate wins an election if it receives votes from a majority
    /// of the servers in the full cluster for the same term. Once a
    /// candidate wins an election, it becomes leader.
    fn upgrade_to_leader(&self) -> Result<(), watch::error::SendError<ServerState>> {
        if *self.state.borrow() != ServerState::Leader {
            naive_logging::log(self.id, "upgrading to leader...");
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

    /// `RequestVote` RPCs are initiated by candidates during elections.
    pub fn request_vote(
        &self,
    ) -> anyhow::Result<
        mpsc::Receiver<rpc::ServerResponse>,
        broadcast::error::SendError<rpc::ServerRequest>,
    > {
        let current_term = self.current_term();

        naive_logging::log(
            self.id,
            &format!(
                "-> REQUEST_VOTE {{ term: {}, candidate_id: {} }}",
                current_term, self.id
            ),
        );

        let (responder, receiver) = mpsc::channel(Self::DEFAULT_MESSAGE_BUFFER_SIZE);

        self.publisher.send(rpc::ServerRequest::new(
            current_term,
            responder,
            rpc::RequestBody::RequestVote {
                candidate_id: self.id,
            },
        ))?;

        Ok(receiver)
    }

    /// `AppendEntries` RPCs are initiated by leaders to replicate log entries
    /// and to provide a form of heartbeat.
    pub fn append_entries(
        &self,
        entries: Vec<String>,
    ) -> anyhow::Result<mpsc::Receiver<rpc::ServerResponse>> {
        naive_logging::log(
            self.id,
            &format!(
                "-> APPEND_ENTRIES {{ term: {}, leader_id: {}, entries: {:?} }}",
                self.current_term(),
                self.id,
                entries
            ),
        );

        let (responder, receiver) = mpsc::channel(Self::DEFAULT_MESSAGE_BUFFER_SIZE);

        self.publisher.send(rpc::ServerRequest::new(
            self.current_term(),
            responder,
            rpc::RequestBody::AppendEntries {
                leader_id: self.id,
                entries,
            },
        ))?;

        Ok(receiver)
    }

    /// The leader handles all client requests; if a client contacts a follower, the
    /// follower redirects it to the leader.
    #[allow(unused)]
    async fn handle_client_request(&self, _: ()) -> anyhow::Result<()> {
        todo!("unimplemented")
    }

    async fn run_heartbeat_routine(&self) -> cluster_node::Result<()> {
        let id = self.id;
        let mut state = self.state_tx.subscribe();

        let timeout = time::sleep(self.heartbeat_interval);
        tokio::pin!(timeout);

        let reset_timeout = |timeout: &mut pin::Pin<&mut time::Sleep>| {
            timeout
                .as_mut()
                .reset(time::Instant::now() + self.heartbeat_interval)
        };

        loop {
            tokio::select! {
                _ = &mut timeout => {
                    if *state.borrow() == ServerState::Leader {
                        if let Err(err) = self.append_entries(vec![]) {
                            return Err(ClusterNodeError::Heartbeat(id, err))
                        }
                    }

                    reset_timeout(&mut timeout);
                },
                res = state.changed() => {
                    if let Err(err) = res {
                        return Err(ClusterNodeError::Unexpected(err.into()));
                    }

                    if *state.borrow() == ServerState::Leader {
                        timeout.as_mut().reset(time::Instant::now());
                    }
                }
            }
        }
    }

    async fn run_election_routine(&self) -> cluster_node::Result<()> {
        let id = self.id;
        let mut state = self.state_tx.subscribe();

        loop {
            if *state.borrow() == ServerState::Candidate {
                'election_loop: loop {
                    self.set_current_term(|prev| prev + 1);

                    match self.request_vote() {
                        Ok(mut response) => {
                            let timeout = self.election_timeout_range.random();

                            if let Err(err) = self
                                .handle_request_vote_response(timeout, &mut response)
                                .await
                            {
                                match err {
                                    ClusterNodeError::Timeout(_) => {
                                        naive_logging::log(
                                            id,
                                            "election ended before enough votes were received.",
                                        );
                                        naive_logging::log(id, "restarting election...");
                                    }
                                    other_err => return Err(other_err),
                                }
                            } else {
                                break 'election_loop;
                            }
                        }
                        Err(err) => {
                            return Err(ClusterNodeError::OutgoingClusterConnection(id, err));
                        }
                    }
                }
            } else if let Err(err) = state.changed().await {
                return Err(ClusterNodeError::Unexpected(err.into()));
            }
        }
    }

    async fn handle_request_vote_response(
        &self,
        timeout: time::Duration,
        response: &mut mpsc::Receiver<rpc::ServerResponse>,
    ) -> cluster_node::Result<()> {
        let current_term = self.current_term();
        let current_cluster_node_count = self.cluster_node_count();

        let mut total_votes_over_term = 0u64;

        loop {
            match time::timeout(timeout, response.recv()).await {
                Ok(Some(res)) => match res.body() {
                    rpc::ResponseBody::RequestVote { vote_granted } => {
                        if *vote_granted {
                            naive_logging::log(self.id, "received vote for this term");
                            total_votes_over_term += 1;

                            if total_votes_over_term * 2 > current_cluster_node_count {
                                naive_logging::log(
                                    self.id,
                                    &format!("won election for term {current_term}"),
                                );

                                if let Err(err) = self.upgrade_to_leader() {
                                    return Err(ClusterNodeError::Unexpected(err.into()));
                                }

                                return Ok(());
                            }
                        } else if let Err(err) = self.sync_term(res.term()).await {
                            return Err(ClusterNodeError::Unexpected(err.into()));
                        }
                    }
                    rpc::ResponseBody::AppendEntries { .. } => {
                        return Err(ClusterNodeError::Unexpected(anyhow::anyhow!(
                            "invalid response [AppendEntries] to RequestVote RPC"
                        )));
                    }
                },
                Ok(None) => continue,
                Err(err) => return Err(ClusterNodeError::Timeout(err)),
            }
        }
    }

    async fn run_base_routine(&self) -> cluster_node::Result<()> {
        let id = self.id;
        let state = self.state_tx.subscribe();
        let mut subscriber = self.publisher.subscribe();

        let timeout = time::sleep(self.election_timeout_range.random());
        tokio::pin!(timeout);

        let reset_timeout = |timeout: &mut pin::Pin<&mut time::Sleep>| {
            timeout
                .as_mut()
                .reset(time::Instant::now() + self.election_timeout_range.random())
        };

        loop {
            tokio::select! {
                _ = &mut timeout => {
                    if *state.borrow() == ServerState::Follower {
                        naive_logging::log(id, "timed out waiting for a response...");
                        naive_logging::log(id, "starting election...");

                        self.vote(Some(id));

                        if let Err(err) = self.upgrade_to_candidate() {
                            return Err(ClusterNodeError::Unexpected(err.into()));
                        }
                    }
                },
                res = subscriber.recv() => {
                    match res {
                        Ok(request) => {
                            match request.body() {
                                rpc::RequestBody::AppendEntries { leader_id, entries } => {
                                    if id != *leader_id {
                                        naive_logging::log(
                                            self.id,
                                            &format!(
                                                "<- APPEND_ENTRIES {{ term: {}, leader_id: {}, entries: {:?} }}",
                                                request.term(), leader_id, entries
                                            ),
                                        );

                                        let current_term =  self.current_term();

                                        let is_stale_request = request.term() < current_term;

                                        if is_stale_request {
                                            naive_logging::log(self.id, &format!("rejecting request from stale leader {leader_id}"));
                                        } else {
                                            // Received request from leader; reset the election timeout:
                                            reset_timeout(&mut timeout);

                                            if self.voted_for().is_none_or(|id| id != *leader_id) {
                                                naive_logging::log(self.id, &format!("acknowledging new leader {leader_id}"));
                                                self.vote(Some(*leader_id));
                                            }
                                        }

                                        if request.can_respond() {
                                            if let Err(err) = request.respond(
                                                current_term,
                                                rpc::ResponseBody::AppendEntries { },
                                            ).await {
                                                return Err(ClusterNodeError::Unexpected(err.into()));
                                            }
                                        }

                                        if let Err(err) = self.sync_term(request.term()).await {
                                            return Err(ClusterNodeError::Unexpected(err.into()));
                                        }
                                    }
                                }
                                rpc::RequestBody::RequestVote { candidate_id, .. } => {
                                    if id != *candidate_id {
                                        // Received request from candidate:
                                        naive_logging::log(
                                            self.id,
                                            &format!(
                                                "<- REQUEST_VOTE {{ term: {}, candidate_id: {} }}",
                                                request.term(),
                                                candidate_id,
                                            ),
                                        );

                                        let current_term = self.current_term();

                                        let vote_granted = request.term() >= current_term;

                                        if vote_granted {
                                            reset_timeout(&mut timeout);
                                            naive_logging::log(self.id, &format!("granting vote to candidate {candidate_id}"));
                                            self.vote(Some(*candidate_id));
                                        } else {
                                            naive_logging::log(self.id, &format!("refusing vote for candidate {candidate_id}: term={current_term}, req_term={}", request.term()));
                                        }

                                        if request.can_respond() {
                                            if let Err(err) = request.respond(
                                                current_term,
                                                rpc::ResponseBody::RequestVote { vote_granted },
                                            ).await {
                                                return Err(ClusterNodeError::Unexpected(err.into()));
                                            }
                                        }

                                        if let Err(err) = self.sync_term(request.term()).await {
                                            return Err(ClusterNodeError::Unexpected(err.into()));
                                        }
                                    }
                                }
                            }
                        },
                        // Tracing would be nice here..
                        Err(err) => return Err(ClusterNodeError::IncomingClusterConnection(id, err))
                    }
                }
            }
        }
    }
}

impl cluster_node::ClusterNode for Server {
    async fn run(&self) -> Result<domain::node_id::NodeId, ClusterNodeError> {
        naive_logging::log(self.id, "running...");

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
        naive_logging::log(self.id, "Shutting down...");
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
