use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(uuid::Uuid);

impl NodeId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let id = self.0.to_string();
        write!(f, "[ {}... ]", &id[..11])
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}
