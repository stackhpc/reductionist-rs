use crate::chunk_cache::ChunkCache;
use crate::chunk_downloader_http;
use crate::chunk_downloader_s3;
use crate::cli::CommandLineArgs;
use crate::error::ActiveStorageError;
use crate::metrics::LOCAL_CACHE_MISSES;
use crate::models;
use crate::resource_manager::ResourceManager;
use tokio::sync::SemaphorePermit;

use axum::{
    headers::authorization::{Authorization, Basic},
    TypedHeader,
};
use bytes::Bytes;
use tracing::Instrument;

/// Chunk downloader trait.
///
/// Defines the interface for chunk downloaders.
///
/// # Lifetime Parameters
/// * `'a`: Lifetime of the semaphore permit used for memory management.
///     
/// # Methods
/// * `is_authorised`: Check if access is authorised.
/// * `download`: Download the requested data.
pub trait ChunkDownloader<'a> {
    /// Are we authorized to access the data?
    ///
    /// Returns true if authorized, false otherwise.
    ///
    /// # Arguments
    ///
    /// * `auth`: Optional authorization header
    /// * `request_data`: RequestData object for the request
    fn is_authorised(
        &self,
        auth: &Option<TypedHeader<Authorization<Basic>>>,
        request_data: &models::RequestData,
    ) -> impl std::future::Future<Output = Result<bool, ActiveStorageError>>;

    /// Download requested data.
    ///
    /// Returns bytes.
    ///
    /// # Arguments
    ///
    /// * `auth`: Optional authorization header
    /// * `request_data`: RequestData object for the request
    /// * `resource_manager`: ResourceManager object
    /// * `mem_permits`: Memory permits for the request
    fn download(
        &self,
        auth: &Option<TypedHeader<Authorization<Basic>>>,
        request_data: &models::RequestData,
        resource_manager: &ResourceManager,
        mem_permits: Option<SemaphorePermit<'a>>,
    ) -> impl std::future::Future<Output = Result<Bytes, ActiveStorageError>>;
}

/// Chunk store.
#[derive(Debug)]
pub struct ChunkStore {
    /// Downloader for HTTP.
    downloader_http: chunk_downloader_http::ChunkDownloaderHTTP,
    /// Downloader for S3.
    downloader_s3: chunk_downloader_s3::ChunkDownloaderS3,

    /// Object chunk cache
    chunk_cache_enabled: bool,
    chunk_cache: Option<ChunkCache>,
    chunk_cache_key: String,
    chunk_cache_bypass_auth: bool,
}

impl<'a> ChunkStore {
    /// Initialize the chunk store.
    ///
    /// # Arguments
    ///
    /// * `args`: Command line arguments
    pub fn new(args: &CommandLineArgs) -> Self {
        let chunk_cache = if args.use_chunk_cache {
            let path = args
                .chunk_cache_path
                .as_ref()
                .expect("The chunk cache path must be specified when the chunk cache is enabled");
            Some(ChunkCache::new(
                path,
                args.chunk_cache_age,
                args.chunk_cache_prune_interval,
                args.chunk_cache_size_limit.clone(),
                args.chunk_cache_buffer_size,
            ))
        } else {
            None
        };

        Self {
            downloader_http: chunk_downloader_http::ChunkDownloaderHTTP::new(),
            downloader_s3: chunk_downloader_s3::ChunkDownloaderS3::new(),

            chunk_cache_enabled: args.use_chunk_cache,
            chunk_cache,
            chunk_cache_key: args.chunk_cache_key.clone(),
            chunk_cache_bypass_auth: args.chunk_cache_bypass_auth,
        }
    }

    /// Get requested data.
    ///
    /// Returns bytes.
    ///
    /// # Arguments
    /// * `auth`: Optional authorization header
    /// * `request_data`: RequestData object for the request
    /// * `resource_manager`: ResourceManager object
    /// * `mem_permits`: Memory permits for the request
    #[tracing::instrument(
        level = "DEBUG",
        skip(auth, request_data, resource_manager, mem_permits)
    )]
    pub async fn get(
        &self,
        auth: &Option<TypedHeader<Authorization<Basic>>>,
        request_data: &models::RequestData,
        resource_manager: &ResourceManager,
        mem_permits: Option<SemaphorePermit<'a>>,
    ) -> Result<bytes::Bytes, ActiveStorageError> {
        match (self.chunk_cache_enabled, &self.chunk_cache) {
            (false, _) => {
                self.download(auth, request_data, resource_manager, mem_permits).await
            }
            (true, Some(_)) => {
                self.cached_download(auth, request_data, resource_manager, mem_permits).await
            }
            (true, None) => panic!(
                "Chunk cache enabled but no chunk cache provided.\nThis is a bug. Please report it to the application developers."
            ),
        }
    }

    /// Retrieve requested data checking cache.
    ///
    /// Returns bytes.
    ///
    /// # Arguments
    /// * `auth`: Optional authorization header
    /// * `request_data`: RequestData object for the request
    /// * `resource_manager`: ResourceManager object
    /// * `mem_permits`: Memory permits for the request
    #[tracing::instrument(
        level = "DEBUG",
        skip(auth, request_data, resource_manager, mem_permits)
    )]
    async fn cached_download(
        &self,
        auth: &Option<TypedHeader<Authorization<Basic>>>,
        request_data: &models::RequestData,
        resource_manager: &ResourceManager,
        mut mem_permits: Option<SemaphorePermit<'a>>,
    ) -> Result<bytes::Bytes, ActiveStorageError> {
        // The default chunk key is built from the template "%url-%offset-%size-%auth"
        let key = self.generate_cache_key(self.chunk_cache_key.clone(), request_data, auth);

        if let Some(metadata) = self
            .chunk_cache
            .as_ref()
            .unwrap()
            .get_metadata(&key)
            .await?
        {
            if !self.chunk_cache_bypass_auth {
                // To avoid including the client authentication as part of the cache key,
                // which means we'd have a separate cache for each authorised client,
                // we instead defer the authentication to the remote source which has
                // the potential of sharing cached data between authorised clients.
                // This incurs a performance penalty, a cache per client would be faster.
                let authorised = self.is_authorised(auth, request_data).await?;
                if !authorised {
                    return Err(ActiveStorageError::Forbidden);
                }
            }

            // Update memory requested from resource manager to account for actual
            // size of data if we were previously unable to guess the size from request
            // data's size + offset parameters.
            // FIXME: how to account for compressed data?
            let mem_permits = &mut mem_permits;
            match mem_permits {
                None => {
                    *mem_permits = resource_manager.memory(metadata.size_bytes).await?;
                }
                Some(permits) => {
                    if permits.num_permits() == 0 {
                        *mem_permits = resource_manager.memory(metadata.size_bytes).await?;
                    }
                }
            }
            // We only want to get chunks for which the metadata check succeeded too,
            // otherwise chunks which are missing metadata could bypass the resource
            // manager and exhaust system resources
            let cache_value = self
                .chunk_cache
                .as_ref()
                .unwrap()
                .get(&key)
                .instrument(tracing::Span::current())
                .await?;
            if let Some(bytes) = cache_value {
                return Ok(bytes);
            }
        }

        let data = self
            .download(auth, request_data, resource_manager, mem_permits)
            .await?;

        // Write data to cache
        self.chunk_cache.as_ref().unwrap().set(&key, &data).await?;

        // Increment the prometheus metric for cache misses
        LOCAL_CACHE_MISSES.with_label_values(&["disk"]).inc();

        Ok(data)
    }

    /// Are we authorized to access the data?
    ///
    /// Returns true if authorized, false otherwise.
    ///
    /// # Arguments
    ///
    /// * `auth`: Optional authorization header
    /// * `request_data`: RequestData object for the request
    #[tracing::instrument(level = "DEBUG", skip(auth, request_data))]
    async fn is_authorised(
        &self,
        auth: &Option<TypedHeader<Authorization<Basic>>>,
        request_data: &models::RequestData,
    ) -> Result<bool, ActiveStorageError> {
        // Dispatch to appropriate downloader based on storage type
        match request_data.interface_type.as_str() {
            "http" | "https" => self.downloader_http.is_authorised(auth, request_data).await,
            "s3" => self.downloader_s3.is_authorised(auth, request_data).await,
            _ => Err(ActiveStorageError::UnsupportedInterfaceType {
                interface_type: request_data.interface_type.clone(),
            }),
        }
    }

    /// Download requested data.
    ///
    /// Returns bytes.
    ///
    /// # Arguments
    ///
    /// * `auth`: Optional authorization header
    /// * `request_data`: RequestData object for the request
    /// * `resource_manager`: ResourceManager object
    /// * `mem_permits`: Memory permits for the request
    #[tracing::instrument(
        level = "DEBUG",
        skip(auth, request_data, resource_manager, mem_permits)
    )]
    async fn download(
        &self,
        auth: &Option<TypedHeader<Authorization<Basic>>>,
        request_data: &models::RequestData,
        resource_manager: &ResourceManager,
        mem_permits: Option<SemaphorePermit<'a>>,
    ) -> Result<Bytes, ActiveStorageError> {
        // Dispatch to appropriate downloader based on storage type
        match request_data.interface_type.as_str() {
            "http" | "https" => {
                self.downloader_http
                    .download(auth, request_data, resource_manager, mem_permits)
                    .await
            }
            "s3" => {
                self.downloader_s3
                    .download(auth, request_data, resource_manager, mem_permits)
                    .await
            }
            _ => Err(ActiveStorageError::UnsupportedInterfaceType {
                interface_type: request_data.interface_type.clone(),
            }),
        }
    }

    /// Replace token with given value in chunk cache key.
    /// This is used to generate a cache key for the given request data and auth.
    ///
    /// # Arguments
    ///
    /// * `key`: Cache key template with tokens to replace
    /// * `request_data`: RequestData object for the request
    /// * `auth`: Optional authorization header
    fn generate_cache_key(
        &self,
        key: String,
        request_data: &models::RequestData,
        auth: &Option<TypedHeader<Authorization<Basic>>>,
    ) -> String {
        // The default chunk key is built from the template "%url-%offset-%size-%auth"
        let key = self.replace_cache_key_token(key, "%url", request_data.url.as_str());
        let key =
            self.replace_cache_key_token(key, "%offset", &format!("{:?}", request_data.offset));
        let key = self.replace_cache_key_token(key, "%size", &format!("{:?}", request_data.size));
        let key = self.replace_cache_key_token(key, "%dtype", &format!("{}", request_data.dtype));
        let key = self.replace_cache_key_token(
            key,
            "%byte_order",
            &format!("{:?}", request_data.byte_order),
        );
        let key = self.replace_cache_key_token(
            key,
            "%compression",
            &format!("{:?}", request_data.compression),
        );
        let auth_repr = match auth {
            Some(TypedHeader(Authorization(basic))) => {
                format!("{}:{}", basic.username(), basic.password())
            }
            None => "anon".to_string(),
        };
        let key = self.replace_cache_key_token(key, "%auth", &auth_repr);
        // No tokens should remain if the supplied key template is valid.
        if key.find('%').is_some() {
            panic!("Invalid cache key: {}", key);
        }
        key
    }

    /// Replace token with given value in chunk cache key.
    /// This is a helper function for `generate_cache_key`.
    /// We ensure the replacement value doesn't contain any '%' characters
    /// so we can detect remaining tokens in the cache key template after replacement.
    ///
    /// # Arguments
    ///
    /// * `key`: Cache key template with tokens to replace
    /// * `token`: Token to replace (e.g. "%url")
    /// * `value`: Value to replace token with  (e.g. the URL string)
    fn replace_cache_key_token(&self, key: String, token: &str, value: &str) -> String {
        key.replace(token, &value.replace('%', "_"))
    }
}
