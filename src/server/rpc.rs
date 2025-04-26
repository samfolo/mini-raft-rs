mod append_entries;
mod request_vote;

use tokio::sync::mpsc;

use crate::domain;

/// ServerRequestBody represents the body of a ServerRequest.
#[derive(Clone, Debug)]
pub enum ServerRequestBody {
    AppendEntries {
        leader_id: domain::node_id::NodeId,
        entries: Vec<String>, // try and remove owned string later.
    },
    RequestVote {
        candidate_id: domain::node_id::NodeId,
    },
}

/// ServerRequest represents a request sent by a Server.
#[derive(Clone, Debug)]
pub struct ServerRequest {
    term: usize,
    responder: mpsc::Sender<ServerResponse>,
    body: ServerRequestBody,
}

impl ServerRequest {
    pub fn new(
        term: usize,
        responder: mpsc::Sender<ServerResponse>,
        body: ServerRequestBody,
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

    pub fn body(&self) -> &ServerRequestBody {
        &self.body
    }

    pub fn can_respond(&self) -> bool {
        !self.responder.is_closed()
    }

    pub async fn respond(
        &self,
        term: usize,
        body: ServerResponseBody,
    ) -> Result<(), mpsc::error::SendError<ServerResponse>> {
        self.responder.send(ServerResponse { term, body }).await
    }
}

/// ServerResponseBody represents the body of a ServerResponse
#[derive(Clone, Debug)]
pub enum ServerResponseBody {
    AppendEntries {},
    RequestVote { vote_granted: bool },
}

/// ServerResponse represents a response received from a Server.
#[derive(Clone, Debug)]
pub struct ServerResponse {
    term: usize,
    body: ServerResponseBody,
}

impl ServerResponse {
    pub fn term(&self) -> usize {
        self.term
    }

    pub fn body(&self) -> &ServerResponseBody {
        &self.body
    }
}
