use postero::{
    config::Config,
    filesystem::S3FileSystem,
    zotero::ZoteroClient,
    Result,
};
use clap::{Arg, Command};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{info, error};
use tracing_subscriber;

async fn sync_data(
    config: &Config,
    db: &PgPool,
    fs: Arc<dyn postero::filesystem::FileSystem>,
) -> Result<()> {
    let zotero = ZoteroClient::new(
        &config.endpoint,
        &config.apikey,
        db.clone(),
        fs,
        &config.db.schema,
        config.new_group_active(),
    ).await?;

    info!("Current key: {:?}", zotero.current_key());

    if let Some(current_key) = zotero.current_key() {
        let group_versions = zotero.get_user_group_versions(current_key.user_id).await?;
        info!("Group versions: {:?}", group_versions);

        let mut group_ids = Vec::new();
        for (group_id, version) in &group_versions {
            // Filter by synconly if specified
            let synconly = config.synconly();
            if !synconly.is_empty() && !synconly.contains(group_id) {
                continue;
            }

            group_ids.push(*group_id);

            let group = match zotero.load_group_local(*group_id).await {
                Ok(mut group) => {
                    if !group.active {
                        info!("Ignoring inactive group #{}", group_id);
                        continue;
                    }

                    // Set up group with client references for sync operations
                    group.set_client(
                        std::sync::Arc::new(zotero.clone()), 
                        db.clone(), 
                        config.db.schema.clone(),
                        zotero.filesystem().clone()
                    );

                    // Clear group if requested
                    let clear_before_sync = config.clear_before_sync();
                    if clear_before_sync.contains(group_id) {
                        if let Err(e) = group.clear_local().await {
                            error!("Cannot clear group {}: {}", group_id, e);
                            return Err(e);
                        }
                    }

                    // Sync the group
                    if let Err(e) = group.sync().await {
                        error!("Cannot sync group #{}: {}", group_id, e);
                        continue;
                    }

                    group
                }
                Err(e) if e.is_empty_result() => {
                    // Create empty group locally
                    let (created, _sync_direction) = zotero.create_empty_group_local(*group_id).await?;
                    if created {
                        info!("Created empty group #{}", group_id);
                    }
                    continue;
                }
                Err(e) => {
                    error!("Cannot load group local {}: {}", group_id, e);
                    return Err(e);
                }
            };

            info!("Group {}[{} <-> {}]", group_id, group.version, version);

            // Check if we need to update from cloud
            if group.version < *version || group.deleted || group.is_modified {
                let mut new_group = zotero.get_group_cloud(*group_id).await?;
                new_group.collection_version = group.collection_version;
                new_group.item_version = group.item_version;
                new_group.tag_version = group.tag_version;
                new_group.deleted = group.deleted;

                info!("Updating group {}[{}]", group_id, version);
                if let Err(e) = new_group.update_local().await {
                    error!("Cannot update group {}: {}", group_id, e);
                    return Err(e);
                }
            }
        }

        // Delete unknown groups
        if let Err(e) = zotero.delete_unknown_groups_local(&group_ids).await {
            error!("Cannot delete unknown groups: {}", e);
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let matches = Command::new("postero-sync")
        .version("1.0")
        .about("Zotero synchronization tool")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
        )
        .arg(
            Arg::new("clear")
                .long("clear")
                .action(clap::ArgAction::SetTrue)
                .help("Clear all data of group")
        )
        .arg(
            Arg::new("group")
                .long("group")
                .value_name("ID")
                .help("ID of zotero group to sync")
                .value_parser(clap::value_parser!(i64))
        )
        .get_matches();

    // Load configuration
    let config_file = matches.get_one::<String>("config")
        .map(|s| s.as_str())
        .unwrap_or("postero.toml");

    let mut config = Config::load(config_file)?;

    // Override config with command line arguments
    if let Some(group_id) = matches.get_one::<i64>("group") {
        config.synconly = Some(vec![*group_id]);
        config.clear_before_sync = if matches.get_flag("clear") {
            Some(vec![*group_id])
        } else {
            Some(vec![])
        };
    }

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

    info!("Starting sync process");

    // Run sync
    if let Err(e) = sync_data(&config, &db, fs).await {
        error!("Sync failed: {}", e);
        std::process::exit(1);
    }

    info!("Sync completed successfully");
    Ok(())
} 