pub mod rpc;

use rpc::ServerRequest;
use std::cmp;
use tokio::sync::{broadcast, mpsc};

use crate::cluster_node;

/// At any given time each server is in one of three states:
/// leader, follower, or candidate.
#[derive(Debug, PartialEq)]
pub enum ServerState {
    Leader,
    Follower,
    Candidate,
}

/// A Server handles requests from Clients. Within the same Cluster,
/// Only one server can be the Leader at any one time.
pub struct Server {
    id: uuid::Uuid,
    state: ServerState,
    /// Latest term Server has seen. Iinitialised to 0 on first boot,
    /// increases monotonically.
    current_term: usize,
    publisher: broadcast::Sender<rpc::ServerRequest>,
    subscriber: broadcast::Receiver<rpc::ServerRequest>,
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
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            state: ServerState::Follower,
            current_term: 0,
            publisher,
            subscriber,
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
        if self.state != ServerState::Follower {
            self.state == ServerState::Follower;
        }
    }

    /// `RequestVote` RPCs are initiated by candidates during elections.
    pub async fn request_vote(&self) -> anyhow::Result<()> {
        let (responder, receiver) = mpsc::channel(Self::MESSAGE_BUFFER_SIZE);

        self.publisher.send(rpc::ServerRequest::new(
            self.current_term,
            responder,
            Some(rpc::RequestPayload::RequestVote {}),
        ))?;

        // Do something with this in a thread
        let _ = receiver;

        Ok(())
    }

    /// `AppendEntries` RPCs are initiated by leaders to replicate log entries
    /// and to provide a form of heartbeat.
    pub async fn append_entries(&self) -> anyhow::Result<()> {
        let (responder, receiver) = mpsc::channel(Self::MESSAGE_BUFFER_SIZE);

        self.publisher.send(rpc::ServerRequest::new(
            self.current_term,
            responder,
            Some(rpc::RequestPayload::AppendEntries {}),
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
    async fn run(&mut self) -> Result<uuid::Uuid, broadcast::error::RecvError> {
        loop {
            match self.subscriber.recv().await {
                Ok(_) => todo!("unimplemented"),
                Err(err) => {
                    // Tracing would be nice..
                    eprintln!("error received for server {}: {err:?}", self.id);
                    return Err(err);
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
        let _ = Server::new(publisher, subscriber);

        Ok(())
    }
}
