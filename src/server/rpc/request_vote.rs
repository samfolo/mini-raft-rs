use tokio::sync::broadcast;
use tokio::sync::mpsc;

use crate::{naive_logging, server};
use server::{Server, rpc};

impl Server {
    /// `RequestVote` RPCs are initiated by candidates during elections.
    pub(in crate::server) fn request_vote(
        &self,
    ) -> anyhow::Result<
        mpsc::Receiver<rpc::ServerResponse>,
        broadcast::error::SendError<rpc::ServerRequest>,
    > {
        let current_term = self.current_term();

        naive_logging::log(
            &self.id,
            &format!(
                "{} -> REQUEST_VOTE {{ term: {}, candidate_id: {} }}",
                self.listener, current_term, self.id
            ),
        );

        let (responder, receiver) = mpsc::channel(Self::DEFAULT_MESSAGE_BUFFER_SIZE);

        self.cluster_conn.send(rpc::ServerRequest::new(
            current_term,
            responder,
            rpc::ServerRequestBody::RequestVote {
                candidate_id: self.id,
            },
        ))?;

        Ok(receiver)
    }
}

#[cfg(test)]
mod tests {}
