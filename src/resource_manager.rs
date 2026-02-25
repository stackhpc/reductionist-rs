//! Resource management

use crate::error::ActiveStorageError;

use tokio::sync::{Semaphore, SemaphorePermit};

/// [crate::resource_manager::ResourceManager] provides a simple way to allocate various resources
/// to tasks. Resource management is performed using a Tokio Semaphore for each type of resource.
pub struct ResourceManager {
    /// Optional semaphore for HTTP connections.
    connections_http: Option<Semaphore>,

    /// Optional semaphore for S3 connections.
    connections_s3: Option<Semaphore>,

    /// Optional semaphore for memory (bytes).
    memory: Option<Semaphore>,

    /// Optional total memory pool in bytes.
    total_memory: Option<usize>,

    /// Optional semaphore for tasks.
    tasks: Option<Semaphore>,
}

impl ResourceManager {
    /// Returns a new ResourceManager object.
    pub fn new(
        connection_limit_http: Option<usize>,
        connection_limit_s3: Option<usize>,
        memory_limit: Option<usize>,
        task_limit: Option<usize>,
    ) -> Self {
        Self {
            connections_http: connection_limit_http.map(Semaphore::new),
            connections_s3: connection_limit_s3.map(Semaphore::new),
            memory: memory_limit.map(Semaphore::new),
            total_memory: memory_limit,
            tasks: task_limit.map(Semaphore::new),
        }
    }

    /// Acquire an HTTP connection resource.
    pub async fn connection_http(&self) -> Result<Option<SemaphorePermit<'_>>, ActiveStorageError> {
        optional_acquire(&self.connections_http, 1).await
    }

    /// Acquire an S3 connection resource.
    pub async fn connection_s3(&self) -> Result<Option<SemaphorePermit<'_>>, ActiveStorageError> {
        optional_acquire(&self.connections_s3, 1).await
    }

    /// Acquire memory resource.
    pub async fn memory(
        &self,
        bytes: usize,
    ) -> Result<Option<SemaphorePermit<'_>>, ActiveStorageError> {
        if let Some(total_memory) = self.total_memory {
            if bytes > total_memory {
                return Err(ActiveStorageError::InsufficientMemory {
                    requested: bytes,
                    total: total_memory,
                });
            };
        };
        optional_acquire(&self.memory, bytes).await
    }

    /// Acquire a task resource.
    pub async fn task(&self) -> Result<Option<SemaphorePermit<'_>>, ActiveStorageError> {
        optional_acquire(&self.tasks, 1).await
    }
}

/// Acquire permits on an optional Semaphore, if present.
async fn optional_acquire(
    sem: &Option<Semaphore>,
    n: usize,
) -> Result<Option<SemaphorePermit<'_>>, ActiveStorageError> {
    let n = n.try_into()?;
    if let Some(sem) = sem {
        sem.acquire_many(n)
            .await
            .map(Some)
            .map_err(|err| err.into())
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tokio::sync::TryAcquireError;

    #[tokio::test]
    async fn no_resource_management() {
        let rm = ResourceManager::new(None, None, None, None);
        assert!(rm.connections_http.is_none());
        assert!(rm.connections_s3.is_none());
        assert!(rm.memory.is_none());
        assert!(rm.tasks.is_none());
        let _c1 = rm.connection_http().await.unwrap();
        let _c2 = rm.connection_s3().await.unwrap();
        let _m = rm.memory(1).await.unwrap();
        let _t = rm.task().await.unwrap();
        assert!(_c1.is_none());
        assert!(_c2.is_none());
        assert!(_m.is_none());
        assert!(_t.is_none());
    }

    #[tokio::test]
    async fn full_resource_management() {
        let rm = ResourceManager::new(Some(1), Some(1), Some(1), Some(1));
        assert!(rm.connections_s3.is_some());
        assert!(rm.connections_http.is_some());
        assert!(rm.memory.is_some());
        assert!(rm.tasks.is_some());
        let _c1 = rm.connection_s3().await.unwrap();
        let _c2 = rm.connection_http().await.unwrap();
        let _m = rm.memory(1).await.unwrap();
        let _t = rm.task().await.unwrap();
        assert!(_c1.is_some());
        assert!(_c2.is_some());
        assert!(_m.is_some());
        assert!(_t.is_some());
        // Check that there are no more resources (without blocking).
        assert_eq!(
            rm.connections_http.as_ref().unwrap().try_acquire().err(),
            Some(TryAcquireError::NoPermits)
        );
        assert_eq!(
            rm.connections_s3.as_ref().unwrap().try_acquire().err(),
            Some(TryAcquireError::NoPermits)
        );
        assert_eq!(
            rm.memory.as_ref().unwrap().try_acquire().err(),
            Some(TryAcquireError::NoPermits)
        );
        assert_eq!(
            rm.tasks.as_ref().unwrap().try_acquire().err(),
            Some(TryAcquireError::NoPermits)
        );
    }
}
