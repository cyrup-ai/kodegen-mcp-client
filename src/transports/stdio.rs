// packages/mcp-client/src/transports/stdio.rs
use super::create_client_info;
use crate::{ClientError, KodegenClient, KodegenConnection};
use rmcp::{ServiceExt, transport::TokioChildProcess};
use std::{collections::HashMap, path::PathBuf};
use tokio::{process::Command, time::Duration};

/// Default timeout for MCP operations
/// 
/// Based on worst-case tool timings:
/// - Terminal operations: 10s max (5s reader + 5s writer cleanup)
/// - Most other tools: < 5s
/// - Network operations: variable
/// 
/// Set to 12s = 10s worst-case + 2s overhead buffer
/// 
/// For tools with different characteristics, use client.with_timeout()
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(12);

/// Builder for creating stdio-based MCP clients
///
/// Provides a fluent API for configuring child process execution with full control over:
/// - Command and arguments
/// - Environment variables
/// - Working directory
/// - Operation timeout
/// - Client identification
///
/// # Example
///
/// ```ignore
/// use kodegen_mcp_client::StdioClientBuilder;
/// use std::time::Duration;
///
/// // Advanced configuration
/// let (client, _conn) = StdioClientBuilder::new("node")
///     .arg("my-server.js")
///     .env("NODE_ENV", "production")
///     .env("DEBUG", "mcp:*")
///     .current_dir("/path/to/server")
///     .timeout(Duration::from_secs(60))
///     .client_name("my-app-client")
///     .build()
///     .await?;
///
/// let tools = client.list_tools().await?;
/// ```
#[derive(Debug, Clone)]
pub struct StdioClientBuilder {
    command: String,
    args: Vec<String>,
    envs: HashMap<String, String>,
    clear_env: bool,
    env_removes: Vec<String>,
    current_dir: Option<PathBuf>,
    timeout: Duration,
    client_name: Option<String>,
}

impl StdioClientBuilder {
    /// Create a new builder for a stdio-based MCP client
    ///
    /// # Arguments
    ///
    /// * `command` - The command to execute (e.g., "uvx", "node", "python3")
    ///
    /// # Example
    ///
    /// ```ignore
    /// let builder = StdioClientBuilder::new("uvx");
    /// ```
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            envs: HashMap::new(),
            clear_env: false,
            env_removes: Vec::new(),
            current_dir: None,
            timeout: DEFAULT_TIMEOUT,
            client_name: None,
        }
    }

    /// Add a single argument to the command
    ///
    /// # Example
    ///
    /// ```ignore
    /// let builder = StdioClientBuilder::new("uvx")
    ///     .arg("mcp-server-git")
    ///     .arg("--verbose");
    /// ```
    #[must_use]
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Add multiple arguments to the command
    ///
    /// # Example
    ///
    /// ```ignore
    /// let builder = StdioClientBuilder::new("node")
    ///     .args(&["server.js", "--port", "8080"]);
    /// ```
    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// Add a single environment variable to the child process
    ///
    /// By default, the child process **inherits all environment variables from the parent**
    /// and this method adds or overrides specific variables. To prevent inheritance of
    /// sensitive variables, use `env_clear()` before adding variables, or use `env_remove()`
    /// to selectively exclude specific variables.
    ///
    /// # Security Warning
    ///
    /// Without calling `env_clear()`, the child process will have access to **all** parent
    /// environment variables, including potentially sensitive values like API keys,
    /// database credentials, and authentication tokens.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Child inherits parent environment + adds NODE_ENV
    /// let (client, _conn) = StdioClientBuilder::new("node")
    ///     .env("NODE_ENV", "production")
    ///     .build()
    ///     .await?;
    /// ```
    #[must_use]
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.envs.insert(key.into(), value.into());
        self
    }

    /// Add multiple environment variables to the child process
    ///
    /// By default, the child process **inherits all environment variables from the parent**
    /// and this method adds or overrides specific variables.
    ///
    /// # Security Warning
    ///
    /// Without calling `env_clear()`, the child process will have access to **all** parent
    /// environment variables. See `env()` method documentation for details.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use std::collections::HashMap;
    ///
    /// let mut env_vars = HashMap::new();
    /// env_vars.insert("NODE_ENV".to_string(), "production".to_string());
    /// env_vars.insert("DEBUG".to_string(), "mcp:*".to_string());
    ///
    /// let builder = StdioClientBuilder::new("node")
    ///     .envs(env_vars);
    /// ```
    #[must_use]
    pub fn envs(mut self, envs: HashMap<String, String>) -> Self {
        self.envs.extend(envs);
        self
    }

    /// Clear all inherited environment variables
    ///
    /// After calling this method, the child process will **only** have environment variables
    /// that are explicitly set via `env()` or `envs()`. No variables from the parent process
    /// will be inherited.
    ///
    /// This is the recommended approach for security-sensitive applications where you want
    /// explicit control over what the child process can access.
    ///
    /// # Example - Secure Execution
    ///
    /// ```ignore
    /// // Child has ONLY these variables (parent environment completely cleared)
    /// let (client, _conn) = StdioClientBuilder::new("node")
    ///     .env_clear()
    ///     .env("PATH", "/usr/bin:/usr/local/bin")
    ///     .env("NODE_ENV", "production")
    ///     .env("HOME", "/tmp/sandbox")
    ///     .build()
    ///     .await?;
    /// ```
    ///
    /// # Example - Preventing Secret Leakage
    ///
    /// ```ignore
    /// // Parent has AWS_SECRET_ACCESS_KEY, DATABASE_URL, etc.
    /// // Child will NOT inherit any of them
    /// let (client, _conn) = StdioClientBuilder::new("untrusted-script")
    ///     .env_clear()
    ///     .env("SAFE_VAR", "value")
    ///     .build()
    ///     .await?;
    /// ```
    #[must_use]
    pub fn env_clear(mut self) -> Self {
        self.clear_env = true;
        self
    }

    /// Remove a specific environment variable from the child process
    ///
    /// This method prevents a specific variable from being inherited from the parent,
    /// while allowing all other parent variables to be inherited. This is useful when
    /// you want mostly-inherited environment but need to exclude specific variables.
    ///
    /// Multiple calls to `env_remove()` accumulate (each variable is tracked separately).
    ///
    /// # Example - Selective Removal
    ///
    /// ```ignore
    /// // Inherit all parent vars EXCEPT these sensitive ones
    /// let (client, _conn) = StdioClientBuilder::new("node")
    ///     .env_remove("AWS_SECRET_ACCESS_KEY")
    ///     .env_remove("DATABASE_PASSWORD")
    ///     .env_remove("API_TOKEN")
    ///     .env("NODE_ENV", "production")
    ///     .build()
    ///     .await?;
    /// ```
    ///
    /// # Example - Override After Remove
    ///
    /// ```ignore
    /// // Remove inherited HOME, then set custom value
    /// let (client, _conn) = StdioClientBuilder::new("bash")
    ///     .env_remove("HOME")
    ///     .env("HOME", "/tmp/sandbox")
    ///     .build()
    ///     .await?;
    /// ```
    #[must_use]
    pub fn env_remove(mut self, key: impl Into<String>) -> Self {
        self.env_removes.push(key.into());
        self
    }

    /// Set the working directory for the child process
    ///
    /// # Example
    ///
    /// ```ignore
    /// let builder = StdioClientBuilder::new("node")
    ///     .arg("server.js")
    ///     .current_dir("/path/to/server");
    /// ```
    #[must_use]
    pub fn current_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.current_dir = Some(dir.into());
        self
    }

    /// Set a custom timeout for MCP operations
    ///
    /// Default is 30 seconds if not specified.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use std::time::Duration;
    ///
    /// let builder = StdioClientBuilder::new("python3")
    ///     .arg("slow_server.py")
    ///     .timeout(Duration::from_secs(120));
    /// ```
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set a custom client name for MCP identification
    ///
    /// Default is "kodegen-stdio-client" if not specified.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let builder = StdioClientBuilder::new("uvx")
    ///     .arg("mcp-server-git")
    ///     .client_name("my-app-git-client");
    /// ```
    #[must_use]
    pub fn client_name(mut self, name: impl Into<String>) -> Self {
        self.client_name = Some(name.into());
        self
    }

    /// Build and connect the MCP client
    ///
    /// Returns a tuple of (client, connection):
    /// - `client`: Clone-able handle for MCP operations
    /// - `connection`: Lifecycle manager that controls the spawned process
    ///
    /// The child process is spawned with stdin/stdout for JSON-RPC communication.
    /// When the connection is dropped, the process is gracefully terminated.
    ///
    /// # Errors
    ///
    /// Returns `ClientError::Connection` if:
    /// - Command is empty or whitespace-only
    /// - Command contains spaces (arguments should use .arg())
    /// - Command not found in PATH
    /// - Process spawn fails
    ///
    /// Returns `ClientError::InitError` if MCP initialization fails.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let (client, _conn) = StdioClientBuilder::new("uvx")
    ///     .arg("mcp-server-git")
    ///     .build()
    ///     .await?;
    ///
    /// let tools = client.list_tools().await?;
    /// // _conn dropped → process gracefully terminated
    /// ```
    pub async fn build(self) -> Result<(KodegenClient, KodegenConnection), ClientError> {
        // Validate non-empty command
        let trimmed_command = self.command.trim();
        if trimmed_command.is_empty() {
            return Err(ClientError::Connection {
                message: "Command cannot be empty or whitespace-only".to_string(),
                transport_type: Some(crate::TransportType::Stdio),
                endpoint: None,
            });
        }

        // Validate no spaces in command (common mistake)
        if self.command.contains(' ') {
            return Err(ClientError::Connection {
                message: format!(
                    "Command '{}' contains spaces. Arguments should be passed via .arg() method, not in the command string.\n\
                    Example: StdioClientBuilder::new(\"node\").arg(\"server.js\")",
                    self.command
                ),
                transport_type: Some(crate::TransportType::Stdio),
                endpoint: Some(self.command.clone()),
            });
        }

        // Validate command exists in PATH
        if let Err(e) = which::which(&self.command) {
            return Err(ClientError::Connection {
                message: format!(
                    "Command '{}' not found in PATH: {}\n\
                    Please ensure the command is installed and available in your system PATH.",
                    self.command, e
                ),
                transport_type: Some(crate::TransportType::Stdio),
                endpoint: Some(self.command.clone()),
            });
        }

        // Build tokio Command with configuration
        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args);

        // Apply environment variable configuration
        if self.clear_env {
            cmd.env_clear();
        }

        // Remove specific environment variables
        for key in &self.env_removes {
            cmd.env_remove(key);
        }

        // Add/override custom environment variables
        if !self.envs.is_empty() {
            cmd.envs(&self.envs);
        }

        if let Some(dir) = &self.current_dir {
            cmd.current_dir(dir);
        }

        // Create transport - TokioChildProcess automatically sets stdin/stdout to piped
        //
        // CLEANUP BEHAVIOR: When the returned KodegenConnection is dropped or closed:
        //   1. rmcp's DropGuard cancels the service task via CancellationToken
        //   2. Service loop exits and calls transport.close()
        //   3. TokioChildProcess.graceful_shutdown() executes:
        //      - Closes stdin to signal server exit
        //      - Waits up to 3 seconds for graceful termination (MAX_WAIT_ON_DROP_SECS)
        //      - Force kills process if timeout exceeded (SIGKILL/TerminateProcess)
        //   4. tokio::process::Child.wait() reaps zombie processes
        //
        // See: ./tmp/rmcp/crates/rmcp/src/transport/child_process.rs:114-137 (graceful_shutdown)
        // See: ./tmp/rmcp/crates/rmcp/src/service.rs:839 (transport.close() call)
        // See: ./tmp/rmcp/crates/rmcp/src/transport/child_process.rs:12 (MAX_WAIT_ON_DROP_SECS = 3)
        let transport = TokioChildProcess::new(cmd).map_err(|e| ClientError::Connection {
            message: format!("Failed to spawn process '{}': {}", self.command, e),
            transport_type: Some(crate::TransportType::Stdio),
            endpoint: Some(self.command.clone()),
        })?;

        // Create client info with metadata
        let client_info = create_client_info(
            self.client_name
                .unwrap_or_else(|| "kodegen-stdio-client".to_string()),
        );

        // Initialize MCP connection
        let service = client_info
            .serve(transport)
            .await?;

        // Wrap in connection and extract client with configured timeout
        let connection = KodegenConnection::from_service(service);
        let client = connection.client().with_timeout(self.timeout);

        Ok((client, connection))
    }
}

/// Create a stdio-based MCP client by spawning a child process
///
/// This is a convenience function for simple cases. For advanced configuration
/// (environment variables, working directory, custom timeout, etc.), use
/// [`StdioClientBuilder`] instead.
///
/// Returns a tuple of (client, connection):
/// - `client`: Clone-able handle for MCP operations
/// - `connection`: Lifecycle manager that controls the spawned process
///
/// The child process is spawned with stdin/stdout for JSON-RPC communication.
/// When the connection is dropped, the process is gracefully terminated.
///
/// # Example
///
/// ```ignore
/// use kodegen_mcp_client::create_stdio_client;
///
/// // Simple usage
/// let (client, _conn) = create_stdio_client("uvx", &["mcp-server-git"]).await?;
/// let tools = client.list_tools().await?;
/// // _conn dropped → process gracefully terminated
/// ```
///
/// # Errors
///
/// Returns `ClientError::Io` if the process spawn fails,
/// or `ClientError::InitError` if MCP initialization fails.
pub async fn create_stdio_client(
    command: &str,
    args: &[&str],
) -> Result<(KodegenClient, KodegenConnection), ClientError> {
    StdioClientBuilder::new(command)
        .args(args.iter().copied())
        .build()
        .await
}
