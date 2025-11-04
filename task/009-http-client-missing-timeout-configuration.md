# Task: HTTP Client Missing Timeout Configuration

## Location
`src/transports/http.rs:27-58` (create_http_client)
`src/transports/http.rs:75-103` (create_streamable_client)

## Issue Type
- API Inconsistency
- Usability
- Missing Feature

## Description
The HTTP client creation functions (`create_http_client` and `create_streamable_client`) don't provide a way to configure custom timeouts during creation, unlike `StdioClientBuilder` which has a `.timeout()` method.

## Problem

### Current API Asymmetry

#### Stdio Transport (Good)
```rust
let (client, conn) = StdioClientBuilder::new("node")
    .arg("server.js")
    .timeout(Duration::from_secs(120))  // ✅ Configure timeout at build time
    .build()
    .await?;

// Client already has custom timeout configured
let tools = client.list_tools().await?;
```

#### HTTP Transport (Awkward)
```rust
let (client, conn) = create_http_client("http://localhost:8080/mcp").await?;

// ❌ No way to set timeout during creation
// ⚠️ Must remember to configure timeout separately
let client = client.with_timeout(Duration::from_secs(120));

// Now client has custom timeout
let tools = client.list_tools().await?;
```

### Issues with Current Approach

1. **Easy to Forget**: Users can easily forget to call `.with_timeout()` after creating the client, leading to unexpected 30-second timeouts

2. **Verbose**: Requires two steps (create + configure) instead of one

3. **Inconsistent API**: Stdio transport has timeout configuration in builder, HTTP transport doesn't

4. **Discovery Problem**: Users looking at `create_http_client()` docs won't know about `.with_timeout()` unless they also read `KodegenClient` docs

5. **Confusing Error Messages**: If a user expects a longer timeout but forgets to configure it:
   ```rust
   let (client, _conn) = create_http_client(url).await?;
   // Oops, forgot to set timeout!
   let result = client.call_tool(tools::EXECUTE_LONG_QUERY, args).await?;
   // Error: "Tool 'execute_long_query' timed out after 30s"
   // User: "But I expected it to wait 5 minutes!"
   ```

## Real-World Impact

### Scenario 1: Long-Running Operations
```rust
// User knows their SQL queries take 2-3 minutes
let (client, _conn) = create_http_client("http://db-server:8080/mcp").await?;

// Forgot to set timeout!
for query in long_running_queries {
    let result = client.call_tool(tools::EXECUTE_SQL, json!({
        "query": query
    })).await?;
    // ❌ Fails after 30s even though query would succeed in 120s
}
```

### Scenario 2: Documentation Example Confusion
```rust
// User follows README example:
let (client, _conn) = create_streamable_client("http://localhost:8000/mcp").await?;
let tools = client.list_tools().await?;

// Works fine with default timeout

// Later, user adds operation that needs longer timeout:
let result = client.call_tool(tools::START_CRAWL, json!({
    "url": "https://large-site.com",
    "depth": 5
})).await?;

// ❌ Times out after 30s
// User: "Why does the stdio example let me configure timeout but HTTP doesn't?"
```

### Scenario 3: Copy-Paste Errors
```rust
// User copies stdio code and tries to adapt for HTTP:
let (client, conn) = StdioClientBuilder::new("node")
    .arg("server.js")
    .timeout(Duration::from_secs(120))
    .build()
    .await?;

// Try to do the same with HTTP:
let (client, conn) = create_streamable_client("http://localhost:8000/mcp")
    .timeout(Duration::from_secs(120))  // ❌ Doesn't compile!
    .await?;
```

## Comparison with Other Libraries

### Standard Pattern: Builder with Configuration
Most Rust HTTP libraries follow this pattern:

```rust
// reqwest
let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(120))
    .build()?;

// hyper (lower-level)
let client = hyper::Client::builder()
    .pool_idle_timeout(Duration::from_secs(90))
    .build(...);

// aws-sdk
let config = aws_config::from_env()
    .timeout_config(TimeoutConfig::builder()
        .operation_timeout(Duration::from_secs(120))
        .build())
    .load()
    .await;
```

Our library's stdio transport follows this pattern, but HTTP transports don't.

## Recommended Fixes

### Option 1: Add Builder for HTTP Transports (Recommended)
```rust
/// Builder for creating HTTP SSE-based MCP clients
#[derive(Debug, Clone)]
pub struct HttpClientBuilder {
    url: String,
    timeout: Duration,
    client_name: Option<String>,
}

impl HttpClientBuilder {
    /// Create a new HTTP client builder
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            timeout: Duration::from_secs(30),  // DEFAULT_TIMEOUT
            client_name: None,
        }
    }

    /// Set custom timeout for MCP operations
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set custom client name
    #[must_use]
    pub fn client_name(mut self, name: impl Into<String>) -> Self {
        self.client_name = Some(name.into());
        self
    }

    /// Build and connect the HTTP client
    pub async fn build(self) -> Result<(KodegenClient, KodegenConnection), ClientError> {
        let transport = SseClientTransport::start(self.url)
            .await
            .map_err(|e| ClientError::Connection(format!("Failed to connect: {e}")))?;

        let client_info = ClientInfo {
            protocol_version: Default::default(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: self.client_name.unwrap_or_else(|| "kodegen-http-client".to_string()),
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

        let connection = KodegenConnection::from_service(service);
        let client = connection.client().with_timeout(self.timeout);

        Ok((client, connection))
    }
}

/// Create an HTTP client (convenience function)
///
/// For advanced configuration (custom timeout, client name), use [`HttpClientBuilder`].
pub async fn create_http_client(
    url: &str,
) -> Result<(KodegenClient, KodegenConnection), ClientError> {
    HttpClientBuilder::new(url).build().await
}
```

Similarly for streamable HTTP:
```rust
pub struct StreamableClientBuilder {
    // Similar to HttpClientBuilder
}

pub async fn create_streamable_client(url: &str) -> Result<...> {
    StreamableClientBuilder::new(url).build().await
}
```

**Usage**:
```rust
// Simple usage (unchanged, backward compatible)
let (client, conn) = create_http_client(url).await?;

// Advanced usage (new)
let (client, conn) = HttpClientBuilder::new(url)
    .timeout(Duration::from_secs(120))
    .client_name("my-custom-client")
    .build()
    .await?;
```

### Option 2: Add Timeout Parameter to Functions
```rust
pub async fn create_http_client_with_config(
    url: &str,
    config: ClientConfig,
) -> Result<(KodegenClient, KodegenConnection), ClientError> {
    // ...
}

pub struct ClientConfig {
    pub timeout: Option<Duration>,
    pub client_name: Option<String>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            timeout: Some(Duration::from_secs(30)),
            client_name: None,
        }
    }
}
```

**Pros**: Simpler than builder
**Cons**: Less extensible, awkward for optional configs

### Option 3: Only Document .with_timeout() Better
Keep current API but improve documentation:

```rust
/// Create an HTTP client from a URL (StreamableHttpService transport)
///
/// Returns a tuple of (client, connection) with a default 30-second timeout.
///
/// **To configure a custom timeout**, use `.with_timeout()` on the returned client:
///
/// ```ignore
/// let (client, conn) = create_http_client("http://localhost:8080/mcp").await?;
/// let client = client.with_timeout(Duration::from_secs(120));
/// ```
///
/// For more control over client configuration, consider using a builder pattern
/// (see [`HttpClientBuilder`] - **Note**: this doesn't exist yet, see issue #X).
pub async fn create_http_client(url: &str) -> Result<...> {
```

**Pros**: No code changes, backward compatible
**Cons**: Still awkward API, easy to forget

## Recommended Approach

**Option 1** (Add Builder) is strongly recommended because:

1. **Consistent with Stdio Transport**: Users familiar with `StdioClientBuilder` will immediately understand `HttpClientBuilder`

2. **Discoverable**: Users see the builder and know how to configure it

3. **Extensible**: Easy to add more configuration options later (request headers, TLS config, etc.)

4. **Backward Compatible**: Existing `create_http_client()` functions can remain as convenience wrappers

5. **Standard Rust Pattern**: Matches what users expect from other HTTP libraries

## Implementation Steps

1. Create `HttpClientBuilder` and `StreamableClientBuilder` structs
2. Add `.timeout()` and `.client_name()` methods
3. Implement `.build()` method (move logic from `create_*` functions)
4. Keep `create_http_client()` and `create_streamable_client()` as wrappers for backward compatibility
5. Update documentation to recommend builders for advanced usage
6. Add examples showing builder usage
7. Add tests for custom timeouts

## Testing

```rust
#[tokio::test]
async fn test_http_builder_custom_timeout() {
    let (client, _conn) = HttpClientBuilder::new("http://localhost:8080/mcp")
        .timeout(Duration::from_secs(120))
        .build()
        .await?;

    // Verify client has custom timeout
    // (requires exposing timeout getter or indirect testing)
}

#[tokio::test]
async fn test_http_builder_custom_client_name() {
    let (client, _conn) = HttpClientBuilder::new("http://localhost:8080/mcp")
        .client_name("test-client")
        .build()
        .await?;

    // Verify server sees correct client name
    assert_eq!(client.server_info().unwrap().client_info.name, "test-client");
}
```

## Priority
**MEDIUM** - API inconsistency and usability issue, but has a workaround (`.with_timeout()`)

## Related Tasks
- Task 013: Duplicated client info construction (will be affected by builder)

## Migration Guide (for users)

When builders are added:

```rust
// Before (still works):
let (client, conn) = create_http_client(url).await?;
let client = client.with_timeout(Duration::from_secs(120));

// After (preferred):
let (client, conn) = HttpClientBuilder::new(url)
    .timeout(Duration::from_secs(120))
    .build()
    .await?;
```
