mod request;

pub mod error;

use error::ClientRequestError;
pub use request::ClientRequest;
use request::{ClientRequestBody, ClientResponse, Op, StateKey};
use tokio::{
    sync::{broadcast, mpsc},
    time,
};

use crate::{cluster, naive_logging, timeout};

pub type Result<T> = anyhow::Result<T, error::ClientRequestError>;

pub trait Client {
    type NextType;

    fn connect_to_cluster(self, cluster: &cluster::Cluster) -> Self::NextType;
}

pub struct DisconnectedRandomDataClient {
    id: String,
    min_request_interval_ms: u64,
    max_request_interval_ms: u64,
}

impl DisconnectedRandomDataClient {
    pub fn with_request_interval_range(mut self, min: u64, max: u64) -> Self {
        assert!(min < max);
        self.min_request_interval_ms = min;
        self.max_request_interval_ms = max;
        self
    }
}

impl Client for DisconnectedRandomDataClient {
    type NextType = RandomDataClient;

    fn connect_to_cluster(self, cluster: &cluster::Cluster) -> Self::NextType {
        naive_logging::log(&self.id, "connected to cluster.");

        let client = Self::NextType {
            id: self.id,
            conn: cluster.client_conn(),
            min_request_interval_ms: self.min_request_interval_ms,
            max_request_interval_ms: self.max_request_interval_ms,
        };

        return client;
    }
}

pub struct RandomDataClient {
    id: String,
    conn: broadcast::Sender<ClientRequest>,
    min_request_interval_ms: u64,
    max_request_interval_ms: u64,
}

impl RandomDataClient {
    const DEFAULT_REQUEST_BUFFER_SIZE: usize = 32;
    const DEFAULT_MIN_ELECTION_TIMEOUT_MS: u64 = 150;
    const DEFAULT_MAX_ELECTION_TIMEOUT_MS: u64 = 300;
    const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 200;

    const OPS: [Op; 3] = [Op::Increment, Op::Decrement, Op::Replace];
    const STATE_KEYS: [StateKey; 3] = [StateKey::X, StateKey::Y, StateKey::Z];

    pub fn new() -> DisconnectedRandomDataClient {
        DisconnectedRandomDataClient {
            id: "[ RndmDataClient ]".to_string(),
            min_request_interval_ms: Self::DEFAULT_MIN_ELECTION_TIMEOUT_MS,
            max_request_interval_ms: Self::DEFAULT_MAX_ELECTION_TIMEOUT_MS,
        }
    }

    fn make_random_request(&self) -> self::Result<mpsc::Receiver<ClientResponse>> {
        let (responder, receiver) = mpsc::channel(Self::DEFAULT_REQUEST_BUFFER_SIZE);

        let op = Self::OPS[rand::random_range(0..3) as usize];
        let state_key = Self::STATE_KEYS[rand::random_range(0..3) as usize];
        let body = ClientRequestBody::new(op, state_key, rand::random_range(0..=100));

        if let Err(err) = self.conn.send(ClientRequest { responder, body }) {
            return Err(ClientRequestError::Unexpected(err.into()));
        }

        Ok(receiver)
    }

    pub async fn make_random_requests(&self) -> self::Result<()> {
        let timeout_range =
            timeout::TimeoutRange::new(self.min_request_interval_ms, self.max_request_interval_ms);

        loop {
            match self.make_random_request() {
                Ok(mut response) => {
                    if let Err(err) = time::timeout(
                        time::Duration::from_millis(Self::DEFAULT_REQUEST_TIMEOUT_MS),
                        response.recv(),
                    )
                    .await
                    {
                        return Err(ClientRequestError::Timeout(err));
                    }

                    time::sleep(timeout_range.random()).await
                }
                Err(err) => return Err(ClientRequestError::Unexpected(err.into())),
            }
        }
    }
}
