# Task: String Allocation on Every Tool Call (Hot Path Performance)

## Location
`src/lib.rs:88-97` (call_tool method)

## Issue Type
- **Runtime Performance** (hot path)
- Memory allocation inefficiency

## Description
The `call_tool` method allocates a new `String` on every invocation:

```rust
pub async fn call_tool(
    &self,
    name: &str,
    arguments: serde_json::Value,
) -> Result<CallToolResult, ClientError> {
    let call = self.peer.call_tool(CallToolRequestParam {
        // name.to_string() allocation is required because CallToolRequestParam
        // expects Cow<'static, str>. Cannot use borrowed reference from &str parameter
        // as it doesn't satisfy the 'static lifetime requirement.
        name: name.to_string().into(),  // ← ALLOCATION HERE
        arguments: match arguments {
            serde_json::Value::Object(map) => Some(map),
            _ => None,
        },
    });
    // ...
}
```

## Problem

### 1. Unnecessary Allocation in Hot Path
- `call_tool` is one of the most frequently called methods in the entire library
- Every call allocates a new String, even when the tool name is a static constant (which it usually is)
- String allocation involves heap allocation, which is relatively expensive

### 2. API Design Forces Inefficiency
The method signature accepts `&str`, which seems like the right choice for flexibility. However, this forces a `.to_string()` conversion because the underlying `CallToolRequestParam` requires `Cow<'static, str>`.

### 3. Lost Opportunity for Zero-Cost Abstraction
When users call `client.call_tool(tools::READ_FILE, ...)`, the constant `tools::READ_FILE` is a `&'static str`, which could be used directly without allocation. But the current API throws away this information.

## Performance Impact

### Benchmarks (estimated)
- String allocation: ~20-50ns per call (depending on allocator)
- String copy: ~10-30ns for typical tool names (10-30 chars)
- Total overhead: ~30-80ns per tool call

### Real-World Scenarios

#### Scenario 1: Batch File Operations
```rust
// Read 1000 files
for file_path in file_paths {
    let result = client.call_tool(tools::READ_FILE, json!({
        "path": file_path
    })).await?;
}
// Current: 1000 allocations = ~30-80µs wasted
// Optimal: 0 allocations
```

#### Scenario 2: High-Frequency Terminal Polling
```rust
// Poll terminal output every 100ms
loop {
    let output = client.call_tool(tools::READ_TERMINAL_OUTPUT, json!({
        "pid": pid
    })).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
}
// Current: 10 allocations/second indefinitely
// Optimal: 0 allocations
```

#### Scenario 3: Parallel Operations
```rust
// Execute 100 SQL queries in parallel
let futures = queries.into_iter().map(|query| {
    client.call_tool(tools::EXECUTE_SQL, json!({"query": query}))
});
let results = futures::future::join_all(futures).await;
// Current: 100 allocations
// Optimal: 0 allocations
```

## Root Cause Analysis

The issue stems from the API design of `CallToolRequestParam` in the `rmcp` crate:

```rust
pub struct CallToolRequestParam {
    pub name: Cow<'static, str>,
    // ...
}
```

The `'static` lifetime requirement means the string must either:
1. Be a string literal (`&'static str`)
2. Be an owned `String` that's been leaked or converted to `'static`

Since our `call_tool` method accepts `&str` (non-static), we're forced to allocate.

## Recommended Fixes

### Option 1: Generic Parameter with Into<Cow<'static, str>> (Best)
Accept either `&'static str` or `String`:

```rust
pub async fn call_tool<'a>(
    &self,
    name: impl Into<Cow<'static, str>>,
    arguments: serde_json::Value,
) -> Result<CallToolResult, ClientError> {
    let call = self.peer.call_tool(CallToolRequestParam {
        name: name.into(),  // Zero-cost for &'static str
        arguments: match arguments {
            serde_json::Value::Object(map) => Some(map),
            _ => None,
        },
    });
    // ...
}
```

Usage remains the same:
```rust
// Zero-cost (const)
client.call_tool(tools::READ_FILE, args).await?;

// Allocates (when needed)
let tool_name = format!("dynamic_{}", suffix);
client.call_tool(tool_name, args).await?;

// Zero-cost (string literal)
client.call_tool("read_file", args).await?;
```

### Option 2: Separate Methods
Provide two methods:

```rust
// For static strings (zero allocation)
pub async fn call_tool_static(
    &self,
    name: &'static str,
    arguments: serde_json::Value,
) -> Result<CallToolResult, ClientError> {
    self.call_tool_impl(Cow::Borrowed(name), arguments).await
}

// For dynamic strings (allocates)
pub async fn call_tool(
    &self,
    name: impl Into<String>,
    arguments: serde_json::Value,
) -> Result<CallToolResult, ClientError> {
    self.call_tool_impl(Cow::Owned(name.into()), arguments).await
}

// Shared implementation
async fn call_tool_impl(
    &self,
    name: Cow<'static, str>,
    arguments: serde_json::Value,
) -> Result<CallToolResult, ClientError> {
    // ...
}
```

### Option 3: Cache Tool Names (Hack)
Use a lazy static cache of tool names:

```rust
use once_cell::sync::Lazy;
use std::collections::HashMap;

static TOOL_NAME_CACHE: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    // Pre-populate with all 75+ tool names
    let mut map = HashMap::new();
    map.insert("read_file", "read_file");
    // ... all other tools
    map
});

pub async fn call_tool(&self, name: &str, ...) -> ... {
    let static_name = TOOL_NAME_CACHE.get(name).copied()
        .unwrap_or_else(|| {
            // Fallback: allocate for dynamic names
            Box::leak(name.to_string().into_boxed_str())
        });
    // Use static_name...
}
```

**Warning**: This leaks memory for dynamic tool names!

## Recommended Approach
**Option 1** is strongly recommended:
- Maintains API compatibility (mostly)
- Zero-cost for the common case (static tool names)
- Allows dynamic tool names when needed
- Uses standard Rust idioms (`Into<Cow<>>`)

## Implementation Steps
1. Change method signature to `name: impl Into<Cow<'static, str>>`
2. Remove `.to_string()` call
3. Update call_tool_typed similarly
4. Add compile-time tests to verify zero-allocation for static strings
5. Add benchmark to measure performance improvement

## Testing
```rust
#[test]
fn test_call_tool_with_static_str() {
    // Should compile without allocation
    client.call_tool(tools::READ_FILE, json!({})).await;
}

#[test]
fn test_call_tool_with_dynamic_string() {
    // Should compile with allocation
    let name = format!("tool_{}", 42);
    client.call_tool(name, json!({})).await;
}

#[bench]
fn bench_call_tool_allocation() {
    // Measure allocation count
}
```

## Priority
**MEDIUM-HIGH** - Affects every tool call, but the absolute overhead is small (tens of nanoseconds)

## Compatibility Note
Changing to `impl Into<Cow<'static, str>>` is **not** a breaking change because:
- `&'static str` implements `Into<Cow<'static, str>>`
- `String` implements `Into<Cow<'static, str>>`
- `&str` does NOT implement `Into<Cow<'static, str>>`, but this forces callers to be explicit about allocation

If we want to maintain `&str` support (non-breaking), we'd need a different approach or accept the allocation cost.
