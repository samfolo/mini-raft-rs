use std::fmt;

use crate::{
    domain::{self, node_id},
    server,
};

/// Message represents a message sent from or received by a Server.
#[derive(Clone, Debug)]
pub enum Message {
    Request(ServerRequest),
    Response(ServerResponse),
}

impl From<ServerRequest> for Message {
    fn from(req: ServerRequest) -> Self {
        Self::Request(req)
    }
}

impl From<ServerResponse> for Message {
    fn from(res: ServerResponse) -> Self {
        Self::Response(res)
    }
}

/// ServerMessagePayload represents the methods available on the payload of a server-specific Message
pub trait ServerMessagePayload<Body: Clone + fmt::Debug> {
    fn sender_id(&self) -> node_id::NodeId;

    fn term(&self) -> usize;

    fn body(&self) -> &Body;
}

/// ServerRequestHeaders represents the headers of a ServerRequest
#[derive(Clone, Debug)]
pub struct ServerRequestHeaders {
    pub node_id: node_id::NodeId,
    pub term: usize,
}

/// ServerRequestBody represents the body of a ServerRequest.
#[derive(Clone, Debug)]
pub enum ServerRequestBody {
    AppendEntries {
        // so follower can redirect clients
        leader_id: domain::node_id::NodeId,
        // index of log entry immediately preceding new ones
        prev_log_index: usize,
        // term of prevLogIndex entry
        prev_log_term: usize,
        // log entries to store (empty for heartbeat; may send more
        // than one for efficiency)
        entries: Vec<server::ServerLogEntry>,
        // leader’s commitIndex
        leader_commit: usize,
    },
    RequestVote {
        // candidate requesting vote
        candidate_id: domain::node_id::NodeId,
    },
}

/// ServerRequest represents a request sent by a Server.
#[derive(Clone, Debug)]
pub struct ServerRequest {
    headers: ServerRequestHeaders,
    body: ServerRequestBody,
}

impl ServerRequest {
    pub fn new(headers: ServerRequestHeaders, body: ServerRequestBody) -> Self {
        Self { headers, body }
    }
}

impl ServerMessagePayload<ServerRequestBody> for ServerRequest {
    fn sender_id(&self) -> node_id::NodeId {
        self.headers.node_id
    }

    fn term(&self) -> usize {
        self.headers.term
    }

    fn body(&self) -> &ServerRequestBody {
        &self.body
    }
}

/// ServerResponseHeaders represents the headers of a ServerResponse
#[derive(Clone, Debug)]
pub struct ServerResponseHeaders {
    pub node_id: node_id::NodeId,
    pub term: usize,
}

/// ServerResponseBody represents the body of a ServerResponse
#[derive(Clone, Debug)]
pub enum ServerResponseBody {
    AppendEntries { success: bool },
    RequestVote { vote_granted: bool },
}

/// ServerResponse represents a response received from a Server.
#[derive(Clone, Debug)]
pub struct ServerResponse {
    headers: ServerResponseHeaders,
    body: ServerResponseBody,
}

impl ServerResponse {
    pub fn new(headers: ServerResponseHeaders, body: ServerResponseBody) -> Self {
        Self { headers, body }
    }
}

impl ServerMessagePayload<ServerResponseBody> for ServerResponse {
    fn sender_id(&self) -> node_id::NodeId {
        self.headers.node_id
    }

    fn term(&self) -> usize {
        self.headers.term
    }

    fn body(&self) -> &ServerResponseBody {
        &self.body
    }
}
