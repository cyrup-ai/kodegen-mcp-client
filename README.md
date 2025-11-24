<div align="center">
  <img src="assets/img/banner.png" alt="Kodegen AI Banner" width="100%" />
</div>

# kodegen-mcp-client

[![License](https://img.shields.io/badge/license-Apache%202.0%20OR%20MIT-blue.svg)](LICENSE.md)
[![Rust](https://img.shields.io/badge/rust-nightly-orange.svg)](https://www.rust-lang.org)

A Rust client library for interacting with [MCP (Model Context Protocol)](https://modelcontextprotocol.io) servers, specifically designed for **KODEGEN.ᴀɪ** database query and schema exploration tools.

## Features

✨ **Type-Safe MCP Client**
- Strongly-typed response structures for all 75+ KODEGEN tools
- Compile-time type checking with `call_tool_typed<T>()`
- Support for both `camelCase` and `snake_case` field names

🚀 **Async & Efficient**
- Built on Tokio for high-performance async operations
- Cheap-to-clone client handles (Arc-based internally)
- Configurable timeouts with sensible defaults

🔌 **Multiple Transports**
- HTTP/SSE (Server-Sent Events)
- Streamable HTTP
- Easy transport selection

🛡️ **Robust Error Handling**
- Comprehensive error types with context
- Clear error messages for debugging
- Timeout tracking with operation details

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
kodegen_mcp_client = "0.1"
```

**Requirements:**
- Rust nightly toolchain
- Tokio async runtime

## Quick Start

```rust
use kodegen_mcp_client::{create_streamable_client, tools, X_SESSION_PWD, X_SESSION_GITROOT};
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build session context headers
    let mut headers = HeaderMap::new();
    let cwd = std::env::current_dir()?;
    headers.insert(X_SESSION_PWD, HeaderValue::from_str(&cwd.to_string_lossy())?);
    // Add git root if available
    if let Some(git_root) = find_git_root(&cwd) {
        headers.insert(X_SESSION_GITROOT, HeaderValue::from_str(&git_root.to_string_lossy())?);
    }

    // Create client connection with session headers
    let (client, _conn) = create_streamable_client("http://localhost:8000/mcp", headers).await?;

    // List available tools
    let tools = client.list_tools().await?;
    println!("Available tools: {}", tools.len());

    // Call a tool with JSON arguments
    let result = client.call_tool(
        tools::LIST_SCHEMAS,
        json!({}),
    ).await?;

    println!("Result: {:?}", result);

    Ok(())
}

fn find_git_root(start: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}
```

## Usage Examples

### Type-Safe Tool Calls

Use strongly-typed responses instead of manual JSON parsing:

```rust
use kodegen_mcp_client::responses::StartSearchResponse;
use kodegen_mcp_client::tools;

// Type-safe tool call with automatic deserialization
let response: StartSearchResponse = client
    .call_tool_typed(tools::START_SEARCH, json!({
        "path": "/project",
        "pattern": "*.rs",
        "searchType": "files"
    }))
    .await?;

// Use the typed response
println!("Search session ID: {}", response.session_id);
```


### Clone-able Client Handles

Client handles are cheap to clone and can be shared across tasks:

```rust
use reqwest::header::HeaderMap;

let (client, _conn) = create_streamable_client(url, HeaderMap::new()).await?;

// Clone the client for concurrent operations
let client2 = client.clone();
tokio::spawn(async move {
    client2.list_tools().await
});

// Original client still works
client.call_tool(tools::READ_FILE, args).await?;
```

### Custom Timeouts

```rust
use reqwest::header::HeaderMap;
use tokio::time::Duration;

let (client, _conn) = create_streamable_client(url, HeaderMap::new()).await?;

// Set custom timeout (default is 30 seconds)
let client = client.with_timeout(Duration::from_secs(60));
```

### GitHub Integration

```rust
use kodegen_mcp_client::responses::{GitHubIssuesResponse, GitHubIssue};

let response: GitHubIssuesResponse = client
    .call_tool_typed(tools::LIST_ISSUES, json!({
        "owner": "myorg",
        "repo": "myrepo",
        "state": "open"
    }))
    .await?;

for issue in response.issues {
    println!("#{}: {}", issue.number, issue.title);
}
```


### Database Operations

```rust
use kodegen_mcp_client::tools;

// Execute SQL query
let result = client.call_tool(tools::EXECUTE_SQL, json!({
    "query": "SELECT * FROM users WHERE active = true",
    "database": "production"
})).await?;

// Get table schema
let schema = client.call_tool(tools::GET_TABLE_SCHEMA, json!({
    "table_name": "users",
    "schema": "public"
})).await?;
```

## Architecture

### Handle + Connection Pattern

The library uses a two-struct pattern for resource management:

- **`KodegenClient`**: Cheap-to-clone handle for performing MCP operations
  - Clone freely and share across async tasks
  - Wraps `Arc` internally for efficient cloning
  - Thread-safe and shareable

- **`KodegenConnection`**: Non-clonable lifecycle manager
  - Must be held as long as the connection should remain active
  - Provides `close()` for graceful shutdown
  - Automatically cancels connection when dropped

```rust
use reqwest::header::HeaderMap;

let (client, _conn) = create_streamable_client(url, HeaderMap::new()).await?;
// client: Clone freely
// _conn: Hold until shutdown desired (auto-cleanup on drop)
```

## Available Tool Categories

The library provides type-safe access to 75+ KODEGEN.ᴀɪ tools across multiple categories:


- **Filesystem** (14 tools): File operations, search, directory management
- **Terminal** (5 tools): Command execution, process management
- **Database** (7 tools): Schema exploration, query execution, connection pooling
- **Git** (20 tools): Repository operations, branching, commits, worktrees
- **GitHub** (25 tools): Issues, PRs, reviews, code search, repositories
- **Claude Agents** (5 tools): Multi-agent orchestration, spawning, communication
- **Web Crawling** (4 tools): Site crawling, search, content extraction
- **Configuration** (2 tools): Server config management
- **Prompts** (4 tools): Template management and rendering
- **Sequential Thinking** (1 tool): Chain-of-thought reasoning

All tool names are available as constants in the `tools` module.

## Error Handling

The library provides comprehensive error types with context:

```rust
use kodegen_mcp_client::ClientError;

match client.call_tool(tools::READ_FILE, args).await {
    Ok(result) => println!("Success: {:?}", result),
    Err(ClientError::Timeout(msg)) => eprintln!("Timeout: {}", msg),
    Err(ClientError::ParseError(msg)) => eprintln!("Parse error: {}", msg),
    Err(ClientError::Connection(msg)) => eprintln!("Connection failed: {}", msg),
    Err(e) => eprintln!("Other error: {}", e),
}
```

Error variants:
- `Protocol`: MCP protocol errors
- `Timeout`: Operation timeouts (includes duration)
- `ParseError`: Response deserialization failures
- `Connection`: Transport connection failures
- `ServiceError`, `InitError`, `Io`, `JoinError`: Lower-level errors


## Transport Options

### Streamable HTTP (Recommended)

```rust
use kodegen_mcp_client::create_streamable_client;
use reqwest::header::HeaderMap;

let (client, conn) = create_streamable_client("http://localhost:8000/mcp", HeaderMap::new()).await?;
```

### HTTP/SSE (Server-Sent Events)

```rust
use kodegen_mcp_client::transports::http::create_http_client;

let (client, conn) = create_http_client("http://localhost:8000/mcp").await?;
```

## Development

### Building

```bash
cargo build              # Debug build
cargo build --release    # Release build
```

### Testing

```bash
cargo test               # Run all tests
cargo test --lib         # Library tests only
```

### Linting & Formatting

```bash
cargo clippy             # Run linter
cargo fmt                # Format code
```

### Documentation

```bash
cargo doc --open         # Build and open docs
```

## License

Dual-licensed under Apache 2.0 OR MIT terms.

See [LICENSE.md](LICENSE.md) for details.

## Links

- **Homepage**: [https://kodegen.ai](https://kodegen.ai)
- **Repository**: [https://github.com/cyrup-ai/kodegen-mcp-client](https://github.com/cyrup-ai/kodegen-mcp-client)
- **MCP Protocol**: [https://modelcontextprotocol.io](https://modelcontextprotocol.io)

---

Copyright © 2025 David Maple / KODEGEN.ᴀɪ
