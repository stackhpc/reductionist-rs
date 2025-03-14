use crate::cli::CommandLineArgs;
use crate::error::ActiveStorageError;

use byte_unit::Byte;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, ops::Add, path::PathBuf, sync::Arc, time::{SystemTime, UNIX_EPOCH}};
use tokio::{fs, sync::mpsc, spawn};

struct KeyValueMessage {
    key: String,
    value: Bytes,
}

impl KeyValueMessage {
    fn new(key: String, value: Bytes) -> Self {
        // Make sure the message owns the Bytes so we don't see unexpected, but not incorrect, behaviour caused by zero copy.
        let value = Bytes::from(value.to_vec());
        Self {
            key,
            value,
        }
    }
}

pub struct ChunkCache {
    cache: Arc<SimpleDiskCache>,
    sender: mpsc::Sender<KeyValueMessage>,
}

impl ChunkCache {
    pub fn new(args: &CommandLineArgs) -> Self {
        // Path to the cache directory.
        let path = <Option<String> as Clone>::clone(&args.chunk_cache_path)
            .expect("The chunk cache path must be specified when the chunk cache is enabled");
        // TTL/lifespan of a cache chunk in seconds. Default is 1 day.
        let lifespan = args.chunk_cache_age;
        // Minimum period in seconds between pruning the expired chunks on ttl. Default is 1 hour.
        let prune_interval_seconds = args.chunk_cache_prune_interval;
        // Maximum cache size in bytes. Can be specified as "1TB".
        let max_size_bytes = if let Some(size_limit) = &args.chunk_cache_size_limit {
            let bytes = Byte::parse_str(size_limit, /* ignore case */ true).expect("Invalid cache size limit").as_u64();
            Some(usize::try_from(bytes).unwrap())
        } else {
            None
        };
        // Size of the MPSC channel buffer, i.e. how many chunks we can queue.
        let chunk_cache_queue_size = args.chunk_cache_queue_size.unwrap_or(32);

        let cache = Arc::new(SimpleDiskCache::new(
            "chunk_cache",
            &path,
            lifespan,
            prune_interval_seconds,
            max_size_bytes
        ));
        let cache_clone = cache.clone();
        let (sender, mut receiver) = mpsc::channel::<KeyValueMessage>(chunk_cache_queue_size);
        spawn(async move {
            while let Some(message) = receiver.recv().await {
                cache_clone.set(message.key.as_str(), message.value).await.unwrap();
            }
        });
    
        Self {
            cache,
            sender,
        }
    }

    pub async fn set(&self, key: &str, value: Bytes) -> Result<Option<Bytes>, ActiveStorageError> {
        match self.sender.send(KeyValueMessage::new(String::from(key), value)).await {
            Ok(_) => {
                Ok(None)
            },
            Err(e) => {
                Err(ActiveStorageError::ChunkCacheError{ error: format!("{:?}", e) })
            }
        }
    }

    pub async fn get(&self, key: &str) -> Result<Option<Bytes>, ActiveStorageError> {
        match self.cache.get(key).await {
            Ok(value) => {
                Ok(value)
            },
            Err(e) => {
                Err(ActiveStorageError::ChunkCacheError{ error: format!("{:?}", e) })
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Metadata {
    /// Seconds after unix epoch for ache item expiry
    expires: u64,
    /// Cache value size
    size_bytes: usize,
}

impl Metadata {
    fn new(size: usize, ttl: u64) -> Self {
        let expires = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .add(ttl);
        Metadata {
            expires,
            size_bytes: size,
        }
    }
}

type CacheKeys = HashMap<String, Metadata>;

#[derive(Debug, Serialize, Deserialize)]
struct State {
    /// Per cached chunk metadata
    metadata: CacheKeys,
    /// Current cache size
    current_size_bytes: usize,
    /// Next expiry in seconds from epoch
    next_prune: u64,
}

impl State {
    fn new(prune_interval_secs: u64) -> Self {
        let next_prune = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() + prune_interval_secs;
        State {
            metadata: CacheKeys::new(),
            current_size_bytes: 0,
            next_prune,
        }
    }
}

#[derive(Debug)]
struct SimpleDiskCache {
    /// Cache folder name
    name: String,
    /// Cache parent directory
    dir: PathBuf,
    /// Max time to live for a single cache entry
    ttl_seconds: u64,
    /// Prune expired chunks at interval seconds
    prune_interval_seconds: u64,
    /// Option to limit maximum cache size in bytes
    max_size_bytes: Option<usize>,
}

impl SimpleDiskCache {

    const STATE_FILE: &'static str = "state.json";

    pub fn new(
        name: &str,
        dir: &str,
        ttl_seconds: u64,
        prune_interval_seconds: u64,
        max_size_bytes: Option<usize>
    ) -> Self {
        let name = name.to_string();
        let dir = PathBuf::from(dir);
        let path = dir.join(&name);
        if !dir.as_path().exists() {
            panic!("Cache parent dir {:?} must exist", dir)
        } else if path.exists() {
            panic!("Cache folder {:?} already exists", path.to_str())
        } else {
            std::fs::create_dir(&path).unwrap();
        }
        SimpleDiskCache {
            name,
            dir,
            ttl_seconds,
            prune_interval_seconds,
            max_size_bytes,
        }
    }

    async fn load_state(&self) -> State {
        let file = self.dir.join(&self.name).join(SimpleDiskCache::STATE_FILE);
        if file.exists() {
            serde_json::from_str(fs::read_to_string(file).await.unwrap().as_str()).unwrap()
        } else {
            State::new(self.prune_interval_seconds)
        }
    }

    async fn save_state(&self, data: State) {
        let file = self.dir.join(&self.name).join(SimpleDiskCache::STATE_FILE);
        fs::write(file, serde_json::to_string(&data).unwrap())
            .await
            .unwrap();
    }

    async fn filename_for_key(&self, key: &str) -> String {
        // Cater for long URL keys causing filename too long filesystem errors.
        format!("{:?}", md5::compute(key))
    }

    async fn get(&self, key: &str) -> Result<Option<Bytes>, String> {
        match fs::read(self.dir.join(&self.name).join(self.filename_for_key(key).await)).await {
            Ok(val) => Ok(Some(Bytes::from(val))),
            Err(err) => match err.kind() {
                std::io::ErrorKind::NotFound => {
                    Ok(None)
                },
                _ => {
                    Err(format!("{}", err))
                }
            }   
        }
    }

    async fn set(&self, key: &str, value: Bytes) -> Result<(), String > {
        let mut state = self.load_state().await;
        // Prepare the metadata
        let size = value.len();
        let path = self.dir.join(&self.name).join(self.filename_for_key(key).await);
        // Write the cache value and then update the metadata
        state.metadata.insert(key.to_owned(), Metadata::new(size, self.ttl_seconds));
        if let Err(e) = fs::write(path, value).await {
            return Err(format!("{:?}", e));
        }
        state.current_size_bytes += size;
        self.save_state(state).await;
        self.prune().await;
        Ok(())
    }

    async fn remove(&self, key: &str) {
        let mut state = self.load_state().await;
        let (mut remove, mut size_bytes) = (false, 0);
        if let Some(data) = state.metadata.get(key) {
            (remove, size_bytes) = (true, data.size_bytes);
        }
        if remove {
            let path = self.dir.join(&self.name).join(self.filename_for_key(key).await);
            fs::remove_file(path).await.unwrap();   
            state.metadata.remove(key);
            state.current_size_bytes -= size_bytes;
            self.save_state(state).await;
        }
    }

    // Removes expired cache entries
    async fn prune_expired(&self) {
        let state = self.load_state().await;
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        for (key, data) in state.metadata.iter() {
            if data.expires <= timestamp {
                self.remove(key).await;
            }
        }
    }

    // Removes items which are closest to expiry to free up disk space
    async fn prune_disk_space(&self) {
        if let Some(max_size_bytes) = self.max_size_bytes {
            let state = self.load_state().await;
            let mut current_size_bytes: usize = state.metadata.values().map(|value| value.size_bytes).sum(); // (0, |_, (_, item)| item.size_bytes);
            if current_size_bytes >= max_size_bytes {
                let mut metadata = state.metadata.iter().collect::<Vec<(&String, &Metadata)>>();
                metadata.sort_by_key(|key_value_tuple| key_value_tuple.1.expires);
                for (key, data) in metadata {
                    self.remove(key).await;
                    // Repeat size calculation (outside of remove) to avoid reloading state.
                    current_size_bytes -= data.size_bytes;
                    if current_size_bytes < max_size_bytes {
                        break;
                    }
                }
            }
        }
    }

    async fn prune(&self) {
        let mut state = self.load_state().await;
        // Prune expired when we go over the size threshold - this is optional.
        let mut prune_expired = false;
        if let Some(max_size_bytes) = self.max_size_bytes {
            prune_expired = state.current_size_bytes >= max_size_bytes;
        }
        // We also prune expired periodically.
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        prune_expired |= state.next_prune <= timestamp;
        // Prune if size or expiry threshold crossed.
        if prune_expired {
            // Prune cache of expired chunks.
            self.prune_expired().await;
            // Prune oldest but not yet expired chunks to keep within max size threshold.
            state = self.load_state().await;
            if let Some(max_size_bytes) = self.max_size_bytes {
                if state.current_size_bytes >= max_size_bytes {
                    self.prune_disk_space().await;
                }
            }
            // Record time for next purge of expired.
            state = self.load_state().await;
            state.next_prune = timestamp + self.prune_interval_seconds;
            self.save_state(state).await;
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate tempdir;

    use super::*;
    use std::time::Duration;
    use tempdir::TempDir;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_simple_disk_cache() {
        // Arrange
        let tmp_dir = TempDir::new("simple_disk_cache").unwrap();
        let cache = SimpleDiskCache::new(
            "test-cache-1",
            tmp_dir.path().to_str().unwrap(),
            10,  // ttl
            60,  // purge period
            None // max size
        );

        // Act
        let key_1 = "item-1";
        let value_1 = Bytes::from(vec![1, 2, 3, 4]);
        cache.set(key_1, value_1.clone()).await.unwrap();
        let cache_item_1 = cache.get(key_1).await;

        // Assert
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata.get(key_1).unwrap().size_bytes, value_1.len());
        assert_eq!(cache_item_1.unwrap(), Some(value_1));

        // Act
        let key_2 = "item-2";
        let value_2 = Bytes::from("Test123");
        cache.set(key_2, value_2.clone()).await.unwrap();
        let cache_item_2 = cache.get(key_2).await;

        // Assert
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 2);
        assert_eq!(metadata.get(key_2).unwrap().size_bytes, value_2.len());
        assert_eq!(cache_item_2.unwrap(), Some(value_2));

        // Act
        cache.remove(key_1).await;
        let cache_item_1 = cache.get(key_1).await;

        // Assert
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata.contains_key(key_1), false);
        assert_eq!(metadata.contains_key(key_2), true);
        assert_eq!(cache_item_1.unwrap(), None);
    }

    #[tokio::test]
    async fn test_simple_disk_cache_prune_expired_all() {

        let ttl = 1;
        let time_between_inserts = 1;
        let tmp_dir = TempDir::new("simple_disk_cache").unwrap();
        let cache = SimpleDiskCache::new(
            "test-cache-2",
            tmp_dir.path().to_str().unwrap(),
            ttl,    // ttl for cache entries
            1000, // purge expired interval set large to not trigger expiry on "set"
            None  // max cache size unset
        );

        // Action: populate cache
        let key_1 = "item-1";
        let value_1 = Bytes::from(vec![1, 2, 3, 4]);
        cache.set(key_1, value_1).await.unwrap();

        // Assert: cache populated
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 1);

        // Action: prune expired
        cache.prune_expired().await;

        // Assert: nothing expired
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 1);

        // Action: sleep past expiry time then prune expired
        sleep(Duration::from_secs(time_between_inserts)).await;
        cache.prune_expired().await;

        // Assert: cache empty
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 0);
    }

    #[tokio::test]
    async fn test_simple_disk_cache_prune_expired_stepped() {

        let ttl = 1;
        let time_between_inserts = ttl;
        let tmp_dir = TempDir::new("simple_disk_cache").unwrap();
        let cache = SimpleDiskCache::new(
            "test-cache-3",
            tmp_dir.path().to_str().unwrap(),
            ttl,    // ttl for cache entries
            1000, // purge expired interval set large to not trigger expiry on "set"
            None  // max cache size unset
        );

        // Action: populate cache with 2 entries ttl seconds apart
        let key_1 = "item-1";
        let value_1 = Bytes::from(vec![1, 2, 3, 4]);
        cache.set(key_1, value_1).await.unwrap();
        sleep(Duration::from_secs(time_between_inserts)).await;
        // The first entry should have already expired
        // but we're not hitting any thresholds for "set" to kick off pruning.
        let key_2 = "item-2";
        let value_2 = Bytes::from(vec![5, 6, 7, 8]);
        cache.set(key_2, value_2).await.unwrap();

        // Assert: cache populated
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 2);

        // Action: prune expired
        cache.prune_expired().await;

        // Assert: first entry pruned
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata.contains_key(key_1), false);
        assert_eq!(metadata.contains_key(key_2), true);

        // Action: sleep ttl then prune expired
        sleep(Duration::from_secs(time_between_inserts)).await;
        cache.prune_expired().await;

        // Assert: cache empty
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 0);
    }

    #[tokio::test]
    async fn test_simple_disk_cache_prune_size_triggered() {
        // It's the size threshold that triggers a prune on "set".
        // The prune expires entries by ttl first and this comes within the size threshold.
        // set() -> prune() -> [size threshold hit] -> prune_expired() -> [within size limit]
        let ttl = 1;
        let time_between_inserts = ttl;
        let size = 1000;
        let chunk = vec![0; size];
        let tmp_dir = TempDir::new("simple_disk_cache").unwrap();
        let cache = SimpleDiskCache::new(
            "test-cache-4",
            tmp_dir.path().to_str().unwrap(),
            ttl,           // ttl for cache entries
            1000,        // purge expired interval set large to not trigger expiry on "set"
            Some(size*2) // max cache size accomodates two entries
        );

        // Action: populate cache with large entry
        let key_1 = "item-1";
        let value_1 = Bytes::from(chunk.clone());
        cache.set(key_1, value_1).await.unwrap();

        // Assert: cache populated
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 1);

        // Action: wait ttl and populate cache with 2nd large entry
        sleep(Duration::from_secs(time_between_inserts)).await;
        let key_2 = "item-2";
        let value_2 = Bytes::from(chunk.clone());
        cache.set(key_2, value_2).await.unwrap();

        // Assert: 1st entry has been pruned
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata.contains_key(key_1), false);
        assert_eq!(metadata.contains_key(key_2), true);
    }

    #[tokio::test]
    async fn test_simple_disk_cache_prune_size_triggered_2() {
        // It's the size threshold that triggers a prune on "set".
        // The prune expires entries by ttl first but this doesn't reduce to within the size threshold,
        // nothing has expired!
        // The size pruning brings the cache to within the threshold.
        // set() -> prune() -> [size threshold hit] -> prune_expired() -> [size threshold hit] -> prune_disk_space() -> [within size limit]
        let ttl = 10;
        let time_between_inserts = 1;
        let size = 1000;
        let chunk = vec![0; size];
        let tmp_dir = TempDir::new("simple_disk_cache").unwrap();
        let cache = SimpleDiskCache::new(
            "test-cache-5",
            tmp_dir.path().to_str().unwrap(),
            ttl,           // ttl for cache entries
            1000,        // purge expired interval set large to not trigger expiry on "set"
            Some(size*2) // max cache size accomodates two entries
        );

        // Action: populate cache with large entry
        let key_1 = "item-1";
        let value_1 = Bytes::from(chunk.clone());
        cache.set(key_1, value_1).await.unwrap();

        // Assert: cache populated
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 1);

        // Action: wait 1 sec (less than ttl) and populate cache with 2nd large entry
        sleep(Duration::from_secs(time_between_inserts)).await;
        let key_2 = "item-2";
        let value_2 = Bytes::from(chunk.clone());
        cache.set(key_2, value_2).await.unwrap();

        // Assert: 1st entry has been pruned
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata.contains_key(key_1), false);
        assert_eq!(metadata.contains_key(key_2), true);
    }

    #[tokio::test]
    async fn test_simple_disk_cache_prune_periodic_expiry_triggered() {
        // It's the periodic expiry check that triggers a prune on "set".
        // The expiry time is set so low it should expire the 1st as we add the 2nd entry, and so on.
        // set(1st) -> prune() -> [no threshold hit] -> set(2nd) -> [periodic expiry hit] -> prune() -> prune_expired() -> [1st removed]
        let ttl = 1;
        let time_between_inserts = ttl;
        let tmp_dir = TempDir::new("simple_disk_cache").unwrap();
        let cache = SimpleDiskCache::new(
            "test-cache-6",
            tmp_dir.path().to_str().unwrap(),
            ttl, // ttl for cache entries
            ttl, // purge expired interval
            None
        );

        // Action: populate cache with 1st entry
        let key_1 = "item-1";
        let value_1 = Bytes::from(vec![1, 2, 3, 4]);
        cache.set(key_1, value_1).await.unwrap();

        // Assert: cache populated
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 1);

        // Action: wait ttl sec and populate cache with 2nd entry
        sleep(Duration::from_secs(time_between_inserts)).await;
        let key_2 = "item-2";
        let value_2 = Bytes::from(vec![1, 2, 3, 4]);
        cache.set(key_2, value_2).await.unwrap();

        // Assert: 1st entry has been pruned
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata.contains_key(key_1), false);
        assert_eq!(metadata.contains_key(key_2), true);

        // Action: wait ttl sec and populate cache with 3rd entry
        sleep(Duration::from_secs(time_between_inserts)).await;
        let key_3 = "item-3";
        let value_3 = Bytes::from(vec![1, 2, 3, 4]);
        cache.set(key_3, value_3).await.unwrap();

        // Assert: 2nd entry has been pruned
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata.contains_key(key_2), false);
        assert_eq!(metadata.contains_key(key_3), true);
    }
}
