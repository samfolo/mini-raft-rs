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
    #[allow(unused)]
    term: usize,
    #[allow(unused)]
    responder: mpsc::Sender<ServerResponse>,
    payload: Option<RequestPayload>,
}

impl ServerRequest {
    pub fn new(term: usize, responder: mpsc::Sender<ServerResponse>) -> Self {
        Self {
            term,
            responder,
            payload: None,
        }
    }

    pub fn with_payload(&mut self, payload: RequestPayload) -> Self {
        self.payload = Some(payload);
        self.to_owned()
    }
}

/// ServerResponse represents a response received from a Server.
#[derive(Clone, Debug)]
pub struct ServerResponse {
    #[allow(unused)]
    term: usize,
}
