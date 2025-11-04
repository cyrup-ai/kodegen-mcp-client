# Task: Potential Resource Leak on Build Error

## Location
`src/transports/stdio.rs:219-263` (StdioClientBuilder::build method)

## Issue Type
- **CRITICAL**: Resource Leak
- Memory/Process Management
- Security (leaked process with env vars)

## Description
In the `build()` method, if MCP initialization fails after the child process is spawned, the child process may not be properly terminated, leading to a resource leak.

## Problem Code

```rust
pub async fn build(self) -> Result<(KodegenClient, KodegenConnection), ClientError> {
    // ... setup cmd ...

    // Create transport - TokioChildProcess automatically sets stdin/stdout to piped
    let transport = TokioChildProcess::new(cmd).map_err(|e| {  // ← PROCESS SPAWNED HERE
        ClientError::Connection(format!("Failed to spawn process '{}': {}", self.command, e))
    })?;

    // Create client info with metadata
    let client_info = ClientInfo {
        // ...
    };

    // Initialize MCP connection
    let service = client_info
        .serve(transport)
        .await
        .map_err(ClientError::InitError)?;  // ← IF THIS FAILS, WHAT HAPPENS TO THE PROCESS?

    // Wrap in connection and extract client with configured timeout
    let connection = KodegenConnection::from_service(service);
    let client = connection.client().with_timeout(self.timeout);

    Ok((client, connection))
}
```

## Analysis

### Question: Does TokioChildProcess clean up on error?

The critical question is: If `client_info.serve(transport).await` fails, does the `TokioChildProcess` (which owns the spawned child) properly terminate the child process?

Looking at typical Rust Drop semantics:
1. `TokioChildProcess::new(cmd)` spawns the process and returns a struct owning it
2. If `.serve(transport)` fails, `transport` (the `TokioChildProcess`) is dropped
3. Does `TokioChildProcess::Drop` kill the child process?

### Likely Scenarios

#### Scenario 1: TokioChildProcess implements proper Drop
```rust
impl Drop for TokioChildProcess {
    fn drop(&mut self) {
        // Kill child process or detach
        let _ = self.child.start_kill();
    }
}
```

In this case, the child is properly killed when `build()` returns an error. **No leak**.

#### Scenario 2: TokioChildProcess does NOT implement Drop
The child process becomes orphaned and continues running until:
- It exits on its own (if it detects stdin/stdout closure)
- Init process adopts and cleans it up (on Unix)
- It runs indefinitely (potential leak)

**Likely leak** if the server doesn't detect stdin closure.

#### Scenario 3: TokioChildProcess detaches the child
Some process wrappers explicitly detach children so they outlive the parent. In this case, the child continues running even after `build()` fails. **Definite leak**.

## Real-World Impact

### Impact 1: Resource Exhaustion
If initialization failures are common (network issues, misconfiguration, etc.), leaked processes accumulate:

```rust
// User's code with retry logic
for attempt in 0..100 {
    match StdioClientBuilder::new("node")
        .arg("mcp-server.js")
        .build()
        .await
    {
        Ok((client, conn)) => return Ok((client, conn)),
        Err(e) => {
            eprintln!("Attempt {} failed: {}", attempt, e);
            tokio::time::sleep(Duration::from_secs(1)).await;
            // ⚠️ If child leaked, we now have 1 extra node process
        }
    }
}
// After 100 failed attempts: 100 leaked node processes!
```

### Impact 2: Port/Resource Conflicts
Leaked processes may hold onto ports or file locks:

```rust
// Server binds to port 8080
let builder = StdioClientBuilder::new("python3")
    .arg("mcp-server.py")
    .build()
    .await?;  // Fails during MCP init, but process is running

// Retry immediately
let builder = StdioClientBuilder::new("python3")
    .arg("mcp-server.py")
    .build()
    .await?;  // Fails! Port 8080 already in use by leaked process
```

### Impact 3: Security Issue - Leaked Processes with Sensitive Env Vars
The leaked process has access to all environment variables passed to it:

```rust
let (client, conn) = StdioClientBuilder::new("node")
    .arg("mcp-server.js")
    .env("DATABASE_PASSWORD", "super_secret")
    .env("AWS_SECRET_KEY", "very_secret")
    .build()
    .await?;  // Fails during MCP init

// ⚠️ SECURITY ISSUE:
// - Node process is still running
// - It has DATABASE_PASSWORD and AWS_SECRET_KEY in memory
// - It might log them, expose them via /proc, or be compromised
// - User doesn't know it's running (no client/connection returned)
```

### Impact 4: Difficult Debugging
Users notice:
- Increasing number of processes (`ps aux | grep node`)
- Memory usage growing
- Port conflicts
- File descriptor exhaustion

But they don't know why because the library doesn't report the leak.

## Testing the Current Behavior

```rust
#[tokio::test]
async fn test_build_error_cleanup() {
    use sysinfo::{ProcessExt, System, SystemExt};

    let mut sys = System::new_all();
    sys.refresh_all();

    // Count node processes before
    let before = sys.processes().iter()
        .filter(|(_, p)| p.name().contains("node"))
        .count();

    // Attempt to create client that will fail during init
    // (echo is not an MCP server, so serve() will fail)
    let result = StdioClientBuilder::new("node")
        .arg("-e")
        .arg("console.log('not mcp')")
        .build()
        .await;

    assert!(result.is_err());

    // Wait a bit for cleanup
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Count node processes after
    sys.refresh_all();
    let after = sys.processes().iter()
        .filter(|(_, p)| p.name().contains("node"))
        .count();

    // Should be the same (no leak)
    assert_eq!(before, after, "Leaked {} node process(es)", after - before);
}
```

## Recommended Fixes

### Fix 1: Explicit Cleanup on Error (Defensive)
```rust
pub async fn build(self) -> Result<(KodegenClient, KodegenConnection), ClientError> {
    // ... setup cmd ...

    // Create transport
    let mut transport = TokioChildProcess::new(cmd).map_err(|e| {
        ClientError::Connection(format!("Failed to spawn process '{}': {}", self.command, e))
    })?;

    // Create client info
    let client_info = ClientInfo {
        // ...
    };

    // Initialize MCP connection with cleanup on error
    let service = match client_info.serve(transport).await {
        Ok(s) => s,
        Err(e) => {
            // Explicitly kill the child process before returning error
            // Note: This assumes TokioChildProcess provides a kill() method
            // If not, we need to keep a handle to the child
            let _ = transport.kill().await;  // Best effort kill
            return Err(ClientError::InitError(e));
        }
    };

    // Wrap in connection and extract client
    let connection = KodegenConnection::from_service(service);
    let client = connection.client().with_timeout(self.timeout);

    Ok((client, connection))
}
```

**Problem**: `TokioChildProcess` may not expose a `kill()` method, and it's been moved into `serve()` by the time we need to kill it.

### Fix 2: Keep Child Handle Separately
```rust
pub async fn build(self) -> Result<(KodegenClient, KodegenConnection), ClientError> {
    // ... setup cmd ...

    // Spawn process but keep the Child handle
    let mut child = cmd.spawn().map_err(|e| {
        ClientError::Connection(format!("Failed to spawn process '{}': {}", self.command, e))
    })?;

    // Create a wrapper that we can kill if needed
    struct ChildGuard(Option<tokio::process::Child>);

    impl Drop for ChildGuard {
        fn drop(&mut self) {
            if let Some(mut child) = self.0.take() {
                // Best effort kill
                let _ = child.start_kill();
                tracing::warn!("Killed orphaned child process during cleanup");
            }
        }
    }

    let mut guard = ChildGuard(Some(child));

    // Create transport from child
    let transport = TokioChildProcess::from_child(guard.0.take().unwrap())?;

    // Initialize MCP connection
    let service = client_info
        .serve(transport)
        .await
        .map_err(ClientError::InitError)?;

    // Success - disable guard (child is now managed by service)
    std::mem::forget(guard);

    let connection = KodegenConnection::from_service(service);
    let client = connection.client().with_timeout(self.timeout);

    Ok((client, connection))
}
```

**Problem**: May not be possible if `TokioChildProcess` doesn't provide `from_child()`.

### Fix 3: Verify and Document Current Behavior
If `TokioChildProcess` properly implements `Drop` with child termination:

1. **Verify** with test (as shown above)
2. **Document** the behavior:
   ```rust
   /// # Error Handling
   ///
   /// If this method returns an error after spawning the child process,
   /// the child is automatically terminated. You do not need to manually
   /// clean up the process.
   pub async fn build(self) -> Result<(KodegenClient, KodegenConnection), ClientError> {
   ```
3. **Add test** to prevent regression

### Fix 4: Add Explicit Timeout for MCP Init
Even if cleanup works, a slow-to-initialize server could tie up resources:

```rust
pub async fn build(self) -> Result<(KodegenClient, KodegenConnection), ClientError> {
    // ... setup and spawn ...

    // Initialize with timeout
    let init_timeout = Duration::from_secs(30);
    let service = tokio::time::timeout(
        init_timeout,
        client_info.serve(transport)
    )
    .await
    .map_err(|_| ClientError::Connection(format!(
        "MCP initialization timed out after {}s",
        init_timeout.as_secs()
    )))?
    .map_err(ClientError::InitError)?;

    // ...
}
```

## Investigation Steps

1. **Check rmcp source code** for `TokioChildProcess::Drop` implementation
2. **Run test** (shown above) to verify current behavior
3. **Check with different servers** (node, python, etc.) to see if they exit on stdin close
4. **Monitor `/proc`** during failed build to see if processes remain

## Priority
**HIGH** - Potential resource leak and security issue

## Related Tasks
- Task 004: No exit status checking (need to know if/how child exited)
- Task 005: Env inheritance docs (leaked process has sensitive env vars)
- Task 002: Missing stderr handling (might help detect if child is still running)

## References
- Tokio Child::start_kill: https://docs.rs/tokio/latest/tokio/process/struct.Child.html#method.start_kill
- Tokio Child::kill: https://docs.rs/tokio/latest/tokio/process/struct.Child.html#method.kill
- RAII and Drop: https://doc.rust-lang.org/book/ch15-03-drop.html
