use std::{fmt, sync::Arc};

use tokio::{sync::broadcast, task};

use crate::errors;

pub type Result<T> = anyhow::Result<T, ClusterNodeError>;

pub type ClusterNodeHandle = task::JoinHandle<Result<uuid::Uuid>>;

pub trait ClusterNode: Send + 'static {
    fn run(self: &mut Self) -> impl Future<Output = Result<uuid::Uuid>> + Send;
}

#[derive(thiserror::Error)]
pub enum ClusterNodeError {
    #[error("Failed to send heartbeat to cluster")]
    HeartbeatError(uuid::Uuid, #[source] anyhow::Error),
    #[error("Lost connection to cluster")]
    ClusterConnectionError(uuid::Uuid, #[source] broadcast::error::RecvError),
    #[error("Something went wrong")]
    UnexpectedError(#[from] anyhow::Error),
}

impl fmt::Debug for ClusterNodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        errors::error_chain_fmt(self, f)
    }
}
