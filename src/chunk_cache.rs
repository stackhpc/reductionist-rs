use crate::cli::CommandLineArgs;

use bytes::Bytes;
use byte_unit::Byte;
use cached::IOCached;
use cached::stores::{DiskCacheBuilder, DiskCache};
use hashbrown::HashSet;
use serde::{Serialize, Deserialize};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::thread;
use std::sync::Arc;
use std::time::Duration;

pub struct SelfPruningChunkCache {
    chunk_cache: Arc<ChunkCache>,
}

impl SelfPruningChunkCache {
    pub fn new(args: &CommandLineArgs) -> Self {
        let chunk_cache = Arc::new(ChunkCache::new(args));

        let clone_chunk_cache = chunk_cache.clone();
        thread::spawn(move || {
            println!("Starting prune thread");
            loop {
                clone_chunk_cache.prune();
                // TODO: set the duration based on the cache TTL?
                thread::sleep(std::time::Duration::from_secs(60));
            }
        });

        Self {
            chunk_cache,
        }
    }

    pub fn set(&self, key: &String, value: &Bytes) -> Result<Option<Bytes>, String> {
        self.chunk_cache.set(key, value)
    }

    pub fn get(&self, key: &String) -> Result<Option<Bytes>, String> {
        self.chunk_cache.get(key)
    }
}

#[derive(Serialize, Deserialize)]
struct ChunkMetadata {
    path: String, // Full path to the chunk on disk
    size: usize,  // Size of the chunk in bytes
}

pub struct ChunkCache {
    chunk_metadata: DiskCache<String, ChunkMetadata>,
    chunk_cache_path: String,
    chunk_lifespan: u64,
    chunk_refresh: bool,
    chunk_cache_size_limit: Option<usize>,
}

impl ChunkCache {
    pub fn new(args: &CommandLineArgs) -> Self {

        // Specify the path to the cache directory,
        // this is required when the optional cache is enabled.
        let chunk_cache_path = <Option<String> as Clone>::clone(&args.chunk_cache_path)
            .expect("The chunk cache path must be specified when the chunk cache is enabled");
        // Specify the cache fTTL/lifespan in seconds.
        let chunk_lifespan: u64 = args.chunk_cache_age.unwrap_or(86400);
        // Specify whether cache hits refresh the TTL.
        // NOTE: we do not pass this option to the DiskCacheBuilder
        //       because we want to be able to get cache entries internally
        //       (when pruning) without updating their ttl.
        let chunk_refresh: bool = args.chunk_cache_age_refresh;
        // Optional cache size limit.
        let chunk_cache_size_limit = if let Some(size_limit) = &args.chunk_cache_size_limit {
            let bytes = Byte::parse_str(size_limit, /* ignore case */ true).expect("Invalid size limit").as_u64();
            Some(usize::try_from(bytes).unwrap())
        } else {
            None
        };

        let chunk_metadata = if args.use_chunk_cache {
            DiskCacheBuilder::new("chunk_metadata")
                .set_disk_directory(chunk_cache_path.clone())
                .set_lifespan(chunk_lifespan)
                .build()
            .expect("Failed to build chunk metadata cache")
        } else {
            // Create a dummy cache that does nothing, it's not used.
                DiskCacheBuilder::new("DUMMY CHUNK METADATA CACHE")
                    .build()
                    .expect("Failed to build dummy chunk metadata cache")
        };

        Self {
            chunk_metadata,
            chunk_cache_path,
            chunk_lifespan,
            chunk_refresh,
            chunk_cache_size_limit,
        }
    }

    pub fn set(&self, key: &String, value: &Bytes) -> Result<Option<Bytes>, String> {
        // Store the chunk on disk, the filename is the hash of the key.
        let md5_string = md5::compute(key);
        let chunk_path = format!("{}/{:?}", self.chunk_cache_path, md5_string);
        let chunk_file = File::create(&chunk_path).expect(format!("Unable to create file '{}'", chunk_path).as_str());
        let mut writer = BufWriter::new(chunk_file);
        writer.write_all(value).expect(format!("Unable to create file '{}'", chunk_path).as_str());
        // So that we can track the data and its size we store this in the cache.
        let metadata = ChunkMetadata {
            path: chunk_path,
            size: value.len(),
        };
        // Store the metadata against the key.
        match self.chunk_metadata.cache_set(String::from(key), metadata) {
            Ok(_) => {
                Ok(None)
            },
            Err(e) => {
                return Err(format!("{:?}", e));
            }
        }
    }

    pub fn get(&self, key: &String) -> Result<Option<Bytes>, String> {
        // Return the chunk if we have it.
        match self.chunk_metadata.cache_get(key) {
            Ok(metadata) => {
                if let Some(metadata) = metadata {
                    // Copy the path so metadata can be handed over to the chunk refresh.
                    let chunk_path = metadata.path.clone();
                    // If the cache hit is refreshing the TTL of the chunk
                    // we're doing this manually, outside of the DiskCache,
                    // so we can use DiskCache::cache_get without updating the TTL.
                    if self.chunk_refresh {
                        self.chunk_metadata.cache_set(String::from(key), metadata).expect("Failed to update the TTL");
                    }
                    // Pull the actual chunk data off disk.
                    let chunk_file = File::open(&chunk_path).expect(format!("Unable to open file '{}'", chunk_path).as_str());
                    let mut reader = BufReader::new(chunk_file);
                    let mut value = Vec::<u8>::new();
                    reader.read_to_end(&mut value).expect(format!("Unable to read file '{}'", chunk_path).as_str());
                    let value: Bytes = Bytes::from(value);
                    return Ok(Some(value));
                }
                Ok(None)
            },
            Err(e) => {
                return Err(format!("{:?}", e));
            }
        }
    }

    pub fn prune(&self) {

        println!("Start pruning the cache");

        // Get the underlying (SLED) cache database.
        let db = self.chunk_metadata.connection();

        // Iterate over the keys in the database.
        // We could pull out key/value pairs but the value is a private serde encoded DiskCacheEntry.
        // We'll lookup metadata by key instead, hence the non-refresh TTL configuration.
        struct Chunk {
            key: String,
            path: String,
            size: usize,
            purge: bool,
        }
        let mut chunks: Vec<Chunk> = Vec::new();
        for key in db.iter().keys() {
            if let Ok(iv) = key {
                // NOTE iv is of type sled::ivec::IVec
                if let Ok(key) = std::str::from_utf8(iv.as_ref()) {
                    let key = String::from(key);
                    self.chunk_metadata.cache_get(&key).map(|metadata| {
                        if let Some(metadata) = metadata {
                            chunks.push(Chunk {
                                key: key.clone(),
                                path: metadata.path.clone(),
                                size: metadata.size,
                                purge: false,
                            });
                        }
                    }).expect("Failed to get metadata");
                }
            }
        }
        // Sort the chunks largest to smallest.
        chunks.sort_by_key(|key_size_tuple| key_size_tuple.size);

        // The cache retains expired entries untill purged,
        // trigger the purge and use a subscriber on the sled DB to catch the removed entries.
        // https://docs.rs/sled/latest/sled/struct.Tree.html#method.watch_prefix
        let mut subscriber = db.watch_prefix(vec![]);
        self.chunk_metadata.remove_expired_entries().expect("Failed to remove expired entries");
        // Process subscriber events, blocks for the specified duration if no events.
        let mut keys_inserted = HashSet::new();
        let mut keys_removed = HashSet::new();
        while let Ok(event) = subscriber.next_timeout(Duration::from_secs(1)) {
            match event {
                // Concurrency: we'll ignore any inserted entry which may also appear removed
                // because an initial chunk_get test for the key "removes" and chunk_set "inserts" it.
                sled::Event::Insert{ key, .. } => {
                    if let Ok(key) = std::str::from_utf8(key.as_ref()) {
                        keys_inserted.insert(String::from(key));
                    }
                }
                sled::Event::Remove { key } => {
                    if let Ok(key) = std::str::from_utf8(key.as_ref()) {
                        keys_removed.insert(String::from(key));
                    }
                }
            }
        }
        // Mark chunks for deletion based on the subscriber events.
        let mut total_used: usize = 0;
        let mut total_to_purge: usize = 0;
        for chunk in chunks.iter_mut() {
            if keys_removed.contains(&chunk.key) && !keys_inserted.contains(&chunk.key) {
                chunk.purge = true;
                total_to_purge += chunk.size;
            }
            total_used += chunk.size;
        }
        // Do we need to free up any additional space?
        let mut total_excess = if let Some(size_limit) = &self.chunk_cache_size_limit {
            let total_remaining = total_used - total_to_purge;
            if total_remaining > *size_limit {
                total_remaining - *size_limit
            } else {
                0
            }
        } else {
            0
        };
        // Now delete purged chunks and any other chunks needed to free up the excess.
        for chunk in chunks.iter_mut() {
            // Add to purged chunges to release excess space.
            if total_excess > 0 && !chunk.purge{
                chunk.purge = true;
                total_excess -= chunk.size;
            }
            if chunk.purge {
                self.chunk_metadata.cache_remove(&chunk.key).expect("Cache remove failed");
                std::fs::remove_file(&chunk.path).expect(format!("Unable to remove file '{}'", chunk.path).as_str());
            }
        }

        println!("Finished pruning the cache");
    }
}
