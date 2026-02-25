use crate::chunk_store;
use crate::error::ActiveStorageError;
use crate::models;
use crate::resource_manager::ResourceManager;

use axum::{
    headers::authorization::{Authorization, Basic},
    TypedHeader,
};
use bytes::Bytes;
use tokio::sync::SemaphorePermit;

/// HTTP chunk downloader.
///
/// Implements [chunk_store::ChunkDownloader] for HTTP.
#[derive(Debug)]
pub struct ChunkDownloaderHTTP {
    reqwest_client: reqwest::Client,
}

impl ChunkDownloaderHTTP {
    /// Create a new HTTP chunk downloader.
    pub fn new() -> Self {
        Self {
            reqwest_client: reqwest::Client::new(),
        }
    }
}

impl Default for ChunkDownloaderHTTP {
    /// Create a default HTTP chunk downloader.
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> chunk_store::ChunkDownloader<'a> for ChunkDownloaderHTTP {
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
        // Prepare an authenticated HEAD request ignoring range
        let request = if let Some(TypedHeader(auth)) = auth {
            self.reqwest_client
                .head(request_data.url.as_str())
                .basic_auth(auth.username(), Some(auth.password()))
        } else {
            self.reqwest_client.head(request_data.url.as_str())
        };
        // Send the request
        match request.send().await {
            Ok(response) => match response.status() {
                reqwest::StatusCode::OK => Ok(true),
                _ => Ok(false),
            },
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
        // Acquire connection permit to be freed via drop when this function returns
        let _conn_permits = resource_manager.connection_http().await?;
        // Prepare an authenticated GET request
        let request = if let Some(TypedHeader(auth)) = auth {
            self.reqwest_client
                .get(request_data.url.as_str())
                .basic_auth(auth.username(), Some(auth.password()))
        } else {
            self.reqwest_client.get(request_data.url.as_str())
        };
        // Append range header if offset and size are specified
        let request = if let (Some(offset), Some(size)) = (request_data.offset, request_data.size) {
            let range_header_value = format!("bytes={}-{}", offset, offset + size - 1);
            request.header(reqwest::header::RANGE, range_header_value)
        } else {
            request
        };
        // Send the request
        let response = match request.send().await {
            Ok(response) => match response.status() {
                reqwest::StatusCode::OK | reqwest::StatusCode::PARTIAL_CONTENT => response,
                _ => {
                    return Err(ActiveStorageError::HTTPRequestError {
                        error: format!("HTTP request failed with status: {}", response.status()),
                    })
                }
            },
            Err(e) => return Err(ActiveStorageError::from(e)),
        };
        // Fail if the content length header is missing.
        let content_length: usize = response
            .content_length()
            .ok_or(ActiveStorageError::HTTPContentLengthMissing)?
            .try_into()?;
        // Update memory requested from resource manager to account for actual
        // size of data if we were previously unable to guess the size from request
        // data's size + offset parameters.
        // FIXME: how to account for compressed data?
        let mem_permits = &mut mem_permits;
        match mem_permits {
            None => {
                *mem_permits = resource_manager.memory(content_length).await?;
            }
            Some(permits) => {
                if permits.num_permits() == 0 {
                    *mem_permits = resource_manager.memory(content_length).await?;
                }
            }
        }
        // The data returned by the client does not have any alignment guarantees.
        // In order to reinterpret the data as an array of numbers with a higher alignment than 1,
        // we need to return the data in Bytes object in which the underlying data has a higher alignment.
        // For now we're hard-coding an alignment of 8 bytes, although this should depend on the
        // data type, and potentially whether there are any SIMD requirements.
        // Create an 8-byte aligned Vec<u8>.
        let mut buf = maligned::align_first::<u8, maligned::A8>(content_length);
        buf.extend_from_slice(&response.bytes().await?);
        // Return as Bytes.
        Ok(buf.into())
    }
}
