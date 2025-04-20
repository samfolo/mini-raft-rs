use tokio::sync::broadcast;

pub type Result<T> = anyhow::Result<T, broadcast::error::RecvError>;

pub trait ClusterNode {
    fn run(&mut self) -> impl Future<Output = Result<()>> + Send;
}
