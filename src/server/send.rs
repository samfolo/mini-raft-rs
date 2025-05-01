use futures_util::stream::StreamExt;
use std::pin;
use tokio::{sync::mpsc, task::JoinSet, time};

use crate::{cluster_node, server};
use cluster_node::error::ClusterNodeError;
use server::{Server, ServerState};

impl Server {
    pub(in crate::server) async fn run_send_routine(&self) {
        // create join set of all nodes in peer list (move handle into each routine)
    }
}

#[cfg(test)]
mod tests {}
