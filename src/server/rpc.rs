mod append_entries;
mod request_vote;

use tokio::sync::oneshot;

use crate::{
    domain::{self, node_id},
    server,
};

use super::log::ServerLogEntry;

/// ServerRequestBody represents the body of a ServerRequest.
#[derive(Clone, Debug)]
pub enum ServerRequestBody {
    AppendEntries {
        // so follower can redirect clients
        leader_id: domain::node_id::NodeId,

        // // index of log entry immediately preceding new ones
        // prev_log_index: usize,
        // // term of prevLogIndex entry
        // prev_log_term: usize,
        // // leader’s commitIndex
        // leader_commit: usize,

        // log entries to store (empty for heartbeat; may send more
        // than one for efficiency)
        entries: Vec<ServerLogEntry>,
    },
    RequestVote {
        // candidate requesting vote
        candidate_id: domain::node_id::NodeId,
    },
}

/// ServerRequest represents a request sent by a Server.
#[derive(Debug)]
pub struct ServerRequest {
    term: usize,
    responder: oneshot::Sender<server::ServerResponse>,
    body: ServerRequestBody,
}

impl ServerRequest {
    pub fn new(
        term: usize,
        responder: oneshot::Sender<ServerResponse>,
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

    pub fn respond(
        self,
        headers: ServerResponseHeaders,
        body: ServerResponseBody,
    ) -> Result<(), ServerResponse> {
        self.responder.send(ServerResponse { headers, body })
    }
}

/// ServerResponseBody represents the headers of a ServerResponse
#[derive(Clone, Debug)]
pub struct ServerResponseHeaders {
    pub node_id: node_id::NodeId,
    pub term: usize,
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
    headers: ServerResponseHeaders,
    body: ServerResponseBody,
}

impl ServerResponse {
    pub fn sender_id(&self) -> node_id::NodeId {
        self.headers.node_id
    }

    pub fn term(&self) -> usize {
        self.headers.term
    }

    pub fn body(&self) -> &ServerResponseBody {
        &self.body
    }
}
