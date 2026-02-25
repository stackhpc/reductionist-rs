use crate::chunk_store;
use crate::cli::CommandLineArgs;
use crate::resource_manager::ResourceManager;

use std::sync::Arc;

/// Shared application state passed to each operation request handler.
pub struct AppState {
    /// Command line arguments.
    pub args: CommandLineArgs,

    /// Resource manager.
    pub resource_manager: ResourceManager,

    // Chunk store.
    pub chunk_store: chunk_store::ChunkStore,
}

impl AppState {
    /// Create and return an [AppState].
    pub fn new(args: &CommandLineArgs) -> Self {
        let task_limit = args.thread_limit.or_else(|| Some(num_cpus::get() - 1));
        let resource_manager = ResourceManager::new(
            args.connection_limit_http,
            args.connection_limit_s3,
            args.memory_limit,
            task_limit,
        );
        let chunk_store = chunk_store::ChunkStore::new(args);

        Self {
            args: args.clone(),
            resource_manager,
            chunk_store,
        }
    }
}

/// AppState wrapped in an Atomic Reference Count (Arc) to allow multiple references.
pub type SharedAppState = Arc<AppState>;
