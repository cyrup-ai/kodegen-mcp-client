# Task: No Child Process Exit Status Checking

## Location
`src/transports/stdio.rs` (entire module)
`src/lib.rs:196-219` (KodegenConnection::close and wait methods)

## Issue Type
- **Hidden Errors**
- Debugging/Observability
- Production Readiness

## Description
When a child process spawned by `create_stdio_client()` or `StdioClientBuilder` terminates, there is no mechanism to check or report the process's exit status. The library doesn't distinguish between:

- Clean exit (exit code 0)
- Error exit (exit code 1-255)
- Signal termination (SIGTERM, SIGKILL, SIGSEGV, etc.)
- Abnormal termination (crash, panic, etc.)

## Problem

### 1. Hidden Failures
When a child process crashes or exits with an error code, the library simply reports a generic connection error. The actual cause (exit status) is never exposed to the user.

### 2. Difficult Debugging
Users cannot determine if:
- The process exited cleanly but the protocol failed
- The process crashed (segfault, panic, etc.)
- The process was killed by a signal
- The process exited with an error code

### 3. No Observability
There's no way to log or monitor child process health. Operations teams cannot alert on child process crashes vs. normal shutdowns.

### 4. Protocol Ambiguity
When the connection closes, it could be because:
- Client called `conn.close()` (expected)
- Server shut down cleanly (expected)
- Server crashed (unexpected - should be visible)
- Server was killed (unexpected - should be visible)

## Real-World Impact

### Scenario 1: Node.js Server Crashes
```rust
let (client, conn) = create_stdio_client("node", &["mcp-server.js"]).await?;

// Server crashes with uncaught exception
let result = client.call_tool(tools::READ_FILE, args).await;

// Current error: "Service error: connection closed"
// Desired error: "Child process exited with code 1: Uncaught TypeError: ..."
```

### Scenario 2: Python Server Segfault
```rust
let (client, conn) = create_stdio_client("python3", &["mcp-server.py"]).await?;

// Server hits segfault in native extension
let result = client.call_tool(tools::EXECUTE_SQL, args).await;

// Current error: "Service error: connection closed"
// Desired error: "Child process terminated by signal 11 (SIGSEGV)"
```

### Scenario 3: Server Killed by OOM Killer
```rust
let (client, conn) = create_stdio_client("java", &["-jar", "mcp-server.jar"]).await?;

// Java process killed by OOM killer (SIGKILL)
let result = client.call_tool(tools::SEARCH_CODE, args).await;

// Current error: "Service error: connection closed"
// Desired error: "Child process terminated by signal 9 (SIGKILL) - likely out of memory"
```

### Scenario 4: Graceful Shutdown Verification
```rust
let (client, conn) = create_stdio_client("uvx", &["mcp-server-git"]).await?;

// Do work...

conn.close().await?;

// Current: No way to verify child exited cleanly
// Desired: Can check exit status to ensure clean shutdown
```

## Technical Details

The `TokioChildProcess` from the `rmcp` crate wraps a `tokio::process::Child`. The `Child` struct provides a `wait()` method that returns an `ExitStatus`:

```rust
pub struct ExitStatus { /* ... */ }

impl ExitStatus {
    pub fn success(&self) -> bool { /* ... */ }
    pub fn code(&self) -> Option<i32> { /* ... */ }
    // On Unix:
    pub fn signal(&self) -> Option<i32> { /* ... */ }
}
```

However, this information is never extracted or exposed by the MCP client library.

## Current Code Issues

### Issue 1: No Exit Status in KodegenConnection
```rust
pub struct KodegenConnection {
    service: RunningService<RoleClient, ClientInfo>,
}

pub async fn close(self) -> Result<(), ClientError> {
    self.service
        .cancel()
        .await
        .map(|_| ())  // ← Exit status lost here
        .map_err(ClientError::from)
}
```

The `RunningService::cancel()` presumably waits for the child to exit, but the exit status is discarded.

### Issue 2: No Exit Status Tracking During Lifetime
Even if `close()` returned exit status, there's no way to check exit status if:
- The child exits unexpectedly during operation
- The connection is dropped (not explicitly closed)
- An error occurs and connection is abandoned

## Recommended Fixes

### Option 1: Add Exit Status to Connection (Recommended)
```rust
pub struct KodegenConnection {
    service: RunningService<RoleClient, ClientInfo>,
}

impl KodegenConnection {
    /// Close connection and return child process exit status
    pub async fn close(self) -> Result<ProcessExit, ClientError> {
        // Extract exit status from service
        let exit_status = self.service.cancel().await?;
        Ok(ProcessExit::from(exit_status))
    }

    /// Wait for natural close and return exit status
    pub async fn wait(self) -> Result<ProcessExit, ClientError> {
        let exit_status = self.service.waiting().await?;
        Ok(ProcessExit::from(exit_status))
    }

    /// Check if child has exited without consuming connection
    pub fn try_exit_status(&self) -> Option<ProcessExit> {
        // Non-blocking check if child has exited
        self.service.try_exit_status()
    }
}

/// Represents child process exit status
#[derive(Debug, Clone)]
pub enum ProcessExit {
    /// Exited with code (0 = success)
    Code(i32),
    /// Terminated by signal (Unix only)
    #[cfg(unix)]
    Signal(i32),
    /// Unknown exit reason
    Unknown,
}

impl ProcessExit {
    pub fn success(&self) -> bool {
        matches!(self, ProcessExit::Code(0))
    }

    pub fn code(&self) -> Option<i32> {
        match self {
            ProcessExit::Code(c) => Some(*c),
            _ => None,
        }
    }
}
```

### Option 2: Log Exit Status Automatically
```rust
impl Drop for KodegenConnection {
    fn drop(&mut self) {
        // Spawn task to wait for exit and log status
        let service = self.service.clone();
        tokio::spawn(async move {
            if let Some(exit_status) = service.exit_status().await {
                match exit_status.code() {
                    Some(0) => tracing::debug!("Child process exited successfully"),
                    Some(code) => tracing::error!("Child process exited with code {}", code),
                    None => tracing::error!("Child process terminated by signal"),
                }
            }
        });
    }
}
```

### Option 3: Enhanced Error Types
```rust
#[derive(Error, Debug)]
pub enum ClientError {
    // ... existing variants ...

    #[error("Child process exited with code {code}: {message}")]
    ProcessExited { code: i32, message: String },

    #[error("Child process terminated by signal {signal}")]
    ProcessSignaled { signal: i32 },
}
```

When a tool call fails due to connection loss, check exit status:
```rust
pub async fn call_tool(&self, name: &str, arguments: serde_json::Value)
    -> Result<CallToolResult, ClientError>
{
    match self.peer.call_tool(...).await {
        Ok(result) => Ok(result),
        Err(e) if is_connection_error(&e) => {
            // Check if child exited and why
            if let Some(exit_status) = self.check_child_exit() {
                return Err(ClientError::from_exit_status(exit_status));
            }
            Err(ClientError::from(e))
        }
        Err(e) => Err(ClientError::from(e))
    }
}
```

## Implementation Challenges

### Challenge 1: rmcp Crate Limitations
The `rmcp` crate's `TokioChildProcess` and `RunningService` may not expose exit status information. This might require:
- Updating the `rmcp` crate
- Wrapping `TokioChildProcess` with our own type
- Maintaining a separate handle to the child process

### Challenge 2: Async Drop Limitations
Rust's `Drop` trait is synchronous, so we can't `await` exit status in drop. Solutions:
- Spawn a background task (as shown in Option 2)
- Require explicit `close()` or `wait()` (Option 1)
- Use a separate "monitor" task that tracks exit status

### Challenge 3: HTTP Transports
HTTP-based transports don't have child processes, so any exit status API must be optional or transport-specific.

## Recommended Approach

**Combination of Options 1 and 2**:

1. **Add exit status to `close()` and `wait()` methods** (breaking change, but necessary)
2. **Add `try_exit_status()` for non-blocking checks**
3. **Log exit status on drop** for cases where connection is abandoned
4. **Enhance error messages** to include exit status when available

This provides:
- Explicit control for careful users (`close()`, `wait()`)
- Automatic logging for casual users (Drop)
- Observability for monitoring (logs)

## Testing
```rust
#[tokio::test]
async fn test_exit_status_success() {
    // Create mock child that exits with 0
    let (client, conn) = create_stdio_client(...).await?;
    drop(client);
    let exit = conn.close().await?;
    assert!(exit.success());
}

#[tokio::test]
async fn test_exit_status_error() {
    // Create mock child that exits with 1
    let (client, conn) = create_stdio_client(...).await?;
    drop(client);
    let exit = conn.close().await?;
    assert_eq!(exit.code(), Some(1));
}

#[tokio::test]
async fn test_exit_status_signal() {
    // Create mock child that's killed with SIGTERM
    let (client, conn) = create_stdio_client(...).await?;
    // Kill child
    let exit = conn.close().await?;
    assert!(matches!(exit, ProcessExit::Signal(15))); // SIGTERM
}
```

## Priority
**HIGH** - Critical for production debuggability and reliability

## Related Tasks
- Task 002: Missing stderr handling (combined, these provide full process observability)
- Task 008: Resource leak on build error (need to check exit status during cleanup)

## References
- tokio::process::ExitStatus: https://docs.rs/tokio/latest/tokio/process/struct.ExitStatus.html
- std::process::ExitStatus: https://doc.rust-lang.org/std/process/struct.ExitStatus.html
