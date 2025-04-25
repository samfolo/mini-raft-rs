pub mod error;

use tokio::task;

pub type Result<T> = anyhow::Result<T, error::ClusterNodeError>;

pub type ClusterNodeHandle = task::JoinHandle<Result<uuid::Uuid>>;

pub trait ClusterNode: Send + 'static {
    fn run(&self) -> impl Future<Output = Result<uuid::Uuid>> + Send;
}
