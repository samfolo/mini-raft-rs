use std::fmt;

use crate::client;

#[derive(Debug)]
pub struct ServerLog {
    entries: Vec<ServerLogEntry>,
}

impl fmt::Display for ServerLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "[")?;
        for entry in &self.entries[..] {
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

#[derive(Debug)]
pub struct ServerLogEntry {
    term: usize,
    command: client::ClientRequestBody,
}

impl fmt::Display for ServerLogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:<3}: {}", self.term, self.command)
    }
}
