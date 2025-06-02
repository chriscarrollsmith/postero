use postero::{
    config::Config,
    filesystem::S3FileSystem,
    zotero::ZoteroClient,
    Result,
    zotero::Library,
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
        let mut all_library_ids = Vec::new();
        
        // 1. Sync user's personal library
        let user_library_id = current_key.user_id;
        info!("Processing user library: {}", user_library_id);
        
        // Check if user library should be synced (if synconly is specified)
        let synconly = config.synconly();
        let should_sync_user = synconly.is_empty() || synconly.contains(&user_library_id);
        
        if should_sync_user {
            all_library_ids.push(user_library_id);
            
            // Try to load user library locally, create if doesn't exist
            let user_library = match zotero.load_user_local(user_library_id).await {
                Ok(mut library) => {
                    if !library.active {
                        info!("Ignoring inactive user library #{}", user_library_id);
                    } else {
                        // Set up library with client references for sync operations
                        library.set_client(
                            std::sync::Arc::new(zotero.clone()), 
                            db.clone(), 
                            config.db.schema.clone(),
                            zotero.filesystem().clone()
                        );

                        // Clear library if requested
                        let clear_before_sync = config.clear_before_sync();
                        if clear_before_sync.contains(&user_library_id) {
                            if let Err(e) = library.clear_local().await {
                                error!("Cannot clear user library {}: {}", user_library_id, e);
                                return Err(e);
                            }
                        }

                        // Sync the user library
                        if let Err(e) = library.sync().await {
                            error!("Cannot sync user library #{}: {}", user_library_id, e);
                        } else {
                            info!("Successfully synced user library #{}", user_library_id);
                        }
                    }
                    Some(library)
                }
                Err(e) if e.is_empty_result() => {
                    // Create empty user library locally
                    let (created, _sync_direction) = zotero.create_empty_user_local(user_library_id).await?;
                    if created {
                        info!("Created empty user library #{}", user_library_id);
                    }
                    None
                }
                Err(e) => {
                    error!("Cannot load user library local {}: {}", user_library_id, e);
                    return Err(e);
                }
            };
        }

        // 2. Sync group libraries
        let group_versions = zotero.get_user_group_versions(current_key.user_id).await?;
        info!("Group versions: {:?}", group_versions);

        for (library_id, version) in &group_versions {
            // Filter by synconly if specified
            if !synconly.is_empty() && !synconly.contains(library_id) {
                continue;
            }

            all_library_ids.push(*library_id);

            let library = match zotero.load_group_local(*library_id).await {
                Ok(mut library) => {
                    if !library.active {
                        info!("Ignoring inactive group library #{}", library_id);
                        continue;
                    }

                    // Set up library with client references for sync operations
                    library.set_client(
                        std::sync::Arc::new(zotero.clone()), 
                        db.clone(), 
                        config.db.schema.clone(),
                        zotero.filesystem().clone()
                    );

                    // Clear library if requested
                    let clear_before_sync = config.clear_before_sync();
                    if clear_before_sync.contains(library_id) {
                        if let Err(e) = library.clear_local().await {
                            error!("Cannot clear group library {}: {}", library_id, e);
                            return Err(e);
                        }
                    }

                    // Sync the library
                    if let Err(e) = library.sync().await {
                        error!("Cannot sync group library #{}: {}", library_id, e);
                        continue;
                    }

                    library
                }
                Err(e) if e.is_empty_result() => {
                    // Create empty group library locally
                    let (created, _sync_direction) = zotero.create_empty_group_local(*library_id).await?;
                    if created {
                        info!("Created empty group library #{}", library_id);
                    }
                    continue;
                }
                Err(e) => {
                    error!("Cannot load group library local {}: {}", library_id, e);
                    return Err(e);
                }
            };

            info!("Group library {}[{} <-> {}]", library_id, library.version, version);

            // Check if we need to update from cloud
            if library.version < *version || library.deleted || library.is_modified {
                let group_data = zotero.get_group_cloud(*library_id).await?;
                let mut new_library = Library::from_group_data(&group_data);
                
                // Preserve local sync state
                new_library.collection_version = library.collection_version;
                new_library.item_version = library.item_version;
                new_library.tag_version = library.tag_version;
                new_library.deleted = library.deleted;
                new_library.active = library.active;
                new_library.sync_direction = library.sync_direction;
                new_library.sync_tags = library.sync_tags;
                
                // Set up database connection for update
                new_library.db = Some(db.clone());
                new_library.db_schema = Some(config.db.schema.clone());

                info!("Updating group library {}[{}]", library_id, version);
                if let Err(e) = new_library.update_local().await {
                    error!("Cannot update group library {}: {}", library_id, e);
                    return Err(e);
                }
            }
        }

        // Delete unknown libraries (both user and group)
        if let Err(e) = zotero.delete_unknown_libraries_local(&all_library_ids).await {
            error!("Cannot delete unknown libraries: {}", e);
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