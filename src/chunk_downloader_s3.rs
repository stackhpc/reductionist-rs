use crate::chunk_store;
use crate::error::ActiveStorageError;
use crate::models;
use crate::resource_manager::ResourceManager;
use crate::s3_client;

use axum::{
    headers::authorization::{Authorization, Basic},
    TypedHeader,
};
use bytes::Bytes;
use tokio::sync::SemaphorePermit;
use tracing::Instrument;

/// S3 chunk downloader.
///
/// Implements [chunk_store::ChunkDownloader] for S3.
#[derive(Debug)]
pub struct ChunkDownloaderS3 {
    /// Map of S3 client objects.
    s3_client_map: s3_client::S3ClientMap,
}

impl ChunkDownloaderS3 {
    /// Create a new S3 chunk downloader.
    pub fn new() -> Self {
        Self {
            s3_client_map: s3_client::S3ClientMap::new(),
        }
    }
}

impl Default for ChunkDownloaderS3 {
    /// Create a default S3 chunk downloader.
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> chunk_store::ChunkDownloader<'a> for ChunkDownloaderS3 {
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
        let credentials = if let Some(TypedHeader(auth)) = auth {
            s3_client::S3Credentials::access_key(auth.username(), auth.password())
        } else {
            s3_client::S3Credentials::None
        };

        let s3_client = self
            .s3_client_map
            .get(&request_data.source, credentials)
            .instrument(tracing::Span::current())
            .await;

        match s3_client
            .is_authorised(&request_data.bucket, &request_data.object)
            .await
        {
            Ok(auth) => Ok(auth),
            Err(e) => Err(ActiveStorageError::from(e)),
        }
    }

    /// Download requested chunk.
    ///
    /// Returns bytes.
    ///
    /// # Arguments
    ///
    /// * `auth`: Optional authorization header
    /// * `request_data`: RequestData object for the request
    /// * `resource_manager`: ResourceManager object
    /// * `mem_permits`: Memory permits for the request
    #[tracing::instrument(level = "DEBUG", skip(auth, request_data, resource_manager))]
    async fn download(
        &self,
        auth: &Option<TypedHeader<Authorization<Basic>>>,
        request_data: &models::RequestData,
        resource_manager: &ResourceManager,
        mut mem_permits: Option<SemaphorePermit<'a>>,
    ) -> Result<Bytes, ActiveStorageError> {
        // Convert request data to byte range for S3 request
        let range = s3_client::get_range(request_data.offset, request_data.size);
        // Acquire connection permit to be freed via drop when this function returns
        let _conn_permits = resource_manager.s3_connection().await?;

        let credentials = if let Some(TypedHeader(auth)) = auth {
            s3_client::S3Credentials::access_key(auth.username(), auth.password())
        } else {
            s3_client::S3Credentials::None
        };

        let s3_client = self
            .s3_client_map
            .get(&request_data.source, credentials)
            .instrument(tracing::Span::current())
            .await;

        s3_client
            .download_object(
                &request_data.bucket,
                &request_data.object,
                range,
                resource_manager,
                &mut mem_permits,
            )
            .await
    }
}
