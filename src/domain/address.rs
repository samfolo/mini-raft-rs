use std::{
    fmt,
    net::{SocketAddr, TcpListener},
};

#[derive(Debug)]
pub struct Address {
    listener: TcpListener,
    address: SocketAddr,
}

impl PartialEq for Address {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address
    }
}

impl Eq for Address {}

impl Address {
    pub fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        Self {
            address: listener.local_addr().unwrap(),
            listener,
        }
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "localhost:{}", self.address.port().to_string())
    }
}
