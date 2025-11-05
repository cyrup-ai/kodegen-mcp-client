// Example to verify validation works correctly

use kodegen_mcp_client::responses::*;
use kodegen_mcp_client::validation::Validate;

fn main() {
    println!("Testing validation...\n");
    
    // Test 1: Empty session ID should fail
    println!("Test 1: Empty session ID");
    let json = r#"{"session_id": ""}"#;
    let result: Result<StartSearchResponse, _> = serde_json::from_str(json);
    match result {
        Ok(_) => println!("  ❌ FAILED: Should have rejected empty session_id"),
        Err(e) => println!("  ✅ PASSED: Rejected with error: {}", e),
    }
    
    // Test 2: Valid session ID should succeed
    println!("\nTest 2: Valid session ID");
    let json = r#"{"session_id": "valid-id-123"}"#;
    let result: Result<StartSearchResponse, _> = serde_json::from_str(json);
    match result {
        Ok(resp) => println!("  ✅ PASSED: Accepted with session_id: {}", resp.session_id),
        Err(e) => println!("  ❌ FAILED: Should have accepted valid session_id. Error: {}", e),
    }
    
    // Test 3: Negative PID should fail
    println!("\nTest 3: Negative PID");
    let json = r#"{"pid": -1}"#;
    let result: Result<StartTerminalCommandResponse, _> = serde_json::from_str(json);
    match result {
        Ok(_) => println!("  ❌ FAILED: Should have rejected negative PID"),
        Err(e) => println!("  ✅ PASSED: Rejected with error: {}", e),
    }
    
    // Test 4: Valid positive PID should succeed
    println!("\nTest 4: Valid positive PID");
    let json = r#"{"pid": 12345}"#;
    let result: Result<StartTerminalCommandResponse, _> = serde_json::from_str(json);
    match result {
        Ok(resp) => println!("  ✅ PASSED: Accepted with PID: {}", resp.pid),
        Err(e) => println!("  ❌ FAILED: Should have accepted valid PID. Error: {}", e),
    }
    
    // Test 5: Zero PID should fail
    println!("\nTest 5: Zero PID");
    let json = r#"{"pid": 0}"#;
    let result: Result<StartTerminalCommandResponse, _> = serde_json::from_str(json);
    match result {
        Ok(_) => println!("  ❌ FAILED: Should have rejected zero PID"),
        Err(e) => println!("  ✅ PASSED: Rejected with error: {}", e),
    }
    
    // Test 6: Zero GitHub user ID should fail
    println!("\nTest 6: Zero GitHub user ID");
    let json = r#"{"id": 0, "login": "user"}"#;
    let result: Result<GitHubUser, _> = serde_json::from_str(json);
    match result {
        Ok(_) => println!("  ❌ FAILED: Should have rejected zero user ID"),
        Err(e) => println!("  ✅ PASSED: Rejected with error: {}", e),
    }
    
    // Test 7: Empty GitHub user login should fail
    println!("\nTest 7: Empty GitHub user login");
    let json = r#"{"id": 123, "login": ""}"#;
    let result: Result<GitHubUser, _> = serde_json::from_str(json);
    match result {
        Ok(_) => println!("  ❌ FAILED: Should have rejected empty login"),
        Err(e) => println!("  ✅ PASSED: Rejected with error: {}", e),
    }
    
    // Test 8: Valid GitHub user should succeed
    println!("\nTest 8: Valid GitHub user");
    let json = r#"{"id": 123, "login": "user"}"#;
    let result: Result<GitHubUser, _> = serde_json::from_str(json);
    match result {
        Ok(user) => println!("  ✅ PASSED: Accepted with user: {} (ID: {})", user.login, user.id),
        Err(e) => println!("  ❌ FAILED: Should have accepted valid user. Error: {}", e),
    }
    
    // Test 9: Empty string in session_ids array should fail
    println!("\nTest 9: Empty string in session_ids array");
    let json = r#"{"session_ids": ["valid-id", "", "another-id"], "worker_count": 3}"#;
    let result: Result<SpawnClaudeAgentResponse, _> = serde_json::from_str(json);
    match result {
        Ok(_) => println!("  ❌ FAILED: Should have rejected empty string in array"),
        Err(e) => println!("  ✅ PASSED: Rejected with error: {}", e),
    }
    
    // Test 10: Count mismatch should fail validation
    println!("\nTest 10: Count mismatch in GitHubIssuesResponse");
    let json = r#"{"count": 5, "issues": []}"#;
    let result: Result<GitHubIssuesResponse, _> = serde_json::from_str(json);
    match result {
        Ok(resp) => {
            match resp.validate() {
                Ok(_) => println!("  ❌ FAILED: Should have failed validation for count mismatch"),
                Err(e) => println!("  ✅ PASSED: Validation failed with: {}", e),
            }
        },
        Err(e) => println!("  ❌ FAILED: Deserialization failed unexpectedly. Error: {}", e),
    }
    
    // Test 11: Valid worker_count matching session_ids length
    println!("\nTest 11: Valid worker_count matching session_ids length");
    let json = r#"{"session_ids": ["id1", "id2"], "worker_count": 2}"#;
    let result: Result<SpawnClaudeAgentResponse, _> = serde_json::from_str(json);
    match result {
        Ok(resp) => {
            match resp.validate() {
                Ok(_) => println!("  ✅ PASSED: Validation succeeded for matching count"),
                Err(e) => println!("  ❌ FAILED: Should have passed validation. Error: {}", e),
            }
        },
        Err(e) => println!("  ❌ FAILED: Deserialization failed. Error: {}", e),
    }
    
    // Test 12: Mismatched worker_count
    println!("\nTest 12: Mismatched worker_count");
    let json = r#"{"session_ids": ["id1", "id2"], "worker_count": 5}"#;
    let result: Result<SpawnClaudeAgentResponse, _> = serde_json::from_str(json);
    match result {
        Ok(resp) => {
            match resp.validate() {
                Ok(_) => println!("  ❌ FAILED: Should have failed validation for count mismatch"),
                Err(e) => println!("  ✅ PASSED: Validation failed with: {}", e),
            }
        },
        Err(e) => println!("  ❌ FAILED: Deserialization failed unexpectedly. Error: {}", e),
    }
    
    println!("\n=== All validation tests complete ===");
}
