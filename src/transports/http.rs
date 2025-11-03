// packages/mcp-client/src/transports/http.rs
use crate::{ClientError, KodegenClient, KodegenConnection};
use rmcp::{
    ServiceExt,
    model::{ClientCapabilities, ClientInfo, Implementation},
    transport::{SseClientTransport, StreamableHttpClientTransport},
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
    // SseClientTransport requires async start
    let transport = SseClientTransport::start(url.to_owned())
        .await
        .map_err(|e| ClientError::Connection(format!("Failed to connect to HTTP endpoint: {e}")))?;

    let client_info = ClientInfo {
        protocol_version: Default::default(),
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "kodegen-http-client".to_string(),
            title: None,
            version: env!("CARGO_PKG_VERSION").to_string(),
            website_url: None,
            icons: None,
        },
    };

    // Use () as the client type for HTTP (no custom client needed)
    let service = client_info
        .serve(transport)
        .await
        .map_err(ClientError::InitError)?;

    // Use KodegenConnection to wrap service, then extract client
    let connection = KodegenConnection::from_service(service);
    let client = connection.client();

    Ok((client, connection))
}

/// Create a Streamable HTTP client from a URL
///
/// Returns a tuple of (client, connection):
/// - `client`: Clone-able handle for MCP operations, share freely across tasks
/// - `connection`: Lifecycle manager, must be held until shutdown desired
///
/// # Example
/// ```ignore
/// let (client, _conn) = create_streamable_client("http://localhost:8000/mcp").await?;
/// let client2 = client.clone();  // Cheap clone!
/// ```
///
/// # Errors
///
/// Returns `ClientError::InitError` if the MCP initialization fails.
pub async fn create_streamable_client(
    url: &str,
) -> Result<(KodegenClient, KodegenConnection), ClientError> {
    // StreamableHttpClientTransport has simpler constructor
    let transport = StreamableHttpClientTransport::from_uri(url);

    let client_info = ClientInfo {
        protocol_version: Default::default(),
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "kodegen-streamable-client".to_string(),
            title: None,
            version: env!("CARGO_PKG_VERSION").to_string(),
            website_url: None,
            icons: None,
        },
    };

    let service = client_info
        .serve(transport)
        .await
        .map_err(ClientError::InitError)?;

    // Use KodegenConnection to wrap service, then extract client
    let connection = KodegenConnection::from_service(service);
    let client = connection.client();

    Ok((client, connection))
}
