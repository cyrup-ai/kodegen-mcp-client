// packages/kodegen-mcp-client/src/headers.rs
//! Infrastructure context headers for kodegen stdio → HTTP backend communication.
//!
//! These headers pass infrastructure context from the kodegen stdio server to HTTP backend servers,
//! enabling resource isolation, CWD tracking, and session management.

/// Header containing the stdio connection ID (UUID).
/// Used for isolating resources (terminals, sessions) between different kodegen instances.
pub const X_KODEGEN_CONNECTION_ID: &str = "x-kodegen-connection-id";

/// Header containing the current working directory from which kodegen was spawned.
/// Used by backend servers for path resolution and as default CWD for operations.
pub const X_KODEGEN_PWD: &str = "x-kodegen-pwd";

/// Header containing the git repository root directory.
/// Used for repository-aware operations and path resolution.
pub const X_KODEGEN_GITROOT: &str = "x-kodegen-gitroot";
