# Task: Duplicated Client Info Construction

## Location
`src/transports/stdio.rs:237-250` (build method)
`src/transports/http.rs:35-45` (create_http_client)
`src/transports/http.rs:81-91` (create_streamable_client)

## Issue Type
- Code Duplication
- Maintenance Burden
- Inconsistency Risk

## Description
The `ClientInfo` construction code is duplicated across three locations (stdio transport, HTTP SSE transport, and HTTP streamable transport) with only minor differences in client names.

## Problem

### Duplicated Code

#### Stdio Transport
```rust
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
```

#### HTTP SSE Transport
```rust
let client_info = ClientInfo {
    protocol_version: Default::default(),
    capabilities: ClientCapabilities::default(),
    client_info: Implementation {
        name: "kodegen-http-client".to_string(),
        title: None,
        version: env!("CARGO_PKG_VERSION").to_string(),
        website_url: None,
        icons: None,
    },
};
```

#### HTTP Streamable Transport
```rust
let client_info = ClientInfo {
    protocol_version: Default::default(),
    capabilities: ClientCapabilities::default(),
    client_info: Implementation {
        name: "kodegen-streamable-client".to_string(),
        title: None,
        version: env!("CARGO_PKG_VERSION").to_string(),
        website_url: None,
        icons: None,
    },
};
```

### Issues

1. **Maintenance Burden**: If we want to add/change any field (e.g., `title`, `website_url`), we must update three places

2. **Inconsistency Risk**: Easy to update one location and forget the others

3. **No Central Configuration**: Client metadata is scattered across multiple files

4. **Limited Customization**: HTTP clients can't customize client name (hardcoded)

## Real-World Impact

### Scenario 1: Adding Website URL
```rust
// Developer wants to add website_url to Implementation
// Must update 3 files:

// src/transports/stdio.rs
website_url: Some("https://kodegen.ai".to_string()),

// src/transports/http.rs (line 43)
website_url: Some("https://kodegen.ai".to_string()),

// src/transports/http.rs (line 89)
website_url: Some("https://kodegen.ai".to_string()),

// ❌ Easy to miss one and have inconsistent metadata
```

### Scenario 2: Version Mismatch
```rust
// Developer accidentally changes version in one place:

// stdio.rs
version: "0.1.3".to_string(),

// http.rs (SSE)
version: env!("CARGO_PKG_VERSION").to_string(),  // ← Using macro

// http.rs (Streamable)
version: env!("CARGO_PKG_VERSION").to_string(),

// ⚠️ Inconsistency: Stdio client reports different version!
```

### Scenario 3: Custom Client Name for HTTP
```rust
// User wants to set custom client name for HTTP client
let (client, conn) = create_http_client("http://localhost:8080/mcp").await?;

// ❌ Can't do it! Client name is hardcoded as "kodegen-http-client"
//
// With stdio, you can:
let (client, conn) = StdioClientBuilder::new("node")
    .client_name("my-custom-client")  // ✅ Works
    .build()
    .await?;
//
// Why the inconsistency?
```

### Scenario 4: Adding Client Capabilities
```rust
// Want to set non-default capabilities:
let client_info = ClientInfo {
    protocol_version: Default::default(),
    capabilities: ClientCapabilities {
        sampling: Some(...),  // ← Need to add this
        experimental: Some(...),
        // ...
    },
    // ...
};

// Must update all 3 locations!
```

## Recommended Fixes

### Option 1: Helper Function (Simple)
```rust
/// src/lib.rs or src/client_info.rs

/// Create default client info with given name
pub(crate) fn create_client_info(name: impl Into<String>) -> ClientInfo {
    ClientInfo {
        protocol_version: Default::default(),
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: name.into(),
            title: None,
            version: env!("CARGO_PKG_VERSION").to_string(),
            website_url: Some("https://kodegen.ai".to_string()),
            icons: None,
        },
    }
}

/// Create client info with custom capabilities
pub(crate) fn create_client_info_with_capabilities(
    name: impl Into<String>,
    capabilities: ClientCapabilities,
) -> ClientInfo {
    ClientInfo {
        protocol_version: Default::default(),
        capabilities,
        client_info: Implementation {
            name: name.into(),
            title: None,
            version: env!("CARGO_PKG_VERSION").to_string(),
            website_url: Some("https://kodegen.ai".to_string()),
            icons: None,
        },
    }
}
```

Usage:
```rust
// src/transports/stdio.rs
let client_info = create_client_info(
    self.client_name.unwrap_or_else(|| "kodegen-stdio-client".to_string())
);

// src/transports/http.rs
let client_info = create_client_info("kodegen-http-client");

// src/transports/http.rs
let client_info = create_client_info("kodegen-streamable-client");
```

**Pros**:
- Simple
- Centralized
- Easy to maintain

**Cons**:
- Still can't customize from outside

### Option 2: Builder Pattern (Better)
```rust
/// Builder for creating client info
#[derive(Debug, Clone)]
pub(crate) struct ClientInfoBuilder {
    name: String,
    title: Option<String>,
    capabilities: ClientCapabilities,
}

impl ClientInfoBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            title: None,
            capabilities: ClientCapabilities::default(),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn capabilities(mut self, capabilities: ClientCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    pub fn build(self) -> ClientInfo {
        ClientInfo {
            protocol_version: Default::default(),
            capabilities: self.capabilities,
            client_info: Implementation {
                name: self.name,
                title: self.title,
                version: env!("CARGO_PKG_VERSION").to_string(),
                website_url: Some("https://kodegen.ai".to_string()),
                icons: None,
            },
        }
    }
}
```

Usage:
```rust
// src/transports/stdio.rs
let client_info = ClientInfoBuilder::new(
    self.client_name.unwrap_or_else(|| "kodegen-stdio-client".to_string())
).build();

// src/transports/http.rs
let client_info = ClientInfoBuilder::new("kodegen-http-client").build();
```

**Pros**:
- Extensible
- Can add more customization options easily
- Type-safe

**Cons**:
- More code

### Option 3: Constant Defaults with Override
```rust
/// src/lib.rs

const DEFAULT_TITLE: Option<&str> = None;
const DEFAULT_WEBSITE: Option<&str> = Some("https://kodegen.ai");
const DEFAULT_ICONS: Option<()> = None;

pub(crate) struct ClientInfoConfig {
    pub name: String,
    pub title: Option<String>,
    pub capabilities: ClientCapabilities,
}

impl ClientInfoConfig {
    pub fn stdio(name: Option<String>) -> Self {
        Self {
            name: name.unwrap_or_else(|| "kodegen-stdio-client".to_string()),
            title: DEFAULT_TITLE.map(|s| s.to_string()),
            capabilities: ClientCapabilities::default(),
        }
    }

    pub fn http() -> Self {
        Self {
            name: "kodegen-http-client".to_string(),
            title: DEFAULT_TITLE.map(|s| s.to_string()),
            capabilities: ClientCapabilities::default(),
        }
    }

    pub fn streamable() -> Self {
        Self {
            name: "kodegen-streamable-client".to_string(),
            title: DEFAULT_TITLE.map(|s| s.to_string()),
            capabilities: ClientCapabilities::default(),
        }
    }

    pub fn into_client_info(self) -> ClientInfo {
        ClientInfo {
            protocol_version: Default::default(),
            capabilities: self.capabilities,
            client_info: Implementation {
                name: self.name,
                title: self.title,
                version: env!("CARGO_PKG_VERSION").to_string(),
                website_url: DEFAULT_WEBSITE.map(|s| s.to_string()),
                icons: DEFAULT_ICONS,
            },
        }
    }
}
```

Usage:
```rust
// src/transports/stdio.rs
let client_info = ClientInfoConfig::stdio(self.client_name).into_client_info();

// src/transports/http.rs
let client_info = ClientInfoConfig::http().into_client_info();
```

**Pros**:
- Centralized defaults
- Easy to create transport-specific configs
- Type-safe

**Cons**:
- More complex

## Recommended Approach

**Option 1** (Helper Function) for immediate improvement:
- Simple and effective
- Reduces duplication
- Easy to implement

**Option 2** (Builder Pattern) for long-term:
- If we need more customization
- Matches the StdioClientBuilder pattern
- More extensible

Suggested implementation:

```rust
// src/client_info.rs (new file)

//! Client information construction helpers

use rmcp::model::{ClientCapabilities, ClientInfo, Implementation};

/// Create client info with default settings
///
/// This is used internally by transport builders to create consistent
/// client identification across all transports.
pub(crate) fn create_client_info(name: impl Into<String>) -> ClientInfo {
    ClientInfo {
        protocol_version: Default::default(),
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: name.into(),
            title: Some("KODEGEN.ᴀɪ MCP Client".to_string()),
            version: env!("CARGO_PKG_VERSION").to_string(),
            website_url: Some("https://kodegen.ai".to_string()),
            icons: None,
        },
    }
}

/// Create client info with custom capabilities
pub(crate) fn create_client_info_with_capabilities(
    name: impl Into<String>,
    capabilities: ClientCapabilities,
) -> ClientInfo {
    let mut info = create_client_info(name);
    info.capabilities = capabilities;
    info
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_info_consistency() {
        let info1 = create_client_info("test-client");
        let info2 = create_client_info("test-client");

        assert_eq!(info1.client_info.version, info2.client_info.version);
        assert_eq!(info1.client_info.website_url, info2.client_info.website_url);
        assert_eq!(info1.client_info.title, info2.client_info.title);
    }
}
```

Then update all transports:

```rust
// src/transports/stdio.rs
use crate::client_info::create_client_info;

let client_info = create_client_info(
    self.client_name.unwrap_or_else(|| "kodegen-stdio-client".to_string())
);

// src/transports/http.rs
use crate::client_info::create_client_info;

let client_info = create_client_info("kodegen-http-client");

let client_info = create_client_info("kodegen-streamable-client");
```

## Testing

```rust
#[test]
fn test_all_transports_use_same_version() {
    let stdio_info = create_client_info("stdio");
    let http_info = create_client_info("http");
    let streamable_info = create_client_info("streamable");

    assert_eq!(stdio_info.client_info.version, http_info.client_info.version);
    assert_eq!(http_info.client_info.version, streamable_info.client_info.version);
}

#[test]
fn test_client_info_has_website() {
    let info = create_client_info("test");
    assert!(info.client_info.website_url.is_some());
    assert!(info.client_info.website_url.unwrap().contains("kodegen.ai"));
}
```

## Priority
**MEDIUM** - Code quality and maintainability issue, doesn't affect functionality immediately

## Related Tasks
- Task 009: HTTP client missing timeout configuration (when adding builders, can unify client info construction)

## Implementation Steps
1. Create `src/client_info.rs` module
2. Add helper function `create_client_info()`
3. Update stdio transport to use helper
4. Update HTTP SSE transport to use helper
5. Update HTTP streamable transport to use helper
6. Add tests for consistency
7. Update documentation

## Bonus: Future Enhancement

If we add builders for HTTP clients (Task 009), we can add client name customization:

```rust
let (client, conn) = HttpClientBuilder::new(url)
    .client_name("my-custom-client")
    .build()
    .await?;
```

Then the helper function supports this naturally.
