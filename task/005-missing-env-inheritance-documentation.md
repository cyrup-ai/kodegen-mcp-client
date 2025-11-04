# Task: Missing Documentation on Environment Variable Inheritance

## Location
`src/transports/stdio.rs:14-51` (StdioClientBuilder documentation)
`src/transports/stdio.rs:108-141` (env/envs method documentation)

## Issue Type
- Code Clarity
- Documentation
- API Usability

## Description
The `StdioClientBuilder` documentation does not clearly explain how environment variables work for spawned child processes. Specifically, it doesn't document that:

1. **Child processes inherit the parent's environment by default**
2. Variables added via `.env()` or `.envs()` are **added to** (not replacing) the inherited environment
3. There's no way to start with a clean environment (no `.env_clear()` method)
4. There's no way to remove specific inherited variables (no `.env_remove()` method)

## Problem

### 1. Ambiguous Behavior
Users reading the current documentation might reasonably assume:
- ❌ "If I don't call `.env()`, the child starts with an empty environment"
- ❌ "Calling `.env()` replaces the inherited environment"
- ❌ "I need to explicitly pass all environment variables the child needs"

The actual behavior (inheritance + additive) is not documented.

### 2. Missing Use Case Guidance
The documentation doesn't explain:
- How to ensure a child doesn't inherit sensitive environment variables (like `AWS_SECRET_KEY`)
- How to override an inherited variable
- How to start with a clean environment if desired

### 3. Inconsistent with Other APIs
Some process spawning APIs (like `std::process::Command::env_clear()`) provide explicit control over environment inheritance. The lack of such methods here is not explained.

## Real-World Impact

### Scenario 1: Sensitive Environment Variable Leakage
```rust
// Parent process has sensitive credentials
std::env::set_var("DATABASE_PASSWORD", "super_secret");
std::env::set_var("AWS_SECRET_KEY", "very_secret");

// User spawns child, assuming clean environment
let (client, _conn) = StdioClientBuilder::new("node")
    .arg("mcp-server.js")
    .env("NODE_ENV", "production")
    .build()
    .await?;

// ⚠️ PROBLEM: Child process now has access to:
//   - DATABASE_PASSWORD=super_secret
//   - AWS_SECRET_KEY=very_secret
//   - NODE_ENV=production
//
// User expected only NODE_ENV!
```

### Scenario 2: Debugging Confusion
```rust
// Developer's environment has DEBUG=* set globally
std::env::set_var("DEBUG", "*");

// User creates child, trying to disable debugging
let (client, _conn) = StdioClientBuilder::new("node")
    .arg("mcp-server.js")
    // User doesn't call .env("DEBUG", ""), expecting clean environment
    .build()
    .await?;

// ⚠️ Child still has DEBUG=* (inherited)
// User is confused why debug logs still appear
```

### Scenario 3: PATH Override Confusion
```rust
// User wants custom PATH for child
let (client, _conn) = StdioClientBuilder::new("python3")
    .arg("mcp-server.py")
    .env("PATH", "/custom/bin")  // Intended to REPLACE PATH
    .build()
    .await?;

// ⚠️ This actually OVERRIDES PATH entirely (Tokio behavior)
// But user might not know if inherited PATH is replaced or prepended
```

## Current Documentation Issues

### Issue 1: Module-Level Documentation Missing
The module (`stdio.rs`) has no overview documentation explaining environment handling.

### Issue 2: Builder Documentation Unclear
```rust
/// Builder for creating stdio-based MCP clients
///
/// Provides a fluent API for configuring child process execution with full control over:
/// - Command and arguments
/// - Environment variables
/// - Working directory
/// - Operation timeout
/// - Client identification
```

This says "full control over environment variables" but doesn't explain:
- What's the default environment?
- How are variables added/removed?
- What happens to inherited variables?

### Issue 3: Method Documentation Incomplete
```rust
/// Add a single environment variable
///
/// # Example
/// ```ignore
/// let builder = StdioClientBuilder::new("node")
///     .env("NODE_ENV", "production")
///     .env("DEBUG", "1");
/// ```
pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
```

This doesn't explain:
- Variables are **added** to inherited environment
- Calling `.env("VAR", "value")` **overrides** an inherited `VAR` if it exists
- There's no way to remove an inherited variable

## Recommended Fixes

### Fix 1: Add Module Documentation
```rust
//! # Stdio Transport
//!
//! This module provides a transport for running MCP servers as child processes
//! communicating over stdin/stdout.
//!
//! ## Environment Variable Handling
//!
//! **By default, child processes inherit all environment variables from the parent process.**
//!
//! When you call `.env(key, value)` or `.envs(map)`, you are **adding** or **overriding**
//! variables in the inherited environment. You cannot remove inherited variables or
//! start with a clean environment.
//!
//! ### Examples
//!
//! ```
//! # Inherit parent environment + add variables
//! let (client, _conn) = StdioClientBuilder::new("node")
//!     .arg("server.js")
//!     .env("NODE_ENV", "production")  // Adds NODE_ENV
//!     .build()
//!     .await?;
//! // Child has: All parent env vars + NODE_ENV=production
//! ```
//!
//! ```
//! # Override inherited variable
//! std::env::set_var("DEBUG", "verbose");
//!
//! let (client, _conn) = StdioClientBuilder::new("node")
//!     .arg("server.js")
//!     .env("DEBUG", "")  // Overrides inherited DEBUG
//!     .build()
//!     .await?;
//! // Child has: All parent env vars + DEBUG="" (overridden)
//! ```
//!
//! ### Security Considerations
//!
//! Be aware that child processes can access **all** parent environment variables,
//! including potentially sensitive ones like:
//! - `AWS_SECRET_ACCESS_KEY`
//! - `DATABASE_PASSWORD`
//! - `GITHUB_TOKEN`
//! - etc.
//!
//! If you need to spawn a child in a controlled environment:
//! 1. Clear sensitive variables in the parent before spawning
//! 2. Run the parent with minimal environment
//! 3. Use a wrapper script that sanitizes the environment
//!
//! **Note**: Unlike `std::process::Command`, this builder does not provide
//! `env_clear()` or `env_remove()` methods. This is a limitation of the
//! underlying `TokioChildProcess` wrapper.
```

### Fix 2: Enhance Builder Documentation
```rust
/// Builder for creating stdio-based MCP clients
///
/// Provides a fluent API for configuring child process execution with control over:
/// - Command and arguments
/// - Environment variables (added to inherited environment)
/// - Working directory
/// - Operation timeout
/// - Client identification
///
/// ## Environment Inheritance
///
/// **Important**: Child processes inherit all environment variables from the parent.
/// Methods like `.env()` **add to** or **override** inherited variables, they do not
/// replace the entire environment.
///
/// See module-level documentation for details and security considerations.
```

### Fix 3: Enhance Method Documentation
```rust
/// Add or override a single environment variable
///
/// The child process will inherit all parent environment variables.
/// This method adds a new variable or overrides an inherited one.
///
/// **Note**: There is no way to remove an inherited variable or start
/// with a clean environment using this builder.
///
/// # Example
///
/// ```ignore
/// // Parent has FOO=inherited
/// let builder = StdioClientBuilder::new("node")
///     .env("FOO", "overridden")  // Overrides inherited FOO
///     .env("BAR", "new");        // Adds new variable BAR
/// // Child will have: FOO=overridden, BAR=new, + all other parent vars
/// ```
pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
```

### Fix 4: Add env_clear() and env_remove() Methods (Optional, Breaking Change)
```rust
/// Start with a clean environment (no inherited variables)
///
/// After calling this method, the child will only have variables
/// explicitly added via `.env()` or `.envs()`.
///
/// # Example
///
/// ```ignore
/// let builder = StdioClientBuilder::new("node")
///     .env_clear()                      // Start clean
///     .env("NODE_ENV", "production")    // Only variable
///     .build()
///     .await?;
/// // Child has ONLY: NODE_ENV=production
/// ```
pub fn env_clear(mut self) -> Self {
    self.clear_env = true;
    self
}

/// Remove an inherited environment variable
///
/// # Example
///
/// ```ignore
/// // Parent has AWS_SECRET_KEY set
/// let builder = StdioClientBuilder::new("node")
///     .env_remove("AWS_SECRET_KEY")  // Don't inherit this
///     .build()
///     .await?;
/// // Child won't have AWS_SECRET_KEY
/// ```
pub fn env_remove(mut self, key: impl Into<String>) -> Self {
    self.removed_envs.insert(key.into());
    self
}
```

Implementation:
```rust
pub struct StdioClientBuilder {
    // ... existing fields ...
    clear_env: bool,
    removed_envs: HashSet<String>,
}

pub async fn build(self) -> Result<...> {
    let mut cmd = Command::new(&self.command);
    cmd.args(&self.args);

    if self.clear_env {
        cmd.env_clear();
    }

    for removed in &self.removed_envs {
        cmd.env_remove(removed);
    }

    if !self.envs.is_empty() {
        cmd.envs(&self.envs);
    }

    // ... rest of implementation
}
```

## Testing

Add tests to verify and document behavior:

```rust
#[tokio::test]
async fn test_env_inheritance() {
    std::env::set_var("TEST_INHERITED", "inherited_value");

    let (client, _conn) = StdioClientBuilder::new("env")
        .build()
        .await?;

    // Verify child can see TEST_INHERITED
    // (requires actual verification mechanism)
}

#[tokio::test]
async fn test_env_override() {
    std::env::set_var("TEST_VAR", "original");

    let (client, _conn) = StdioClientBuilder::new("env")
        .env("TEST_VAR", "overridden")
        .build()
        .await?;

    // Verify child sees TEST_VAR=overridden
}

#[tokio::test]
async fn test_env_clear() {
    std::env::set_var("TEST_VAR", "should_not_inherit");

    let (client, _conn) = StdioClientBuilder::new("env")
        .env_clear()
        .env("ONLY_VAR", "value")
        .build()
        .await?;

    // Verify child only has ONLY_VAR, not TEST_VAR
}
```

## Priority
**MEDIUM-HIGH** - Important for security and correct usage, especially in production environments

## Backward Compatibility

- Adding documentation: **Not breaking**
- Adding `env_clear()` and `env_remove()`: **Not breaking** (new methods)
- Changing default behavior: **WOULD BE BREAKING** (don't do this)

## Related Tasks
- Task 001: Unnecessary conditional (related to env handling)
- Task 008: Resource leak on build error (security: leaked child might have sensitive env)

## References
- tokio::process::Command: https://docs.rs/tokio/latest/tokio/process/struct.Command.html
- std::process::Command::env_clear: https://doc.rust-lang.org/std/process/struct.Command.html#method.env_clear
- Security implications: https://wiki.sei.cmu.edu/confluence/display/c/ENV03-C.+Sanitize+the+environment+when+invoking+external+programs
