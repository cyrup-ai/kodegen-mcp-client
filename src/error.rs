use rmcp::service::ClientInitializeError;
use std::time::Duration;
use thiserror::Error;

/// Transport type for connection errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportType {
    /// HTTP/HTTPS transport (Streamable HTTP)
    Http,
    /// Standard I/O (stdio) transport
    Stdio,
}

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("MCP protocol error: {0}")]
    Protocol(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Client initialization error: {0}")]
    InitError(Box<ClientInitializeError>),

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

// Manual From implementation to handle boxing of ClientInitializeError
impl From<ClientInitializeError> for ClientError {
    fn from(error: ClientInitializeError) -> Self {
        Self::InitError(Box::new(error))
    }
}

impl ClientError {
    /// Check if this is an initialization error
    pub fn is_init_error(&self) -> bool {
        matches!(self, ClientError::InitError(_))
    }

    /// Check if this error indicates a broken/closed connection
    pub fn is_connection_broken(&self) -> bool {
        match self {
            ClientError::InitError(init_err) => {
                matches!(
                    init_err.as_ref(),
                    ClientInitializeError::ConnectionClosed(_)
                        | ClientInitializeError::TransportError { .. }
                )
            }
            ClientError::ServiceError(service_err) => {
                matches!(
                    service_err,
                    rmcp::ServiceError::TransportClosed
                        | rmcp::ServiceError::TransportSend(_)
                )
            }
            ClientError::Io(_) => true,
            _ => false,
        }
    }

    /// Get a human-readable error kind for logging
    pub fn error_kind(&self) -> &'static str {
        match self {
            ClientError::InitError(init_err) => match init_err.as_ref() {
                ClientInitializeError::ConnectionClosed(_) => "connection closed during init",
                ClientInitializeError::TransportError { .. } => "transport error during init",
                ClientInitializeError::Cancelled => "init cancelled",
                ClientInitializeError::ExpectedInitResponse(_) => "unexpected init response",
                ClientInitializeError::ExpectedInitResult(_) => "unexpected init result",
                ClientInitializeError::ConflictInitResponseId(_, _) => "init response id conflict",
            },
            ClientError::ServiceError(service_err) => match service_err {
                rmcp::ServiceError::TransportClosed => "transport closed",
                rmcp::ServiceError::TransportSend(_) => "transport send failed",
                rmcp::ServiceError::Cancelled { .. } => "service cancelled",
                rmcp::ServiceError::Timeout { .. } => "service timeout",
                rmcp::ServiceError::McpError(_) => "mcp protocol error",
                rmcp::ServiceError::UnexpectedResponse => "unexpected response",
                _ => "service error",
            },
            ClientError::Timeout { .. } => "timeout",
            ClientError::Connection { .. } => "connection error",
            ClientError::Protocol(_) => "protocol error",
            ClientError::ParseError { .. } => "parse error",
            ClientError::Io(_) => "io error",
            ClientError::JoinError(_) => "task join error",
        }
    }

    /// Get detailed context for InitError variants (for logging)
    pub fn init_error_context(&self) -> Option<String> {
        match self {
            ClientError::InitError(init_err) => match init_err.as_ref() {
                ClientInitializeError::ConnectionClosed(ctx) => {
                    Some(format!("connection closed during: {}", ctx))
                }
                ClientInitializeError::TransportError { context, error } => {
                    Some(format!("transport error during {}: {}", context, error))
                }
                ClientInitializeError::Cancelled => Some("initialization cancelled".to_string()),
                other => Some(format!("{:?}", other)),
            },
            _ => None,
        }
    }

    /// Check if this error indicates a session/authentication failure
    /// 
    /// Returns true if the error is an MCP error with session-related content.
    /// These errors typically indicate that:
    /// - HTTP session has expired (401 Unauthorized)
    /// - Session ID is missing or invalid
    /// - Authentication has failed
    /// 
    /// When this returns true, the caller should attempt to reconnect and retry.
    pub fn is_session_error(&self) -> bool {
        match self {
            ClientError::ServiceError(rmcp::ServiceError::McpError(mcp_err)) => {
                // Check message for session-related keywords
                let msg = mcp_err.message.to_lowercase();
                
                msg.contains("session")
                    || msg.contains("unauthorized")
                    || msg.contains("401")
                    || msg.contains("authentication")
                    || msg.contains("auth")
            }
            _ => false,
        }
    }
}
