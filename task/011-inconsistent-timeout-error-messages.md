# Task: Inconsistent Timeout Error Messages

## Location
`src/lib.rs:66-75` (list_tools timeout error)
`src/lib.rs:99-107` (call_tool timeout error)

## Issue Type
- Code Clarity
- Inconsistency
- User Experience

## Description
The timeout error messages for `list_tools` and `call_tool` follow different patterns, making error handling and parsing inconsistent.

## Current Code

### list_tools Error
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
        .map_err(ClientError::from)
}
```

Error message: `"list_tools timed out after 30s"`

### call_tool Error
```rust
pub async fn call_tool(
    &self,
    name: &str,
    arguments: serde_json::Value,
) -> Result<CallToolResult, ClientError> {
    // ...
    timeout(self.default_timeout, call)
        .await
        .map_err(|_| {
            ClientError::Timeout(format!(
                "Tool '{}' timed out after {}s",
                name,
                self.default_timeout.as_secs()
            ))
        })?
        .map_err(ClientError::from)
}
```

Error message: `"Tool 'read_file' timed out after 30s"`

## Problem

### Inconsistency
- `list_tools` error: `"list_tools timed out after 30s"`
- `call_tool` error: `"Tool 'read_file' timed out after 30s"`

Different formats:
1. `list_tools` uses raw function name (`list_tools`)
2. `call_tool` uses capitalized descriptor (`Tool '...'`)
3. One is a method name, other is a user-provided string

### Why This Matters

#### Issue 1: Error Parsing
If users want to parse error messages programmatically:

```rust
match err.to_string().as_str() {
    s if s.contains("list_tools timed out") => {
        // Handle list_tools timeout
    }
    s if s.contains("Tool") && s.contains("timed out") => {
        // Handle call_tool timeout
        // But how to extract the tool name?
    }
    _ => {}
}
```

Inconsistent format makes parsing harder.

#### Issue 2: User-Facing Messages
When shown to users:

```
Error: list_tools timed out after 30s
```

vs.

```
Error: Tool 'execute_sql' timed out after 30s
```

The first looks like an internal function name (confusing for non-developers), the second looks like a user-facing message.

#### Issue 3: Logging and Monitoring
When aggregating errors in monitoring systems:

```
10:30:15 - list_tools timed out after 30s
10:30:16 - Tool 'read_file' timed out after 30s
10:30:17 - Tool 'execute_sql' timed out after 30s
```

It's not immediately clear these are all timeout errors that should be grouped together.

#### Issue 4: I18N/Localization
If error messages need to be translated:

```rust
// Needs different translation keys
match err {
    ClientError::Timeout(msg) if msg.contains("list_tools") => {
        translate!("error.list_tools_timeout", duration)
    }
    ClientError::Timeout(msg) if msg.contains("Tool") => {
        // How to extract tool name from message?
        translate!("error.tool_timeout", tool, duration)
    }
    // ...
}
```

## Recommended Fixes

### Option 1: Consistent Format (Simple)
Make both follow the same pattern:

```rust
pub async fn list_tools(&self) -> Result<Vec<rmcp::model::Tool>, ClientError> {
    timeout(self.default_timeout, self.peer.list_all_tools())
        .await
        .map_err(|_| {
            ClientError::Timeout(format!(
                "Operation 'list_tools' timed out after {}s",
                self.default_timeout.as_secs()
            ))
        })?
        .map_err(ClientError::from)
}

pub async fn call_tool(
    &self,
    name: &str,
    arguments: serde_json::Value,
) -> Result<CallToolResult, ClientError> {
    // ...
    timeout(self.default_timeout, call)
        .await
        .map_err(|_| {
            ClientError::Timeout(format!(
                "Operation 'call_tool({})' timed out after {}s",
                name,
                self.default_timeout.as_secs()
            ))
        })?
        .map_err(ClientError::from)
}
```

Result:
- `"Operation 'list_tools' timed out after 30s"`
- `"Operation 'call_tool(read_file)' timed out after 30s"`

**Pros**: Consistent, easy to parse
**Cons**: Longer messages, less user-friendly

### Option 2: Structured Timeout Variant (Best)
Change `ClientError::Timeout` to carry structured data:

```rust
#[derive(Error, Debug)]
pub enum ClientError {
    // ... existing variants ...

    #[error("{operation} timed out after {}s", .duration.as_secs())]
    Timeout {
        operation: String,
        duration: Duration,
    },

    // Old variant for backward compatibility (deprecated)
    #[deprecated = "Use Timeout { operation, duration } instead"]
    #[error("Operation timed out: {0}")]
    TimeoutMessage(String),
}
```

Usage:
```rust
pub async fn list_tools(&self) -> Result<Vec<rmcp::model::Tool>, ClientError> {
    timeout(self.default_timeout, self.peer.list_all_tools())
        .await
        .map_err(|_| ClientError::Timeout {
            operation: "list_tools".to_string(),
            duration: self.default_timeout,
        })?
        .map_err(ClientError::from)
}

pub async fn call_tool(
    &self,
    name: &str,
    arguments: serde_json::Value,
) -> Result<CallToolResult, ClientError> {
    // ...
    timeout(self.default_timeout, call)
        .await
        .map_err(|_| ClientError::Timeout {
            operation: format!("call_tool('{}')", name),
            duration: self.default_timeout,
        })?
        .map_err(ClientError::from)
}
```

Benefits:
```rust
// Easy to match on timeout errors
match err {
    ClientError::Timeout { operation, duration } => {
        eprintln!("Timeout in {}: {}s", operation, duration.as_secs());
        // Can extract operation name programmatically!
    }
    // ...
}

// Logging with structure
match err {
    ClientError::Timeout { operation, duration } => {
        tracing::error!(
            operation = %operation,
            timeout_secs = duration.as_secs(),
            "Operation timed out"
        );
        // ✅ Structured logging fields!
    }
    // ...
}
```

**Pros**:
- Structured data
- Easy to parse
- Good for monitoring
- Type-safe

**Cons**:
- Breaking change (enum shape changes)
- Need to migrate old code

### Option 3: Separate Error Variants
```rust
#[derive(Error, Debug)]
pub enum ClientError {
    // ... existing variants ...

    #[error("Failed to list tools: operation timed out after {}s", .0.as_secs())]
    ListToolsTimeout(Duration),

    #[error("Tool '{tool}' timed out after {}s", .duration.as_secs())]
    CallToolTimeout {
        tool: String,
        duration: Duration,
    },
}
```

**Pros**: Very type-safe, easy to match
**Cons**: Too many variants, doesn't scale

### Option 4: Add Context (with Option 2)
Combine structured timeout with operation context from Task 007:

```rust
#[error("{context}: {source}")]
TimeoutWithContext {
    context: String,
    source: Box<ClientError>,
},
```

Usage:
```rust
timeout(self.default_timeout, self.peer.list_all_tools())
    .await
    .map_err(|_| ClientError::Timeout {
        operation: "list_tools",
        duration: self.default_timeout,
    })?
    .map_err(|e| ClientError::from(e).with_context("Failed to list tools"))
```

Result:
```
Error: Failed to list tools: list_tools timed out after 30s
```

## Recommended Approach

**Option 2** (Structured Timeout Variant) is recommended:

1. Provides structured data for programmatic handling
2. Consistent error format
3. Easy to extend with more fields (operation type, server info, etc.)
4. Better for monitoring and observability
5. Can be combined with context from Task 007

If breaking changes are not acceptable, use **Option 1** (Consistent Format) as a minimal fix.

## Implementation Steps

1. Update `ClientError` enum with new `Timeout` variant structure
2. Update `list_tools()` to use new structure
3. Update `call_tool()` to use new structure
4. Update `call_tool_typed()` to use new structure
5. Update error tests
6. Update documentation with examples

## Testing

```rust
#[tokio::test]
async fn test_timeout_error_format() {
    let (client, _conn) = create_slow_client().await;

    // Test list_tools timeout
    let err = client.list_tools().await.unwrap_err();
    match err {
        ClientError::Timeout { operation, duration } => {
            assert_eq!(operation, "list_tools");
            assert_eq!(duration.as_secs(), 30);
        }
        _ => panic!("Expected Timeout error"),
    }

    // Test call_tool timeout
    let err = client.call_tool(tools::READ_FILE, json!({})).await.unwrap_err();
    match err {
        ClientError::Timeout { operation, duration } => {
            assert!(operation.contains("call_tool"));
            assert!(operation.contains("read_file"));
            assert_eq!(duration.as_secs(), 30);
        }
        _ => panic!("Expected Timeout error"),
    }
}

#[test]
fn test_timeout_error_message_consistent() {
    let err1 = ClientError::Timeout {
        operation: "list_tools".to_string(),
        duration: Duration::from_secs(30),
    };

    let err2 = ClientError::Timeout {
        operation: "call_tool('read_file')".to_string(),
        duration: Duration::from_secs(30),
    };

    let msg1 = err1.to_string();
    let msg2 = err2.to_string();

    // Both should follow same format
    assert!(msg1.contains("timed out after"));
    assert!(msg2.contains("timed out after"));
    assert!(msg1.contains("30s"));
    assert!(msg2.contains("30s"));
}
```

## Priority
**LOW-MEDIUM** - Quality of life improvement, doesn't affect functionality

## Related Tasks
- Task 007: Error context loss (related error handling improvements)
- Task 011: No validation of tool names (both affect error messages)

## Migration Guide

If breaking change:

```rust
// Before:
match err {
    ClientError::Timeout(msg) => {
        eprintln!("Timeout: {}", msg);
    }
}

// After:
match err {
    ClientError::Timeout { operation, duration } => {
        eprintln!("Timeout in {}: {}s", operation, duration.as_secs());
    }
}
```
