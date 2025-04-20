use tokio::sync::broadcast;

pub type Result<T> = anyhow::Result<T, broadcast::error::RecvError>;

pub type ClusterNodeHandle = tokio::task::JoinHandle<Result<()>>;

pub trait ClusterNode: Send + 'static {
    fn run(&mut self) -> impl Future<Output = Result<()>> + Send;
}
