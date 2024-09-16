//! Filter implementations.

pub mod shuffle;

use crate::error::ActiveStorageError;
use crate::models;

use axum::body::Bytes;

/// Decodes some bytes using the specified filter and returns the result.
///
/// # Arguments
///
/// * `filter`: Filter algorithm
/// * `data`: Filtered data [Bytes]
pub fn decode(filter: &models::Filter, data: &Bytes) -> Result<Bytes, ActiveStorageError> {
    match filter {
        models::Filter::Shuffle { element_size } => Ok(shuffle::deshuffle(data, *element_size)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filters;

    #[test]
    fn test_decode_shuffle() {
        let data = [1, 2, 3, 4, 5, 6, 7, 8];
        let bytes = Bytes::copy_from_slice(&data);
        let shuffled = filters::shuffle::test_utils::shuffle(&bytes, 4);
        let filter = models::Filter::Shuffle { element_size: 4 };
        let result = decode(&filter, &shuffled).unwrap();
        assert_eq!(data.as_ref(), result);
    }
}
