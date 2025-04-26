use tokio::sync::broadcast;
use tokio::sync::mpsc;

use crate::{naive_logging, server};
use server::{Server, rpc};

impl Server {
    /// `AppendEntries` RPCs are initiated by leaders to replicate log entries
    /// and to provide a form of heartbeat.
    pub(in crate::server) fn append_entries(
        &self,
        entries: Vec<String>,
    ) -> anyhow::Result<
        mpsc::Receiver<rpc::ServerResponse>,
        broadcast::error::SendError<rpc::ServerRequest>,
    > {
        naive_logging::log(
            &self.id,
            &format!(
                "{} -> APPEND_ENTRIES {{ term: {}, leader_id: {}, entries: {:?} }}",
                self.listener,
                self.current_term(),
                self.id,
                entries
            ),
        );

        let (responder, receiver) = mpsc::channel(Self::DEFAULT_MESSAGE_BUFFER_SIZE);

        self.cluster_conn.send(rpc::ServerRequest::new(
            self.current_term(),
            responder,
            rpc::ServerRequestBody::AppendEntries {
                leader_id: self.id,
                entries,
            },
        ))?;

        Ok(receiver)
    }
}

#[cfg(test)]
mod tests {}
