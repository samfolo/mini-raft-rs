mod request;

pub mod error;

pub use request::ClientRequest;
use tokio::sync::broadcast;

use crate::{cluster, naive_logging};

pub type Result<T> = anyhow::Result<T, error::ClientRequestError>;

pub trait Client {
    type NextType;

    fn connect_to_cluster(self, cluster: &cluster::Cluster) -> Self::NextType;
}

pub struct DisconnectedRandomDataClient {
    id: String,
}

impl Client for DisconnectedRandomDataClient {
    type NextType = RandomDataClient;

    fn connect_to_cluster(self, cluster: &cluster::Cluster) -> Self::NextType {
        naive_logging::log(&self.id, "connected to cluster.");

        let client = Self::NextType {
            id: self.id,
            conn: cluster.client_conn(),
        };

        return client;
    }
}

pub struct RandomDataClient {
    id: String,
    conn: broadcast::Sender<ClientRequest>,
}

impl RandomDataClient {
    pub fn new() -> DisconnectedRandomDataClient {
        DisconnectedRandomDataClient {
            id: "[ RndmDataClient ]".to_string(),
        }
    }
}
