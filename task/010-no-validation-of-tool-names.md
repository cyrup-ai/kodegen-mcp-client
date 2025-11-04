# Task: No Validation of Tool Names

## Location
`src/lib.rs:83-109` (call_tool method)
`src/lib.rs:132-155` (call_tool_typed method)

## Issue Type
- Code Clarity
- Runtime Error Detection
- Developer Experience

## Description
The `call_tool` and `call_tool_typed` methods accept any string as a tool name without validation. While this provides flexibility, it means typos and invalid tool names are only caught at runtime when the server responds with an error.

## Problem

### Current Behavior
```rust
// Typo: "read_fiel" instead of "read_file"
let result = client.call_tool("read_fiel", json!({
    "path": "/etc/passwd"
})).await?;

// ❌ Error only at runtime:
// "Service error: Unknown tool 'read_fiel'"
```

The library provides constants in `tools.rs`:
```rust
pub const READ_FILE: &str = "read_file";
```

But using them is optional:
```rust
// ✅ Type-safe (typo won't compile)
client.call_tool(tools::READ_FILE, args).await?;

// ❌ Not type-safe (typo compiles but fails at runtime)
client.call_tool("read_file", args).await?;
```

### Why This Matters

1. **Late Error Detection**: Typos aren't caught until the code runs and reaches that specific call

2. **No Autocomplete**: IDEs can't provide autocomplete for string literals

3. **Refactoring Difficulty**: If a tool name changes, string literals won't be updated

4. **Testing Gap**: Tests might not cover all tool names, so typos slip through

## Real-World Impact

### Scenario 1: Typo in Production Code
```rust
async fn read_config_file(client: &KodegenClient, path: &str) -> Result<String> {
    let result = client.call_tool("read_flie", json!({  // ← Typo!
        "path": path
    })).await?;

    // Extract content...
}

// Unit tests pass (mocked client)
// Integration tests don't cover this path
// Deploys to production
// First real user triggers this code path
// ❌ Production error: "Unknown tool 'read_flie'"
```

### Scenario 2: Copy-Paste Error
```rust
// Developer copies code and forgets to update tool name
async fn list_files(client: &KodegenClient, dir: &str) -> Result<Vec<String>> {
    let result = client.call_tool("read_file", json!({  // ← Wrong tool!
        "path": dir
    })).await?;

    // Expects list_directory response but gets read_file response
    // Parser fails with confusing error
}
```

### Scenario 3: IDE Doesn't Catch Mistake
```rust
// Developer types:
client.call_tool("execute_sq", json!({...})).await?;
                      // ↑ Incomplete, should be "execute_sql"

// IDE shows no error (it's a valid string)
// Code compiles
// Fails at runtime
```

### Scenario 4: Dynamic Tool Names (Valid Use Case)
```rust
// Sometimes dynamic tool names are needed:
let tool_name = format!("{}_{}", operation, target);
client.call_tool(&tool_name, args).await?;

// Or from configuration:
let tool_name = config.get_tool_name();
client.call_tool(tool_name, args).await?;
```

These are valid use cases that should be supported.

## Considered Solutions

### Option 1: Make Tool Names Strongly Typed
```rust
/// Type-safe tool name
#[derive(Debug, Clone, Copy)]
pub struct ToolName(&'static str);

impl ToolName {
    pub const READ_FILE: ToolName = ToolName("read_file");
    pub const WRITE_FILE: ToolName = ToolName("write_file");
    // ... all 75+ tools
}

impl From<ToolName> for &'static str {
    fn from(name: ToolName) -> Self {
        name.0
    }
}

// API:
pub async fn call_tool(
    &self,
    name: ToolName,  // ← Now type-safe!
    arguments: serde_json::Value,
) -> Result<CallToolResult, ClientError> {
    // ...
}

// Usage:
client.call_tool(ToolName::READ_FILE, args).await?;
```

**Pros**:
- Compile-time type safety
- IDE autocomplete
- Can't typo

**Cons**:
- Breaking change (existing code won't compile)
- No support for dynamic tool names
- Verbose

### Option 2: Enum for Tool Names
```rust
#[derive(Debug, Clone, Copy)]
pub enum Tool {
    ReadFile,
    WriteFile,
    ExecuteSql,
    // ... 75+ variants
}

impl Tool {
    pub fn name(&self) -> &'static str {
        match self {
            Tool::ReadFile => "read_file",
            Tool::WriteFile => "write_file",
            // ...
        }
    }
}

pub async fn call_tool_safe(
    &self,
    tool: Tool,
    arguments: serde_json::Value,
) -> Result<CallToolResult, ClientError> {
    self.call_tool(tool.name(), arguments).await
}
```

**Pros**:
- Type-safe
- Can still use strings for dynamic names

**Cons**:
- Breaking change for primary API
- Duplication (enum + strings)
- Maintenance burden (keep enum in sync with server)

### Option 3: Validation Function (Non-Breaking)
```rust
/// Validate that a tool name is known
///
/// Returns Ok(()) if the tool name matches one of the known tools,
/// or Err with suggestions if not.
pub fn validate_tool_name(name: &str) -> Result<(), String> {
    const KNOWN_TOOLS: &[&str] = &[
        "read_file",
        "write_file",
        "execute_sql",
        // ... all 75+ tools
    ];

    if KNOWN_TOOLS.contains(&name) {
        Ok(())
    } else {
        // Find close matches (fuzzy matching)
        let suggestions = find_similar_tools(name, KNOWN_TOOLS);
        Err(format!(
            "Unknown tool '{}'. Did you mean: {}?",
            name,
            suggestions.join(", ")
        ))
    }
}

// Optional: validate in call_tool
pub async fn call_tool(
    &self,
    name: &str,
    arguments: serde_json::Value,
) -> Result<CallToolResult, ClientError> {
    // Optional validation (could be behind a feature flag)
    #[cfg(feature = "validate-tool-names")]
    if let Err(e) = validate_tool_name(name) {
        return Err(ClientError::InvalidToolName(e));
    }

    // ... existing implementation
}
```

**Pros**:
- Not breaking change
- Provides helpful error messages with suggestions
- Can be opt-in via feature flag

**Cons**:
- Still allows runtime errors (just catches them earlier)
- Requires maintaining list of valid tools
- Adds runtime overhead (string comparison)

### Option 4: Proc Macro for Compile-Time Validation
```rust
// Use proc macro to validate at compile time
client.call_tool!(tools::READ_FILE, args).await?;

// Or
call_tool!(client, "read_file", args).await?;
//                  ^^^^^^^^^^^ Validated at compile time
```

**Pros**:
- Compile-time validation
- Feels like regular function call

**Cons**:
- Complex implementation
- Confusing error messages
- Can't support dynamic names

## Recommended Approach

**Option 3** (Validation Function) with the following additions:

1. **Don't validate by default** (to avoid breaking changes and support dynamic names)

2. **Provide opt-in validation** via builder method:
   ```rust
   let (client, conn) = StdioClientBuilder::new("node")
       .arg("server.js")
       .validate_tool_names(true)  // ← Opt-in
       .build()
       .await?;
   ```

3. **Provide standalone validation helper** for users who want to validate explicitly:
   ```rust
   use kodegen_mcp_client::validate_tool_name;

   let tool_name = "read_flie";
   validate_tool_name(tool_name)?;  // ← Fails here with suggestion
   client.call_tool(tool_name, args).await?;
   ```

4. **Document that using constants is recommended**:
   ```rust
   /// Call a tool by name with JSON arguments
   ///
   /// **Recommendation**: Use constants from the [`tools`] module instead of
   /// string literals to avoid typos:
   ///
   /// ```ignore
   /// // Recommended (type-safe)
   /// client.call_tool(tools::READ_FILE, args).await?;
   ///
   /// // Not recommended (typo-prone)
   /// client.call_tool("read_file", args).await?;
   /// ```
   ///
   /// If you need to validate a tool name at runtime, use [`validate_tool_name`].
   ```

## Alternative: Improve Documentation

If we decide not to add validation, at least improve documentation:

```rust
// In tools.rs:
//! # Tool Name Constants
//!
//! This module provides constants for all 75 KODEGEN MCP tools.
//!
//! **Always use these constants instead of string literals** to avoid typos:
//!
//! ```ignore
//! // ✅ Good - compile-time checked
//! client.call_tool(tools::READ_FILE, args).await?;
//!
//! // ❌ Bad - typos not caught until runtime
//! client.call_tool("read_flie", args).await?;  // ← Typo!
//! ```

// In lib.rs call_tool method:
/// **Important**: Use constants from [`tools`] module instead of string literals
/// to prevent typos. For example, use `tools::READ_FILE` instead of `"read_file"`.
pub async fn call_tool(
    &self,
    name: &str,
    arguments: serde_json::Value,
) -> Result<CallToolResult, ClientError> {
```

## Testing

```rust
#[test]
fn test_validate_tool_name_known() {
    assert!(validate_tool_name("read_file").is_ok());
    assert!(validate_tool_name("execute_sql").is_ok());
}

#[test]
fn test_validate_tool_name_unknown() {
    let result = validate_tool_name("read_flie");  // Typo
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("read_file"));  // Suggestion
}

#[test]
fn test_validate_tool_name_suggestions() {
    let result = validate_tool_name("execute_sq");  // Incomplete
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("execute_sql"));  // Suggestion
}
```

## Priority
**LOW-MEDIUM** - Quality of life improvement, not a critical issue

## Related Tasks
- Task 007: Error context loss (better error messages help here too)
- Task 003: String allocation (if we make tool names typed, allocation goes away)

## References
- did-you-mean crate: https://crates.io/crates/did-you-mean (for fuzzy matching)
- strsim crate: https://crates.io/crates/strsim (for string similarity)
