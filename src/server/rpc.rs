use tokio::sync::mpsc;

/// RequestBody represents the body of a ServerRequest.
#[derive(Clone, Debug)]
pub enum RequestBody {
    AppendEntries {
        leader_id: uuid::Uuid,
        entries: Vec<String>, // try and remove owned string later.
    },
    RequestVote {
        candidate_id: uuid::Uuid,
    },
}

/// ServerRequest represents a request sent by a Server.
#[derive(Clone, Debug)]
pub struct ServerRequest {
    term: usize,
    responder: mpsc::Sender<ServerResponse>,
    body: RequestBody,
}

impl ServerRequest {
    pub fn new(term: usize, responder: mpsc::Sender<ServerResponse>, body: RequestBody) -> Self {
        Self {
            term,
            responder,
            body,
        }
    }

    pub fn term(&self) -> usize {
        self.term
    }

    pub fn body(&self) -> &RequestBody {
        &self.body
    }

    pub fn can_respond(&self) -> bool {
        !self.responder.is_closed()
    }

    pub async fn respond(
        &self,
        term: usize,
        body: ResponseBody,
    ) -> Result<(), mpsc::error::SendError<ServerResponse>> {
        self.responder.send(ServerResponse { term, body }).await
    }
}

/// ResponseBody represents the body of a ServerResponse
#[derive(Clone, Debug)]
pub enum ResponseBody {
    AppendEntries { success: bool },
    RequestVote { vote_granted: bool },
}

/// ServerResponse represents a response received from a Server.
#[derive(Clone, Debug)]
pub struct ServerResponse {
    term: usize,
    body: ResponseBody,
}

impl ServerResponse {
    pub fn term(&self) -> usize {
        self.term
    }

    pub fn body(&self) -> &ResponseBody {
        &self.body
    }
}
