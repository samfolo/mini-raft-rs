use crate::server;

pub struct Cluster {
    servers: Vec<server::Server>,
}

impl Cluster {
    pub fn new() -> Self {
        Self {
            servers: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init() -> anyhow::Result<()> {
        let _ = Cluster::new();
        Ok(())
    }
}
