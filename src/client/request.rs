use tokio::sync::mpsc;

use crate::state_machine;

/// ClientRequest represents a request sent by a Client.
#[derive(Debug, Clone)]
pub struct ClientRequest {
    pub responder: mpsc::Sender<ClientResponse>,
    pub body: state_machine::Command,
}

impl ClientRequest {
    pub fn new(responder: mpsc::Sender<ClientResponse>, body: state_machine::Command) -> Self {
        Self { responder, body }
    }

    pub fn body(&self) -> &state_machine::Command {
        &self.body
    }

    pub fn can_respond(&self) -> bool {
        !self.responder.is_closed()
    }

    pub async fn respond(
        self,
        success: bool,
    ) -> Result<(), mpsc::error::SendError<ClientResponse>> {
        self.responder.send(ClientResponse { success }).await
    }
}

/// ClientResponse represents a response received from a Client.
#[derive(Debug, Clone, Copy)]
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
