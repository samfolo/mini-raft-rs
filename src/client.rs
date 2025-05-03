mod handle;
mod request;

pub mod error;

use error::ClientRequestError;

pub use handle::ClientHandle;
pub use request::{ClientRequest, ClientResponse, Message};

use tokio::{sync::mpsc, time};

use crate::{
    message, naive_logging, server,
    state_machine::{self, Op, StateKey},
    timeout,
};

pub type Result<T> = anyhow::Result<T, error::ClientRequestError>;

pub trait Client {
    type NextType;

    fn connect_to_cluster(self, peer_list: server::ServerPeerList) -> Self::NextType;
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

    fn connect_to_cluster(self, peer_list: server::ServerPeerList) -> Self::NextType {
        naive_logging::log(&self.id, "connected to cluster.");

        Self::NextType {
            id: self.id,
            peer_list,
            min_request_interval_ms: self.min_request_interval_ms,
            max_request_interval_ms: self.max_request_interval_ms,
        }
    }
}

pub struct RandomDataClient {
    id: String,
    peer_list: server::ServerPeerList,
    min_request_interval_ms: u64,
    max_request_interval_ms: u64,
}

impl RandomDataClient {
    const DEFAULT_MESSAGE_BUFFER_SIZE: usize = 16;
    const DEFAULT_MIN_ELECTION_TIMEOUT_MS: u64 = 150;
    const DEFAULT_MAX_ELECTION_TIMEOUT_MS: u64 = 300;

    const OPS: [Op; 3] = [Op::Increment, Op::Decrement, Op::Replace];
    const STATE_KEYS: [StateKey; 3] = [StateKey::X, StateKey::Y, StateKey::Z];

    pub fn init() -> DisconnectedRandomDataClient {
        DisconnectedRandomDataClient {
            id: "[ RndmDataClient ]".to_string(),
            min_request_interval_ms: Self::DEFAULT_MIN_ELECTION_TIMEOUT_MS,
            max_request_interval_ms: Self::DEFAULT_MAX_ELECTION_TIMEOUT_MS,
        }
    }

    async fn make_random_request(&self, tx: mpsc::Sender<message::Message>) -> self::Result<()> {
        let op = Self::OPS[rand::random_range(0..3) as usize];
        let state_key = Self::STATE_KEYS[rand::random_range(0..3) as usize];
        let body = state_machine::Command::new(op, state_key, rand::random_range(0..=100));

        naive_logging::log(
            &self.id,
            &format!("-> sending request to the cluster: {}", body),
        );

        let (_, handle) = self.peer_list.random_peer();
        handle
            .handle_client_request(
                &self.id,
                ClientRequest::new(body, ClientHandle::new(tx)),
                false,
            )
            .await
            .map_err(|err| ClientRequestError::Unexpected(err.into()))?;

        Ok(())
    }

    pub async fn make_random_requests(&self) -> self::Result<()> {
        let timeout_range =
            timeout::TimeoutRange::new(self.min_request_interval_ms, self.max_request_interval_ms);

        let (tx, mut rx) = mpsc::channel(Self::DEFAULT_MESSAGE_BUFFER_SIZE);
        loop {
            match self.make_random_request(tx.clone()).await {
                Ok(_) => time::sleep(timeout_range.random()).await,
                Err(err) => return Err(ClientRequestError::Unexpected(err.into())),
            }
        }
    }
}
