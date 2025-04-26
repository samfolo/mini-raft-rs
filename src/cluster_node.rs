pub mod error;

use tokio::task;

use crate::domain;

pub type Result<T> = anyhow::Result<T, error::ClusterNodeError>;

pub type ClusterNodeJoinSet = task::JoinSet<Result<domain::node_id::NodeId>>;

pub trait ClusterNode: Send + 'static {
    fn run(&self) -> impl Future<Output = Result<domain::node_id::NodeId>> + Send;
}
