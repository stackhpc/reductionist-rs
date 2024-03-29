//! Numerical operations.
//!
//! Each operation is implemented as a struct that implements the
//! [Operation](crate::operation::Operation) trait.

use crate::array;
use crate::error::ActiveStorageError;
use crate::models;
use crate::operation::{Element, NumOperation};
use crate::types::Missing;

use axum::body::Bytes;
use ndarray::ArrayView;
use ndarray_stats::{errors::MinMaxError, QuantileExt};
// Bring trait into scope to use as_bytes method.
use zerocopy::AsBytes;

/// Returns a filter function that can be used with the Iterator trait's filter() method to filter
/// out missing data.
///
/// # Arguments
///
/// * `missing`: Missing data description.
fn missing_filter<'a, T: Element>(missing: &'a Missing<T>) -> Box<dyn Fn(&T) -> bool + 'a> {
    match missing {
        Missing::MissingValue(value) => Box::new(move |x: &T| *x != *value),
        Missing::MissingValues(values) => Box::new(move |x: &T| !values.contains(x)),
        Missing::ValidMin(min) => Box::new(move |x: &T| *x >= *min),
        Missing::ValidMax(max) => Box::new(move |x: &T| *x <= *max),
        Missing::ValidRange(min, max) => Box::new(move |x: &T| *x >= *min && *x <= *max),
    }
}

/// Count the non-missing elements in an array with missing data.
///
/// # Arguments
///
/// * `array`: The array to count
/// * `request_data`: RequestData object for the request
fn count_non_missing<T: Element>(
    array: &ArrayView<T, ndarray::Dim<ndarray::IxDynImpl>>,
    missing: &Missing<T>,
) -> Result<usize, ActiveStorageError> {
    let filter = missing_filter(missing);
    Ok(array.iter().copied().filter(filter).count())
}

/// Return the number of selected elements in the array.
pub struct Count {}

impl NumOperation for Count {
    fn execute_t<T: Element>(
        request_data: &models::RequestData,
        mut data: Vec<u8>,
    ) -> Result<models::Response, ActiveStorageError> {
        let array = array::build_array::<T>(request_data, &mut data)?;
        let slice_info = array::build_slice_info::<T>(&request_data.selection, array.shape());
        let sliced = array.slice(slice_info);
        let count = if let Some(missing) = &request_data.missing {
            let missing = Missing::<T>::try_from(missing)?;
            count_non_missing(&sliced, &missing)?
        } else {
            sliced.len()
        };
        let count = i64::try_from(count)?;
        let body = count.to_ne_bytes();
        // Need to copy to provide ownership to caller.
        let body = Bytes::copy_from_slice(&body);
        Ok(models::Response::new(
            body,
            models::DType::Int64,
            vec![],
            count,
        ))
    }
}

/// Return the maximum of selected elements in the array.
pub struct Max {}

impl NumOperation for Max {
    fn execute_t<T: Element>(
        request_data: &models::RequestData,
        mut data: Vec<u8>,
    ) -> Result<models::Response, ActiveStorageError> {
        let array = array::build_array::<T>(request_data, &mut data)?;
        let slice_info = array::build_slice_info::<T>(&request_data.selection, array.shape());
        let sliced = array.slice(slice_info);
        let (max, count) = if let Some(missing) = &request_data.missing {
            let missing = Missing::<T>::try_from(missing)?;
            // Use a fold to simultaneously max and count the non-missing data.
            // TODO: separate float impl?
            // TODO: inifinite/NaN
            let (max, count) = sliced
                .iter()
                .copied()
                .filter(missing_filter(&missing))
                .fold((None, 0), |(a, count), b| {
                    let max = match (a, b) {
                        (None, b) => Some(b), //FIXME: if b.is_finite() { Some(b) } else { None },
                        (Some(a), b) => Some(std::cmp::max_by(a, b, |x, y| {
                            x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Greater)
                        })),
                    };
                    (max, count + 1)
                });
            let max = max.ok_or(ActiveStorageError::EmptyArray { operation: "max" })?;
            (max, count)
        } else {
            let max = *sliced.max().map_err(|err| match err {
                MinMaxError::EmptyInput => ActiveStorageError::EmptyArray { operation: "max" },
                MinMaxError::UndefinedOrder => panic!("unexpected undefined order error for max"),
            })?;
            let count = sliced.len();
            (max, count)
        };
        let count = i64::try_from(count)?;
        let body = max.as_bytes();
        // Need to copy to provide ownership to caller.
        let body = Bytes::copy_from_slice(body);
        Ok(models::Response::new(
            body,
            request_data.dtype,
            vec![],
            count,
        ))
    }
}

/// Return the minimum of selected elements in the array.
pub struct Min {}

impl NumOperation for Min {
    fn execute_t<T: Element>(
        request_data: &models::RequestData,
        mut data: Vec<u8>,
    ) -> Result<models::Response, ActiveStorageError> {
        let array = array::build_array::<T>(request_data, &mut data)?;
        let slice_info = array::build_slice_info::<T>(&request_data.selection, array.shape());
        let sliced = array.slice(slice_info);
        let (min, count) = if let Some(missing) = &request_data.missing {
            let missing = Missing::<T>::try_from(missing)?;
            // Use a fold to simultaneously min and count the non-missing data.
            // TODO: separate float impl?
            // TODO: inifinite/NaN
            let (min, count) = sliced
                .iter()
                .copied()
                .filter(missing_filter(&missing))
                .fold((None, 0), |(a, count), b| {
                    let min = match (a, b) {
                        (None, b) => Some(b), //FIXME: if b.is_finite() { Some(b) } else { None },
                        (Some(a), b) => Some(std::cmp::min_by(a, b, |x, y| {
                            x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Less)
                        })),
                    };
                    (min, count + 1)
                });
            let min = min.ok_or(ActiveStorageError::EmptyArray { operation: "min" })?;
            (min, count)
        } else {
            let min = *sliced.min().map_err(|err| match err {
                MinMaxError::EmptyInput => ActiveStorageError::EmptyArray { operation: "min" },
                MinMaxError::UndefinedOrder => panic!("unexpected undefined order error for min"),
            })?;
            let count = sliced.len();
            (min, count)
        };
        let count = i64::try_from(count)?;
        let body = min.as_bytes();
        // Need to copy to provide ownership to caller.
        let body = Bytes::copy_from_slice(body);
        Ok(models::Response::new(
            body,
            request_data.dtype,
            vec![],
            count,
        ))
    }
}

/// Return all selected elements in the array.
pub struct Select {}

impl NumOperation for Select {
    fn execute_t<T: Element>(
        request_data: &models::RequestData,
        mut data: Vec<u8>,
    ) -> Result<models::Response, ActiveStorageError> {
        let array = array::build_array::<T>(request_data, &mut data)?;
        let slice_info = array::build_slice_info::<T>(&request_data.selection, array.shape());
        let sliced = array.slice(slice_info);
        let count = if let Some(missing) = &request_data.missing {
            let missing = Missing::<T>::try_from(missing)?;
            count_non_missing(&sliced, &missing)?
        } else {
            sliced.len()
        };
        let count = i64::try_from(count)?;
        let shape = sliced.shape().to_vec();
        // Transpose Fortran ordered arrays before iterating.
        let body = if !array.is_standard_layout() {
            let sliced_ordered = sliced.t();
            sliced_ordered.iter().copied().collect::<Vec<T>>()
        } else {
            sliced.iter().copied().collect::<Vec<T>>()
        };
        let body = body.as_bytes();
        // Need to copy to provide ownership to caller.
        let body = Bytes::copy_from_slice(body);
        Ok(models::Response::new(
            body,
            request_data.dtype,
            shape,
            count,
        ))
    }
}

/// Return the sum of selected elements in the array.
pub struct Sum {}

impl NumOperation for Sum {
    fn execute_t<T: Element>(
        request_data: &models::RequestData,
        mut data: Vec<u8>,
    ) -> Result<models::Response, ActiveStorageError> {
        let array = array::build_array::<T>(request_data, &mut data)?;
        let slice_info = array::build_slice_info::<T>(&request_data.selection, array.shape());
        let sliced = array.slice(slice_info);
        let (sum, count) = if let Some(missing) = &request_data.missing {
            let missing = Missing::<T>::try_from(missing)?;
            // Use a fold to simultaneously sum and count the non-missing data.
            sliced
                .iter()
                .copied()
                .filter(missing_filter(&missing))
                .fold((T::zero(), 0), |(a, count), b| (a + b, count + 1))
        } else {
            (sliced.sum(), sliced.len())
        };
        let count = i64::try_from(count)?;
        let body = sum.as_bytes();
        // Need to copy to provide ownership to caller.
        let body = Bytes::copy_from_slice(body);
        Ok(models::Response::new(
            body,
            request_data.dtype,
            vec![],
            count,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::operation::Operation;
    use crate::test_utils;
    use crate::types::DValue;

    #[test]
    fn count_i32_1d() {
        let request_data = test_utils::get_test_request_data();
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let response = Count::execute(&request_data, data).unwrap();
        // A Vec<u8> of 8 elements == a u32 slice with 2 elements
        // Count is always i64.
        let expected: i64 = 2;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(8, response.body.len()); // Assert that count value is 8 bytes (i.e. i64)
        assert_eq!(models::DType::Int64, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
        assert_eq!(expected, response.count);
    }

    #[test]
    fn count_u32_1d_missing_value() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Uint32;
        request_data.missing = Some(Missing::MissingValue(0x04030201.into()));
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let response = Count::execute(&request_data, data).unwrap();
        // A Vec<u8> of 8 elements == a u32 slice with 2 elements
        // Count is always i64.
        let expected: i64 = 1;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(8, response.body.len()); // Assert that count value is 8 bytes (i.e. i64)
        assert_eq!(models::DType::Int64, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
        assert_eq!(expected, response.count);
    }

    #[test]
    fn max_i64_1d() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Int64;
        // data:
        // A Vec<u8> of 8 elements == a single i64 value
        // where each element is 2 hexadecimal digits
        // and the order is reversed on little-endian systems
        // so [1, 2, 3] is 0x030201 as an i64 in hexadecimal
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let response = Max::execute(&request_data, data).unwrap();
        let expected: i64 = 0x0807060504030201;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(8, response.body.len());
        assert_eq!(models::DType::Int64, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
        assert_eq!(1, response.count);
    }

    #[test]
    fn max_i64_1d_missing_values() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Int64;
        request_data.missing = Some(Missing::MissingValues(vec![0x0807060504030201_i64.into()]));
        // data:
        // A Vec<u8> of 16 elements == two i64 values
        // where each element is 2 hexadecimal digits
        // and the order is reversed on little-endian systems
        // so [1, 2, 3] is 0x030201 as an i64 in hexadecimal
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let response = Max::execute(&request_data, data).unwrap();
        let expected: i64 = 0x100f0e0d0c0b0a09;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(8, response.body.len());
        assert_eq!(models::DType::Int64, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
        assert_eq!(1, response.count);
    }

    #[test]
    fn max_f32_1d_infinity() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Float32;
        let floats = [1.0, f32::INFINITY];
        let data = floats.as_bytes();
        let response = Max::execute(&request_data, data.into()).unwrap();
        let expected = f32::INFINITY;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(4, response.body.len());
        assert_eq!(models::DType::Float32, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
        assert_eq!(2, response.count);
    }

    #[test]
    fn max_f32_1d_infinity_first() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Float32;
        let floats = [f32::INFINITY, 1.0];
        let data = floats.as_bytes();
        let response = Max::execute(&request_data, data.into()).unwrap();
        let expected = f32::INFINITY;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(4, response.body.len());
        assert_eq!(models::DType::Float32, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
        assert_eq!(2, response.count);
    }

    #[test]
    fn min_u64_1d() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Uint64;
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let response = Min::execute(&request_data, data).unwrap();
        let expected: u64 = 0x0807060504030201;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(8, response.body.len());
        assert_eq!(models::DType::Uint64, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
        assert_eq!(1, response.count);
    }

    #[test]
    fn min_i64_1d_valid_min() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Int64;
        // Minimum is one greater than smallest element.
        request_data.missing = Some(Missing::ValidMin(0x0807060504030202_i64.into()));
        // data:
        // A Vec<u8> of 16 elements == two i64 values
        // where each element is 2 hexadecimal digits
        // and the order is reversed on little-endian systems
        // so [1, 2, 3] is 0x030201 as an i64 in hexadecimal
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let response = Min::execute(&request_data, data).unwrap();
        let expected: i64 = 0x100f0e0d0c0b0a09;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(8, response.body.len());
        assert_eq!(models::DType::Int64, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
        assert_eq!(1, response.count);
    }

    #[test]
    fn min_f32_1d_infinity() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Float32;
        let floats = [1.0, f32::INFINITY];
        let data = floats.as_bytes();
        let response = Min::execute(&request_data, data.into()).unwrap();
        let expected = 1.0_f32;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(4, response.body.len());
        assert_eq!(models::DType::Float32, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
        assert_eq!(2, response.count);
    }

    #[test]
    fn min_f32_1d_infinity_first() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Float32;
        let floats = [f32::INFINITY, 1.0];
        let data = floats.as_bytes();
        let response = Min::execute(&request_data, data.into()).unwrap();
        let expected = 1.0_f32;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(4, response.body.len());
        assert_eq!(models::DType::Float32, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
        assert_eq!(2, response.count);
    }

    #[test]
    #[should_panic(expected = "unexpected undefined order error for min")]
    fn min_f32_1d_nan() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Float32;
        let floats = [1.0, f32::NAN];
        let data = floats.as_bytes();
        let response = Min::execute(&request_data, data.into()).unwrap();
        let expected = 1.0_f32;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(4, response.body.len());
        assert_eq!(models::DType::Float32, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
        assert_eq!(2, response.count);
    }

    #[test]
    #[should_panic(expected = "unexpected undefined order error for min")]
    fn min_f32_1d_nan_first() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Float32;
        let floats = [f32::NAN, 1.0];
        let data = floats.as_bytes();
        let response = Min::execute(&request_data, data.into()).unwrap();
        let expected = 1.0_f32;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(4, response.body.len());
        assert_eq!(models::DType::Float32, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
        assert_eq!(2, response.count);
    }

    #[test]
    fn min_f32_1d_nan_missing_value() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Float32;
        request_data.missing = Some(Missing::MissingValue(DValue::from_f64(42.0).unwrap()));
        let floats = [1.0, f32::NAN];
        let data = floats.as_bytes();
        let response = Min::execute(&request_data, data.into()).unwrap();
        let expected = 1.0_f32;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(4, response.body.len());
        assert_eq!(models::DType::Float32, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
        assert_eq!(2, response.count);
    }

    #[test]
    fn min_f32_1d_nan_first_missing_value() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Float32;
        request_data.missing = Some(Missing::MissingValue(DValue::from_f64(42.0).unwrap()));
        let floats = [f32::NAN, 1.0];
        let data = floats.as_bytes();
        let response = Min::execute(&request_data, data.into()).unwrap();
        // FIXME: Ignore NANs?
        let expected = f32::NAN; //1.0_f32;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(4, response.body.len());
        assert_eq!(models::DType::Float32, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
        assert_eq!(2, response.count);
    }

    #[test]
    fn select_f32_1d() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Float32;
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let response = Select::execute(&request_data, data).unwrap();
        let expected: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(8, response.body.len());
        assert_eq!(models::DType::Float32, response.dtype);
        assert_eq!(vec![2], response.shape);
        assert_eq!(2, response.count);
    }

    #[test]
    fn select_f64_2d() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Float64;
        request_data.shape = Some(vec![2, 1]);
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let response = Select::execute(&request_data, data).unwrap();
        let expected: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(16, response.body.len());
        assert_eq!(models::DType::Float64, response.dtype);
        assert_eq!(vec![2, 1], response.shape);
        assert_eq!(2, response.count);
    }

    #[test]
    fn select_f32_2d_with_selection() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Float32;
        request_data.shape = Some(vec![2, 2]);
        request_data.selection = Some(vec![
            models::Slice::new(0, 2, 1),
            models::Slice::new(1, 2, 1),
        ]);
        // 2x2 array, select second row of each column.
        // [[0x04030201, 0x08070605], [0x12111009, 0x16151413]]
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let response = Select::execute(&request_data, data).unwrap();
        // [[0x08070605], [0x16151413]]
        let expected: [u8; 8] = [5, 6, 7, 8, 13, 14, 15, 16];
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(8, response.body.len());
        assert_eq!(models::DType::Float32, response.dtype);
        assert_eq!(vec![2, 1], response.shape);
        assert_eq!(2, response.count);
    }

    #[test]
    fn sum_u32_1d() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Uint32;
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let response = Sum::execute(&request_data, data).unwrap();
        let expected: u32 = 0x04030201 + 0x08070605;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(4, response.body.len());
        assert_eq!(models::DType::Uint32, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
        assert_eq!(2, response.count);
    }

    #[test]
    fn sum_u32_1d_valid_max() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Uint32;
        request_data.missing = Some(Missing::ValidMax((0x08070605 - 1).into()));
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let response = Sum::execute(&request_data, data).unwrap();
        let expected: u32 = 0x04030201;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(4, response.body.len());
        assert_eq!(models::DType::Uint32, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
        assert_eq!(1, response.count);
    }

    #[test]
    fn sum_f32_1d_infinity() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Float32;
        let floats = [1.0, f32::INFINITY];
        let data = floats.as_bytes();
        let response = Sum::execute(&request_data, data.into()).unwrap();
        let expected = f32::INFINITY;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(4, response.body.len());
        assert_eq!(models::DType::Float32, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
        assert_eq!(2, response.count);
    }

    #[test]
    fn sum_f64_1d_nan() {
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Float64;
        let floats = [f64::NAN, 1.0];
        let data = floats.as_bytes();
        let response = Sum::execute(&request_data, data.into()).unwrap();
        let expected = f64::NAN;
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(8, response.body.len());
        assert_eq!(models::DType::Float64, response.dtype);
        assert_eq!(vec![0; 0], response.shape);
        assert_eq!(2, response.count);
    }

    #[test]
    fn partial_cmp_behaviour() {
        assert_eq!(
            f64::INFINITY.partial_cmp(&1.0),
            Some(std::cmp::Ordering::Greater)
        );
        assert_eq!(f64::NAN.partial_cmp(&1.0), None);
        assert_eq!(
            f64::INFINITY.partial_cmp(&f64::NEG_INFINITY),
            Some(std::cmp::Ordering::Greater)
        );
    }
}
