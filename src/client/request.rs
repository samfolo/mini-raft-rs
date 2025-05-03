use crate::state_machine;

use super::handle::ClientHandle;

/// Message represents a message sent from or received by a Client.
#[derive(Debug, Clone)]
pub enum Message {
    Request(ClientRequest),
    Response(ClientResponse),
}

impl From<ClientRequest> for Message {
    fn from(req: ClientRequest) -> Self {
        Self::Request(req)
    }
}

impl From<ClientResponse> for Message {
    fn from(res: ClientResponse) -> Self {
        Self::Response(res)
    }
}

/// ClientRequest represents a request sent by a Client.
#[derive(Debug, Clone)]
pub struct ClientRequest {
    pub body: ClientRequestBody,
    pub responder: ClientHandle,
}

impl ClientRequest {
    pub fn new(body: ClientRequestBody, responder: ClientHandle) -> Self {
        Self { body, responder }
    }
}

#[derive(Debug, Clone)]
pub enum ClientRequestBody {
    Read,
    Write { command: state_machine::Command },
}

/// ClientResponse represents a response received from a Client.
#[derive(Debug, Clone)]
pub struct ClientResponse {
    pub body: ClientResponseBody,
}

impl ClientResponse {
    pub fn new(body: ClientResponseBody) -> Self {
        Self { body }
    }
}

#[derive(Debug, Clone)]
pub enum ClientResponseBody {
    Read {
        snapshot: state_machine::InMemoryStateMachineSnapshot,
    },
    Write {
        success: bool,
    },
}
