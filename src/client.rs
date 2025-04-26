mod request;

pub mod error;

pub use request::ClientRequest;

pub type Result<T> = anyhow::Result<T, error::ClientRequestError>;
