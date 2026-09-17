# AgilePlus Governance

A governance system for the AgilePlus ecosystem that provides policy enforcement, audit logging, and release channel management.

## Features

- **Release Channel Governance**: Enforces promotion order (alpha → canary → beta → rc → stable)
- **Policy Engine**: Rule-based checks with configurable allow/deny/audit actions
- **Audit Logging**: SQLite-backed event logging with filtering and statistics
- **Rate Limiting**: Token bucket implementation for API throttling
- **Remote Sync**: Optional synchronization with remote governance servers

## Installation

```toml
# Cargo.toml
[dependencies]
agileplus-governance = { path = "crates/agileplus-governance" }
```

### Feature Flags

| Feature | Description |
|---------|-------------|
| `sync` | Enable remote synchronization |
| `remote` | Enable remote governance client |
| `cli` | Build CLI binary |

## Quick Start

```rust
use agileplus_governance::{GovernanceClient, PolicyCheck, PolicyContext};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = GovernanceClient::with_defaults().await?;

    // Check a policy
    let check = PolicyCheck {
        resource: "my-app".to_string(),
        action: "deploy".to_string(),
        context: PolicyContext::new()
            .with_channel(ReleaseChannel::Canary),
    };

    let result = client.check_policy(check).await?;
    println!("Allowed: {}", result.allowed);

    Ok(())
}
```

## Release Channels

The system enforces a strict promotion order:

```
alpha → canary → beta → rc → stable
```

| Channel | Purpose |
|---------|---------|
| `alpha` | Initial development releases |
| `canary` | Early testing with real users |
| `beta` | Pre-release testing |
| `rc` | Release candidate |
| `stable` | Production-ready |

## Policy Engine

Define policies using the configuration:

```rust
let config = GovernanceConfig::from_env()?;
config.add_policy(Policy {
    id: "require-tests".to_string(),
    name: "Require Tests".to_string(),
    description: "All promotions require passing tests".to_string(),
    action: PolicyAction::Deny,
    resource_pattern: ".*".to_string(),
    conditions: vec![PolicyCondition::RequireTests],
    ..Default::default()
});
```

## CLI Usage

```bash
# Build CLI
cargo build -p agileplus-governance --features cli

# Check governance status
./target/debug/agileplus-governance status

# Check policy
./target/debug/agileplus-governance policy \
    --action promote \
    --resource my-app \
    --channel canary

# Promote release
./target/debug/agileplus-governance promote \
    --crate-name my-app \
    --from canary \
    --to beta
```

## Configuration

| Environment Variable | Default | Description |
|----------------------|---------|-------------|
| `GOVERNANCE_DB_PATH` | `.agileplus/governance.db` | Local database path |
| `GOVERNANCE_ENABLED` | `true` | Enable governance |
| `GOVERNANCE_REMOTE_URL` | `http://localhost:8080` | Remote server URL |
| `GOVERNANCE_REMOTE_ENABLED` | `false` | Enable remote sync |
| `GOVERNANCE_SYNC_INTERVAL` | `60` | Sync interval in seconds |
| `GOVERNANCE_RATE_LIMIT` | `100` | Max requests per window |
| `GOVERNANCE_RATE_WINDOW` | `60` | Rate limit window in seconds |

## Architecture

```
┌─────────────────────────────────────────────┐
│              GovernanceClient               │
├─────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────────────┐   │
│  │PolicyEngine│  │   AuditLogger       │   │
│  └─────────────┘  └─────────────────────┘   │
│  ┌─────────────┐  ┌─────────────────────┐   │
│  │RateLimiter │  │ ReleaseChannel      │   │
│  └─────────────┘  └─────────────────────┘   │
└─────────────────────────────────────────────┘
```

## License

MIT
