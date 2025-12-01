// packages/kodegen-mcp-client/src/headers.rs
//! Infrastructure context headers for kodegen stdio → HTTP backend communication.
//!
//! These headers pass infrastructure context from the kodegen stdio server to HTTP backend servers,
//! enabling CWD tracking and git root detection.
//!
//! Note: Session/connection identification uses the MCP standard `Mcp-Session-Id` header.

/// Header containing the current working directory from which kodegen was spawned.
/// Used by backend servers for path resolution and as default CWD for operations.
pub const X_KODEGEN_PWD: &str = "x-kodegen-pwd";

/// Header containing the git repository root directory.
/// Used for repository-aware operations and path resolution.
pub const X_KODEGEN_GITROOT: &str = "x-kodegen-gitroot";
