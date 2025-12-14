# Postero

A high-performance Zotero synchronization tool written in Rust.

## What It Does

Postero syncs your Zotero library (groups, items, collections, tags, attachments) to a local PostgreSQL database, enabling:

- **Full SQL access** to your reference library
- **Offline backup** of all Zotero data
- **REST API** via PostgREST for programmatic access
- **S3/MinIO storage** for attachments

## Prerequisites

- Rust 1.70+
- PostgreSQL 12+
- Docker & Docker Compose (for local development)
- MinIO or S3-compatible storage (optional, for attachments)

## Quick Start

### 1. Clone and Build

```bash
git clone https://github.com/chriscarrollsmith/postero
cd postero
cargo build --release
```

### 2. Start Services

```bash
./setup-database.sh
```

This starts PostgreSQL, MinIO, and PostgREST via Docker Compose and initializes the database schema.

### 3. Configure

Create `postero.toml`:

```toml
endpoint = "https://api.zotero.org"
apikey = "your-zotero-api-key"
loglevel = "info"
newgroupactive = true

[database]
servertype = "postgres"
dsn = "postgres://postgres:postgres@localhost:5432/zotero?sslmode=disable"
schema = "public"

[s3]
endpoint = "localhost:9000"
accessKeyId = "minioadmin"
secretAccessKey = "minioadmin"
useSSL = false
```

### 4. Sync

```bash
# Sync all groups
cargo run --bin sync

# Sync specific group
cargo run --bin sync -- --group 12345

# Clear and re-sync a group
cargo run --bin sync -- --group 12345 --clear

# Use custom config file
cargo run --bin sync -- --config /path/to/config.toml
```

## Architecture

```
src/
├── bin/sync.rs      # CLI sync tool
├── config.rs        # TOML configuration
├── error.rs         # Error types
├── lib.rs           # Library exports
├── filesystem/      # S3 storage abstraction
│   ├── mod.rs
│   └── s3.rs
└── zotero/          # Zotero API client
    ├── mod.rs
    ├── client.rs    # API client
    ├── group.rs     # Group sync
    ├── item.rs      # Item handling
    ├── collection.rs
    ├── tag.rs
    ├── user.rs
    ├── sync.rs      # Sync logic
    └── types.rs     # Data types
```

## Key Features

- **Async/await** - Built on Tokio for efficient I/O
- **Type-safe SQL** - SQLx with compile-time query verification
- **Structured logging** - Tracing for configurable log output
- **Flexible config** - Accepts both camelCase and lowercase field names

## Development

```bash
# Run tests
cargo test

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt
```

## Roadmap

- [ ] User library sync (in progress on feature branch)
- [ ] Vector search via pgvector
- [ ] Full-text search optimization
- [ ] Prometheus metrics

## License

MIT
