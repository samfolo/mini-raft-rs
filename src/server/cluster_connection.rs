use tokio::sync::{broadcast, mpsc, watch};

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
    cluster_node_count: watch::Receiver<u64>,
}

impl ClusterConnection {
    const MESSAGE_BUFFER_SIZE: usize = 32;

    pub fn new(
        node_id: uuid::Uuid,
        tx: broadcast::Sender<rpc::ServerRequest>,
        cluster_node_count: watch::Receiver<u64>,
    ) -> Self {
        Self {
            node_id,
            current_term: 0,
            tx,
            cluster_node_count,
        }
    }

    /// `RequestVote` RPCs are initiated by candidates during elections.
    pub fn request_vote(
        &mut self,
    ) -> anyhow::Result<
        mpsc::Receiver<rpc::ServerResponse>,
        broadcast::error::SendError<rpc::ServerRequest>,
    > {
        naive_logging::log(
            self.node_id,
            &format!(
                "-> REQUEST_VOTE {{ term: {}, candidate_id: {} }}",
                self.current_term, self.node_id
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

    /// `AppendEntries` RPCs are initiated by leaders to replicate log entries
    /// and to provide a form of heartbeat.
    pub fn append_entries(
        &self,
        entries: Vec<String>,
    ) -> anyhow::Result<mpsc::Receiver<rpc::ServerResponse>> {
        naive_logging::log(
            self.node_id,
            &format!(
                "-> APPEND_ENTRIES {{ term: {}, leader_id: {}, entries: {:?} }}",
                self.current_term, self.node_id, entries
            ),
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

    pub fn current_term(&self) -> usize {
        self.current_term
    }

    pub fn sync_term(&mut self, sender_term: usize) -> SyncTermSideEffect {
        if self.current_term < sender_term {
            self.current_term = sender_term;
            return SyncTermSideEffect::Downgrade;
        }

        SyncTermSideEffect::None
    }

    pub fn increment_term(&mut self) {
        self.current_term += 1
    }

    pub fn cluster_node_count(&self) -> u64 {
        *self.cluster_node_count.borrow()
    }
}
