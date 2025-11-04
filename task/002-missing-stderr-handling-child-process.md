# Task: Missing stderr Handling for Child Process

## Location
`src/transports/stdio.rs:219-263` (build method)

## Issue Type
- **CRITICAL**: Hidden Errors
- Production Readiness
- Debugging/Observability

## Description
When spawning a child process via `TokioChildProcess::new(cmd)`, the code only uses stdin/stdout for JSON-RPC communication. There is no handling, capture, or logging of stderr output from the child process.

## Problem
1. **Lost Diagnostic Information**: If the child process writes error messages, warnings, or diagnostic information to stderr, these messages are completely lost. They don't appear in logs, don't bubble up as errors, and provide no indication something is wrong.

2. **Debugging Nightmare**: When a child process misbehaves or fails, developers have no way to see what the process was trying to communicate via stderr. This makes debugging extremely difficult.

3. **Silent Failures**: A child process might be writing critical error messages to stderr before crashing, but users will never see these messages.

4. **Inconsistent with Best Practices**: Most process execution libraries (like std::process::Command) provide explicit control over stderr handling (inherit, pipe, null, etc.).

## Real-World Impact

### Scenario 1: Node.js MCP Server with Errors
```bash
# User spawns a Node.js MCP server
let (client, _conn) = create_stdio_client("node", &["mcp-server.js"]).await?;
```

If `mcp-server.js` writes to console.error():
```javascript
console.error("WARNING: Database connection failing, retrying...");
console.error("ERROR: Unable to load configuration file");
```

**Current Behavior**: These messages vanish into the void. The user has no idea why the server is failing.

### Scenario 2: Python MCP Server with Stack Traces
```bash
let (client, _conn) = create_stdio_client("python3", &["mcp-server.py"]).await?;
```

If the Python server crashes with a stack trace to stderr:
```
Traceback (most recent call last):
  File "mcp-server.py", line 45, in handle_request
    result = process_data(None)
TypeError: expected string, got None
```

**Current Behavior**: The connection just closes with a generic "connection failed" error. The actual cause (TypeError) is never visible.

### Scenario 3: uvx Package Download Messages
```bash
let (client, _conn) = create_stdio_client("uvx", &["mcp-server-git"]).await?;
```

uvx writes progress messages to stderr during package installation. Users don't see:
- "Downloading mcp-server-git..."
- "Installing dependencies..."
- "Warning: outdated package version"

## Technical Details

The `TokioChildProcess` wrapper (from the `rmcp` crate) internally calls `Command::spawn()` which by default inherits stderr from the parent process. However, in a library context, this means:

1. **If parent has stderr**: Messages appear in parent's stderr (often a terminal), which may not be monitored
2. **If parent redirects stderr**: Messages go wherever parent stderr goes (log file, /dev/null, etc.)
3. **No control by library user**: The library user cannot choose how to handle child stderr

## Recommended Fixes

### Option 1: Pipe and Log stderr (Recommended)
Capture stderr and log it using the tracing/logging framework:

```rust
use tokio::io::{AsyncBufReadExt, BufReader};

// Configure Command to pipe stderr
cmd.stderr(std::process::Stdio::piped());

// Spawn process
let mut child = cmd.spawn()?;

// Spawn task to read and log stderr
let stderr = child.stderr.take().ok_or(...)?;
let reader = BufReader::new(stderr);
tokio::spawn(async move {
    let mut lines = reader.lines();
    while let Ok(Some(line)) = lines.next_line().await {
        tracing::warn!(target: "kodegen_mcp_client::child_stderr", "{}", line);
    }
});
```

### Option 2: Provide Builder Option
Let users choose stderr handling:

```rust
pub enum StderrMode {
    Inherit,      // Default: inherit parent's stderr
    Pipe,         // Pipe and log via tracing
    Null,         // Discard stderr
    Custom(File), // Write to custom file
}

impl StdioClientBuilder {
    pub fn stderr(mut self, mode: StderrMode) -> Self {
        self.stderr_mode = mode;
        self
    }
}
```

### Option 3: Always Inherit with Documentation
Document that stderr is inherited and provide guidance:

```rust
/// The child process will inherit the parent's stderr.
/// To capture diagnostic messages, ensure your application's
/// stderr is directed to a log file or monitoring system.
///
/// Example: redirect stderr to a file
/// ```bash
/// my-app 2>> /var/log/mcp-child-errors.log
/// ```
```

## Testing
1. Create a test child process that writes to stderr
2. Verify stderr messages are either captured/logged or documented as inherited
3. Test behavior when child writes large amounts of stderr (ensure no blocking)
4. Test behavior when stderr pipe breaks

## Priority
**HIGH** - This significantly impacts production debuggability and observability

## Related Code
- `rmcp` crate's `TokioChildProcess` implementation
- Similar issue might exist in other transport types

## References
- Tokio Command::stderr: https://docs.rs/tokio/latest/tokio/process/struct.Command.html#method.stderr
- Rust std::process::Stdio: https://doc.rust-lang.org/std/process/struct.Stdio.html
