use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use crate::events::Event;

pub mod json_rpc_stdio;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum ProtocolKind {
    JsonRpc,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum TransportKind {
    Stdio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    pub session_id: uuid::Uuid,
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub session_id: uuid::Uuid,
    pub ok: bool,
    pub result: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHealth {
    pub status: HealthStatus,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[async_trait]
pub trait AgentProtocol: Send + Sync {
    /// Make a single call. Should respect cancellation via `cancel` token.
    async fn call(
        &self,
        req: AgentRequest,
        cancel: CancellationToken,
    ) -> anyhow::Result<AgentResponse>;

    /// Stream events from the agent (thinking, content, `tool_call`, etc.).
    /// Should respect cancellation via `cancel` token.
    async fn stream(
        &self,
        req: AgentRequest,
        cancel: CancellationToken,
        tx: tokio::sync::mpsc::Sender<Event>,
    ) -> anyhow::Result<AgentResponse>;

    async fn health(&self) -> AgentHealth;

    async fn shutdown(&self) -> anyhow::Result<()>;

    /// EP-0013 T-004: take the underlying Child out so the supervisor
    /// can wait() on it. Returns None if already taken or if the
    /// protocol doesn't have a child (e.g. HTTP-based agents).
    async fn take_child(&self) -> Option<tokio::process::Child>;
}

pub type SharedProtocol = Arc<dyn AgentProtocol>;
