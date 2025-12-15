use postero::{
    config::Config,
    filesystem::S3FileSystem,
    zotero::{ZoteroClient, sync_worker::{SyncWorker, SyncWorkerConfig}},
    Result,
};
use clap::{Arg, Command};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, error};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = Command::new("postero-sync-worker")
        .version("1.0")
        .about("Event-driven Zotero sync worker")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
        )
        .arg(
            Arg::new("poll-interval")
                .long("poll-interval")
                .value_name("SECONDS")
                .help("Polling interval in seconds (default: 5)")
                .value_parser(clap::value_parser!(u64))
        )
        .arg(
            Arg::new("batch-size")
                .long("batch-size")
                .value_name("SIZE")
                .help("Batch size for processing (default: 50, max: 50)")
                .value_parser(clap::value_parser!(i32))
        )
        .arg(
            Arg::new("once")
                .long("once")
                .action(clap::ArgAction::SetTrue)
                .help("Run once and exit instead of continuous polling")
        )
        .arg(
            Arg::new("stats")
                .long("stats")
                .action(clap::ArgAction::SetTrue)
                .help("Show queue statistics and exit")
        )
        .get_matches();

    // Load configuration
    let config_file = matches.get_one::<String>("config")
        .map(|s| s.as_str())
        .unwrap_or("postero.toml");

    let config = Config::load(config_file)?;

    // Initialize logging
    let log_level = match config.loglevel() {
        "debug" => tracing::Level::DEBUG,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    };

    let subscriber = tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set tracing subscriber");

    // Connect to database
    let db = PgPool::connect(&config.db.dsn).await?;

    // Test database connection
    sqlx::query("SELECT 1").fetch_one(&db).await?;
    info!("Database connection established");

    // Initialize filesystem
    let fs = Arc::new(
        S3FileSystem::new(
            &config.s3.endpoint,
            &config.s3.access_key_id,
            &config.s3.secret_access_key,
            config.s3.use_ssl,
        ).await?
    ) as Arc<dyn postero::filesystem::FileSystem>;

    // Create Zotero client
    let client = Arc::new(ZoteroClient::new(
        &config.endpoint,
        &config.apikey,
        db.clone(),
        fs.clone(),
        &config.db.schema,
        config.new_group_active(),
    ).await?);

    info!("Zotero client initialized");

    // Configure worker
    let mut worker_config = SyncWorkerConfig::default();
    if let Some(interval) = matches.get_one::<u64>("poll-interval") {
        worker_config.poll_interval = Duration::from_secs(*interval);
    }
    if let Some(size) = matches.get_one::<i32>("batch-size") {
        worker_config.batch_size = (*size).min(50); // Zotero API limit
    }

    // Create worker
    let worker = SyncWorker::new(
        client,
        db,
        config.db.schema.clone(),
        fs,
        worker_config,
    );

    // Handle different modes
    if matches.get_flag("stats") {
        // Show statistics and exit
        let stats = worker.get_stats().await?;
        println!("Sync Queue Statistics:");
        println!("  Pending:   {}", stats.pending);
        println!("  Processed: {}", stats.processed);
        println!("  Failed:    {}", stats.failed);
        return Ok(());
    }

    if matches.get_flag("once") {
        // Run once and exit
        info!("Running single sync iteration");
        if let Err(e) = worker.run_once().await {
            error!("Sync iteration failed: {}", e);
            std::process::exit(1);
        }
        info!("Sync iteration completed");
        return Ok(());
    }

    // Normal operation: continuous polling
    info!("Starting sync worker in continuous mode");
    info!("Press Ctrl+C to stop");

    // Run the worker (runs forever)
    if let Err(e) = worker.run().await {
        error!("Sync worker failed: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
