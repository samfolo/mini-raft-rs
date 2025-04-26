use std::fmt;

use tokio::{sync::broadcast, time};

use crate::{domain, errors};

use super::ClientRequest;

#[derive(thiserror::Error)]
pub enum ClientRequestError {
    #[error("Invalid client request")]
    BadRequest(
        domain::node_id::NodeId,
        #[source] broadcast::error::RecvError,
    ),
    #[error("Failed to respond to client request")]
    InternalClusterError(
        domain::node_id::NodeId,
        #[source] broadcast::error::SendError<ClientRequest>,
    ),
    #[error("Timed out waitng for response")]
    Timeout(#[from] time::error::Elapsed),
    #[error("Something went wrong")]
    Unexpected(#[from] anyhow::Error),
}

impl fmt::Debug for ClientRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        errors::error_chain_fmt(self, f)
    }
}
