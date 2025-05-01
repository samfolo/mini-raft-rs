use std::collections::HashMap;

use tokio::sync::{mpsc, oneshot};

use crate::{domain::node_id, naive_logging, server};

use super::{ServerMessage, ServerRequest, log, rpc};

#[derive(Clone)]
pub struct ServerHandle {
    sender: mpsc::Sender<ServerMessage>,
}

impl ServerHandle {
    pub fn new(sender: mpsc::Sender<ServerMessage>) -> Self {
        Self { sender }
    }

    /// `RequestVote` RPCs are initiated by candidates during elections.
    pub async fn request_vote(
        &self,
        candidate_id: node_id::NodeId,
        current_term: usize,
    ) -> anyhow::Result<(), mpsc::error::SendError<server::ServerMessage>> {
        naive_logging::log(
            &candidate_id,
            &format!(
                "-> REQUEST_VOTE {{ term: {}, candidate_id: {} }}",
                current_term, candidate_id
            ),
        );

        self.sender
            .send(
                rpc::ServerRequest::new(
                    rpc::ServerRequestHeaders {
                        term: current_term,
                        node_id: candidate_id,
                    },
                    rpc::ServerRequestBody::RequestVote { candidate_id },
                )
                .into(),
            )
            .await?;

        Ok(())
    }

    /// `AppendEntries` RPCs are initiated by leaders to replicate log entries
    /// and to provide a form of heartbeat.
    pub async fn append_entries(
        &self,
        leader_id: node_id::NodeId,
        current_term: usize,
        entries: Vec<log::ServerLogEntry>,
    ) -> anyhow::Result<(), mpsc::error::SendError<rpc::ServerMessage>> {
        naive_logging::log(
            &leader_id,
            &format!(
                "-> APPEND_ENTRIES {{ term: {}, leader_id: {}, entries: {:#?} }}",
                current_term, leader_id, entries
            ),
        );

        self.sender
            .send(
                rpc::ServerRequest::new(
                    rpc::ServerRequestHeaders {
                        term: current_term,
                        node_id: leader_id,
                    },
                    rpc::ServerRequestBody::AppendEntries { leader_id, entries },
                )
                .into(),
            )
            .await?;

        Ok(())
    }
}
