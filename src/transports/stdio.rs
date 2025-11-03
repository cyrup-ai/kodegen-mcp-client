// packages/mcp-client/src/transports/stdio.rs
use crate::{ClientError, KodegenClient, KodegenConnection};
use rmcp::{
    ServiceExt,
    model::{ClientCapabilities, ClientInfo, Implementation},
    transport::TokioChildProcess,
};
use std::{collections::HashMap, path::PathBuf};
use tokio::{process::Command, time::Duration};

/// Default timeout for MCP operations (30 seconds)
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

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

    /// Add a single environment variable
    ///
    /// # Example
    ///
    /// ```ignore
    /// let builder = StdioClientBuilder::new("node")
    ///     .env("NODE_ENV", "production")
    ///     .env("DEBUG", "1");
    /// ```
    #[must_use]
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.envs.insert(key.into(), value.into());
        self
    }

    /// Add multiple environment variables
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
    /// Returns `ClientError::Io` if the process spawn fails,
    /// or `ClientError::InitError` if MCP initialization fails.
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
        // Build tokio Command with configuration
        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args);

        if !self.envs.is_empty() {
            cmd.envs(&self.envs);
        }

        if let Some(dir) = &self.current_dir {
            cmd.current_dir(dir);
        }

        // Create transport - TokioChildProcess automatically sets stdin/stdout to piped
        let transport = TokioChildProcess::new(cmd).map_err(|e| {
            ClientError::Connection(format!("Failed to spawn process '{}': {}", self.command, e))
        })?;

        // Create client info with metadata
        let client_info = ClientInfo {
            protocol_version: Default::default(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: self
                    .client_name
                    .unwrap_or_else(|| "kodegen-stdio-client".to_string()),
                title: None,
                version: env!("CARGO_PKG_VERSION").to_string(),
                website_url: None,
                icons: None,
            },
        };

        // Initialize MCP connection
        let service = client_info
            .serve(transport)
            .await
            .map_err(ClientError::InitError)?;

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
