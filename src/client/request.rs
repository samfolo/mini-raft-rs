use crate::state_machine;

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
#[derive(Debug, Clone)]
pub struct ClientResponse {
    success: bool,
}

impl ClientResponse {
    pub fn new(success: bool) -> Self {
        Self { success }
    }

    pub fn success(&self) -> bool {
        self.success
    }
}
