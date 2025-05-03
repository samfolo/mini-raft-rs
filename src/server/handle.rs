use std::collections::HashMap;

use tokio::sync::{mpsc, oneshot};

use crate::{client, domain::node_id, message, naive_logging, server, state_machine};

use super::{ServerRequest, log, request};

#[derive(Clone)]
pub struct ServerHandle {
    sender: mpsc::Sender<message::Message>,
}

impl ServerHandle {
    pub fn new(sender: mpsc::Sender<message::Message>) -> Self {
        Self { sender }
    }

    /// `RequestVote` RPCs are initiated by candidates during elections.
    pub async fn request_vote(
        &self,
        candidate_id: node_id::NodeId,
        current_term: usize,
    ) -> anyhow::Result<(), mpsc::error::SendError<message::Message>> {
        naive_logging::log(
            &candidate_id,
            &format!(
                "-> REQUEST_VOTE (req) {{ term: {}, candidate_id: {} }}",
                current_term, candidate_id
            ),
        );

        self.sender
            .send(
                request::ServerRequest::new(
                    request::ServerRequestHeaders {
                        term: current_term,
                        node_id: candidate_id,
                    },
                    request::ServerRequestBody::RequestVote { candidate_id },
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
    ) -> anyhow::Result<(), mpsc::error::SendError<message::Message>> {
        naive_logging::log(
            &leader_id,
            &format!(
                "-> APPEND_ENTRIES {{ term: {}, leader_id: {}, entries: {:#?} }}",
                current_term, leader_id, entries
            ),
        );

        self.sender
            .send(
                request::ServerRequest::new(
                    request::ServerRequestHeaders {
                        term: current_term,
                        node_id: leader_id,
                    },
                    request::ServerRequestBody::AppendEntries { leader_id, entries },
                )
                .into(),
            )
            .await?;

        Ok(())
    }

    /// ...
    pub async fn handle_client_request(
        &self,
        client_id: String,
        body: state_machine::Command,
    ) -> anyhow::Result<(), mpsc::error::SendError<message::Message>> {
        naive_logging::log(
            &client_id,
            &format!("-> CLIENT_REQUEST {{ body: {} }}", body),
        );

        self.sender
            .send(client::ClientRequest::new(body).into())
            .await?;

        Ok(())
    }
}
