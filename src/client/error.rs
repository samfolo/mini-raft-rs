use std::fmt;

use crate::errors;

#[derive(thiserror::Error)]
pub enum ClientRequestError {
    #[error("Something went wrong")]
    Unexpected(#[from] anyhow::Error),
}

impl fmt::Debug for ClientRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        errors::error_chain_fmt(self, f)
    }
}
