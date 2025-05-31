# Postero - Rust Implementation

A high-performance Rust rewrite of the ZoteroSync Go application, providing enhanced Zotero synchronization capabilities with PostgreSQL and PostgREST integration.

## Features

- **Async/Await Architecture**: Built with Tokio for high-performance async operations
- **Type-Safe Database Operations**: Uses SQLx for compile-time verified SQL queries
- **S3 Storage Integration**: Supports MinIO/S3 compatible storage for attachments
- **Robust Error Handling**: Comprehensive error types with proper context
- **Structured Logging**: Uses tracing for structured, configurable logging
- **Configuration Management**: TOML-based configuration with command-line overrides

## Architecture

### Core Components

- **`postero::config`**: Configuration management and TOML parsing
- **`postero::error`**: Centralized error handling with detailed error types
- **`postero::filesystem`**: Async filesystem abstraction with S3 implementation
- **`postero::zotero`**: Zotero API client and data models

### Key Improvements over Go Version

1. **Memory Safety**: Rust's ownership system prevents common memory bugs
2. **Type Safety**: Strong type system catches errors at compile time
3. **Performance**: Zero-cost abstractions and efficient async runtime
4. **Error Handling**: Explicit error handling with `Result<T, E>` types
5. **Concurrency**: Built-in async/await support with proper cancellation

## Installation

### Prerequisites

- Rust 1.70+ (2021 edition)
- PostgreSQL 12+
- MinIO or S3-compatible storage

### Build from Source

```bash
git clone <repository>
cd postero
cargo build --release
```

### Dependencies

Key dependencies include:
- `tokio`: Async runtime
- `sqlx`: Type-safe SQL toolkit
- `reqwest`: HTTP client for Zotero API
- `serde`: Serialization framework
- `aws-sdk-s3`: S3 client for storage
- `tracing`: Structured logging

## Configuration

Create a `postero.toml` configuration file:

```toml
Endpoint = "https://api.zotero.org"
Apikey = "your-zotero-api-key"
Loglevel = "info"
newgroupactive = true

[database]
ServerType = "postgres"
DSN = "postgresql://user:password@localhost/zotero"
Schema = "public"

[s3]
endpoint = "http://localhost:9000"
accessKeyId = "minioaccesskey"
secretAccessKey = "miniosecretkey"
useSSL = false
```

## Usage

### Basic Sync

```bash
# Sync all groups
cargo run --bin sync

# Sync specific group
cargo run --bin sync -- --group 12345

# Clear and sync specific group
cargo run --bin sync -- --group 12345 --clear

# Use custom config file
cargo run --bin sync -- --config /path/to/config.toml
```

### Programmatic Usage

```rust
use postero::{
    config::Config,
    filesystem::S3FileSystem,
    zotero::ZoteroClient,
};
use sqlx::PgPool;
use std::sync::Arc;

#[tokio::main]
async fn main() -> postero::Result<()> {
    // Load configuration
    let config = Config::load("postero.toml")?;
    
    // Connect to database
    let db = PgPool::connect(&config.db.dsn).await?;
    
    // Initialize filesystem
    let fs = Arc::new(
        S3FileSystem::new(
            &config.s3.endpoint,
            &config.s3.access_key_id,
            &config.s3.secret_access_key,
            config.s3.use_ssl,
        ).await?
    );
    
    // Create Zotero client
    let zotero = ZoteroClient::new(
        &config.endpoint,
        &config.apikey,
        db,
        fs,
        &config.db.schema,
        config.new_group_active(),
    ).await?;
    
    // Get user's groups
    if let Some(key) = zotero.current_key() {
        let groups = zotero.get_user_group_versions(key.user_id).await?;
        println!("Found {} groups", groups.len());
    }
    
    Ok(())
}
```

## Error Handling

The Rust implementation provides comprehensive error handling:

```rust
use postero::{Error, Result};

match zotero.load_group_local(group_id).await {
    Ok(group) => println!("Loaded group: {}", group.name),
    Err(Error::EmptyResult) => println!("Group not found"),
    Err(Error::Database(e)) => eprintln!("Database error: {}", e),
    Err(Error::Api { code, message }) => {
        eprintln!("API error {}: {}", code, message)
    }
    Err(e) => eprintln!("Other error: {}", e),
}
```

## Development

### Running Tests

```bash
cargo test
```

### Linting

```bash
cargo clippy -- -D warnings
```

### Formatting

```bash
cargo fmt
```

### Database Setup

The application expects the same PostgreSQL schema as the original Go version. Use the provided init scripts:

```bash
./setup-database.sh
```

## Performance Considerations

### Async Operations

All I/O operations are async and can be efficiently multiplexed:

```rust
// Parallel group processing
let futures: Vec<_> = group_ids.iter()
    .map(|&id| zotero.load_group_local(id))
    .collect();

let groups = futures::future::join_all(futures).await;
```

### Memory Usage

- Zero-copy deserialization where possible
- Streaming for large file operations
- Efficient JSON handling with serde

### Database Performance

- Connection pooling via SQLx
- Prepared statements for repeated queries
- Batch operations for bulk inserts/updates

## Migration from Go Version

### Breaking Changes

1. **Configuration**: Minor field name changes in TOML structure
2. **Error Handling**: Different error types and handling patterns
3. **Async API**: All operations are now async

### Compatibility

- Same database schema
- Same S3 storage layout
- Compatible with existing PostgREST setup

## Roadmap

### Planned Features

1. **Enhanced Sync**: Better conflict resolution and merge strategies
2. **Vector Search**: Integration with embedding models for semantic search
3. **Full-Text Search**: PostgreSQL FTS integration
4. **Metrics**: Prometheus metrics for monitoring
5. **Clustering**: Support for distributed sync operations

### Extensions

The modular architecture supports easy extensions:

- Custom storage backends
- Additional Zotero API endpoints
- Custom sync strategies
- Integration with other reference managers

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Run `cargo test` and `cargo clippy`
6. Submit a pull request

## License

Same license as the original Go implementation.

## Support

For issues and questions:
1. Check the GitHub issues
2. Review the original Go documentation
3. Consult the Zotero API documentation 