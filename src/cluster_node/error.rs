use std::fmt;

use tokio::{sync::broadcast, time};

use crate::{errors, server::rpc};

#[derive(thiserror::Error)]
pub enum ClusterNodeError {
    #[error("Failed to send heartbeat to cluster")]
    Heartbeat(uuid::Uuid, #[source] anyhow::Error),
    #[error("Lost connection to cluster")]
    IncomingClusterConnection(uuid::Uuid, #[source] broadcast::error::RecvError),
    #[error("Lost connection to cluster")]
    OutgoingClusterConnection(
        uuid::Uuid,
        #[source] broadcast::error::SendError<rpc::ServerRequest>,
    ),
    #[error("Timed out waitng for response")]
    Timeout(#[from] time::error::Elapsed),
    #[error("Something went wrong")]
    Unexpected(#[from] anyhow::Error),
}

impl fmt::Debug for ClusterNodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        errors::error_chain_fmt(self, f)
    }
}
