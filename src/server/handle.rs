use std::fmt;

use tokio::sync::mpsc;

use crate::{client, domain::node_id, message, naive_logging};

use super::{log, request};

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

    pub async fn request_vote_response(
        &self,
        responder_id: node_id::NodeId,
        current_term: usize,
        vote_granted: bool,
    ) -> anyhow::Result<(), mpsc::error::SendError<message::Message>> {
        naive_logging::log(
            &responder_id,
            &format!(
                "-> REQUEST_VOTE (res) {{ term: {current_term}, vote_granted: {vote_granted} }}",
            ),
        );

        self.sender
            .send(
                request::ServerResponse::new(
                    request::ServerResponseHeaders {
                        term: current_term,
                        node_id: responder_id,
                    },
                    request::ServerResponseBody::RequestVote { vote_granted },
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
                "-> APPEND_ENTRIES (req) {{ term: {}, leader_id: {}, entries: {:#?} }}",
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

    pub async fn append_entries_response(
        &self,
        responder_id: node_id::NodeId,
        current_term: usize,
        success: bool,
    ) -> anyhow::Result<(), mpsc::error::SendError<message::Message>> {
        naive_logging::log(
            &responder_id,
            &format!("-> APPEND_ENTRIES (res) {{ term: {current_term}, success: {success} }}"),
        );

        self.sender
            .send(
                request::ServerResponse::new(
                    request::ServerResponseHeaders {
                        term: current_term,
                        node_id: responder_id,
                    },
                    request::ServerResponseBody::AppendEntries { success },
                )
                .into(),
            )
            .await?;

        Ok(())
    }

    /// ...
    pub async fn handle_client_request(
        &self,
        sender_id: &impl fmt::Display,
        request: client::ClientRequest,
        forwarded: bool,
    ) -> anyhow::Result<(), mpsc::error::SendError<message::Message>> {
        match request.body {
            client::ClientRequestBody::Read => {
                naive_logging::log(
                    &sender_id,
                    &format!(
                        "{} CLIENT_READ_CMD (req) {{ }}",
                        if forwarded { ">>" } else { "->" },
                    ),
                );
            }
            client::ClientRequestBody::Write { command } => {
                naive_logging::log(
                    &sender_id,
                    &format!(
                        "{} CLIENT_WRITE_CMD (req) {{ command: {command} }}",
                        if forwarded { ">>" } else { "->" },
                    ),
                );
            }
        }

        self.sender.send(request.into()).await?;

        Ok(())
    }
}
