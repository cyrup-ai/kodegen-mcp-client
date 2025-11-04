# Task: No Working Directory Validation

## Location
`src/transports/stdio.rs:143-156` (current_dir method)
`src/transports/stdio.rs:228-230` (build method where current_dir is used)

## Issue Type
- Code Clarity (poor error messages)
- API Usability
- Early error detection

## Description
The `current_dir()` method accepts any `PathBuf` without validation:

```rust
/// Set the working directory for the child process
pub fn current_dir(mut self, dir: impl Into<PathBuf>) -> Self {
    self.current_dir = Some(dir.into());
    self
}
```

If the directory doesn't exist, is not a directory, or is not accessible, the error won't be discovered until `build()` is called, and the error message will be generic and unhelpful.

## Problem

### 1. Late Error Detection
Errors are not caught at configuration time (when `.current_dir()` is called), but at build time (when `.build()` is called). This can be far from where the mistake was made.

### 2. Poor Error Messages
When the process fails to spawn due to invalid working directory, the error is:
```
Failed to spawn process 'node': No such file or directory (os error 2)
```

This doesn't clearly indicate that the **working directory** is invalid, not the command.

### 3. Confusing Failure Mode
Users might think the command itself doesn't exist, when actually it's the working directory that's invalid.

## Real-World Impact

### Scenario 1: Typo in Directory Path
```rust
let (client, _conn) = StdioClientBuilder::new("node")
    .arg("server.js")
    .current_dir("/path/to/proyect")  // ← Typo: "proyect" instead of "project"
    .build()
    .await?;

// Error: "Failed to spawn process 'node': No such file or directory"
// User thinks: "Why can't it find 'node'? It's in my PATH!"
// Actual problem: /path/to/proyect doesn't exist
```

### Scenario 2: Relative Path Confusion
```rust
// User thinks current_dir is relative to current directory
let (client, _conn) = StdioClientBuilder::new("python3")
    .arg("server.py")
    .current_dir("subdir")  // Might not be what they expect
    .build()
    .await?;

// What is "subdir" relative to?
// - The current process's working directory (correct)
// - The executable's location (wrong assumption)
// - Something else?
```

### Scenario 3: Permission Denied
```rust
let (client, _conn) = StdioClientBuilder::new("node")
    .arg("server.js")
    .current_dir("/root/restricted")  // User doesn't have access
    .build()
    .await?;

// Error: "Failed to spawn process 'node': Permission denied"
// User thinks: "Why can't it execute 'node'? I have permission!"
// Actual problem: Can't access working directory
```

### Scenario 4: File Instead of Directory
```rust
let (client, _conn) = StdioClientBuilder::new("node")
    .arg("server.js")
    .current_dir("/path/to/file.txt")  // ← File, not directory
    .build()
    .await?;

// Error: "Failed to spawn process 'node': Not a directory"
// User thinks: "What's not a directory? node?"
// Actual problem: /path/to/file.txt is a file
```

## Recommended Fixes

### Option 1: Validate at Configuration Time (Eager Validation)
```rust
/// Set the working directory for the child process
///
/// # Errors
///
/// Returns an error immediately if the path doesn't exist,
/// is not a directory, or is not accessible.
///
/// # Example
///
/// ```ignore
/// let builder = StdioClientBuilder::new("node")
///     .arg("server.js")
///     .current_dir("/path/to/project")?;  // ← Can fail here
/// ```
pub fn current_dir(mut self, dir: impl Into<PathBuf>) -> Result<Self, ClientError> {
    let path = dir.into();

    // Validate path exists
    if !path.exists() {
        return Err(ClientError::InvalidConfig(
            format!("Working directory does not exist: {}", path.display())
        ));
    }

    // Validate path is a directory
    if !path.is_dir() {
        return Err(ClientError::InvalidConfig(
            format!("Working directory is not a directory: {}", path.display())
        ));
    }

    // Validate path is accessible (try to read it)
    if let Err(e) = std::fs::read_dir(&path) {
        return Err(ClientError::InvalidConfig(
            format!("Cannot access working directory {}: {}", path.display(), e)
        ));
    }

    self.current_dir = Some(path);
    Ok(self)
}
```

**Pros**:
- Immediate feedback
- Clear error messages
- Fail fast

**Cons**:
- Breaking change (method now returns Result)
- More strict (might reject valid use cases like directories that will be created later)
- Requires I/O at configuration time

### Option 2: Validate at Build Time (Better Error Messages)
Keep the current signature but provide better error messages at build time:

```rust
pub async fn build(self) -> Result<(KodegenClient, KodegenConnection), ClientError> {
    let mut cmd = Command::new(&self.command);
    cmd.args(&self.args);

    if !self.envs.is_empty() {
        cmd.envs(&self.envs);
    }

    // Validate and set working directory
    if let Some(dir) = &self.current_dir {
        // Validate before setting
        if !dir.exists() {
            return Err(ClientError::InvalidConfig(
                format!("Working directory does not exist: {}", dir.display())
            ));
        }
        if !dir.is_dir() {
            return Err(ClientError::InvalidConfig(
                format!("Working directory is not a directory: {}", dir.display())
            ));
        }

        cmd.current_dir(dir);
    }

    // Create transport
    let transport = TokioChildProcess::new(cmd).map_err(|e| {
        // Enhanced error message
        if let Some(dir) = &self.current_dir {
            ClientError::Connection(format!(
                "Failed to spawn process '{}' in directory '{}': {}",
                self.command,
                dir.display(),
                e
            ))
        } else {
            ClientError::Connection(format!(
                "Failed to spawn process '{}': {}",
                self.command,
                e
            ))
        }
    })?;

    // ... rest of implementation
}
```

**Pros**:
- Not a breaking change
- Better error messages
- Validates at build time

**Cons**:
- Still late validation (not at configuration time)
- Requires I/O at build time

### Option 3: Document Behavior and Provide Helper
Keep current implementation but add documentation and a helper:

```rust
impl StdioClientBuilder {
    /// Set the working directory for the child process
    ///
    /// The path is not validated until `build()` is called.
    /// If the path doesn't exist or is not a directory, `build()` will fail.
    ///
    /// Use `validate_current_dir()` to validate the path immediately.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let builder = StdioClientBuilder::new("node")
    ///     .arg("server.js")
    ///     .current_dir("/path/to/project");
    ///
    /// // Validate immediately (optional)
    /// builder.validate_current_dir()?;
    /// ```
    pub fn current_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.current_dir = Some(dir.into());
        self
    }

    /// Validate the working directory (if set)
    ///
    /// Call this after `current_dir()` to validate the path immediately
    /// instead of waiting for `build()`.
    ///
    /// # Errors
    ///
    /// Returns an error if the working directory is set and is invalid.
    pub fn validate_current_dir(&self) -> Result<(), ClientError> {
        if let Some(dir) = &self.current_dir {
            if !dir.exists() {
                return Err(ClientError::InvalidConfig(
                    format!("Working directory does not exist: {}", dir.display())
                ));
            }
            if !dir.is_dir() {
                return Err(ClientError::InvalidConfig(
                    format!("Working directory is not a directory: {}", dir.display())
                ));
            }
        }
        Ok(())
    }
}
```

**Pros**:
- Not a breaking change
- Optional validation for users who want it
- Keeps configuration fast (no I/O unless requested)

**Cons**:
- Easy for users to forget to validate
- Two ways to do the same thing

### Option 4: Accept &Path and Validate Early
```rust
/// Set the working directory for the child process
///
/// The directory must exist and be accessible.
///
/// # Panics
///
/// Panics if the path doesn't exist or is not a directory.
/// For fallible validation, use `try_current_dir()`.
pub fn current_dir(mut self, dir: impl AsRef<Path>) -> Self {
    let path = dir.as_ref();
    assert!(path.exists(), "Working directory does not exist: {}", path.display());
    assert!(path.is_dir(), "Working directory is not a directory: {}", path.display());
    self.current_dir = Some(path.to_path_buf());
    self
}

/// Set the working directory with fallible validation
pub fn try_current_dir(mut self, dir: impl Into<PathBuf>) -> Result<Self, ClientError> {
    // ... validation as in Option 1 ...
}
```

**Pros**:
- Clear distinction between fallible and infallible
- Follows Rust conventions (try_* for fallible)

**Cons**:
- Panicking APIs are frowned upon in libraries
- Confusing to have two methods

## Recommended Approach

**Option 2** (Validate at Build Time) is recommended because:
1. Not a breaking change
2. Provides significantly better error messages
3. No added I/O cost at configuration time
4. Users still get early feedback (at build, not first use)

Optionally combine with **Option 3** (add `validate_current_dir()`) for users who want eager validation.

## New Error Variant

Add a new error variant:

```rust
#[derive(Error, Debug)]
pub enum ClientError {
    // ... existing variants ...

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}
```

## Testing

```rust
#[tokio::test]
async fn test_current_dir_not_exists() {
    let result = StdioClientBuilder::new("node")
        .arg("server.js")
        .current_dir("/nonexistent/directory")
        .build()
        .await;

    assert!(matches!(result, Err(ClientError::InvalidConfig(_))));
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("does not exist"));
    assert!(err_msg.contains("/nonexistent/directory"));
}

#[tokio::test]
async fn test_current_dir_not_a_directory() {
    let temp_file = tempfile::NamedTempFile::new().unwrap();

    let result = StdioClientBuilder::new("node")
        .arg("server.js")
        .current_dir(temp_file.path())
        .build()
        .await;

    assert!(matches!(result, Err(ClientError::InvalidConfig(_))));
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("not a directory"));
}

#[tokio::test]
async fn test_current_dir_valid() {
    let temp_dir = tempfile::tempdir().unwrap();

    let result = StdioClientBuilder::new("echo")
        .arg("test")
        .current_dir(temp_dir.path())
        .build()
        .await;

    // Should not fail due to working directory
    // (will fail because echo isn't an MCP server, but that's expected)
    assert!(result.is_err());  // Echo isn't MCP server
    assert!(!matches!(result, Err(ClientError::InvalidConfig(_))));
}
```

## Priority
**MEDIUM** - Improves developer experience and makes debugging easier, but not critical for functionality

## Related Tasks
- Task 012: Missing builder validation (related validation issues)
- Task 001: Unnecessary conditional (both in build method)
