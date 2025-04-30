use std::collections::HashMap;

use tokio::sync::{mpsc, oneshot};

use crate::{domain::node_id, naive_logging, server};

use super::{ServerRequest, rpc};

#[derive(Clone)]
pub struct ServerHandle {
    sender: mpsc::Sender<ServerRequest>,
}

impl ServerHandle {
    pub fn new(sender: mpsc::Sender<ServerRequest>) -> Self {
        Self { sender }
    }

    /// `RequestVote` RPCs are initiated by candidates during elections.
    pub async fn request_vote(
        &self,
        candidate_id: node_id::NodeId,
        current_term: usize,
        responder: oneshot::Sender<server::ServerResponse>,
    ) -> anyhow::Result<(), mpsc::error::SendError<server::ServerRequest>> {
        naive_logging::log(
            &candidate_id,
            &format!(
                "-> REQUEST_VOTE {{ term: {}, candidate_id: {} }}",
                current_term, candidate_id
            ),
        );

        self.sender
            .send(rpc::ServerRequest::new(
                current_term,
                responder,
                rpc::ServerRequestBody::RequestVote { candidate_id },
            ))
            .await?;

        Ok(())
    }
}
