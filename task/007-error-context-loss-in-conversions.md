# Task: Error Context Loss in Error Conversions

## Location
Multiple locations:
- `src/lib.rs:74` (list_tools)
- `src/lib.rs:108` (call_tool)
- `src/lib.rs:201` (KodegenConnection::close)
- `src/lib.rs:218` (KodegenConnection::wait)

## Issue Type
- Hidden Errors
- Debugging/Observability
- Error Handling

## Description
Multiple methods use `.map_err(ClientError::from)` to convert errors, which loses important context about what operation was being performed when the error occurred.

## Problem

### Current Code
```rust
pub async fn list_tools(&self) -> Result<Vec<rmcp::model::Tool>, ClientError> {
    timeout(self.default_timeout, self.peer.list_all_tools())
        .await
        .map_err(|_| {
            ClientError::Timeout(format!(
                "list_tools timed out after {}s",
                self.default_timeout.as_secs()
            ))
        })?
        .map_err(ClientError::from)  // ← Context lost here
}
```

If `self.peer.list_all_tools()` returns an error like:
- `ServiceError("connection closed")`
- `RmcpError("protocol error: invalid message")`

The error is converted to `ClientError`, but we lose the context that this happened during `list_tools()`.

### Impact on Debugging

#### Example 1: Generic Service Error
```rust
let tools = client.list_tools().await?;

// Error: "Service error: connection closed"
//
// Questions developers ask:
// - What operation failed? (list_tools, call_tool, something else?)
// - When did the connection close? (before the call, during, after?)
// - Was this the first call or did previous calls succeed?
```

With context:
```
Error: Failed to list tools: Service error: connection closed
```

#### Example 2: Protocol Error During Tool Call
```rust
let result = client.call_tool(tools::READ_FILE, json!({
    "path": "/etc/passwd"
})).await?;

// Error: "Protocol error: invalid message format"
//
// Questions:
// - Was this during the request or response?
// - Which tool was being called?
// - What were the arguments?
```

With context:
```
Error: Failed to call tool 'read_file' with args {"path": "/etc/passwd"}:
       Protocol error: invalid message format
```

#### Example 3: Connection Close Error
```rust
let (client, conn) = create_stdio_client(...).await?;
// ... use client ...
conn.close().await?;

// Error: "Service error: failed to cancel"
//
// Questions:
// - What failed during close?
// - Was the connection already closed?
// - Is this a bug in the library or expected?
```

With context:
```
Error: Failed to gracefully close connection: Service error: failed to cancel
       (connection may have already been closed)
```

## Real-World Impact

### Scenario 1: Production Error Logging
```rust
async fn process_files(client: &KodegenClient, files: &[String]) -> Result<()> {
    for file in files {
        match client.call_tool(tools::READ_FILE, json!({"path": file})).await {
            Ok(result) => process_result(result),
            Err(e) => {
                error!("Operation failed: {}", e);
                // ❌ Log message: "Operation failed: Service error: connection closed"
                // ❓ Which file? Which operation? When did this happen?
            }
        }
    }
}
```

With context:
```rust
// ✅ Log message: "Operation failed during call_tool('read_file', {\"path\": \"/etc/shadow\"}):
//                 Service error: connection closed"
// ✅ Clear what happened, which file, what operation
```

### Scenario 2: Error Monitoring / Alerting
Without context, all errors look the same in monitoring:
```
10:30:15 - ClientError: Service error: connection closed
10:30:16 - ClientError: Service error: connection closed
10:30:17 - ClientError: Service error: connection closed
```

Are these:
- All from list_tools?
- All from call_tool with different tools?
- A mix of different operations?
- Connection closing repeatedly or same error propagating?

With context:
```
10:30:15 - list_tools failed: Service error: connection closed
10:30:16 - call_tool('read_file') failed: Service error: connection closed
10:30:17 - call_tool('execute_sql') failed: Service error: connection closed
```

Now we can see:
- Multiple operations affected (not just one)
- Connection closed during active operations (not during idle time)
- Pattern of errors (which tools are affected)

### Scenario 3: User-Facing Error Messages
```rust
// User runs CLI command
$ myapp read-file /etc/passwd

Error: Service error: connection closed

// User confusion:
// - What is a "service error"?
// - Why did the connection close?
// - What should I do to fix this?
```

With context:
```
$ myapp read-file /etc/passwd

Error: Failed to read file /etc/passwd:
       The connection to the MCP server closed unexpectedly.
       This might mean the server process crashed or was killed.
       Check server logs for more details.
```

## Recommended Fixes

### Fix 1: Add Context to Each Error Conversion
```rust
pub async fn list_tools(&self) -> Result<Vec<rmcp::model::Tool>, ClientError> {
    timeout(self.default_timeout, self.peer.list_all_tools())
        .await
        .map_err(|_| {
            ClientError::Timeout(format!(
                "list_tools timed out after {}s",
                self.default_timeout.as_secs()
            ))
        })?
        .map_err(|e| {
            ClientError::Operation(format!("Failed to list tools: {}", e))
        })
}

pub async fn call_tool(
    &self,
    name: &str,
    arguments: serde_json::Value,
) -> Result<CallToolResult, ClientError> {
    let call = self.peer.call_tool(CallToolRequestParam {
        name: name.to_string().into(),
        arguments: match arguments {
            serde_json::Value::Object(map) => Some(map),
            _ => None,
        },
    });

    timeout(self.default_timeout, call)
        .await
        .map_err(|_| {
            ClientError::Timeout(format!(
                "Tool '{}' timed out after {}s",
                name,
                self.default_timeout.as_secs()
            ))
        })?
        .map_err(|e| {
            ClientError::Operation(format!(
                "Failed to call tool '{}' with args {}: {}",
                name,
                serde_json::to_string(&arguments).unwrap_or_else(|_| "...".to_string()),
                e
            ))
        })
}
```

### Fix 2: Use anyhow/eyre for Error Context
```rust
use anyhow::{Context, Result};

pub async fn list_tools(&self) -> Result<Vec<rmcp::model::Tool>> {
    timeout(self.default_timeout, self.peer.list_all_tools())
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "list_tools timed out after {}s",
                self.default_timeout.as_secs()
            )
        })?
        .context("Failed to list tools")?
}

pub async fn call_tool(
    &self,
    name: &str,
    arguments: serde_json::Value,
) -> Result<CallToolResult> {
    // ...
    timeout(self.default_timeout, call)
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "Tool '{}' timed out after {}s",
                name,
                self.default_timeout.as_secs()
            )
        })?
        .with_context(|| format!("Failed to call tool '{}' with args {}", name, arguments))?
}
```

**Pros**: Standard Rust error handling pattern
**Cons**: Changes error type (breaking change)

### Fix 3: Add Context as Separate Field in Error Enum
```rust
#[derive(Error, Debug)]
pub enum ClientError {
    // ... existing variants ...

    #[error("{context}: {source}")]
    WithContext {
        context: String,
        #[source]
        source: Box<ClientError>,
    },
}

impl ClientError {
    pub fn context(self, context: impl Into<String>) -> Self {
        ClientError::WithContext {
            context: context.into(),
            source: Box::new(self),
        }
    }
}

// Usage:
pub async fn list_tools(&self) -> Result<Vec<rmcp::model::Tool>, ClientError> {
    timeout(self.default_timeout, self.peer.list_all_tools())
        .await
        .map_err(|_| {
            ClientError::Timeout(format!(
                "list_tools timed out after {}s",
                self.default_timeout.as_secs()
            ))
        })?
        .map_err(ClientError::from)
        .map_err(|e| e.context("Failed to list tools"))
}
```

## Recommended Approach

**Fix 1** is recommended for this library because:
1. Minimal change to existing code
2. No breaking changes to error types
3. Immediate improvement in error messages
4. Can add a new error variant `ClientError::Operation(String)` if needed

Alternative: If the library is willing to break API compatibility, **Fix 2** (using anyhow) is the most idiomatic Rust approach.

## New Error Variant

Add a new variant to `ClientError`:

```rust
#[derive(Error, Debug)]
pub enum ClientError {
    // ... existing variants ...

    #[error("Operation failed: {0}")]
    Operation(String),
}
```

Or make errors more specific:

```rust
#[error("Failed to list tools: {0}")]
ListToolsError(String),

#[error("Failed to call tool '{tool}': {error}")]
CallToolError {
    tool: String,
    error: String,
},

#[error("Failed to close connection: {0}")]
CloseConnectionError(String),
```

## Testing

```rust
#[tokio::test]
async fn test_error_context_preserved() {
    // Create client that will fail
    let (client, _conn) = create_failing_client().await;

    let result = client.list_tools().await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();

    // Verify error message includes context
    assert!(err_msg.contains("list tools"));
    assert!(err_msg.contains("Failed to") || err_msg.contains("Error during"));
}

#[tokio::test]
async fn test_call_tool_error_includes_tool_name() {
    let (client, _conn) = create_failing_client().await;

    let result = client.call_tool(tools::READ_FILE, json!({"path": "/test"})).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();

    // Verify error includes tool name
    assert!(err_msg.contains("read_file"));
}
```

## Priority
**MEDIUM-HIGH** - Significantly improves debugging and production error handling

## Related Tasks
- Task 002: Missing stderr handling (both affect observability)
- Task 004: No exit status checking (both provide missing diagnostic info)

## Implementation Checklist
- [ ] Add `ClientError::Operation` variant
- [ ] Update `list_tools()` to add context
- [ ] Update `call_tool()` to add context (include tool name and args)
- [ ] Update `call_tool_typed()` to add context
- [ ] Update `KodegenConnection::close()` to add context
- [ ] Update `KodegenConnection::wait()` to add context
- [ ] Add tests verifying error messages include context
- [ ] Update documentation showing improved error messages
