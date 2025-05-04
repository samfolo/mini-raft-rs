use std::fmt;

use std::sync::RwLock;

use crate::state_machine;

#[derive(Debug, Default)]
pub struct ServerLog {
    entries: RwLock<Vec<ServerLogEntry>>,
}

impl ServerLog {
    pub fn is_empty(&self) -> bool {
        match self.entries.read() {
            Ok(entries) => entries.is_empty(),
            Err(err) => panic!("failed to read log entries: {err:?}"),
        }
    }

    // usage is not well enough understood. verify. are you sure of `.unwrap_or(entries.len() - 1)`?
    pub fn entries_from(&self, target_index: usize) -> Vec<ServerLogEntry> {
        match self.entries.read() {
            Ok(entries) => {
                if entries.is_empty() {
                    entries.to_vec()
                } else {
                    let start_index = entries
                        .iter()
                        .enumerate()
                        .find_map(|(idx, entry)| (entry.index == target_index).then_some(idx))
                        .unwrap_or(entries.len() - 1);

                    entries[start_index..].to_vec()
                }
            }
            Err(err) => panic!("failed to read log entries: {err:?}"),
        }
    }

    pub fn find(&self, matcher: impl FnMut(&&ServerLogEntry) -> bool) -> Option<ServerLogEntry> {
        match self.entries.read() {
            Ok(entries) => entries.iter().find(matcher).cloned(),
            Err(err) => panic!("failed to read log entries: {err:?}"),
        }
    }

    pub fn last(&self) -> Option<ServerLogEntry> {
        match self.entries.read() {
            Ok(entries) => entries.last().cloned(),
            Err(err) => panic!("failed to read log entries: {err:?}"),
        }
    }

    pub fn append_cmd(&self, index: usize, term: usize, command: state_machine::Command) {
        match self.entries.write() {
            Ok(mut entries) => {
                entries.push(ServerLogEntry {
                    index,
                    term,
                    command,
                });
            }
            Err(err) => panic!("failed to append entry to log: {err:?}"),
        };
    }
}

impl fmt::Display for ServerLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "[")?;
        for entry in self.entries_from(0) {
            writeln!(f, "  {}", entry)?;
        }
        write!(f, "]")
    }
}

#[derive(Clone, Debug)]
pub struct ServerLogEntry {
    index: usize,
    term: usize,
    command: state_machine::Command,
}

impl ServerLogEntry {
    pub fn index(&self) -> usize {
        self.index
    }

    pub fn term(&self) -> usize {
        self.term
    }
}

impl fmt::Display for ServerLogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:<3}: {}", self.term, self.command)
    }
}
