mod cluster_connection;
pub mod rpc;

use std::pin;
use std::sync::{Arc, RwLock};
use tokio::sync::Mutex;
use tokio::{
    sync::{broadcast, watch},
    time,
};

use crate::cluster_node::error::ClusterNodeError;
use crate::{cluster_node, naive_logging, timeout};

/// At any given time each server is in one of three states:
/// leader, follower, or candidate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ServerMode {
    Leader,
    Follower,
    Candidate,
}

/// A Server handles requests from Clients. Within the same Cluster,
/// Only one server can be the Leader at any one time.
pub struct Server {
    id: uuid::Uuid,
    mode: watch::Receiver<ServerMode>,
    mode_tx: watch::Sender<ServerMode>,
    publisher: broadcast::Sender<rpc::ServerRequest>,
    cluster_conn: Arc<Mutex<cluster_connection::ClusterConnection>>,
    timeout_range: timeout::TimeoutRange,
    voted_for: RwLock<Option<uuid::Uuid>>,
}

/// Raft servers communicate using remote procedure calls (RPCs), and the basic
/// consensus algorithm requires only two types of RPCs:
/// - `RequestVote`
/// - `AppendEntries`
impl Server {
    pub fn new(
        publisher: broadcast::Sender<rpc::ServerRequest>,
        timeout_range: timeout::TimeoutRange,
        cluster_node_count: watch::Receiver<u64>,
    ) -> Self {
        let id = uuid::Uuid::new_v4();
        naive_logging::log(id, "initialised.");

        let (mode_tx, mode) = watch::channel(ServerMode::Follower);

        let cluster_conn =
            cluster_connection::ClusterConnection::new(id, publisher.clone(), cluster_node_count);
        let cluster_conn = Arc::new(Mutex::new(cluster_conn));

        Self {
            id,
            mode,
            mode_tx,
            publisher,
            cluster_conn,
            timeout_range,
            voted_for: RwLock::new(None),
        }
    }

    /// Current terms are exchanged whenever Servers communicate; if
    /// one Server’s current term is smaller than the other’s, then it updates
    /// its current term to the larger value.
    async fn sync_term(
        &self,
        other_server_term: usize,
    ) -> Result<(), watch::error::SendError<ServerMode>> {
        let effect = { self.cluster_conn.lock().await.sync_term(other_server_term) };
        if let cluster_connection::SyncTermSideEffect::Downgrade = effect {
            return self.downgrade_to_follower();
        }

        Ok(())
    }

    /// If a candidate or leader discovers that its term is out of date, it
    /// immediately reverts to follower mode.
    fn downgrade_to_follower(&self) -> Result<(), watch::error::SendError<ServerMode>> {
        if *self.mode.borrow() != ServerMode::Follower {
            naive_logging::log(self.id, "downgrading to follower...");
            return self.mode_tx.send(ServerMode::Follower);
        }

        Ok(())
    }

    /// To begin an election, a follower increments its current term and
    /// transitions to candidate mode
    fn upgrade_to_candidate(&self) -> Result<(), watch::error::SendError<ServerMode>> {
        if *self.mode.borrow() != ServerMode::Candidate {
            naive_logging::log(self.id, "upgrading to candidate...");
            return self.mode_tx.send(ServerMode::Candidate);
        }

        Ok(())
    }

    /// A candidate wins an election if it receives votes from a majority
    /// of the servers in the full cluster for the same term. Once a
    /// candidate wins an election, it becomes leader.
    fn upgrade_to_leader(&self) -> Result<(), watch::error::SendError<ServerMode>> {
        if *self.mode.borrow() != ServerMode::Leader {
            naive_logging::log(self.id, "upgrading to leader...");
            return self.mode_tx.send(ServerMode::Leader);
        }

        Ok(())
    }

    fn vote(&self, candidate_id: Option<uuid::Uuid>) {
        match self.voted_for.write() {
            Ok(mut res) => {
                *res = candidate_id;
            }
            Err(err) => panic!("{err:?}"),
        };
    }

    /// The leader handles all client requests; if a client contacts a follower, the
    /// follower redirects it to the leader.
    #[allow(unused)]
    async fn handle_client_request(&self, _: ()) -> anyhow::Result<()> {
        todo!("unimplemented")
    }

    async fn run_heartbeat_routine(&self) -> cluster_node::Result<()> {
        let id = self.id;
        let mut mode = self.mode_tx.subscribe();
        let timeout_range = self.timeout_range.clone();

        let timeout = time::sleep(timeout_range.random());
        tokio::pin!(timeout);

        let reset_timeout = |timeout: &mut pin::Pin<&mut time::Sleep>| {
            timeout
                .as_mut()
                .reset(time::Instant::now() + timeout_range.random())
        };

        loop {
            tokio::select! {
                _ = &mut timeout => {
                    if *mode.borrow() == ServerMode::Leader {
                        if let Err(err) = {
                            let conn = self.cluster_conn.lock().await;
                            conn.append_entries(vec![])
                        } {
                            return Err(ClusterNodeError::Heartbeat(id, err))
                        }
                    }

                    reset_timeout(&mut timeout);
                },
                res = mode.changed() => {
                    if let Err(err) = res {
                        return Err(ClusterNodeError::Unexpected(err.into()));
                    }

                    if *mode.borrow() == ServerMode::Leader {
                        timeout.as_mut().reset(time::Instant::now());
                    }
                }
            }
        }
    }

    async fn run_election_routine(&self) -> cluster_node::Result<()> {
        let id = self.id;
        let mut mode = self.mode_tx.subscribe();

        loop {
            if *mode.borrow() == ServerMode::Candidate {
                loop {
                    let request_vote_result = {
                        let mut conn = self.cluster_conn.lock().await;
                        conn.increment_term();
                        conn.request_vote()
                    };

                    match request_vote_result {
                        Ok(mut response) => {
                            let timeout = self.timeout_range.random();
                            let (current_term, current_cluster_node_count) = {
                                let conn = self.cluster_conn.lock().await;
                                (conn.current_term(), conn.cluster_node_count())
                            };

                            let mut total_votes_over_term = 0u64;

                            while let Ok(Some(res)) = time::timeout(timeout, response.recv()).await
                            {
                                match res.body() {
                                    rpc::ResponseBody::RequestVote { vote_granted } => {
                                        if *vote_granted {
                                            naive_logging::log(id, "received vote for this term");
                                            total_votes_over_term += 1;
                                            if total_votes_over_term * 2
                                                > current_cluster_node_count
                                            {
                                                naive_logging::log(
                                                    id,
                                                    &format!(
                                                        "won election for term {current_term}"
                                                    ),
                                                );

                                                if let Err(err) = self.upgrade_to_leader() {
                                                    return Err(ClusterNodeError::Unexpected(
                                                        err.into(),
                                                    ));
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
                                }
                            }

                            naive_logging::log(
                                id,
                                "election ended before enough votes were received.",
                            );
                            naive_logging::log(id, "restarting election...");
                        }
                        Err(err) => {
                            return Err(ClusterNodeError::OutgoingClusterConnection(id, err));
                        }
                    }
                }
            } else if let Err(err) = mode.changed().await {
                return Err(ClusterNodeError::Unexpected(err.into()));
            }
        }
    }

    async fn run_base_routine(&self) -> cluster_node::Result<()> {
        let id = self.id;
        let mode = self.mode_tx.subscribe();
        let mut subscriber = self.publisher.subscribe();

        let timeout = time::sleep(self.timeout_range.random());
        tokio::pin!(timeout);

        let reset_timeout = |timeout: &mut pin::Pin<&mut time::Sleep>| {
            timeout
                .as_mut()
                .reset(time::Instant::now() + self.timeout_range.random())
        };

        loop {
            tokio::select! {
                _ = &mut timeout => {
                    if *mode.borrow() == ServerMode::Follower {
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
                                rpc::RequestBody::AppendEntries { leader_id, .. } => {
                                    if id != *leader_id {
                                        naive_logging::log(self.id, &format!("received APPEND_ENTRIES from {leader_id}"));

                                        let current_term = {
                                            let conn = self.cluster_conn.lock().await;
                                            conn.current_term()
                                        };

                                        let is_stale_request = request.term() < current_term;
                                        let previously_voted_for_leader = {
                                            match self.voted_for.read() {
                                                Ok(vote) => vote.is_some_and(|id| id == *leader_id),
                                                Err(err) => return Err(ClusterNodeError::Unexpected(anyhow::anyhow!(
                                                    "failed to read current vote: {:?}", err
                                                )))
                                            }
                                        };

                                        if is_stale_request {
                                            naive_logging::log(self.id, &format!("rejecting request from stale leader {leader_id}"));
                                        } else {
                                            // Received request from leader; reset the election timeout:
                                            reset_timeout(&mut timeout);

                                            if !previously_voted_for_leader {
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

                                        if let Err(err) =self.sync_term(request.term()).await {
                                            return Err(ClusterNodeError::Unexpected(err.into()));
                                        }
                                    }
                                }
                                rpc::RequestBody::RequestVote { candidate_id, .. } => {
                                    if id != *candidate_id {
                                        // Received request from candidate:
                                        naive_logging::log(self.id, &format!("received REQUEST_VOTE from {} with term {}", candidate_id, request.term()));

                                        let current_term = {
                                            let conn = self.cluster_conn.lock().await;
                                            conn.current_term()
                                        };

                                        let is_currently_leader = *mode.borrow() == ServerMode::Leader;

                                        let should_grant_vote = if is_currently_leader {
                                            naive_logging::log(self.id, &format!("leader? {}, req? {} <=> ", current_term, request.term()));
                                            request.term() > current_term
                                        } else {
                                            request.term() >= current_term
                                        };

                                        if should_grant_vote {
                                            reset_timeout(&mut timeout);

                                            naive_logging::log(self.id, &format!("granting vote to candidate {candidate_id}"));
                                            self.vote(Some(*candidate_id));
                                        } else {
                                            naive_logging::log(self.id, &format!("refusing vote for candidate {candidate_id}: term={current_term}, req_term={}", request.term()));
                                        }

                                        if request.can_respond() {
                                            if let Err(err) = request.respond(
                                                current_term,
                                                rpc::ResponseBody::RequestVote { vote_granted: should_grant_vote },
                                            ).await {
                                                naive_logging::log(self.id, &format!("failed to respond to {candidate_id}: {err}"));
                                                continue;
                                            }
                                        }

                                        if let Err(err) =self.sync_term(request.term()).await {
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
    async fn run(&self) -> Result<uuid::Uuid, ClusterNodeError> {
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

    const TEST_CANNEL_CAPACITY: usize = 16;

    #[test]
    fn starts() -> anyhow::Result<()> {
        let (publisher, _subscriber) = broadcast::channel(TEST_CANNEL_CAPACITY);
        let (_, cluster_node_count) = watch::channel(1);

        let _ = Server::new(
            publisher,
            timeout::TimeoutRange::new(10, 20),
            cluster_node_count,
        );

        Ok(())
    }
}
