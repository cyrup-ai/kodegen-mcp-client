# Task: Missing Builder Validation

## Location
`src/transports/stdio.rs:52-73` (StdioClientBuilder::new)
`src/transports/stdio.rs:219-263` (StdioClientBuilder::build)

## Issue Type
- Error Handling
- Code Clarity
- User Experience

## Description
`StdioClientBuilder::new()` accepts any string as the command, including empty strings and invalid values. These issues are only caught during `build()` with generic error messages.

## Problem

### Current Code
```rust
pub fn new(command: impl Into<String>) -> Self {
    Self {
        command: command.into(),
        args: Vec::new(),
        envs: HashMap::new(),
        current_dir: None,
        timeout: DEFAULT_TIMEOUT,
        client_name: None,
    }
}
```

No validation is performed on `command`.

### Invalid Inputs Accepted

#### Empty Command
```rust
let builder = StdioClientBuilder::new("");
let result = builder.build().await;
// ❌ Error: "Failed to spawn process '': No such file or directory"
```

#### Whitespace-Only Command
```rust
let builder = StdioClientBuilder::new("   ");
let result = builder.build().await;
// ❌ Error: "Failed to spawn process '   ': No such file or directory"
```

#### Path with Null Bytes (Invalid)
```rust
let builder = StdioClientBuilder::new("node\0evil");
let result = builder.build().await;
// ❌ Error: May panic or produce undefined behavior
```

#### Invalid Characters
On Windows, certain characters are invalid in commands:
```rust
let builder = StdioClientBuilder::new("node<>|");
// Error only at spawn time
```

## Real-World Impact

### Scenario 1: Configuration Error
```rust
// Load command from config file
let config = load_config()?;
let command = config.get("mcp_command").unwrap_or("");  // ← Empty string!

let (client, _conn) = StdioClientBuilder::new(command)
    .build()
    .await?;

// Error: "Failed to spawn process '': No such file or directory"
// User: "What process? Why is the name empty?"
```

### Scenario 2: User Input
```rust
// User provides command via CLI
let command = std::env::args().nth(1).unwrap_or_default();

let (client, _conn) = StdioClientBuilder::new(command)
    .build()
    .await?;

// If user forgets to provide command:
// $ myapp
// Error: "Failed to spawn process '': No such file or directory"
//
// Better error:
// Error: Command cannot be empty. Provide a command like 'node' or 'python3'
```

### Scenario 3: Typo Detection
```rust
let (client, _conn) = StdioClientBuilder::new("  node  ")  // ← Extra spaces
    .arg("server.js")
    .build()
    .await?;

// May fail with confusing error depending on OS handling of spaces
```

### Scenario 4: Absolute vs Relative Paths
```rust
// User provides absolute path, not realizing it might not be portable
let (client, _conn) = StdioClientBuilder::new("/usr/local/bin/node")
    .arg("server.js")
    .build()
    .await?;

// Works on developer's machine, fails in Docker container
// Error: "Failed to spawn process '/usr/local/bin/node': No such file or directory"
//
// Could be caught earlier with a warning:
// Warning: Using absolute path '/usr/local/bin/node' may not be portable
```

## Recommended Fixes

### Option 1: Validate in Constructor (Strict)
```rust
impl StdioClientBuilder {
    /// Create a new builder for a stdio-based MCP client
    ///
    /// # Arguments
    ///
    /// * `command` - The command to execute (e.g., "uvx", "node", "python3")
    ///
    /// # Panics
    ///
    /// Panics if command is empty or contains only whitespace.
    /// For fallible validation, use [`try_new`](Self::try_new).
    pub fn new(command: impl Into<String>) -> Self {
        let command = command.into();

        assert!(!command.is_empty(), "Command cannot be empty");
        assert!(command.trim() == command, "Command contains leading/trailing whitespace");

        Self {
            command,
            args: Vec::new(),
            envs: HashMap::new(),
            current_dir: None,
            timeout: DEFAULT_TIMEOUT,
            client_name: None,
        }
    }

    /// Create a new builder with fallible validation
    ///
    /// Returns an error if the command is invalid.
    pub fn try_new(command: impl Into<String>) -> Result<Self, ClientError> {
        let command = command.into();

        if command.is_empty() {
            return Err(ClientError::InvalidConfig(
                "Command cannot be empty".to_string()
            ));
        }

        let trimmed = command.trim();
        if trimmed != command {
            return Err(ClientError::InvalidConfig(
                format!("Command contains whitespace: '{}'", command)
            ));
        }

        if command.contains('\0') {
            return Err(ClientError::InvalidConfig(
                "Command contains null byte".to_string()
            ));
        }

        Ok(Self {
            command,
            args: Vec::new(),
            envs: HashMap::new(),
            current_dir: None,
            timeout: DEFAULT_TIMEOUT,
            client_name: None,
        })
    }
}
```

**Pros**: Early error detection
**Cons**: Panicking constructor is frowned upon, `try_new` pattern is uncommon in Rust

### Option 2: Validate in Build (Better Errors)
```rust
pub async fn build(self) -> Result<(KodegenClient, KodegenConnection), ClientError> {
    // Validate command before trying to spawn
    if self.command.is_empty() {
        return Err(ClientError::InvalidConfig(
            "Command cannot be empty. Specify a command like 'node', 'python3', or 'uvx'".to_string()
        ));
    }

    if self.command.trim() != self.command {
        return Err(ClientError::InvalidConfig(
            format!(
                "Command '{}' contains leading or trailing whitespace. Did you mean '{}'?",
                self.command,
                self.command.trim()
            )
        ));
    }

    if self.command.contains('\0') {
        return Err(ClientError::InvalidConfig(
            "Command contains null byte (invalid)".to_string()
        ));
    }

    // ... rest of build implementation
}
```

**Pros**:
- Not a breaking change
- Better error messages
- No panicking constructor

**Cons**:
- Validation happens late (at build time, not construction time)

### Option 3: Normalize Input
```rust
pub fn new(command: impl Into<String>) -> Self {
    let command = command.into();
    let command = command.trim().to_string();  // Auto-trim

    Self {
        command,
        args: Vec::new(),
        envs: HashMap::new(),
        current_dir: None,
        timeout: DEFAULT_TIMEOUT,
        client_name: None,
    }
}

pub async fn build(self) -> Result<(KodegenClient, KodegenConnection), ClientError> {
    // Validate after normalization
    if self.command.is_empty() {
        return Err(ClientError::InvalidConfig(
            "Command cannot be empty".to_string()
        ));
    }

    // ... rest of build
}
```

**Pros**:
- Forgiving (auto-fixes common mistakes)
- Good error for empty commands

**Cons**:
- Silent modification might be unexpected
- Doesn't catch other invalid inputs

### Option 4: Lint-Level Validation (Advanced)
```rust
impl StdioClientBuilder {
    pub fn new(command: impl Into<String>) -> Self {
        let command = command.into();

        // Emit warnings for suspicious inputs
        if command.is_empty() {
            tracing::warn!("StdioClientBuilder created with empty command");
        }
        if command.trim() != command {
            tracing::warn!("Command contains whitespace: '{}'", command);
        }
        if command.starts_with('/') || command.starts_with('.') {
            tracing::warn!("Command is a path: '{}' (may not be portable)", command);
        }

        Self {
            command,
            args: Vec::new(),
            envs: HashMap::new(),
            current_dir: None,
            timeout: DEFAULT_TIMEOUT,
            client_name: None,
        }
    }
}
```

**Pros**: Non-intrusive, helpful for debugging
**Cons**: Warnings might be ignored or missed

## Recommended Approach

**Combination of Options 2 and 3**:

1. **Normalize** whitespace in constructor (trim)
2. **Validate** thoroughly in `build()` with helpful error messages
3. **Log warnings** for suspicious patterns

```rust
impl StdioClientBuilder {
    pub fn new(command: impl Into<String>) -> Self {
        let command = command.into().trim().to_string();

        // Warn about suspicious patterns
        if command.starts_with('/') || command.starts_with('.') {
            tracing::debug!(
                "Command '{}' is an absolute or relative path (may not be portable)",
                command
            );
        }

        Self {
            command,
            args: Vec::new(),
            envs: HashMap::new(),
            current_dir: None,
            timeout: DEFAULT_TIMEOUT,
            client_name: None,
        }
    }
}

// In build():
pub async fn build(self) -> Result<(KodegenClient, KodegenConnection), ClientError> {
    // Validate command
    if self.command.is_empty() {
        return Err(ClientError::InvalidConfig(
            "Command cannot be empty. Specify a command like 'node', 'python3', or 'uvx'.\n\
             Example: StdioClientBuilder::new(\"node\").arg(\"server.js\")".to_string()
        ));
    }

    if self.command.contains('\0') {
        return Err(ClientError::InvalidConfig(
            "Command contains null byte (invalid)".to_string()
        ));
    }

    // Platform-specific validation
    #[cfg(windows)]
    {
        const INVALID_CHARS: &[char] = &['<', '>', '|', '"'];
        if self.command.chars().any(|c| INVALID_CHARS.contains(&c)) {
            return Err(ClientError::InvalidConfig(
                format!("Command '{}' contains invalid characters for Windows", self.command)
            ));
        }
    }

    // Helpful hints for common mistakes
    if self.command.contains(' ') && !self.args.is_empty() {
        tracing::warn!(
            "Command '{}' contains spaces. Did you mean to use .arg() instead?",
            self.command
        );
    }

    // ... rest of build implementation
}
```

## Additional Validation

### Validate Arguments
```rust
pub fn arg(mut self, arg: impl Into<String>) -> Self {
    let arg = arg.into();

    if arg.contains('\0') {
        tracing::error!("Argument contains null byte: '{:?}'", arg);
    }

    self.args.push(arg);
    self
}
```

### Validate Timeout
```rust
pub fn timeout(mut self, timeout: Duration) -> Self {
    if timeout.is_zero() {
        tracing::warn!("Timeout set to zero (operations will fail immediately)");
    }
    if timeout.as_secs() > 3600 {
        tracing::warn!("Timeout set to {}s (very long)", timeout.as_secs());
    }

    self.timeout = timeout;
    self
}
```

## Testing

```rust
#[tokio::test]
async fn test_empty_command() {
    let result = StdioClientBuilder::new("")
        .build()
        .await;

    assert!(matches!(result, Err(ClientError::InvalidConfig(_))));
    let err = result.unwrap_err().to_string();
    assert!(err.contains("empty"));
}

#[tokio::test]
async fn test_whitespace_command() {
    let result = StdioClientBuilder::new("   ")
        .build()
        .await;

    assert!(matches!(result, Err(ClientError::InvalidConfig(_))));
}

#[tokio::test]
async fn test_null_byte_command() {
    let result = StdioClientBuilder::new("node\0evil")
        .build()
        .await;

    assert!(matches!(result, Err(ClientError::InvalidConfig(_))));
}

#[tokio::test]
fn test_whitespace_trimmed() {
    let builder = StdioClientBuilder::new("  node  ");
    assert_eq!(builder.command, "node");
}
```

## Priority
**MEDIUM** - Improves error messages and catches common mistakes

## Related Tasks
- Task 006: No working directory validation (similar validation issues)
- Task 007: Error context loss (better error messages help here too)

## New Error Variant

Add to `ClientError`:

```rust
#[error("Invalid configuration: {0}")]
InvalidConfig(String),
```
