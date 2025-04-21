mod cluster_connection;
pub mod rpc;

use std::pin;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::{
    sync::{broadcast, watch},
    task::JoinHandle,
    time,
};

use crate::{cluster_node, naive_logging};

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
    election_timeout: time::Duration,
}

/// Raft servers communicate using remote procedure calls (RPCs), and the basic
/// consensus algorithm requires only two types of RPCs:
/// - `RequestVote`
/// - `AppendEntries`
impl Server {
    pub fn new(
        publisher: broadcast::Sender<rpc::ServerRequest>,
        subscriber: broadcast::Receiver<rpc::ServerRequest>,
        election_timeout: time::Duration,
    ) -> Self {
        let id = uuid::Uuid::new_v4();
        naive_logging::log(id, "initialised.");

        let (mode_tx, mode) = watch::channel(ServerMode::Follower);

        let cluster_conn =
            cluster_connection::ClusterConnection::new(id, publisher, subscriber, election_timeout);
        let cluster_conn = Arc::new(Mutex::new(cluster_conn));

        Self {
            id,
            mode,
            mode_tx,
            cluster_conn,
            election_timeout,
        }
    }

    /// Current terms are exchanged whenever Servers communicate; if
    /// one Server’s current term is smaller than the other’s, then it updates
    /// its current term to the larger value.
    async fn sync_term(
        &mut self,
        request: &rpc::ServerRequest,
    ) -> Result<(), watch::error::SendError<ServerMode>> {
        if let cluster_connection::SyncTermSideEffect::Downgrade = {
            let mut conn = self.cluster_conn.lock().await;
            conn.sync_term(request.term())
        } {
            return self.downgrade_to_follower();
        }

        Ok(())
    }

    /// If a candidate or leader discovers that its term is out of date, it
    /// immediately reverts to follower mode.
    fn downgrade_to_follower(&mut self) -> Result<(), watch::error::SendError<ServerMode>> {
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

    /// The leader handles all client requests; if a client contacts a follower, the
    /// follower redirects it to the leader.
    #[allow(unused)]
    async fn handle_client_request(&self, _: ()) -> anyhow::Result<()> {
        todo!("unimplemented")
    }
}

impl cluster_node::ClusterNode for Server {
    async fn run(self: &mut Self) -> Result<uuid::Uuid, cluster_node::ClusterNodeError> {
        naive_logging::log(self.id, "running...");

        let server_id = self.id;
        let mut mode = self.mode.clone();
        let cluster_conn = self.cluster_conn.clone();

        let mut heartbeat_routine: Option<JoinHandle<cluster_node::Result<()>>> = None;
        let mut election_routine: Option<JoinHandle<cluster_node::Result<()>>> = None;

        let recv_timeout = time::sleep(self.election_timeout);
        tokio::pin!(recv_timeout);

        let reset_timeout = |timeout: &mut pin::Pin<&mut time::Sleep>| {
            timeout
                .as_mut()
                .reset(time::Instant::now() + self.election_timeout)
        };

        loop {
            let mut conn = cluster_conn.lock().await;

            tokio::select! {
                _ = &mut recv_timeout => {
                    if *mode.borrow() != ServerMode::Candidate {
                        naive_logging::log(self.id, "timed out waiting for a response...");
                        naive_logging::log(self.id, "starting election...");
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
                                        naive_logging::log(self.id, &format!("received APPEND_ENTRIES from {}", leader_id));
                                    }
                                }
                                rpc::RequestBody::RequestVote { candidate_id, .. } => {
                                    if conn.node_id().ne(candidate_id) {
                                        // Received request from leader; reset the election timeout:
                                        reset_timeout(&mut recv_timeout);
                                        naive_logging::log(self.id, &format!("received REQUEST_VOTE from {} with term {}", candidate_id, request.term()));
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
                                                        if let Err(err) = cluster_conn.lock().await.append_entries(vec![]).await {
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
                                        let mut election_mode = mode.clone();

                                        let routine = tokio::spawn(async move {
                                            let mut interval = time::interval(time::Duration::from_millis(1000));
                                            interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
                                            interval.tick().await;

                                            loop {
                                                tokio::select! {
                                                    _ = interval.tick() => {
                                                        let mut conn = cluster_conn.lock().await;
                                                        conn.increment_term();
                                                        if let Err(err) = conn.request_vote().await {
                                                            return Err(cluster_node::ClusterNodeError::HeartbeatError(
                                                                server_id,
                                                                err.into(),
                                                            ))
                                                        }
                                                    }
                                                    res = election_mode.changed() => {
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
        let _ = Server::new(publisher, subscriber, time::Duration::from_millis(10));

        Ok(())
    }
}
