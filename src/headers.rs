// packages/kodegen-mcp-client/src/headers.rs
//! Infrastructure context headers for kodegen stdio → HTTP backend communication.
//!
//! Re-exports header constants from `kodegen_config` for convenience.

pub use kodegen_config::{X_KODEGEN_CONNECTION_ID, X_KODEGEN_GITROOT, X_KODEGEN_PWD};
