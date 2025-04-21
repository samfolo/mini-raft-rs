use tokio::sync::mpsc;

/// RequestBody represents the body of a ServerRequest.
#[derive(Clone, Debug)]
pub enum RequestBody {
    AppendEntries {
        leader_id: uuid::Uuid,
        prev_log_index: usize,
        prev_log_term: usize,
        entries: Vec<String>, // try and remove owned string later.
        leader_commit: usize,
    },
    RequestVote {},
}

/// ServerRequest represents a request sent by a Server.
#[derive(Clone, Debug)]
pub struct ServerRequest {
    term: usize,
    responder: mpsc::Sender<ServerResponse>,
    body: Option<RequestBody>,
}

impl ServerRequest {
    pub fn new(
        term: usize,
        responder: mpsc::Sender<ServerResponse>,
        body: Option<RequestBody>,
    ) -> Self {
        Self {
            term,
            responder,
            body,
        }
    }

    pub fn term(&self) -> usize {
        self.term
    }

    pub fn body(&self) -> &Option<RequestBody> {
        &self.body
    }
}

/// ResponseBody represents the body of a ServerResponse
#[derive(Clone, Debug)]
pub enum ResponseBody {
    AppendEntries { term: usize, success: bool },
    RequestVote { term: usize, vote_granted: bool },
}

/// ServerResponse represents a response received from a Server.
#[derive(Clone, Debug)]
pub struct ServerResponse {
    term: usize,
    body: ResponseBody,
}
