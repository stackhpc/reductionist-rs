//! Byte shuffle filter with SIMD-acceleration.

use axum::body::Bytes;
use byteshuffle::unshuffle;

/// Decode the byte shuffle filter.
///
/// Quoting the [byteshuffle documentation](https://docs.rs/byteshuffle/latest/byteshuffle/)
/// The byte-shuffle is a very efficient way to improve the compressibility of data that consists of an array of fixed-size objects.
/// It rearranges the array in order to group all elements’ least significant bytes together, most-significant bytes together, and everything in between.
/// Since real applications’ arrays often contain consecutive elements that are closely correlated with each other, this filter frequently results in lengthy continuous runs of identical bytes. Such runs are highly compressible by general-purpose compression libraries like gzip, lz4, etc.
///
/// The blosc project was the original inspiration for this library.
/// Blosc is a C library intended primarily for HPC users, and it implements a shuffle filter, among many other things.
/// This crate is a clean reimplementation of Blosc’s shuffle filter.
///
/// # Arguments
///
/// * `data`: `Bytes` to deshuffle.
/// * `element_size`: Size of each element in bytes.
pub fn deshuffle(data: &Bytes, element_size: usize) -> Bytes {
    assert_eq!(data.len() % element_size, 0);
    // Perform the unshuffle operation using the byteshuffle crate, which is SIMD-accelerated.
    let unshuffled = unshuffle(element_size, data);
    unshuffled.into()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn test_deshuffle_2() {
        let shuffled = [0, 2, 4, 6, 1, 3, 5, 7];
        let bytes = Bytes::copy_from_slice(&shuffled);
        let result = deshuffle(&bytes, 2);
        let expected = [0, 1, 2, 3, 4, 5, 6, 7];
        assert_eq!(expected.as_ref(), result);
    }

    #[test]
    fn test_deshuffle_4() {
        let shuffled = [0, 4, 1, 5, 2, 6, 3, 7];
        let bytes = Bytes::copy_from_slice(&shuffled);
        let result = deshuffle(&bytes, 4);
        let expected = [0, 1, 2, 3, 4, 5, 6, 7];
        assert_eq!(expected.as_ref(), result);
    }

    #[test]
    fn test_deshuffle_8() {
        let shuffled = [0, 8, 1, 9, 2, 10, 3, 11, 4, 12, 5, 13, 6, 14, 7, 15];
        let bytes = Bytes::copy_from_slice(&shuffled);
        let result = deshuffle(&bytes, 8);
        let expected = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        assert_eq!(expected.as_ref(), result);
    }
}
