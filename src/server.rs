mod cluster_connection;
pub mod rpc;

use std::pin;
use std::sync::{Arc, RwLock};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::time::Sleep;
use tokio::{
    sync::{broadcast, watch},
    task::JoinHandle,
    time,
};

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
        subscriber: broadcast::Receiver<rpc::ServerRequest>,
        timeout_range: timeout::TimeoutRange,
        cluster_node_count: watch::Receiver<u64>,
    ) -> Self {
        let id = uuid::Uuid::new_v4();
        naive_logging::log(id, "initialised.");

        let (mode_tx, mode) = watch::channel(ServerMode::Follower);

        let cluster_conn = cluster_connection::ClusterConnection::new(
            id,
            publisher,
            subscriber,
            cluster_node_count,
        );
        let cluster_conn = Arc::new(Mutex::new(cluster_conn));

        Self {
            id,
            mode,
            mode_tx,
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
        request: &rpc::ServerRequest,
    ) -> Result<(), watch::error::SendError<ServerMode>> {
        let effect = {
            println!("locked in sync_term");
            self.cluster_conn.lock().await.sync_term(request.term())
        };
        println!("I GET HURR");
        if let cluster_connection::SyncTermSideEffect::Downgrade = effect {
            return self.downgrade_to_follower();
        }

        Ok(())
    }

    /// If a candidate or leader discovers that its term is out of date, it
    /// immediately reverts to follower mode.
    fn downgrade_to_follower(&self) -> Result<(), watch::error::SendError<ServerMode>> {
        if *self.mode.borrow() != ServerMode::Follower {
            return self.mode_tx.send(ServerMode::Follower);
        }

        Ok(())
    }

    /// To begin an election, a follower increments its current term and
    /// transitions to candidate mode
    fn upgrade_to_candidate(&self) -> Result<(), watch::error::SendError<ServerMode>> {
        if *self.mode.borrow() != ServerMode::Candidate {
            return self.mode_tx.send(ServerMode::Candidate);
        }

        Ok(())
    }

    fn vote(&self, candidate_id: uuid::Uuid) {
        match self.voted_for.write() {
            Ok(mut res) => {
                *res = Some(candidate_id);
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

    async fn run_heartbeat_routine(&self) -> JoinHandle<cluster_node::Result<()>> {
        // TODO: return errors in parallel threads back to main thread.

        let id = self.id;
        let mut mode = self.mode_tx.subscribe();
        let cluster_conn = Arc::clone(&self.cluster_conn);
        let timeout_range = self.timeout_range.clone();

        tokio::spawn(async move {
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
                                let conn = cluster_conn.lock().await;
                                conn.append_entries(vec![]).await
                            } {
                                return Err(cluster_node::ClusterNodeError::HeartbeatError(
                                    id,
                                    err.into(),
                                ))
                            }
                        }

                        reset_timeout(&mut timeout);
                    },
                    res = mode.changed() => {
                        if let Err(err) = res {
                            return Err(cluster_node::ClusterNodeError::UnexpectedError(
                                err.into(),
                            ));
                        }

                        if *mode.borrow() == ServerMode::Leader {
                            timeout.as_mut().reset(time::Instant::now());
                        }
                    }
                }
            }
        })
    }

    async fn run_election_routine(&self) -> JoinHandle<cluster_node::Result<()>> {
        // TODO: return errors in parallel threads back to main thread.

        let id = self.id;
        let mode_tx = self.mode_tx.clone();
        let mut req_mode = self.mode_tx.subscribe();

        let cluster_conn = Arc::clone(&self.cluster_conn);
        let timeout_range = self.timeout_range.clone();

        tokio::spawn(async move {
            let timeout = time::sleep(timeout_range.random());
            tokio::pin!(timeout);

            let reset_timeout = |timeout: &mut pin::Pin<&mut time::Sleep>| {
                timeout
                    .as_mut()
                    .reset(time::Instant::now() + timeout_range.random())
            };

            loop {
                let (cancel_election, cancel_election_rx) = mpsc::channel(1);

                tokio::select! {
                    _ = &mut timeout => {
                        if let Err(err) = cancel_election.send(()).await {
                            return Err(cluster_node::ClusterNodeError::UnexpectedError(err.into()));
                        }

                        if *req_mode.borrow() == ServerMode::Candidate {
                            let request_vote_result = {
                                let mut conn = cluster_conn.lock().await;
                                conn.increment_term();
                                conn.request_vote().await
                            };

                            match request_vote_result {
                                Ok(request_vote_rx) => {
                                    let mode = mode_tx.subscribe();
                                    let (current_term, current_cluster_node_count) = {
                                        let conn = cluster_conn.lock().await;
                                        (conn.current_term(), conn.cluster_node_count())
                                    };

                                    // TODO: tally votes in this method and trigger mode changes:
                                    Self::handle_election_response(
                                        id,
                                        current_term,
                                        current_cluster_node_count,
                                        mode,
                                        request_vote_rx,
                                        cancel_election_rx,
                                    );
                                },
                                Err(err) => {
                                    return Err(cluster_node::ClusterNodeError::UnexpectedError(err.into()));
                                }
                            };
                        }

                        reset_timeout(&mut timeout);
                    },
                    res = req_mode.changed() => {
                        if let Err(err) = res {
                            return Err(cluster_node::ClusterNodeError::HeartbeatError(
                                id,
                                err.into(),
                            ));
                        }

                        if *req_mode.borrow() == ServerMode::Leader {
                            timeout.as_mut().reset(time::Instant::now());
                        }
                    }
                }
            }
        })
    }

    fn handle_election_response(
        id: uuid::Uuid,
        current_term: usize,
        initial_node_count: u64,
        mut mode: watch::Receiver<ServerMode>,
        mut request_vote_rx: mpsc::Receiver<rpc::ServerResponse>,
        mut cancel_election_rx: mpsc::Receiver<()>,
    ) -> JoinHandle<cluster_node::Result<()>> {
        // TODO: return errors in parallel threads back to main thread.

        tokio::spawn(async move {
            let mut total_votes_over_term = 0u64;

            loop {
                tokio::select! {
                    Some(res) = request_vote_rx.recv() => {
                        match res.body() {
                            rpc::ResponseBody::RequestVote { vote_granted } => {
                                if *vote_granted {
                                    naive_logging::log(
                                        id,
                                        &format!("received vote for this term"),
                                    );
                                    total_votes_over_term += 1;
                                    if total_votes_over_term * 2 > initial_node_count {
                                        naive_logging::log(
                                            id,
                                            &format!("won election for term {current_term}"),
                                        );
                                    }

                                    if total_votes_over_term == initial_node_count {
                                        return Ok(());
                                    }
                                }
                            }
                            rpc::ResponseBody::AppendEntries { .. } => {
                                return Err(cluster_node::ClusterNodeError::UnexpectedError(
                                    anyhow::anyhow!(
                                        "invalid response [AppendEntries] to RequestVote RPC"
                                    ),
                                ));
                            }
                        };
                    },
                    _ = cancel_election_rx.recv() => {
                        return Ok(());
                    },
                    res = mode.changed() => {
                        if let Err(err) = res {
                            // TODO: return errors in parallel threads back to main thread.
                            return Err(cluster_node::ClusterNodeError::UnexpectedError(
                                err.into(),
                            ));
                        }

                        return Ok(())
                    }
                }
            }
        })
    }
}

impl cluster_node::ClusterNode for Server {
    async fn run(&self) -> Result<uuid::Uuid, cluster_node::ClusterNodeError> {
        naive_logging::log(self.id, "running...");

        let server_id = self.id;
        let mut mode = self.mode.clone();
        let cluster_conn = self.cluster_conn.clone();

        let mut heartbeat_routine: Option<JoinHandle<cluster_node::Result<()>>> = None;
        let mut election_routine: Option<JoinHandle<cluster_node::Result<()>>> = None;

        let recv_timeout = time::sleep(self.timeout_range.random());
        tokio::pin!(recv_timeout);

        let reset_timeout = |timeout: &mut pin::Pin<&mut time::Sleep>| {
            timeout
                .as_mut()
                .reset(time::Instant::now() + self.timeout_range.random())
        };

        loop {
            println!("locked in top_level run loop");
            let mut conn = cluster_conn.lock().await;

            tokio::select! {
                _ = &mut recv_timeout => {
                    if *mode.borrow() != ServerMode::Candidate {
                        naive_logging::log(self.id, "timed out waiting for a response...");
                        naive_logging::log(self.id, "starting election...");

                        if let Some(routine) = heartbeat_routine.take() {
                            routine.abort();
                        }

                        if let Some(routine) = election_routine.take() {
                            routine.abort();
                        }

                        self.vote(self.id);

                        if let Err(err) = self.upgrade_to_candidate() {
                            return Err(cluster_node::ClusterNodeError::UnexpectedError(
                                err.into(),
                            ));
                        }
                    }

                    // Reset the election timeout:
                    reset_timeout(&mut recv_timeout);
                },
                res = conn.recv() => {
                    match res {
                        Ok(request) => {
                            match request.body() {
                                rpc::RequestBody::AppendEntries { leader_id, .. } => {
                                    if conn.node_id().ne(leader_id) {
                                        // Received request from leader; reset the election timeout:
                                        reset_timeout(&mut recv_timeout);
                                        naive_logging::log(self.id, &format!("received APPEND_ENTRIES from {leader_id}"));

                                        let is_stale_request = request.term() < conn.current_term();

                                        if is_stale_request {
                                            naive_logging::log(self.id, &format!("rejecting request from stale leader {leader_id}"));
                                        } else {
                                            naive_logging::log(self.id, &format!("acknowledging new leader {leader_id}, downgrading to follower..."));
                                        }

                                        if let Err(err) = request.respond(
                                            conn.current_term(),
                                            rpc::ResponseBody::AppendEntries { success: !is_stale_request },
                                        ).await {
                                            return Err(cluster_node::ClusterNodeError::UnexpectedError(
                                                err.into(),
                                            ));
                                        }

                                        if let cluster_connection::SyncTermSideEffect::Downgrade = conn.sync_term(request.term()) {
                                            if let Err(err) = self.downgrade_to_follower() {
                                                return Err(cluster_node::ClusterNodeError::UnexpectedError(
                                                    err.into(),
                                                ));
                                            }
                                        };
                                    }
                                }
                                rpc::RequestBody::RequestVote { candidate_id, .. } => {
                                    if conn.node_id().ne(candidate_id) {
                                        // Received request from candidate:
                                        naive_logging::log(self.id, &format!("received REQUEST_VOTE from {} with term {}", candidate_id, request.term()));

                                        let (has_not_yet_voted, should_grant_vote) = {
                                            let voted_for = match self.voted_for.read() {
                                                Ok(id) => id,
                                                Err(err) => return Err(cluster_node::ClusterNodeError::UnexpectedError(
                                                    anyhow::anyhow!("could not read current vote for server {}: {:?}", self.id, err),
                                                ))
                                            };

                                            let has_not_yet_voted = voted_for.is_none();
                                            let should_grant_vote = request.term() >= conn.current_term() && voted_for.is_none_or(|id| id.eq(candidate_id));

                                            (has_not_yet_voted, should_grant_vote)
                                        };

                                        if should_grant_vote {
                                            naive_logging::log(self.id, &format!("granting vote to candidate {candidate_id}, downgrading to follower..."));
                                            reset_timeout(&mut recv_timeout);
                                        } else {
                                            naive_logging::log(self.id, &format!("refusing vote for candidate {candidate_id}"));
                                        }

                                        if let Err(err) = request.respond(
                                            conn.current_term(),
                                            rpc::ResponseBody::RequestVote { vote_granted: should_grant_vote },
                                        ).await {
                                            return Err(cluster_node::ClusterNodeError::UnexpectedError(
                                                err.into(),
                                            ));
                                        }

                                        if let cluster_connection::SyncTermSideEffect::Downgrade = conn.sync_term(request.term()) {
                                            if let Err(err) = self.downgrade_to_follower() {
                                                return Err(cluster_node::ClusterNodeError::UnexpectedError(
                                                    err.into(),
                                                ));
                                            }
                                        };

                                        if has_not_yet_voted {
                                            self.vote(*candidate_id);
                                        }
                                    }
                                }
                            }
                        },
                        // Tracing would be nice here..
                        Err(err) => return Err(cluster_node::ClusterNodeError::ClusterConnectionError(
                            server_id,
                            err.clone(),
                        ))
                    }
                }
                res = mode.changed() => {
                    match res {
                        Ok(_) => {
                            let new_mode = *mode.borrow_and_update();
                            match new_mode {
                                ServerMode::Leader  => {
                                    if let Some(routine) = election_routine.take() {
                                        routine.abort();
                                    }

                                    if heartbeat_routine.is_none() {
                                        let cluster_conn = cluster_conn.clone();
                                        let mut heartbeat_mode = mode.clone();

                                        let routine = tokio::spawn(async move {
                                            let mut interval = time::interval(time::Duration::from_millis(1000));
                                            interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
                                            interval.tick().await;

                                            loop {
                                                tokio::select! {
                                                    _ = interval.tick() => {
                                                        println!("locked in heartbeat_routine loop");
                                                        let locked_conn = cluster_conn.lock().await;
                                                        if let Err(err) = locked_conn.append_entries(vec![]).await {
                                                            return Err(cluster_node::ClusterNodeError::HeartbeatError(
                                                                server_id,
                                                                err.into(),
                                                            ))
                                                        }
                                                    }
                                                    res = heartbeat_mode.changed() => {
                                                        match res {
                                                            Ok(_) => break,
                                                            Err(err) => return Err(cluster_node::ClusterNodeError::HeartbeatError(
                                                                server_id,
                                                                err.into(),
                                                            ))
                                                        }
                                                    }
                                                }
                                            }

                                            Ok(())
                                        });

                                        heartbeat_routine = Some(routine);
                                    }
                                }
                                ServerMode::Candidate => {
                                    if let Some(routine) = heartbeat_routine.take() {
                                        routine.abort();
                                    }

                                    if election_routine.is_none() {
                                        let cluster_conn = cluster_conn.clone();
                                        let mut conn = cluster_conn.lock().await;
                                        let mut election_mode = mode.clone();

                                        // TODO: return errors in parallel threads back to main thread.
                                        let routine = tokio::spawn(async move {
                                            let mut handle_election = async move || {
                                                println!("locked in election_routine loop");
                                                conn.increment_term();
                                                match conn.request_vote().await {
                                                    Ok(mut rx) => {
                                                        // This likely needs to exist in a separate routine... verify.

                                                        let cluster_node_count_at_start_of_term = conn.cluster_node_count();
                                                        let mut total_votes_over_term = 0u64;

                                                        while let Some(response) = rx.recv().await {
                                                            match response.body() {
                                                                rpc::ResponseBody::RequestVote { vote_granted } => {
                                                                    if *vote_granted {
                                                                        naive_logging::log(server_id, &format!("received vote for this term"));
                                                                        total_votes_over_term += 1;
                                                                        if total_votes_over_term * 2 > cluster_node_count_at_start_of_term {
                                                                            naive_logging::log(server_id, &format!("won election for term {}", conn.current_term()));
                                                                        }

                                                                        if total_votes_over_term == cluster_node_count_at_start_of_term {
                                                                            break;
                                                                        }
                                                                    }
                                                                },
                                                                rpc::ResponseBody::AppendEntries { .. } => return Err(cluster_node::ClusterNodeError::UnexpectedError(
                                                                    anyhow::anyhow!("invalid response [AppendEntries] to RequestVote RPC"),
                                                                )),
                                                            }
                                                        }

                                                        Ok(())
                                                    }
                                                    Err(err) => return Err(cluster_node::ClusterNodeError::HeartbeatError(
                                                        server_id,
                                                        err.into(),
                                                    ))
                                                }
                                            };

                                            tokio::select! {
                                                _ = handle_election() => {
                                                    // do something here...
                                                }
                                                res = election_mode.changed() => {
                                                    if let Err(err) = res {
                                                        return Err(cluster_node::ClusterNodeError::HeartbeatError(
                                                            server_id,
                                                            err.into(),
                                                        ));
                                                    }
                                                }
                                            }

                                            Ok(())
                                        });

                                        election_routine = Some(routine);
                                    }
                                },
                                ServerMode::Follower => {
                                    if let Some(routine) = heartbeat_routine.take() {
                                        routine.abort();
                                    }

                                    if let Some(routine) = election_routine.take() {
                                        routine.abort();
                                    }
                                }
                            }
                        },
                        Err(err) => return Err(cluster_node::ClusterNodeError::HeartbeatError(
                            server_id,
                            err.into(),
                        )),
                    }
                }
            }
        }
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
        let (publisher, subscriber) = broadcast::channel(TEST_CANNEL_CAPACITY);
        let (_, cluster_node_count) = watch::channel(1);

        let _ = Server::new(
            publisher,
            subscriber,
            timeout::TimeoutRange::new(10, 20),
            cluster_node_count,
        );

        Ok(())
    }
}
