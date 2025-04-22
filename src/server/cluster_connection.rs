use tokio::{
    sync::{broadcast, mpsc},
    time,
};

use crate::{naive_logging, server::rpc};

#[derive(Debug, PartialEq)]
pub enum SyncTermSideEffect {
    Downgrade,
    None,
}

#[derive(Debug)]
pub struct ClusterConnection {
    node_id: uuid::Uuid,
    /// Latest term Server has seen. Iinitialised to 0 on first boot,
    /// increases monotonically.
    current_term: usize,
    tx: broadcast::Sender<rpc::ServerRequest>,
    rx: broadcast::Receiver<rpc::ServerRequest>,
    voted_for: Option<uuid::Uuid>,
}

impl ClusterConnection {
    const MESSAGE_BUFFER_SIZE: usize = 32;

    pub fn new(
        node_id: uuid::Uuid,
        tx: broadcast::Sender<rpc::ServerRequest>,
        rx: broadcast::Receiver<rpc::ServerRequest>,
    ) -> Self {
        Self {
            node_id,
            current_term: 0,
            tx,
            rx,
            voted_for: None,
        }
    }

    /// `RequestVote` RPCs are initiated by candidates during elections.
    pub async fn request_vote(&mut self) -> anyhow::Result<mpsc::Receiver<rpc::ServerResponse>> {
        naive_logging::log(
            self.node_id,
            &format!(
                "sending REQUEST_VOTE with current_term {}",
                self.current_term
            ),
        );

        let (responder, receiver) = mpsc::channel(Self::MESSAGE_BUFFER_SIZE);

        self.tx.send(rpc::ServerRequest::new(
            self.current_term,
            responder,
            rpc::RequestBody::RequestVote {
                candidate_id: self.node_id,
            },
        ))?;

        Ok(receiver)
    }

    fn vote(&mut self, candidate_id: uuid::Uuid) {
        self.voted_for = Some(candidate_id);
    }

    /// `AppendEntries` RPCs are initiated by leaders to replicate log entries
    /// and to provide a form of heartbeat.
    pub async fn append_entries(
        &self,
        entries: Vec<String>,
    ) -> anyhow::Result<mpsc::Receiver<rpc::ServerResponse>> {
        naive_logging::log(
            self.node_id,
            &format!("sending APPEND_ENTRIES with {} entries", entries.len()),
        );

        let (responder, receiver) = mpsc::channel(Self::MESSAGE_BUFFER_SIZE);

        self.tx.send(rpc::ServerRequest::new(
            self.current_term,
            responder,
            rpc::RequestBody::AppendEntries {
                leader_id: self.node_id,
                entries,
            },
        ))?;

        Ok(receiver)
    }

    pub fn node_id(&self) -> uuid::Uuid {
        self.node_id
    }

    pub fn current_term(&self) -> usize {
        self.current_term
    }

    pub fn sync_term(&mut self, sender_term: usize) -> SyncTermSideEffect {
        if self.current_term < sender_term {
            self.current_term = sender_term;
            return SyncTermSideEffect::Downgrade;
        }

        return SyncTermSideEffect::None;
    }

    pub fn increment_term(&mut self) {
        self.current_term += 1
    }

    pub async fn recv(&mut self) -> Result<rpc::ServerRequest, broadcast::error::RecvError> {
        self.rx.recv().await
    }
}
