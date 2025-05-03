use crate::state_machine;

/// Message represents a message sent from or received by a Client.
#[derive(Debug)]
pub enum Message {
    Request(ClientRequest),
    Response(ClientResponse),
}

/// ClientRequest represents a request sent by a Client.
#[derive(Debug, Clone)]
pub struct ClientRequest {
    pub body: state_machine::Command,
}

impl ClientRequest {
    pub fn new(body: state_machine::Command) -> Self {
        Self { body }
    }

    pub fn body(&self) -> &state_machine::Command {
        &self.body
    }
}

/// ClientResponse represents a response received from a Client.
#[derive(Debug)]
pub struct ClientResponse {
    success: bool,
    snapshot: state_machine::InMemoryStateMachineSnapshot,
}

impl ClientResponse {
    pub fn new(success: bool, snapshot: state_machine::InMemoryStateMachineSnapshot) -> Self {
        Self { success, snapshot }
    }

    pub fn success(&self) -> bool {
        self.success
    }

    pub fn snapshot(&self) -> &state_machine::InMemoryStateMachineSnapshot {
        &self.snapshot
    }
}
