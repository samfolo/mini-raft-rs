pub mod rpc;

use rpc::{RequestBody, ServerRequest};
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{broadcast, mpsc, watch},
    task::JoinHandle,
    time,
};

use crate::{cluster_node, naive_logging};

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
    id: uuid::Uuid,
    state: watch::Receiver<ServerState>,
    state_tx: watch::Sender<ServerState>,
    /// Latest term Server has seen. Iinitialised to 0 on first boot,
    /// increases monotonically.
    current_term: usize,
    publisher: broadcast::Sender<rpc::ServerRequest>,
    subscriber: broadcast::Receiver<rpc::ServerRequest>,
    election_timeout: time::Duration,
}

/// Raft servers communicate using remote procedure calls (RPCs), and the basic
/// consensus algorithm requires only two types of RPCs:
/// - `RequestVote`
/// - `AppendEntries`
impl Server {
    const MESSAGE_BUFFER_SIZE: usize = 32;

    pub fn new(
        publisher: broadcast::Sender<rpc::ServerRequest>,
        subscriber: broadcast::Receiver<rpc::ServerRequest>,
        election_timeout: time::Duration,
    ) -> Self {
        let id = uuid::Uuid::new_v4();
        naive_logging::log(id, "initialised.");

        let (state_tx, state) = watch::channel(ServerState::Follower);

        Self {
            id,
            state,
            state_tx,
            current_term: 0,
            publisher,
            subscriber,
            election_timeout,
        }
    }

    /// Current terms are exchanged whenever Servers communicate; if
    /// one Server’s current term is smaller than the other’s, then it updates
    /// its current term to the larger value.
    fn sync_term(&mut self, request: &ServerRequest) {
        let sender_term = request.term();

        if self.current_term < sender_term {
            self.current_term = sender_term;
            self.revert_to_follower();
        }
    }

    /// If a candidate or leader discovers that its term is out of date, it
    /// immediately reverts to follower state.
    fn revert_to_follower(&mut self) {
        if *self.state.borrow() != ServerState::Follower {
            self.state_tx.send(ServerState::Follower);
        }
    }

    /// `RequestVote` RPCs are initiated by candidates during elections.
    pub async fn request_vote(&self) -> anyhow::Result<()> {
        let (responder, receiver) = mpsc::channel(Self::MESSAGE_BUFFER_SIZE);

        self.publisher.send(rpc::ServerRequest::new(
            self.current_term,
            responder,
            rpc::RequestBody::RequestVote {},
        ))?;

        // Do something with this in a thread
        let _ = receiver;

        Ok(())
    }

    /// `AppendEntries` RPCs are initiated by leaders to replicate log entries
    /// and to provide a form of heartbeat.
    pub async fn append_entries(&self, entries: Vec<String>) -> anyhow::Result<()> {
        naive_logging::log(
            self.id,
            &format!("sending APPEND_ENTRIES with {} entries", entries.len()),
        );

        let (responder, receiver) = mpsc::channel(Self::MESSAGE_BUFFER_SIZE);

        self.publisher.send(rpc::ServerRequest::new(
            self.current_term,
            responder,
            rpc::RequestBody::AppendEntries {
                leader_id: self.id,
                entries,
                prev_log_index: 0,
                prev_log_term: 0,
                leader_commit: 0,
            },
        ))?;

        // Do something with this in a thread
        let _ = receiver;

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
    async fn run(self: Arc<Self>) -> Result<uuid::Uuid, cluster_node::ClusterNodeError> {
        naive_logging::log(self.id, "running...");

        let server_id = self.id;
        let mut subscriber = self.publisher.subscribe();
        let mut state_clone = self.state.clone();

        let mut heartbeat_routine: Option<JoinHandle<cluster_node::Result<()>>> = None;

        loop {
            tokio::select! {
                res = subscriber.recv() => {
                    match res {
                        Ok(request) => {
                            match request.body() {
                                RequestBody::AppendEntries { leader_id, .. } => {
                                    naive_logging::log(self.id, &format!("received APPEND_ENTRIES from {}", leader_id));
                                }
                                _ => todo!("unimplemented")
                            }
                        },
                        // Tracing would be nice here..
                        Err(err) => return Err(cluster_node::ClusterNodeError::ClusterConnectionError(
                            server_id,
                            err.into(),
                        ))
                    }
                }
                res = state_clone.changed() => {
                    match res {
                        Ok(_) => {
                            let new_state = *state_clone.borrow_and_update();
                            match new_state {
                                ServerState::Leader if heartbeat_routine.is_none() => {
                                    let server = Arc::clone(&self);
                                    let mut heartbeat_state_clone = state_clone.clone();

                                    let routine = tokio::spawn(async move {
                                        let mut interval = time::interval(time::Duration::from_millis(1000));
                                        interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
                                        interval.tick().await;

                                        loop {
                                            tokio::select! {
                                                _ = interval.tick() => {
                                                    if let Err(err) = server.append_entries(vec![]).await {
                                                        return Err(cluster_node::ClusterNodeError::HeartbeatError(
                                                            server_id,
                                                            err.into(),
                                                        ))
                                                    }
                                                }
                                                res = heartbeat_state_clone.changed() => {
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
                                ServerState::Leader => {},
                                _ => {
                                    if let Some(routine) = heartbeat_routine.take() {
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
