use std::fmt;

use std::sync::RwLock;

use crate::client::{self, ClientRequestBody};

#[derive(Debug)]
pub struct ServerLog {
    entries: RwLock<Vec<ServerLogEntry>>,
}

impl ServerLog {
    pub fn entries(&self) -> Vec<ServerLogEntry> {
        match self.entries.read() {
            Ok(entries) => entries.to_vec(),
            Err(err) => panic!("failed to read log entries: {err:?}"),
        }
    }

    pub fn append_cmd(&self, term: usize, command: ClientRequestBody) {
        match self.entries.write() {
            Ok(mut entries) => {
                entries.push(ServerLogEntry { term, command });
            }
            Err(err) => panic!("failed to append entry to log: {err:?}"),
        };
    }
}

impl fmt::Display for ServerLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "[")?;
        for entry in self.entries() {
            writeln!(f, "  {}", entry)?;
        }
        write!(f, "]")
    }
}

impl Default for ServerLog {
    fn default() -> Self {
        Self {
            entries: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ServerLogEntry {
    term: usize,
    command: client::ClientRequestBody,
}

impl fmt::Display for ServerLogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:<3}: {}", self.term, self.command)
    }
}
