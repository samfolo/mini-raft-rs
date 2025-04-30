use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::server::log::ServerLogEntry;
use crate::{naive_logging, server};
use server::{Server, rpc};

impl Server {
    /// `AppendEntries` RPCs are initiated by leaders to replicate log entries
    /// and to provide a form of heartbeat.
    pub(in crate::server) fn append_entries(
        &self,
        responder: oneshot::Sender<rpc::ServerResponse>,
        entries: Vec<ServerLogEntry>,
    ) -> anyhow::Result<(), broadcast::error::SendError<rpc::ServerRequest>> {
        let current_term = self.current_term();

        naive_logging::log(
            &self.id,
            &format!(
                "{} -> APPEND_ENTRIES {{ term: {}, leader_id: {}, entries: {:?} }}",
                self.listener, current_term, self.id, entries
            ),
        );

        self.cluster_conn.send(rpc::ServerRequest::new(
            current_term,
            responder,
            rpc::ServerRequestBody::AppendEntries {
                leader_id: self.id,
                entries,
            },
        ))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {}
