mod actors;
mod handle;
mod receiver;
mod request;

pub mod error;

use error::ClientRequestError;

pub use handle::ClientHandle;
pub use request::{ClientRequest, ClientRequestBody, ClientResponse, ClientResponseBody, Message};

use tokio::sync::mpsc;

use crate::{
    naive_logging, server,
    state_machine::{self, Op, StateKey},
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

    async fn make_random_request(&self, tx: mpsc::Sender<request::Message>) -> self::Result<()> {
        let (_, handle) = self.peer_list.random_peer();

        let make_read_request = rand::random_bool(0.25);
        if make_read_request {
            handle
                .handle_client_request(
                    &self.id,
                    ClientRequest::new(request::ClientRequestBody::Read, ClientHandle::new(tx)),
                    false,
                )
                .await
                .map_err(|err| ClientRequestError::Unexpected(err.into()))?;
        } else {
            let op = Self::OPS[rand::random_range(0..3) as usize];
            let state_key = Self::STATE_KEYS[rand::random_range(0..3) as usize];
            let command = state_machine::Command::new(op, state_key, rand::random_range(0..=100));

            handle
                .handle_client_request(
                    &self.id,
                    ClientRequest::new(
                        request::ClientRequestBody::Write { command },
                        ClientHandle::new(tx),
                    ),
                    false,
                )
                .await
                .map_err(|err| ClientRequestError::Unexpected(err.into()))?;
        }

        Ok(())
    }

    pub async fn run(self) -> Result<String> {
        let (publisher, receiver) = mpsc::channel(Self::DEFAULT_MESSAGE_BUFFER_SIZE);

        tokio::try_join!(
            actors::run_outbound_actor(&self, publisher),
            actors::run_inbound_actor(&self, receiver)
        )?;

        Ok(self.id)
    }
}
