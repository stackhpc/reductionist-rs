use crate::error::ActiveStorageError;

use byte_unit::Byte;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ops::Add,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{fs, spawn, sync::mpsc};

/// ChunkKeyValue stores a chunk ready to be cached.
struct ChunkKeyValue {
    /// Key to uniquely identify the chunk in the cache.
    key: String,
    /// Bytes to be cached.
    value: Bytes,
}

impl ChunkKeyValue {
    /// Return a ChunkKeyValue object
    fn new(key: String, value: Bytes) -> Self {
        // Make sure we own the `Bytes` so we don't see unexpected, but not incorrect,
        // behaviour caused by the zero copy of `Bytes`. i.e. let us choose when to copy.
        let value = Bytes::from(value.to_vec());
        Self { key, value }
    }
}

/// ChunkCache wraps a SimpleDiskCache object
/// and makes it async multi-thread safe by buffering all write operations
/// through an async MPSC channel.
/// SimpleDiskCache reads are inherently thread safe
/// and the ChunkCache passes these through unbuffered.
/// Cache writes are MPSC buffered. The task caching the chunk will not be blocked
/// unless the buffer is full, upon which the task will be blocked until
/// buffered space becomes available.
/// Buffer size is configurable.
pub struct ChunkCache {
    /// The underlying cache object.
    cache: Arc<SimpleDiskCache>,
    /// Sync primitive for managing write access to the cache.
    sender: mpsc::Sender<ChunkKeyValue>,
}

impl ChunkCache {
    /// Returns a ChunkCache object.
    ///
    /// # Arguments
    ///
    /// * `path`: Filesystem path where the "chunk_cache" folder is created, such as "/tmp"
    /// * `ttl_seconds`: Time in seconds to keep a chunk in the cache
    /// * `prune_interval_seconds`: Interval in seconds to routinely check and prune the cache of expired chunks
    /// * `max_size_bytes`: An optional maximum cache size expressed as a string, i.e. "100GB"
    /// * `buffer_size`: An optional size for the chunk write buffer
    pub fn new(
        path: String,
        ttl_seconds: u64,
        prune_interval_seconds: u64,
        max_size_bytes: Option<String>,
        buffer_size: Option<usize>,
    ) -> Self {
        let max_size_bytes = if let Some(size_limit) = max_size_bytes {
            let bytes = Byte::parse_str(size_limit, /* ignore case */ true)
                .expect("Invalid cache size limit")
                .as_u64();
            Some(usize::try_from(bytes).unwrap())
        } else {
            None
        };
        let cache = Arc::new(SimpleDiskCache::new(
            "chunk_cache",
            &path,
            ttl_seconds,
            prune_interval_seconds,
            max_size_bytes,
        ));
        // Clone the cache, i.e. increment the Arc's reference counter,
        // give this to an async task we spawn for handling all cache writes.
        let cache_clone = cache.clone();
        // Create a MPSC channel, give the single consumer receiving end to the write task
        // and store the sending end for use in our `set` method.
        // A download request storing to the cache need only wait for the chunk
        // to be sent to the channel.
        let buffer_size = buffer_size.unwrap_or(num_cpus::get() - 1);
        let (sender, mut receiver) = mpsc::channel::<ChunkKeyValue>(buffer_size);
        spawn(async move {
            while let Some(message) = receiver.recv().await {
                cache_clone
                    .set(message.key.as_str(), message.value)
                    .await
                    .unwrap();
            }
        });

        Self { cache, sender }
    }

    /// Stores chunk `Bytes` in the cache against an unique key.
    ///
    /// # Arguments
    ///
    /// * `key`: Unique key identifying the chunk
    /// * `value`: Chunk `Bytes` to be cached
    pub async fn set(&self, key: &str, value: Bytes) -> Result<Option<Bytes>, ActiveStorageError> {
        match self
            .sender
            .send(ChunkKeyValue::new(String::from(key), value))
            .await
        {
            Ok(_) => Ok(None),
            Err(e) => Err(ActiveStorageError::ChunkCacheError {
                error: format!("{:?}", e),
            }),
        }
    }

    /// Retrieves chunk `Bytes` from the cache for an unique key.
    ///
    /// # Arguments
    ///
    /// * `key`: Unique key identifying the chunk
    pub async fn get(&self, key: &str) -> Result<Option<Bytes>, ActiveStorageError> {
        match self.cache.get(key).await {
            Ok(value) => Ok(value),
            Err(e) => Err(ActiveStorageError::ChunkCacheError {
                error: format!("{:?}", e),
            }),
        }
    }
}

/// Metadata stored against each cache chunk.
#[derive(Debug, Serialize, Deserialize)]
struct Metadata {
    /// Seconds after unix epoch for ache item expiry.
    expires: u64,
    /// Cache value size.
    size_bytes: usize,
}

impl Metadata {
    /// Returns a Metadata object.
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

/// State stores the metadata for all cached chunks,
/// the total size of the cache,
/// and the time, as seconds from epoch, when the cache will next be checked
/// and pruned of chunks whose ttl has expired.
#[derive(Debug, Serialize, Deserialize)]
struct State {
    /// Per cached chunk metadata indexed by chunk key.
    metadata: CacheKeys,
    /// Current cache size in bytes.
    current_size_bytes: usize,
    /// When to next check and prune the cache for expired chunks, as seconds from epoch.
    next_prune: u64,
}

impl State {
    /// Returns a State object.
    fn new(prune_interval_secs: u64) -> Self {
        let next_prune = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + prune_interval_secs;
        State {
            metadata: CacheKeys::new(),
            current_size_bytes: 0,
            next_prune,
        }
    }
}

/// The SimpleDiskCache takes chunks of `Bytes` data, identified by an unique key,
/// storing each chunk as a separate file on disk. Keys are stored in a hashmap
/// serialised to a JSON state file on disk.
/// Each chunk stored has a TTL, time to live, stored as a number seconds from epoch,
/// after which the chunk will have expired and can be pruned from the cache.
/// Pruning takes place at time intervals or when the total size of the cache
/// reaches a maximum size threshold.
/// The decision whether to prune the cache is made when chunks are stored.
#[derive(Debug)]
struct SimpleDiskCache {
    /// Cache folder name.
    name: String,
    /// Cache parent directory for the cache folder, such as "/tmp", which must exist.
    dir: PathBuf,
    /// Max time to live for a single cache entry.
    ttl_seconds: u64,
    /// Interval in seconds to routinely check and prune the cache of expired chunks.
    prune_interval_seconds: u64,
    /// Optional, a maximum size for the cache.
    max_size_bytes: Option<usize>,
}

impl SimpleDiskCache {
    /// Names the JSON file used to store all cache keys and metadata.
    const STATE_FILE: &'static str = "state.json";

    /// Returns a SimpleDiskCache object.
    pub fn new(
        name: &str,
        dir: &str,
        ttl_seconds: u64,
        prune_interval_seconds: u64,
        max_size_bytes: Option<usize>,
    ) -> Self {
        let name = name.to_string();
        let dir = PathBuf::from(dir);
        let path = dir.join(&name);
        if !dir.as_path().exists() {
            panic!("Cache parent dir {} must exist", dir.to_str().unwrap())
        } else if path.exists() {
            panic!("Cache folder {} already exists", path.to_str().unwrap())
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

    /// Loads the cache state information from disk.
    ///
    /// Returns a `State` object.
    async fn load_state(&self) -> State {
        let file = self.dir.join(&self.name).join(SimpleDiskCache::STATE_FILE);
        if file.exists() {
            serde_json::from_str(fs::read_to_string(file).await.unwrap().as_str()).unwrap()
        } else {
            State::new(self.prune_interval_seconds)
        }
    }

    /// Saves the cache state information to disk.
    ///
    /// # Arguments
    ///
    /// * `state`: Cache `State` object.
    async fn save_state(&self, data: State) {
        let file = self.dir.join(&self.name).join(SimpleDiskCache::STATE_FILE);
        fs::write(file, serde_json::to_string(&data).unwrap())
            .await
            .unwrap();
    }

    /// Converts a chunk key into a string that can be used for a filename.
    /// Keys must be unique but if too long may overstep the file name limits
    /// of the underlying filesystem used to store the chunk.
    ///
    /// Returns a String.
    ///
    /// # Arguments
    ///
    /// * `key`: Unique key identifying the chunk
    async fn filename_for_key(&self, key: &str) -> String {
        // Cater for long URL keys causing filename too long filesystem errors.
        format!("{:?}", md5::compute(key))
    }

    /// Retrieves chunk `Bytes` from the cache for an unique key.
    /// The chunk simply needs to exist on disk to be returned.
    /// For performance, metadata, including TTL, isn't checked and it's possible
    /// to retrieve an expired chunk within the time window between the chunk expiring
    /// and the next cache pruning.
    /// This function does not modify the state of the cache and is thread safe.
    ///
    /// # Arguments
    ///
    /// * `key`: Unique key identifying the chunk
    async fn get(&self, key: &str) -> Result<Option<Bytes>, String> {
        match fs::read(
            self.dir
                .join(&self.name)
                .join(self.filename_for_key(key).await),
        )
        .await
        {
            Ok(val) => Ok(Some(Bytes::from(val))),
            Err(err) => match err.kind() {
                std::io::ErrorKind::NotFound => Ok(None),
                _ => Err(format!("{}", err)),
            },
        }
    }

    /// Stores chunk `Bytes` in the cache against an unique key.
    /// The cache is checked and if necessary pruned before storing the chunk.
    /// Where a maximum size limit has been set the check will take into account the size
    /// of the chunk being stored and ensure sufficient storage space is available.
    /// This function modifies the state of the cache and is not thread safe.
    ///
    /// # Arguments
    ///
    /// * `key`: Unique key identifying the chunk
    /// * `value`: Chunk `Bytes` to be cached
    async fn set(&self, key: &str, value: Bytes) -> Result<(), String> {
        let size = value.len();
        // Run the prune before storing to ensure we have sufficient space
        self.prune(/* headroom */ size).await?;
        // Write the cache value and then update the metadata
        let path = self
            .dir
            .join(&self.name)
            .join(self.filename_for_key(key).await);
        if let Err(e) = fs::write(path, value).await {
            return Err(format!("{:?}", e));
        }
        let mut state = self.load_state().await;
        state
            .metadata
            .insert(key.to_owned(), Metadata::new(size, self.ttl_seconds));
        state.current_size_bytes += size;
        self.save_state(state).await;
        Ok(())
    }

    /// Removes a chunk from the cache, identified by its key.
    ///
    /// # Arguments
    ///
    /// * `key`: Unique key identifying the chunk
    async fn remove(&self, key: &str) {
        let mut state = self.load_state().await;
        let (mut remove, mut size_bytes) = (false, 0);
        if let Some(data) = state.metadata.get(key) {
            (remove, size_bytes) = (true, data.size_bytes);
        }
        if remove {
            let path = self
                .dir
                .join(&self.name)
                .join(self.filename_for_key(key).await);
            fs::remove_file(path).await.unwrap();
            state.metadata.remove(key);
            state.current_size_bytes -= size_bytes;
            self.save_state(state).await;
        }
    }

    /// Removes all cache entries whose TTL has expired.
    async fn prune_expired(&self) {
        let state = self.load_state().await;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        for (key, data) in state.metadata.iter() {
            if data.expires <= timestamp {
                self.remove(key).await;
            }
        }
    }

    /// If the optional maximum cache size has been set, this function removes cache entries
    /// to ensure the total size of the cache is within the size limit.
    /// Entries are removed in order of TTL, oldest first.
    /// Entries whose TTL hasn't yet expired can be removed to make space.
    /// A value of `headroom_byres` can be specified and this ensures the specified number
    /// of bytes are left available after pruning, to ensure the next chunk can be saved.
    ///
    /// # Arguments
    ///
    /// * `headroom_bytes`: specifies additional free space that must be left available
    async fn prune_disk_space(&self, headroom_bytes: usize) -> Result<(), String> {
        if let Some(max_size_bytes) = self.max_size_bytes {
            if headroom_bytes > max_size_bytes {
                return Err("Chunk cannot fit within cache maximum size threshold".to_string());
            }
            let state = self.load_state().await;
            let mut current_size_bytes: usize =
                state.metadata.values().map(|value| value.size_bytes).sum();
            current_size_bytes += headroom_bytes;
            if current_size_bytes >= max_size_bytes {
                let mut metadata = state.metadata.iter().collect::<Vec<(&String, &Metadata)>>();
                metadata.sort_by_key(|key_value_tuple| key_value_tuple.1.expires);
                for (key, data) in metadata {
                    self.remove(key).await;
                    // Repeat size calculation (outside of `remove`) to avoid reloading state.
                    current_size_bytes -= data.size_bytes;
                    if current_size_bytes < max_size_bytes {
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    /// Prune the cache, this will be called before storing a chunk.
    /// First, entries will be expired based on their TTL.
    /// Second, if there's a maximum size limit on the cache it will be checked.
    /// A value of `headroom_byres` can be specified and this ensures the specified number
    /// of bytes are left available after pruning, to ensure the next chunk can be saved.
    ///
    /// # Arguments
    ///
    /// * `headroom_bytes`: specifies additional free space that must be left available
    async fn prune(&self, headroom_bytes: usize) -> Result<(), String> {
        let mut state = self.load_state().await;
        // Prune when we go over the size threshold - this is optional.
        let mut prune_expired = false;
        if let Some(max_size_bytes) = self.max_size_bytes {
            prune_expired = state.current_size_bytes + headroom_bytes >= max_size_bytes;
        }
        // We also prune at time intervals.
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        prune_expired |= state.next_prune <= timestamp;
        // Prune if either of the above thresholds were crossed.
        if prune_expired {
            // First prune on TTL.
            self.prune_expired().await;
            // Do we need to prune further to keep within a maximum size threshold?
            state = self.load_state().await;
            if let Some(max_size_bytes) = self.max_size_bytes {
                if state.current_size_bytes + headroom_bytes >= max_size_bytes {
                    self.prune_disk_space(headroom_bytes).await?;
                }
            }
            // Update state with the time of the next periodic pruning.
            state = self.load_state().await;
            state.next_prune = timestamp + self.prune_interval_seconds;
            self.save_state(state).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_simple_disk_cache() {
        // Arrange
        let tmp_dir = TempDir::new().unwrap();
        let cache = SimpleDiskCache::new(
            "test-cache-1",
            tmp_dir.path().to_str().unwrap(),
            10,   // ttl
            60,   // purge period
            None, // max size
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
        let tmp_dir = TempDir::new().unwrap();
        let cache = SimpleDiskCache::new(
            "test-cache-2",
            tmp_dir.path().to_str().unwrap(),
            ttl,  // ttl for cache entries
            1000, // purge expired interval set large to not trigger expiry on "set"
            None, // max cache size unset
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
        let tmp_dir = TempDir::new().unwrap();
        let cache = SimpleDiskCache::new(
            "test-cache-3",
            tmp_dir.path().to_str().unwrap(),
            ttl,  // ttl for cache entries
            1000, // purge expired interval set large to not trigger expiry on "set"
            None, // max cache size unset
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
        let tmp_dir = TempDir::new().unwrap();
        let cache = SimpleDiskCache::new(
            "test-cache-4",
            tmp_dir.path().to_str().unwrap(),
            ttl,            // ttl for cache entries
            1000,           // purge expired interval set large to not trigger expiry on "set"
            Some(size * 2), // max cache size accomodates two entries
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
        let tmp_dir = TempDir::new().unwrap();
        let cache = SimpleDiskCache::new(
            "test-cache-5",
            tmp_dir.path().to_str().unwrap(),
            ttl,            // ttl for cache entries
            1000,           // purge expired interval set large to not trigger expiry on "set"
            Some(size * 2), // max cache size accomodates two entries
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
        let tmp_dir = TempDir::new().unwrap();
        let cache = SimpleDiskCache::new(
            "test-cache-6",
            tmp_dir.path().to_str().unwrap(),
            ttl, // ttl for cache entries
            ttl, // purge expired interval
            None,
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

    #[tokio::test]
    async fn test_simple_disk_cache_prune_disk_space_headroom() {
        // Setup the cache with time and size limits that won't trigger pruning
        // when we insert some test data.
        // Check we have the content then prune on disk space with a headroom
        // equal to the cache size, the cache should preemptively clear.
        let max_size_bytes = 10000;
        let tmp_dir = TempDir::new().unwrap();
        let cache = SimpleDiskCache::new(
            "test-cache-7",
            tmp_dir.path().to_str().unwrap(),
            1000,                 // ttl for cache entries that we shouldn't hit
            1000,                 // purge expired interval, too infrequent for us to hit
            Some(max_size_bytes), // a max size threshold our test data shouldn't hit
        );

        // Action: populate cache with 1st entry
        let key_1 = "item-1";
        let value_1 = Bytes::from(vec![1, 2, 3, 4]);
        cache.set(key_1, value_1).await.unwrap();
        let key_2 = "item-2";
        let value_2 = Bytes::from(vec![1, 2, 3, 4]);
        cache.set(key_2, value_2).await.unwrap();

        // Assert: no entries should have been purged
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 2);

        // Action: prune disk space setting the headroom to the cache size
        assert_eq!(cache.prune_disk_space(max_size_bytes).await, Ok(()));

        // Assert: cache is empty
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 0);
    }

    #[tokio::test]
    async fn test_simple_disk_cache_chunk_too_big() {
        // Setup the cache with a size limit so small it can't accomodate our test data.
        let max_size_bytes = 100;
        let tmp_dir = TempDir::new().unwrap();
        let cache = SimpleDiskCache::new(
            "test-cache-8",
            tmp_dir.path().to_str().unwrap(),
            1,                    // ttl irrelevant for test
            60,                   // purge interval irrelevant for test
            Some(max_size_bytes), // a max size threshold too restrictive
        );

        // Action: populate cache with a chunk that just fits
        let key_1 = "item-1";
        let value_1 = Bytes::from(vec![0; max_size_bytes - 1]);
        cache.set(key_1, value_1).await.unwrap();

        // Assert: cache populated
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 1);

        // Action: populate cache with a chunk that fits exactly if the previously stored chunk is removed
        let key_2 = "item-2";
        let value_2 = Bytes::from(vec![0; max_size_bytes]);
        cache.set(key_2, value_2).await.unwrap();

        // Assert: cache content replaced
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata.contains_key(key_2), true);

        // Action: populate cache with a chunk that can't fit
        let key_3 = "item-3";
        let value_3 = Bytes::from(vec![0; max_size_bytes + 1]);
        assert_eq!(
            cache.set(key_3, value_3).await,
            Err(String::from(
                "Chunk cannot fit within cache maximum size threshold"
            ))
        );

        // Assert: cache content hasn't changed
        let metadata = cache.load_state().await.metadata;
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata.contains_key(key_2), true);
    }
}
