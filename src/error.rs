use rmcp::service::ClientInitializeError;
use std::time::Duration;
use thiserror::Error;

/// Transport type for connection errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportType {
    /// HTTP/HTTPS transport
    Http,
    /// Standard I/O (stdio) transport
    Stdio,
    /// Server-Sent Events (SSE) transport
    Sse,
}

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("MCP protocol error: {0}")]
    Protocol(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Client initialization error: {0}")]
    InitError(#[from] ClientInitializeError),

    #[error("Service error: {0}")]
    ServiceError(#[from] rmcp::ServiceError),

    #[error("Task join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),

    #[error("Operation '{operation}' timed out after {duration:?}")]
    Timeout {
        operation: String,
        duration: Duration,
    },

    #[error("Failed to parse response from tool '{tool_name}': {source}")]
    ParseError {
        tool_name: String,
        source: serde_json::Error,
    },

    #[error("Connection error: {message}")]
    Connection {
        message: String,
        transport_type: Option<TransportType>,
        endpoint: Option<String>,
    },
}
