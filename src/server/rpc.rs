use tokio::sync::mpsc;

/// RequestPayload represents the payload of a ServerRequest, should it need one.
#[derive(Clone, Debug)]
pub enum RequestPayload {
    AppendEntries {},
    RequestVote {},
}

/// ServerRequest represents a request sent by a Server.
#[derive(Clone, Debug)]
pub struct ServerRequest {
    term: usize,
    responder: mpsc::Sender<ServerResponse>,
    payload: Option<RequestPayload>,
}

impl ServerRequest {
    pub fn new(
        term: usize,
        responder: mpsc::Sender<ServerResponse>,
        payload: Option<RequestPayload>,
    ) -> Self {
        Self {
            term,
            responder,
            payload,
        }
    }

    pub fn payload(&self) -> &Option<RequestPayload> {
        &self.payload
    }

    pub fn term(&self) -> usize {
        self.term
    }
}

/// ServerResponse represents a response received from a Server.
#[derive(Clone, Debug)]
pub struct ServerResponse {
    term: usize,
}
