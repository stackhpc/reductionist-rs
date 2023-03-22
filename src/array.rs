//! This module provides functions and utilities for working with ndarray objects.

use crate::models;

use anyhow::anyhow;
use axum::body::Bytes;
use ndarray::prelude::*;

/// Convert from Bytes to `&[T]`.
///
/// Zerocopy provides a mechanism for converting between types.
/// Correct alignment of the data is necessary.
///
/// # Arguments
///
/// * `data`: Bytes containing data to convert.
#[allow(dead_code)]
fn from_bytes<T: zerocopy::FromBytes>(data: &Bytes) -> anyhow::Result<&[T]> {
    let layout = zerocopy::LayoutVerified::<_, [T]>::new_slice(&data[..]).ok_or(anyhow!(
        "Failed to convert from bytes to {}",
        std::any::type_name::<T>()
    ))?;
    Ok(layout.into_slice())
}

/// Returns an [ndarray] Shape corresponding to the data in the request.
///
/// # Arguments
///
/// * `size`: Number of elements in the array
/// * `request_data`: RequestData object for the request
#[allow(dead_code)]
fn get_shape(
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

/// Returns an [ndarray::ArrayView](ndarray::ArrayView) corresponding to the data in the request.
///
/// The array view borrows the data, so no copying takes place.
///
/// # Arguments
///
/// * `shape`: The shape of the array
/// * `data`: A slice of type `&[T]` containing the data to be consumed by the array view.
#[allow(dead_code)]
fn build_array_from_shape<T>(
    shape: ndarray::Shape<Dim<ndarray::IxDynImpl>>,
    data: &[T],
) -> Result<ArrayView<T, ndarray::Dim<ndarray::IxDynImpl>>, ndarray::ShapeError> {
    ArrayView::<T, _>::from_shape(shape, data)
}

/// Returns an optional [ndarray] SliceInfo object corresponding to the selection.
#[allow(dead_code)]
pub fn build_slice_info<T>(
    selection: &Option<Vec<models::Slice>>,
    shape: &[usize],
) -> Option<ndarray::SliceInfo<Vec<ndarray::SliceInfoElem>, ndarray::IxDyn, ndarray::IxDyn>> {
    match selection {
        Some(selection) => {
            let si = selection
                .iter()
                .map(|slice| ndarray::SliceInfoElem::Slice {
                    // FIXME: usize should be isize?
                    start: slice.start as isize,
                    end: Some(slice.end as isize),
                    step: slice.stride as isize,
                })
                .collect();
            unsafe { Some(ndarray::SliceInfo::new(si).unwrap()) }
        }
        _ => {
            //let si = (1..shape.len()).map(|index| ndarray::SliceInfoElem::Index(index as isize)).collect();
            let si = shape
                .iter()
                .map(|_| ndarray::SliceInfoElem::Slice {
                    // FIXME: usize should be isize?
                    start: 0,
                    end: None,
                    step: 1,
                })
                .collect();
            unsafe { Some(ndarray::SliceInfo::new(si).unwrap()) }
        }
    }
}

/// Build an [ndarray::ArrayView](ndarray::ArrayView) object corresponding to the request and data Bytes.
///
/// The resulting array will contain a reference to `data`.
///
/// # Arguments
///
/// * `data`: Bytes containing data for the array. Must be at least as aligned as an instance of
///   `T`.
/// * `request_data`: RequestData object for the request
#[allow(dead_code)]
pub fn build_array<'a, T>(
    request_data: &'a models::RequestData,
    data: &'a Bytes,
) -> ArrayView<'a, T, ndarray::Dim<ndarray::IxDynImpl>>
where
    T: zerocopy::FromBytes,
{
    let data = from_bytes::<T>(data).unwrap();
    let shape = get_shape(data.len(), request_data);
    build_array_from_shape(shape, data).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    use url::Url;

    #[test]
    fn from_bytes_u32() {
        assert_eq!(
            [0x04030201_u32],
            from_bytes::<u32>(&Bytes::from_static(&[1, 2, 3, 4])).unwrap()
        );
    }

    #[test]
    fn from_bytes_u64() {
        assert_eq!(
            [0x0807060504030201_u64],
            from_bytes::<u64>(&Bytes::from_static(&[1, 2, 3, 4, 5, 6, 7, 8])).unwrap()
        );
    }

    #[test]
    fn from_bytes_i32() {
        assert_eq!(
            [0x04030201_i32],
            from_bytes::<i32>(&Bytes::from_static(&[1, 2, 3, 4])).unwrap()
        );
    }

    #[test]
    fn from_bytes_i64() {
        assert_eq!(
            [0x0807060504030201_i64],
            from_bytes::<i64>(&Bytes::from_static(&[1, 2, 3, 4, 5, 6, 7, 8])).unwrap()
        );
    }

    #[test]
    fn from_bytes_f32() {
        assert_eq!(
            [1.5399896e-36_f32],
            from_bytes::<f32>(&Bytes::from_static(&[1, 2, 3, 4])).unwrap()
        );
    }

    #[test]
    fn from_bytes_f64() {
        assert_eq!(
            [5.447603722011605e-270_f64],
            from_bytes::<f64>(&Bytes::from_static(&[1, 2, 3, 4, 5, 6, 7, 8])).unwrap()
        );
    }

    #[test]
    #[should_panic(expected = "Failed to convert from bytes to u32")]
    fn from_bytes_u32_too_small() {
        from_bytes::<u32>(&Bytes::from_static(&[1, 2, 3])).unwrap();
    }

    #[test]
    #[should_panic(expected = "Failed to convert from bytes to u32")]
    fn from_bytes_u32_too_big() {
        from_bytes::<u32>(&Bytes::from_static(&[1, 2, 3, 4, 5])).unwrap();
    }

    #[test]
    #[should_panic(expected = "Failed to convert from bytes to u32")]
    fn from_bytes_u32_unaligned() {
        static ARRAY: [u8; 5] = [1, 2, 3, 4, 5];
        from_bytes::<u32>(&Bytes::from_static(&ARRAY[1..])).unwrap();
    }

    #[test]
    fn get_shape_without_shape() {
        let shape = get_shape(
            42,
            &models::RequestData {
                source: Url::parse("http://example.com").unwrap(),
                bucket: "bar".to_string(),
                object: "baz".to_string(),
                dtype: models::DType::Int32,
                offset: None,
                size: None,
                shape: None,
                order: None,
                selection: None,
            },
        );
        assert_eq!([42], shape.raw_dim().as_array_view().as_slice().unwrap());
    }

    #[test]
    fn get_shape_with_shape() {
        let shape = get_shape(
            42,
            &models::RequestData {
                source: Url::parse("http://example.com").unwrap(),
                bucket: "bar".to_string(),
                object: "baz".to_string(),
                dtype: models::DType::Int32,
                offset: None,
                size: None,
                shape: Some(vec![1, 2, 3]),
                order: None,
                selection: None,
            },
        );
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
        match build_array_from_shape(shape, &data) {
            Err(err) => {
                assert_eq!(ndarray::ErrorKind::OutOfBounds, err.kind())
            }
            _ => panic!("Expected out of bounds error"),
        }
    }

    #[test]
    fn build_slice_info_1d_no_selection() {
        let selection = None;
        let shape = [1];
        let slice_info = build_slice_info::<u32>(&selection, &shape).unwrap();
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
        let shape = [];
        let slice_info = build_slice_info::<u32>(&selection, &shape).unwrap();
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
    fn build_slice_info_2d_no_selection() {
        let selection = None;
        let shape = [1, 2];
        let slice_info = build_slice_info::<u32>(&selection, &shape).unwrap();
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
        let shape = [];
        let slice_info = build_slice_info::<u32>(&selection, &shape).unwrap();
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
    fn build_array_1d_u32() {
        let data = [1, 2, 3, 4, 5, 6, 7, 8];
        let request_data = models::RequestData {
            source: Url::parse("http://example.com").unwrap(),
            bucket: "bar".to_string(),
            object: "baz".to_string(),
            dtype: models::DType::Uint32,
            offset: None,
            size: None,
            shape: None,
            order: None,
            selection: None,
        };
        let bytes = Bytes::copy_from_slice(&data);
        let array = build_array::<u32>(&request_data, &bytes);
        assert_eq!(array![0x04030201_u32, 0x08070605_u32].into_dyn(), array);
    }

    #[test]
    fn build_array_2d_i64() {
        let data = [1, 2, 3, 4, 0, 0, 0, 0, 5, 6, 7, 8, 0, 0, 0, 0];
        let request_data = models::RequestData {
            source: Url::parse("http://example.com").unwrap(),
            bucket: "bar".to_string(),
            object: "baz".to_string(),
            dtype: models::DType::Int64,
            offset: None,
            size: None,
            shape: Some(vec![2, 1]),
            order: None,
            selection: None,
        };
        let bytes = Bytes::copy_from_slice(&data);
        let array = build_array::<i64>(&request_data, &bytes);
        assert_eq!(array![[0x04030201_i64], [0x08070605_i64]].into_dyn(), array);
    }
}
