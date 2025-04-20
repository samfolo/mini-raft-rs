use tokio::{sync::broadcast, task};

pub type Result<T> = anyhow::Result<T, broadcast::error::RecvError>;

pub type ClusterNodeHandle = task::JoinHandle<Result<uuid::Uuid>>;

pub trait ClusterNode: Send + 'static {
    fn run(&mut self) -> impl Future<Output = Result<uuid::Uuid>> + Send;
}
