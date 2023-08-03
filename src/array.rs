//! Functions and utilities for working with [ndarray] objects.

use crate::error::ActiveStorageError;
use crate::models;
use crate::types::NON_NATIVE_BYTE_ORDER;

use ndarray::prelude::*;
use std::convert::TryFrom;

/// Convert from data bytes to `&[T]`.
///
/// Zerocopy provides a mechanism for converting between types.
/// Correct alignment of the data is necessary.
///
/// # Arguments
///
/// * `data`: Slice of bytes containing data to convert.
fn from_bytes<T: zerocopy::AsBytes + zerocopy::FromBytes>(
    data: &mut [u8],
) -> Result<&mut [T], ActiveStorageError> {
    let layout = zerocopy::LayoutVerified::<_, [T]>::new_slice(&mut data[..]).ok_or(
        ActiveStorageError::FromBytes {
            type_name: std::any::type_name::<T>(),
        },
    )?;
    Ok(layout.into_mut_slice())
}

/// Returns an [ndarray] Shape corresponding to the data in the request.
///
/// # Arguments
///
/// * `size`: Number of elements in the array
/// * `request_data`: RequestData object for the request
pub fn get_shape(
    size: usize,
    request_data: &models::RequestData,
) -> ndarray::Shape<Dim<ndarray::IxDynImpl>> {
    // Use the provided shape, or fall back to a 1D array.
    let shape = request_data.shape.clone().unwrap_or(vec![size]);
    // Convert the Vec into a Shape.
    let shape = shape.into_shape();
    match request_data.order {
        Some(models::Order::F) => shape.f(),
        _ => shape,
    }
}

/// Returns an [ndarray::ArrayView](ndarray::ArrayView) corresponding to the data in the
/// request.
///
/// The array view borrows the data, so no copying takes place.
///
/// # Arguments
///
/// * `shape`: The shape of the array
/// * `data`: A slice of type `&[T]` containing the data to be consumed by the array view.
fn build_array_from_shape<T>(
    shape: ndarray::Shape<Dim<ndarray::IxDynImpl>>,
    data: &[T],
) -> Result<ArrayViewD<T>, ActiveStorageError> {
    ArrayView::<T, _>::from_shape(shape, data).map_err(ActiveStorageError::ShapeInvalid)
}

/// Returns a mutable [ndarray::ArrayViewMut](ndarray::ArrayViewMut) corresponding to the data in
/// the request.
///
/// The array view borrows the data, so no copying takes place.
///
/// # Arguments
///
/// * `shape`: The shape of the array
/// * `data`: A slice of type `&mut [T]` containing the data to be consumed by the array view.
pub fn build_array_mut_from_shape<T>(
    shape: ndarray::Shape<Dim<ndarray::IxDynImpl>>,
    data: &mut [T],
) -> Result<ArrayViewMutD<T>, ActiveStorageError> {
    ArrayViewMut::<T, _>::from_shape(shape, data).map_err(ActiveStorageError::ShapeInvalid)
}

/// Returns an array index in numpy semantics to an index with ndarray semantics.
///
/// The resulting value will be clamped such that it is safe for indexing in ndarray.
/// This allows us to accept selections with NumPy's less restrictive semantics.
/// When the stride is negative (`reverse` is `true`), the result is offset by one to allow for
/// Numpy's non-inclusive start and inclusive end in this scenario.
///
/// # Arguments
///
/// * `index`: Selection index
/// * `length`: Length of corresponding axis
/// * `reverse`: Whether the stride is negative
fn to_ndarray_index(index: isize, length: usize, reverse: bool) -> isize {
    let length_isize = length.try_into().expect("Length too large!");
    let result = if reverse { index + 1 } else { index };
    if index < 0 {
        std::cmp::max(result + length_isize, 0)
    } else {
        std::cmp::min(result, length_isize)
    }
}

/// Convert a [crate::models::Slice] object with indices in numpy semantics to an
/// [ndarray::SliceInfoElem::Slice] with ndarray semantics.
///
/// See [ndarray docs](https://docs.rs/ndarray/0.15.6/ndarray/macro.s.html#negative-step) for
/// information about ndarray's handling of negative strides.
fn to_ndarray_slice(slice: &models::Slice, length: usize) -> ndarray::SliceInfoElem {
    let reverse = slice.stride < 0;
    let start = to_ndarray_index(slice.start, length, reverse);
    let end = to_ndarray_index(slice.end, length, reverse);
    let (start, end) = if reverse { (end, start) } else { (start, end) };
    ndarray::SliceInfoElem::Slice {
        start,
        end: Some(end),
        step: slice.stride,
    }
}

/// Returns an [ndarray] SliceInfo object corresponding to the selection.
pub fn build_slice_info<T>(
    selection: &Option<Vec<models::Slice>>,
    shape: &[usize],
) -> ndarray::SliceInfo<Vec<ndarray::SliceInfoElem>, ndarray::IxDyn, ndarray::IxDyn> {
    match selection {
        Some(selection) => {
            let si: Vec<ndarray::SliceInfoElem> = std::iter::zip(selection, shape)
                .map(|(slice, length)| to_ndarray_slice(slice, *length))
                .collect();
            ndarray::SliceInfo::try_from(si).expect("SliceInfo should not fail for IxDyn")
        }
        _ => {
            let si: Vec<ndarray::SliceInfoElem> = shape
                .iter()
                .map(|_| ndarray::SliceInfoElem::Slice {
                    start: 0,
                    end: None,
                    step: 1,
                })
                .collect();
            ndarray::SliceInfo::try_from(si).expect("SliceInfo should not fail for IxDyn")
        }
    }
}

/// Reverse the byte order of an array element.
fn reverse_byte_order<T>(element: &mut T)
where
    T: Copy
        + num_traits::FromBytes<Bytes = <T as num_traits::ToBytes>::Bytes>
        + num_traits::ToBytes,
{
    *element = T::from_be_bytes(&element.to_le_bytes());
}

/// Reverse the byte order of an array.
///
/// # Arguments
///
/// * `array`: An [ndarray::ArrayViewMutD] containing the data to be converted.
/// * `selection`: Optional selection. If provided only data in this selection will be converted.
pub fn reverse_array_byte_order<T>(
    array: &mut ArrayViewMutD<T>,
    selection: &Option<Vec<models::Slice>>,
) where
    T: Copy
        + num_traits::FromBytes<Bytes = <T as num_traits::ToBytes>::Bytes>
        + num_traits::ToBytes,
{
    if selection.is_some() {
        let slice_info = build_slice_info::<T>(selection, array.shape());
        let mut sliced = array.slice_mut(slice_info);
        sliced.map_inplace(reverse_byte_order);
    } else {
        array.map_inplace(reverse_byte_order);
    }
}

/// Build an [ndarray::ArrayView](ndarray::ArrayView) object corresponding to the request and data bytes.
///
/// The resulting array will contain a reference to `data`.
///
/// # Arguments
///
/// * `data`: Slice of bytes containing data for the array. Must be at least as aligned as an
///   instance of `T`.
/// * `request_data`: RequestData object for the request
pub fn build_array<'a, T>(
    request_data: &'a models::RequestData,
    data: &'a mut [u8],
) -> Result<ArrayViewD<'a, T>, ActiveStorageError>
where
    T: Copy
        + num_traits::FromBytes<Bytes = <T as num_traits::ToBytes>::Bytes>
        + num_traits::ToBytes
        + zerocopy::AsBytes
        + zerocopy::FromBytes,
{
    let data = from_bytes::<T>(data)?;
    if let Some(NON_NATIVE_BYTE_ORDER) = request_data.byte_order {
        // Create a mutable array to change the byte order.
        let shape = get_shape(data.len(), request_data);
        let mut array = build_array_mut_from_shape(shape, data)?;
        reverse_array_byte_order(&mut array, &request_data.selection);
    }
    let shape = get_shape(data.len(), request_data);
    build_array_from_shape(shape, data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;
    use num_traits::Float;

    #[test]
    fn from_bytes_u32() {
        let value: u32 = 42;
        let mut buf = maligned::align_first::<u8, maligned::A4>(4);
        buf.extend_from_slice(&value.to_ne_bytes());
        assert_eq!([value], from_bytes::<u32>(&mut buf).unwrap());
    }

    #[test]
    fn from_bytes_u64() {
        let value: u64 = u64::max_value();
        let mut buf = maligned::align_first::<u8, maligned::A8>(8);
        buf.extend_from_slice(&value.to_ne_bytes());
        assert_eq!([value], from_bytes::<u64>(&mut buf).unwrap());
    }

    #[test]
    fn from_bytes_i32() {
        let value: i32 = -42;
        let mut buf = maligned::align_first::<u8, maligned::A4>(4);
        buf.extend_from_slice(&value.to_ne_bytes());
        assert_eq!([value], from_bytes::<i32>(&mut buf).unwrap());
    }

    #[test]
    fn from_bytes_i64() {
        let value: i64 = i64::min_value();
        let mut buf = maligned::align_first::<u8, maligned::A8>(8);
        buf.extend_from_slice(&value.to_ne_bytes());
        assert_eq!([value], from_bytes::<i64>(&mut buf).unwrap());
    }

    #[test]
    fn from_bytes_f32() {
        let value: f32 = f32::min_value();
        let mut buf = maligned::align_first::<u8, maligned::A4>(4);
        buf.extend_from_slice(&value.to_ne_bytes());
        assert_eq!([value], from_bytes::<f32>(&mut buf).unwrap());
    }

    #[test]
    fn from_bytes_f64() {
        let value: f64 = f64::max_value();
        let mut buf = maligned::align_first::<u8, maligned::A8>(8);
        buf.extend_from_slice(&value.to_ne_bytes());
        assert_eq!([value], from_bytes::<f64>(&mut buf).unwrap());
    }

    fn assert_from_bytes_error<T: std::fmt::Debug>(result: Result<T, ActiveStorageError>) {
        match result.unwrap_err() {
            ActiveStorageError::FromBytes { type_name: _ } => (),
            _ => panic!("expected from_bytes to fail"),
        };
    }

    #[test]
    fn from_bytes_u32_too_small() {
        assert_from_bytes_error(from_bytes::<u32>(&mut [1, 2, 3]))
    }

    #[test]
    fn from_bytes_u32_too_big() {
        assert_from_bytes_error(from_bytes::<u32>(&mut [1, 2, 3, 4, 5]))
    }

    #[test]
    fn from_bytes_u32_unaligned() {
        static mut ARRAY: [u8; 5] = [1, 2, 3, 4, 5];
        unsafe { assert_from_bytes_error(from_bytes::<u32>(&mut ARRAY[1..])) }
    }

    #[test]
    fn get_shape_without_shape() {
        let shape = get_shape(42, &test_utils::get_test_request_data());
        assert_eq!([42], shape.raw_dim().as_array_view().as_slice().unwrap());
    }

    #[test]
    fn get_shape_with_shape() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.shape = Some(vec![1, 2, 3]);
        let shape = get_shape(42, &request_data);
        assert_eq!(
            [1, 2, 3],
            shape.raw_dim().as_array_view().as_slice().unwrap()
        );
    }

    #[test]
    fn build_array_from_shape_1d() {
        let data = [1, 2, 3];
        let shape = vec![3].into_shape();
        let array = build_array_from_shape(shape, &data).unwrap();
        assert_eq!(array![1, 2, 3].into_dyn(), array);
    }

    #[test]
    fn build_array_from_shape_1d_fortran() {
        let data = [1, 2, 3];
        let shape = vec![3].into_shape().f();
        let array = build_array_from_shape(shape, &data).unwrap();
        assert_eq!(array![1, 2, 3].into_dyn(), array);
    }

    #[test]
    fn build_array_from_shape_2d() {
        let data = [1.0, 2.1, 3.2, 4.3, 5.4, 6.5];
        let shape = vec![2, 3].into_shape();
        let array = build_array_from_shape(shape, &data).unwrap();
        assert_eq!(array![[1.0, 2.1, 3.2], [4.3, 5.4, 6.5]].into_dyn(), array);
    }

    #[test]
    fn build_array_from_shape_2d_fortran() {
        let data = [1.0, 2.1, 3.2, 4.3, 5.4, 6.5];
        let shape = vec![2, 3].into_shape().f();
        let array = build_array_from_shape(shape, &data).unwrap();
        assert_eq!(array![[1.0, 3.2, 5.4], [2.1, 4.3, 6.5]].into_dyn(), array);
    }

    #[test]
    fn build_array_from_shape_3d() {
        let data = [1, 2, 3, 4, 5, 6, 7, 8];
        let shape = vec![2, 2, 2].into_shape();
        let array = build_array_from_shape(shape, &data).unwrap();
        assert_eq!(array![[[1, 2], [3, 4]], [[5, 6], [7, 8]]].into_dyn(), array);
    }

    #[test]
    fn build_array_from_shape_3d_fortran() {
        let data = [1, 2, 3, 4, 5, 6, 7, 8];
        let shape = vec![2, 2, 2].into_shape().f();
        let array = build_array_from_shape(shape, &data).unwrap();
        assert_eq!(array![[[1, 5], [3, 7]], [[2, 6], [4, 8]]].into_dyn(), array);
    }

    #[test]
    fn build_array_from_shape_err() {
        let data = [1, 2, 3];
        let shape = vec![4].into_shape();
        match build_array_from_shape(shape, &data).unwrap_err() {
            ActiveStorageError::ShapeInvalid(err) => {
                assert_eq!(ndarray::ErrorKind::OutOfBounds, err.kind())
            }
            _ => panic!("Expected out of bounds error"),
        }
    }

    #[test]
    fn build_slice_info_1d_no_selection() {
        let selection = None;
        let shape = [1];
        let slice_info = build_slice_info::<u32>(&selection, &shape);
        assert_eq!(
            [ndarray::SliceInfoElem::Slice {
                start: 0,
                end: None,
                step: 1
            }],
            slice_info.as_ref()
        );
    }

    #[test]
    fn build_slice_info_1d_selection() {
        let selection = Some(vec![models::Slice::new(0, 1, 1)]);
        let shape = [1];
        let slice_info = build_slice_info::<u32>(&selection, &shape);
        assert_eq!(
            [ndarray::SliceInfoElem::Slice {
                start: 0,
                end: Some(1),
                step: 1
            }],
            slice_info.as_ref()
        );
    }

    #[test]
    fn build_slice_info_1d_selection_negative_stride() {
        let selection = Some(vec![models::Slice::new(1, 0, -1)]);
        let shape = [1];
        let slice_info = build_slice_info::<u32>(&selection, &shape);
        assert_eq!(
            [ndarray::SliceInfoElem::Slice {
                start: 1,
                end: Some(1),
                step: -1
            }],
            slice_info.as_ref()
        );
    }

    #[test]
    fn build_slice_info_1d_selection_negative_start() {
        let selection = Some(vec![models::Slice::new(-1, 1, 1)]);
        let shape = [1];
        let slice_info = build_slice_info::<u32>(&selection, &shape);
        assert_eq!(
            [ndarray::SliceInfoElem::Slice {
                start: 0,
                end: Some(1),
                step: 1
            }],
            slice_info.as_ref()
        );
    }

    #[test]
    fn build_slice_info_1d_selection_negative_end() {
        let selection = Some(vec![models::Slice::new(0, -1, 1)]);
        let shape = [1];
        let slice_info = build_slice_info::<u32>(&selection, &shape);
        assert_eq!(
            [ndarray::SliceInfoElem::Slice {
                start: 0,
                end: Some(0),
                step: 1
            }],
            slice_info.as_ref()
        );
    }

    #[test]
    fn build_slice_info_2d_no_selection() {
        let selection = None;
        let shape = [1, 2];
        let slice_info = build_slice_info::<u32>(&selection, &shape);
        assert_eq!(
            [
                ndarray::SliceInfoElem::Slice {
                    start: 0,
                    end: None,
                    step: 1
                },
                ndarray::SliceInfoElem::Slice {
                    start: 0,
                    end: None,
                    step: 1
                }
            ],
            slice_info.as_ref()
        );
    }

    #[test]
    fn build_slice_info_2d_selection() {
        let selection = Some(vec![
            models::Slice::new(0, 1, 1),
            models::Slice::new(0, 1, 1),
        ]);
        let shape = [1, 1];
        let slice_info = build_slice_info::<u32>(&selection, &shape);
        assert_eq!(
            [
                ndarray::SliceInfoElem::Slice {
                    start: 0,
                    end: Some(1),
                    step: 1
                },
                ndarray::SliceInfoElem::Slice {
                    start: 0,
                    end: Some(1),
                    step: 1
                }
            ],
            slice_info.as_ref()
        );
    }

    #[test]
    fn reverse_array_byte_order_u32() {
        let mut data = [0, 42, u32::max_value()];
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Uint32;
        let shape = get_shape(data.len(), &request_data);
        let mut array = build_array_mut_from_shape(shape, &mut data).unwrap();
        reverse_array_byte_order(&mut array, &request_data.selection);
        // For numbers < 256, LSB becomes MSB == multiply by 2^24
        assert_eq!([0, 42 << 24, u32::max_value()], data);
    }

    #[test]
    fn build_array_1d_u32() {
        let mut data = [1, 2, 3, 4, 5, 6, 7, 8];
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Uint32;
        let array = build_array::<u32>(&request_data, &mut data).unwrap();
        assert_eq!(array![0x04030201_u32, 0x08070605_u32].into_dyn(), array);
    }

    #[test]
    fn build_array_2d_i64() {
        let mut data = [1, 2, 3, 4, 0, 0, 0, 0, 5, 6, 7, 8, 0, 0, 0, 0];
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Int64;
        request_data.shape = Some(vec![2, 1]);
        let array = build_array::<i64>(&request_data, &mut data).unwrap();
        assert_eq!(array![[0x04030201_i64], [0x08070605_i64]].into_dyn(), array);
    }

    // Helper function for tests that slice an array using a selection.
    fn test_selection(slice: models::Slice, expected: Array1<u32>) {
        let mut data = [1, 2, 3, 4, 5, 6, 7, 8];
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Uint32;
        let array = build_array::<u32>(&request_data, &mut data).unwrap();
        let shape = vec![2];
        let slice_info = build_slice_info::<u32>(&Some(vec![slice]), &shape);
        let sliced = array.slice(slice_info);
        assert_eq!(sliced, expected.into_dyn().view());
    }

    #[test]
    fn build_array_with_selection_all() {
        test_selection(
            models::Slice::new(0, 2, 1),
            array![0x04030201_u32, 0x08070605_u32],
        )
    }

    #[test]
    fn build_array_with_selection_negative_start() {
        test_selection(
            models::Slice::new(-2, 2, 1),
            array![0x04030201_u32, 0x08070605_u32],
        )
    }

    #[test]
    fn build_array_with_selection_start_lt_negative_length() {
        test_selection(
            models::Slice::new(-3, 2, 1),
            array![0x04030201_u32, 0x08070605_u32],
        )
    }

    #[test]
    fn build_array_with_selection_start_eq_length() {
        test_selection(models::Slice::new(2, 2, 1), array![])
    }

    #[test]
    fn build_array_with_selection_start_gt_length() {
        test_selection(models::Slice::new(3, 2, 1), array![])
    }

    #[test]
    fn build_array_with_selection_negative_end() {
        test_selection(models::Slice::new(0, -1, 1), array![0x04030201_u32])
    }

    #[test]
    fn build_array_with_selection_end_lt_negative_length() {
        test_selection(models::Slice::new(0, -3, 1), array![])
    }

    #[test]
    fn build_array_with_selection_end_gt_length() {
        test_selection(
            models::Slice::new(0, 3, 1),
            array![0x04030201_u32, 0x08070605_u32],
        )
    }

    #[test]
    fn build_array_with_selection_all_negative_stride() {
        // Need to end at -3 to read first item.
        // translates to [0, 2]
        test_selection(
            models::Slice::new(1, -3, -1),
            array![0x08070605_u32, 0x04030201_u32],
        )
    }

    #[test]
    fn build_array_with_selection_negative_start_negative_stride() {
        // translates to [0, 2]
        test_selection(
            models::Slice::new(-1, -3, -1),
            array![0x08070605_u32, 0x04030201_u32],
        )
    }

    #[test]
    fn build_array_with_selection_start_lt_negative_length_negative_stride() {
        // translates to [1, 0]
        test_selection(models::Slice::new(-3, 0, -1), array![])
    }

    #[test]
    fn build_array_with_selection_start_eq_length_negative_stride() {
        // translates to [2, 2]
        test_selection(models::Slice::new(2, 1, -1), array![])
    }

    #[test]
    fn build_array_with_selection_start_gt_length_negative_stride() {
        // translates to [2, 2]
        test_selection(models::Slice::new(3, 1, -1), array![])
    }

    #[test]
    fn build_array_with_selection_negative_end_negative_stride() {
        // translates to [2, 2]
        test_selection(models::Slice::new(2, -1, -1), array![])
    }

    #[test]
    fn build_array_with_selection_end_lt_negative_length_negative_stride() {
        // translates to [0, 2]
        test_selection(
            models::Slice::new(1, -3, -1),
            array![0x08070605_u32, 0x04030201_u32],
        )
    }

    #[test]
    fn build_array_with_selection_end_gt_length_negative_stride() {
        // translates to [1, 2]
        test_selection(models::Slice::new(3, 0, -1), array![0x08070605_u32])
    }
}
