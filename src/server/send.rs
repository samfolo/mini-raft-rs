use futures_util::stream::StreamExt;
use std::pin;
use tokio::{sync::mpsc, time};

use crate::{cluster_node, server};
use cluster_node::error::ClusterNodeError;
use server::{Server, ServerState};

impl Server {
    pub(in crate::server) async fn send_routine(&self) {}
}

#[cfg(test)]
mod tests {}
