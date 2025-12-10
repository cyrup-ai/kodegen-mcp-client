// packages/mcp-client/src/transports/http.rs
use super::create_client_info;
use crate::{ClientError, KodegenClient, KodegenConnection};
use reqwest::header::HeaderMap;
use rmcp::{
    ServiceExt,
    transport::{
        StreamableHttpClientTransport,
        streamable_http_client::StreamableHttpClientTransportConfig,
    },
};

/// Create an HTTP client from a URL (StreamableHttpService transport)
///
/// Returns a tuple of (client, connection):
/// - `client`: Clone-able handle for MCP operations, share freely across tasks
/// - `connection`: Lifecycle manager, must be held until shutdown desired
///
/// # Example
/// ```ignore
/// let (client, _conn) = create_http_client("http://localhost:8080/mcp").await?;
/// let client2 = client.clone();  // Cheap clone!
/// client.call_tool("my_tool", args).await?;
/// // _conn dropped here, triggering graceful shutdown
/// ```
///
/// # Errors
///
/// Returns `ClientError::Connection` if the HTTP connection fails,
/// or `ClientError::InitError` if the MCP initialization fails.
pub async fn create_http_client(
    url: &str,
) -> Result<(KodegenClient, KodegenConnection), ClientError> {
    // Use streamable HTTP transport (SSE was removed in rmcp v0.11)
    create_streamable_client(url, HeaderMap::new()).await
}

/// Create a Streamable HTTP client from a URL with default headers
///
/// Returns a tuple of (client, connection):
/// - `client`: Clone-able handle for MCP operations, share freely across tasks
/// - `connection`: Lifecycle manager, must be held until shutdown desired
///
/// Headers are attached via reqwest's `default_headers` and sent with every request.
/// Use this to pass session context (e.g., `x-session-pwd`, `x-session-gitroot`).
///
/// # Example
/// ```ignore
/// use reqwest::header::{HeaderMap, HeaderValue};
/// use kodegen_mcp_client::headers::{X_SESSION_PWD, X_SESSION_GITROOT};
///
/// let mut headers = HeaderMap::new();
/// headers.insert(X_SESSION_PWD, HeaderValue::from_static("/project/frontend"));
/// headers.insert(X_SESSION_GITROOT, HeaderValue::from_static("/project"));
///
/// let (client, _conn) = create_streamable_client("http://localhost:8000/mcp", headers).await?;
/// ```
///
/// # Errors
///
/// Returns `ClientError::Connection` if building the HTTP client fails,
/// or `ClientError::InitError` if the MCP initialization fails.
pub async fn create_streamable_client(
    url: &str,
    headers: HeaderMap,
) -> Result<(KodegenClient, KodegenConnection), ClientError> {
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|e| ClientError::Connection {
            message: format!("Failed to build HTTP client: {e}"),
            transport_type: Some(crate::TransportType::Http),
            endpoint: Some(url.to_string()),
        })?;

    let config = StreamableHttpClientTransportConfig {
        uri: url.into(),
        ..Default::default()
    };

    let transport = StreamableHttpClientTransport::with_client(client, config);

    let client_info = create_client_info("kodegen-streamable-client");

    let service = client_info
        .serve(transport)
        .await?;

    // Use KodegenConnection to wrap service, then extract client
    let connection = KodegenConnection::from_service(service);
    let client = connection.client();

    Ok((client, connection))
}
