pub enum ServerState {
    Leader,
    Follower,
    Candidate,
}

pub struct Server {
    state: ServerState,
}

impl Server {
    pub fn new() -> Self {
        Self {
            state: ServerState::Follower,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts() -> anyhow::Result<()> {
        let _ = Server::new();
        Ok(())
    }
}
