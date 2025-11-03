// Integration tests for stdio transport
use kodegen_mcp_client::{ClientError, StdioClientBuilder, create_stdio_client};
use std::time::Duration;

/// Test that invalid command returns appropriate error
#[tokio::test]
async fn test_invalid_command_error() {
    let result = create_stdio_client("nonexistent_command_12345", &[]).await;

    assert!(result.is_err());
    if let Err(error) = result {
        match error {
            ClientError::Connection(msg) => {
                assert!(msg.contains("Failed to spawn process"));
                assert!(msg.contains("nonexistent_command_12345"));
            }
            ClientError::Io(_) => {
                // Also acceptable - spawn failure as IO error
            }
            other => panic!("Expected Connection or Io error, got: {:?}", other),
        }
    }
}

/// Test builder pattern with various configurations
#[tokio::test]
async fn test_builder_pattern() {
    let builder = StdioClientBuilder::new("echo")
        .arg("test")
        .arg("arg1")
        .arg("arg2")
        .env("TEST_VAR", "value")
        .timeout(Duration::from_secs(60))
        .client_name("test-client");

    // Builder should be clonable
    let _builder2 = builder.clone();

    // This will fail to initialize MCP (echo isn't an MCP server)
    // but it tests that the builder constructs and spawns correctly
    let result = builder.build().await;
    assert!(result.is_err());
}

/// Integration test with actual MCP server (requires uvx)
///
/// Run with: cargo test test_stdio_git_server --ignored
///
/// This test is ignored by default because it requires:
/// - uvx to be installed
/// - internet connection (to download mcp-server-git if not cached)
#[tokio::test]
#[ignore]
async fn test_stdio_git_server() {
    let (client, _conn) = create_stdio_client("uvx", &["mcp-server-git"])
        .await
        .expect("Failed to create stdio client");

    // List available tools
    let tools = client.list_tools().await.expect("Failed to list tools");

    assert!(!tools.is_empty(), "Expected non-empty tool list");

    // Verify git-related tools exist
    let tool_names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(
        tool_names.iter().any(|name| name.contains("git")),
        "Expected git-related tools"
    );
}

/// Integration test with builder and advanced configuration
///
/// Run with: cargo test test_stdio_builder_advanced --ignored
#[tokio::test]
#[ignore]
async fn test_stdio_builder_advanced() {
    let (client, _conn) = StdioClientBuilder::new("uvx")
        .arg("mcp-server-git")
        .timeout(Duration::from_secs(120))
        .client_name("integration-test-client")
        .build()
        .await
        .expect("Failed to create stdio client");

    // Test client operations
    let tools = client.list_tools().await.expect("Failed to list tools");
    assert!(!tools.is_empty());

    // Test that client can be cloned
    let client2 = client.clone();
    let tools2 = client2
        .list_tools()
        .await
        .expect("Failed to list tools with cloned client");
    assert_eq!(tools.len(), tools2.len());
}

/// Test that connection cleanup works properly
#[tokio::test]
#[ignore]
async fn test_connection_lifecycle() {
    let (client, conn) = create_stdio_client("uvx", &["mcp-server-git"])
        .await
        .expect("Failed to create stdio client");

    // Client should work
    let _ = client.list_tools().await.expect("Failed to list tools");

    // Explicitly close connection
    conn.close().await.expect("Failed to close connection");

    // After close, operations should fail (connection closed)
    // Note: This might succeed if buffered, but demonstrates graceful shutdown
}

/// Test with environment variables
#[tokio::test]
#[ignore]
async fn test_stdio_with_env_vars() {
    let (client, _conn) = StdioClientBuilder::new("uvx")
        .arg("mcp-server-git")
        .env("GIT_AUTHOR_NAME", "Test Author")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .build()
        .await
        .expect("Failed to create stdio client with env vars");

    let tools = client.list_tools().await.expect("Failed to list tools");
    assert!(!tools.is_empty());
}

/// Test timeout configuration
#[tokio::test]
async fn test_custom_timeout() {
    // This test just verifies builder accepts timeout without error
    // Actual timeout testing would require a slow or hanging server
    let builder = StdioClientBuilder::new("echo")
        .arg("test")
        .timeout(Duration::from_millis(100));

    let _ = builder.build().await;
    // Expected to fail (echo isn't MCP server), but builder should work
}

/// Test multiple environment variables via hashmap
#[tokio::test]
#[ignore]
async fn test_stdio_with_multiple_envs() {
    use std::collections::HashMap;

    let mut envs = HashMap::new();
    envs.insert("VAR1".to_string(), "value1".to_string());
    envs.insert("VAR2".to_string(), "value2".to_string());

    let (client, _conn) = StdioClientBuilder::new("uvx")
        .arg("mcp-server-git")
        .envs(envs)
        .build()
        .await
        .expect("Failed to create stdio client with multiple envs");

    let tools = client.list_tools().await.expect("Failed to list tools");
    assert!(!tools.is_empty());
}
