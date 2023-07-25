//! Byte shuffle filter

use axum::body::Bytes;

/// Decode the byte shuffle filter.
///
/// The byte shuffle filter encodes data by reordering bytes with the aim of improving compression
/// ratio. For an array of N elements where each element is M bytes, the filter writes the 0th byte
/// of each element first, followed by the 1st byte of each element, and so on. This function
/// inverts the shuffle filter.
///
/// This implementation was inspired by the HDF5 and Zarr shuffle filter implementations.
///
/// # Arguments
///
/// * `data`: `Bytes` to deshuffle.
/// * `element_size`: Size of each element in bytes.
// Benchmarking showed that the "slow" vector initialisation was faster for the non-unrolled case.
#[allow(clippy::slow_vector_initialization)]
pub fn deshuffle(data: &Bytes, element_size: usize) -> Bytes {
    assert_eq!(data.len() % element_size, 0);
    let mut result = Vec::with_capacity(data.len());
    // Convert the Vec to a mutable u8 slice to allow indexing.
    // This was benchmarked in benches/shuffle.rs and provides ~50-100% improvement in wall clock
    // time.
    result.resize(data.len(), 0);
    let m = result.as_mut_slice();
    let num_elements = data.len() / element_size;
    // Unroll the inner loop when element size is 4 or 8.
    // This was benchmarked in benches/shuffle.rs and provides ~50% improvement in wall clock time.
    let mut dest_index = 0;
    if element_size == 4 {
        for i in 0..num_elements {
            let mut src_index = i;
            m[dest_index] = data[src_index];
            src_index += num_elements;
            dest_index += 1;
            m[dest_index] = data[src_index];
            src_index += num_elements;
            dest_index += 1;
            m[dest_index] = data[src_index];
            src_index += num_elements;
            dest_index += 1;
            m[dest_index] = data[src_index];
            dest_index += 1;
        }
    } else if element_size == 8 {
        for i in 0..num_elements {
            let mut src_index = i;
            m[dest_index] = data[src_index];
            src_index += num_elements;
            dest_index += 1;
            m[dest_index] = data[src_index];
            src_index += num_elements;
            dest_index += 1;
            m[dest_index] = data[src_index];
            src_index += num_elements;
            dest_index += 1;
            m[dest_index] = data[src_index];
            src_index += num_elements;
            dest_index += 1;
            m[dest_index] = data[src_index];
            src_index += num_elements;
            dest_index += 1;
            m[dest_index] = data[src_index];
            src_index += num_elements;
            dest_index += 1;
            m[dest_index] = data[src_index];
            src_index += num_elements;
            dest_index += 1;
            m[dest_index] = data[src_index];
            dest_index += 1;
        }
    } else {
        for i in 0..num_elements {
            let mut src_index = i;
            for _ in 0..element_size {
                m[dest_index] = data[src_index];
                src_index += num_elements;
                dest_index += 1;
            }
        }
    }
    result.into()
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

#[cfg(test)]
pub(crate) mod test_utils {
    use super::*;

    // Shuffle isn't required for the server, but is useful for testing.
    pub(crate) fn shuffle(data: &Bytes, element_size: usize) -> Bytes {
        assert_eq!(data.len() % element_size, 0);
        let mut result = Vec::with_capacity(data.len());
        for i in 0..element_size {
            let mut src_index = i;
            for _ in 0..data.len() / element_size {
                result.push(data[src_index]);
                src_index += element_size;
            }
        }
        result.into()
    }

    #[test]
    fn test_shuffle_4() {
        let data = [0, 1, 2, 3, 4, 5, 6, 7];
        let bytes = Bytes::copy_from_slice(&data);
        let result = shuffle(&bytes, 4);
        let expected = [0, 4, 1, 5, 2, 6, 3, 7];
        assert_eq!(expected.as_ref(), result);
    }

    #[test]
    fn test_shuffle_8() {
        let data = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let bytes = Bytes::copy_from_slice(&data);
        let result = shuffle(&bytes, 8);
        let expected = [0, 8, 1, 9, 2, 10, 3, 11, 4, 12, 5, 13, 6, 14, 7, 15];
        assert_eq!(expected.as_ref(), result);
    }
}
