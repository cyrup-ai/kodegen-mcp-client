// packages/mcp-client/src/transports/mod.rs
use rmcp::model::{ClientCapabilities, ClientInfo, Implementation};

pub mod http;
pub mod stdio;

pub use http::create_streamable_client;
pub use stdio::{StdioClientBuilder, create_stdio_client};

/// Create standard ClientInfo for kodegen MCP clients
///
/// Centralizes client metadata initialization to ensure consistency across
/// all transport types (HTTP, SSE, stdio). The only parameter is the client
/// name, which identifies the transport type.
///
/// # Arguments
/// * `name` - Client identifier (e.g., "kodegen-http-client")
///
/// # Returns
/// Fully initialized ClientInfo with default protocol version and capabilities
pub(crate) fn create_client_info(name: impl Into<String>) -> ClientInfo {
    ClientInfo {
        protocol_version: Default::default(),
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: name.into(),
            title: None,
            version: env!("CARGO_PKG_VERSION").to_string(),
            website_url: None,
            icons: None,
        },
    }
}
