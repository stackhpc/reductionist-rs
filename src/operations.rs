//! Numerical operations.
//!
//! Each operation is implemented as a struct that implements the
//! [Operation](crate::operation::Operation) trait.

use std::cmp::{max_by, min_by};

use crate::array;
use crate::error::ActiveStorageError;
use crate::models::{self, Order, ReductionAxes};
use crate::operation::{Element, NumOperation};
use crate::types::Missing;

use axum::body::Bytes;
use ndarray::{ArrayView, Axis};
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
) -> usize {
    let filter = missing_filter(missing);
    array.iter().copied().filter(filter).count()
}

/// Counts the number of non-missing elements along
/// one or more axes of the provided array
fn count_array_multi_axis<T: Element>(
    array: ndarray::ArrayView<T, ndarray::IxDyn>,
    axes: &[usize],
    missing: Option<Missing<T>>,
) -> (Vec<i64>, Vec<usize>) {
    let result = if axes.is_empty() {
        // Emulate numpy semantics of axis = () being
        // equivalent to a 'reduction over no axes'
        array.map(|val| {
            if let Some(missing) = &missing {
                if !missing.is_missing(val) {
                    1
                } else {
                    0
                }
            } else {
                1
            }
        })
    } else {
        // Should never panic here due to axis.is_empty() branch above
        let first_axis = axes.first().expect("axes list to be non-empty");
        // Count non-missing over first axis
        let mut result = array
            .fold_axis(Axis(*first_axis), 0, |running_count, val| {
                if let Some(missing) = &missing {
                    if !missing.is_missing(val) {
                        running_count + 1
                    } else {
                        *running_count
                    }
                } else {
                    running_count + 1
                }
            })
            .into_dyn();
        // Sum counts over remaining axes
        if let Some(remaining_axes) = axes.get(1..) {
            for (n, axis) in remaining_axes.iter().enumerate() {
                result = result
                    .fold_axis(Axis(axis - n - 1), 0, |total_count, count| {
                        total_count + count
                    })
                    .into_dyn();
            }
        }
        result
    };

    // Convert result to owned vec
    let counts = result.iter().copied().collect();
    (counts, result.shape().into())
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

        let typed_missing: Option<Missing<T>> = if let Some(missing) = &request_data.missing {
            let m = Missing::try_from(missing)?;
            Some(m)
        } else {
            None
        };

        match &request_data.axis {
            ReductionAxes::All => {
                let count = if let Some(missing) = typed_missing {
                    count_non_missing(&sliced, &missing)
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
                    vec![count],
                ))
            }
            ReductionAxes::One(axis) => {
                let result = sliced.fold_axis(Axis(*axis), 0, |count, val| {
                    if let Some(missing) = &typed_missing {
                        if !missing.is_missing(val) {
                            count + 1
                        } else {
                            *count
                        }
                    } else {
                        count + 1
                    }
                });
                let counts = result.iter().copied().collect::<Vec<i64>>();
                let body = counts.as_bytes();
                // Need to copy to provide ownership to caller.
                let body = Bytes::copy_from_slice(body);
                Ok(models::Response::new(
                    body,
                    models::DType::Int64,
                    result.shape().into(),
                    counts,
                ))
            }
            ReductionAxes::Multi(axes) => {
                let (counts, shape) = count_array_multi_axis(sliced.view(), axes, typed_missing);
                let body = counts.as_bytes();
                // Need to copy to provide ownership to caller.
                let body = Bytes::copy_from_slice(body);
                Ok(models::Response::new(
                    body,
                    models::DType::Int64,
                    shape,
                    counts,
                ))
            }
        }
    }
}

/// Return the maximum of selected elements in the array.
pub struct Max {}

fn max_element_pairwise<T: Element>(x: &&T, y: &&T) -> std::cmp::Ordering {
    // TODO: How to handle NaN correctly?
    // Numpy seems to behave as follows:
    //
    // np.min([np.nan, 1]) == np.nan
    // np.max([np.nan, 1]) == np.nan
    // np.nan != np.nan
    // np.min([np.nan, 1]) != np.max([np.nan, 1])
    //
    // There are also separate np.nan{min,max} functions
    // which ignore nans instead.
    //
    // Which behaviour do we want to follow?
    //
    // Panic for now (TODO: Make this a user-facing error response instead)
    x.partial_cmp(y)
        // .unwrap_or(std::cmp::Ordering::Less)
        .unwrap_or_else(|| panic!("unexpected undefined order error for min"))
}

/// Emulates numpy behaviour of 'reduction over no axes'
/// when `axis=()` is passed to a numpy.ma function
fn reduction_over_zero_axes<T: Element>(
    array: &ndarray::ArrayView<T, ndarray::IxDyn>,
    missing: Option<Missing<T>>,
    order: &Option<Order>,
) -> ndarray::ArrayBase<ndarray::OwnedRepr<(T, i64)>, ndarray::IxDyn> {
    let func = |val| {
        if let Some(missing) = &missing {
            if !missing.is_missing(val) {
                (*val, 1)
            } else {
                (*val, 0)
            }
        } else {
            (*val, 1)
        }
    };
    let result = match order {
        Some(Order::F) => array.t().map(func),
        _ => array.map(func),
    };
    result
}

/// Performs a max over one or more axes of the provided array
fn max_array_multi_axis<T: Element>(
    array: ndarray::ArrayView<T, ndarray::IxDyn>,
    axes: &[usize],
    missing: Option<Missing<T>>,
    order: &Option<Order>,
) -> (Vec<T>, Vec<i64>, Vec<usize>) {
    let (result, shape) = if axes.is_empty() {
        // Emulate numpy behaviour of 'reduction over no axes'
        let result = reduction_over_zero_axes(&array, missing, order);
        (result, array.shape().to_owned())
    } else {
        // Find maximum over first axis and count elements operated on
        let init = T::min_value();
        let mut result = array
            .fold_axis(Axis(axes[0]), (init, 0), |(running_max, count), val| {
                if let Some(missing) = &missing {
                    if !missing.is_missing(val) {
                        let new_max = max_by(running_max, val, max_element_pairwise);
                        (*new_max, count + 1)
                    } else {
                        (*running_max, *count)
                    }
                } else {
                    let new_max = max_by(running_max, val, max_element_pairwise);
                    (*new_max, count + 1)
                }
            })
            .into_dyn();
        // Find max over remaining axes (where total count is now sum of counts)
        if let Some(remaining_axes) = axes.get(1..) {
            for (n, axis) in remaining_axes.iter().enumerate() {
                result = result
                    .fold_axis(
                        Axis(axis - n - 1),
                        (init, 0),
                        |(global_max, total_count), (running_max, count)| {
                            let new_max = max_by(global_max, running_max, max_element_pairwise);
                            (*new_max, total_count + count)
                        },
                    )
                    .into_dyn();
            }
        }
        let shape = result.shape().to_owned();
        (result, shape)
    };

    // Result is array of (max, count) tuples so separate them here
    let maxes = result.iter().map(|(max, _)| *max).collect::<Vec<T>>();
    let counts = result.iter().map(|(_, count)| *count).collect::<Vec<i64>>();

    (maxes, counts, shape)
}

impl NumOperation for Max {
    fn execute_t<T: Element>(
        request_data: &models::RequestData,
        mut data: Vec<u8>,
    ) -> Result<models::Response, ActiveStorageError> {
        let array = array::build_array::<T>(request_data, &mut data)?;
        let slice_info = array::build_slice_info::<T>(&request_data.selection, array.shape());
        let sliced = array.slice(slice_info);

        let typed_missing: Option<Missing<T>> = if let Some(missing) = &request_data.missing {
            let m = Missing::try_from(missing)?;
            Some(m)
        } else {
            None
        };

        match &request_data.axis {
            ReductionAxes::One(axis) => {
                let init = T::min_value();
                let result =
                    sliced.fold_axis(Axis(*axis), (init, 0), |(running_max, count), val| {
                        if let Some(missing) = &typed_missing {
                            if !missing.is_missing(val) {
                                (*max_by(running_max, val, max_element_pairwise), count + 1)
                            } else {
                                (*running_max, *count)
                            }
                        } else {
                            (*max_by(running_max, val, max_element_pairwise), count + 1)
                        }
                    });
                let maxes = result.iter().map(|(max, _)| *max).collect::<Vec<T>>();
                let counts = result.iter().map(|(_, count)| *count).collect::<Vec<i64>>();
                let body = maxes.as_bytes();
                let body = Bytes::copy_from_slice(body);
                Ok(models::Response::new(
                    body,
                    request_data.dtype,
                    result.shape().into(),
                    counts,
                ))
            }
            ReductionAxes::Multi(axes) => {
                let (maxes, counts, shape) =
                    max_array_multi_axis(sliced, axes, typed_missing, &request_data.order);
                let body = Bytes::copy_from_slice(maxes.as_bytes());
                Ok(models::Response::new(
                    body,
                    request_data.dtype,
                    shape,
                    counts,
                ))
            }
            ReductionAxes::All => {
                let init = T::min_value();
                let (max, count) = sliced.fold((init, 0_i64), |(running_max, count), val| {
                    if let Some(missing) = &typed_missing {
                        if !missing.is_missing(val) {
                            (*max_by(&running_max, val, max_element_pairwise), count + 1)
                        } else {
                            (running_max, count)
                        }
                    } else {
                        (*max_by(&running_max, val, max_element_pairwise), count + 1)
                    }
                });

                let body = max.as_bytes();
                let body = Bytes::copy_from_slice(body);
                Ok(models::Response::new(
                    body,
                    request_data.dtype,
                    vec![],
                    vec![count],
                ))
            }
        }
    }
}

/// Return the minimum of selected elements in the array.
pub struct Min {}

fn min_element_pairwise<T: Element>(x: &&T, y: &&T) -> std::cmp::Ordering {
    // TODO: How to handle NaN correctly?
    // Numpy seems to behave as follows:
    //
    // np.min([np.nan, 1]) == np.nan
    // np.max([np.nan, 1]) == np.nan
    // np.nan != np.nan
    // np.min([np.nan, 1]) != np.max([np.nan, 1])
    //
    // There are also separate np.nan{min,max} functions
    // which ignore nans instead.
    //
    // Which behaviour do we want to follow?
    //
    // Panic for now (TODO: Make this a user-facing error response instead)
    x.partial_cmp(y)
        // .unwrap_or(std::cmp::Ordering::Less)
        .unwrap_or_else(|| panic!("unexpected undefined order error for min"))
}

/// Finds the minimum value over one or more axes of the provided array
fn min_array_multi_axis<T: Element>(
    array: ndarray::ArrayView<T, ndarray::IxDyn>,
    axes: &[usize],
    missing: Option<Missing<T>>,
    order: &Option<Order>,
) -> (Vec<T>, Vec<i64>, Vec<usize>) {
    let (result, shape) = if axes.is_empty() {
        // Emulate numpy behaviour of 'reduction over no axes'
        let result = reduction_over_zero_axes(&array, missing, order);
        (result, array.shape().to_owned())
    } else {
        // Find minimum over first axis and count elements operated on
        let init = T::max_value();
        let mut result = array
            .fold_axis(Axis(axes[0]), (init, 0), |(running_min, count), val| {
                if let Some(missing) = &missing {
                    if !missing.is_missing(val) {
                        let new_min = min_by(running_min, val, min_element_pairwise);
                        (*new_min, count + 1)
                    } else {
                        (*running_min, *count)
                    }
                } else {
                    let new_min = min_by(running_min, val, min_element_pairwise);
                    (*new_min, count + 1)
                }
            })
            .into_dyn();
        // Find min over remaining axes (where total count is now sum of counts)
        if let Some(remaining_axes) = axes.get(1..) {
            for (n, axis) in remaining_axes.iter().enumerate() {
                result = result
                    .fold_axis(
                        Axis(axis - n - 1),
                        (init, 0),
                        |(global_min, total_count), (running_min, count)| {
                            // (*global_min.min(running_min), total_count + count)
                            let new_min = min_by(global_min, running_min, min_element_pairwise);
                            (*new_min, total_count + count)
                        },
                    )
                    .into_dyn();
            }
        }
        let shape = result.shape().to_owned();
        (result, shape)
    };

    // Result is array of (mins, count) tuples so separate them here
    let mins = result.iter().map(|(min, _)| *min).collect::<Vec<T>>();
    let counts = result.iter().map(|(_, count)| *count).collect::<Vec<i64>>();

    (mins, counts, shape)
}

impl NumOperation for Min {
    fn execute_t<T: Element>(
        request_data: &models::RequestData,
        mut data: Vec<u8>,
    ) -> Result<models::Response, ActiveStorageError> {
        let array = array::build_array::<T>(request_data, &mut data)?;
        let slice_info = array::build_slice_info::<T>(&request_data.selection, array.shape());
        let sliced = array.slice(slice_info);

        // Convert Missing<Dtype> to Missing<T: Element>
        let typed_missing: Option<Missing<T>> = if let Some(missing) = &request_data.missing {
            let m = Missing::try_from(missing)?;
            Some(m)
        } else {
            None
        };

        // Use ndarray::fold, ndarray::fold_axis or dispatch to specialised
        // multi-axis function depending on whether we're performing reduction
        // over all axes or only a subset
        match &request_data.axis {
            ReductionAxes::One(axis) => {
                let init = T::max_value();
                let result =
                    sliced.fold_axis(Axis(*axis), (init, 0), |(running_min, count), val| {
                        if let Some(missing) = &typed_missing {
                            if !missing.is_missing(val) {
                                (*min_by(running_min, val, min_element_pairwise), count + 1)
                            } else {
                                (*running_min, *count)
                            }
                        } else {
                            (*min_by(running_min, val, min_element_pairwise), count + 1)
                        }
                    });
                // Unpack the result tuples into separate vectors
                let mins = result.iter().map(|(min, _)| *min).collect::<Vec<T>>();
                let counts = result.iter().map(|(_, count)| *count).collect::<Vec<i64>>();
                let body = mins.as_bytes();
                let body = Bytes::copy_from_slice(body);
                Ok(models::Response::new(
                    body,
                    request_data.dtype,
                    result.shape().into(),
                    counts,
                ))
            }
            ReductionAxes::Multi(axes) => {
                let (mins, counts, shape) =
                    min_array_multi_axis(sliced, axes, typed_missing, &request_data.order);
                let body = Bytes::copy_from_slice(mins.as_bytes());
                Ok(models::Response::new(
                    body,
                    request_data.dtype,
                    shape,
                    counts,
                ))
            }
            ReductionAxes::All => {
                let init = T::max_value();
                let (min, count) = sliced.fold((init, 0_i64), |(running_min, count), val| {
                    if let Some(missing) = &typed_missing {
                        if !missing.is_missing(val) {
                            (*min_by(&running_min, val, min_element_pairwise), count + 1)
                        } else {
                            (running_min, count)
                        }
                    } else {
                        (*min_by(&running_min, val, min_element_pairwise), count + 1)
                    }
                });

                let body = min.as_bytes();
                // Need to copy to provide ownership to caller.
                let body = Bytes::copy_from_slice(body);
                Ok(models::Response::new(
                    body,
                    request_data.dtype,
                    vec![],
                    vec![count],
                ))
            }
        }
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
            count_non_missing(&sliced, &missing)
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
            vec![count],
        ))
    }
}

/// Return the sum of selected elements in the array.
pub struct Sum {}

/// Performs a sum over one or more axes of the provided array
fn sum_array_multi_axis<T: Element>(
    array: ndarray::ArrayView<T, ndarray::IxDyn>,
    axes: &[usize],
    missing: Option<Missing<T>>,
    order: &Option<Order>,
) -> (Vec<T>, Vec<i64>, Vec<usize>) {
    let (result, shape) = if axes.is_empty() {
        // Emulate numpy behaviour of 'reduction over no axes'
        let result = reduction_over_zero_axes(&array, missing, order);
        (result, array.shape().to_owned())
    } else {
        // Sum over first axis and count elements operated on
        let mut result = array
            .fold_axis(Axis(axes[0]), (T::zero(), 0), |(sum, count), val| {
                if let Some(missing) = &missing {
                    if !missing.is_missing(val) {
                        (*sum + *val, count + 1)
                    } else {
                        (*sum, *count)
                    }
                } else {
                    (*sum + *val, count + 1)
                }
            })
            .into_dyn();
        // Sum over remaining axes (where total count is now sum of counts)
        if let Some(remaining_axes) = axes.get(1..) {
            for (n, axis) in remaining_axes.iter().enumerate() {
                result = result
                    .fold_axis(
                        Axis(axis - n - 1),
                        (T::zero(), 0),
                        |(total_sum, total_count), (sum, count)| {
                            (*total_sum + *sum, total_count + count)
                        },
                    )
                    .into_dyn();
            }
        }
        let shape = result.shape().to_owned();
        (result, shape)
    };

    // Result is array of (sum, count) tuples so separate them here
    let sums = result.iter().map(|(sum, _)| *sum).collect::<Vec<T>>();
    let counts = result.iter().map(|(_, count)| *count).collect::<Vec<i64>>();

    (sums, counts, shape)
}

impl NumOperation for Sum {
    fn execute_t<T: Element>(
        request_data: &models::RequestData,
        mut data: Vec<u8>,
    ) -> Result<models::Response, ActiveStorageError> {
        let array = array::build_array::<T>(request_data, &mut data)?;
        let slice_info = array::build_slice_info::<T>(&request_data.selection, array.shape());
        let sliced = array.slice(slice_info);

        // Convert Missing<Dtype> to Missing<T: Element>
        let typed_missing: Option<Missing<T>> = if let Some(missing) = &request_data.missing {
            let m = Missing::try_from(missing)?;
            Some(m)
        } else {
            None
        };

        // Use ndarray::fold or ndarray::fold_axis depending on whether we're
        // performing reduction over all axes or only a subset
        match &request_data.axis {
            ReductionAxes::One(axis) => {
                let result = sliced.fold_axis(Axis(*axis), (T::zero(), 0), |(sum, count), val| {
                    if let Some(missing) = &typed_missing {
                        if !missing.is_missing(val) {
                            (*sum + *val, count + 1)
                        } else {
                            (*sum, *count)
                        }
                    } else {
                        (*sum + *val, count + 1)
                    }
                });
                // Unpack the result tuples into separate vectors
                let sums = result.iter().map(|(sum, _)| *sum).collect::<Vec<T>>();
                let counts = result.iter().map(|(_, count)| *count).collect::<Vec<i64>>();
                let body = sums.as_bytes();
                let body = Bytes::copy_from_slice(body);
                Ok(models::Response::new(
                    body,
                    request_data.dtype,
                    result.shape().into(),
                    counts,
                ))
            }
            ReductionAxes::Multi(axes) => {
                let (sums, counts, shape) =
                    sum_array_multi_axis(sliced, axes, typed_missing, &request_data.order);
                let body = Bytes::copy_from_slice(sums.as_bytes());
                Ok(models::Response::new(
                    body,
                    request_data.dtype,
                    shape,
                    counts,
                ))
            }
            ReductionAxes::All => {
                let (sum, count) = sliced.fold((T::zero(), 0_i64), |(sum, count), val| {
                    if let Some(missing) = &typed_missing {
                        if !missing.is_missing(val) {
                            (sum + *val, count + 1)
                        } else {
                            (sum, count)
                        }
                    } else {
                        (sum + *val, count + 1)
                    }
                });

                let body = sum.as_bytes();
                // Need to copy to provide ownership to caller.
                let body = Bytes::copy_from_slice(body);
                Ok(models::Response::new(
                    body,
                    request_data.dtype,
                    vec![],
                    vec![count],
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    use crate::models::ReductionAxes;
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
        let expected = vec![2];
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
        let expected = vec![1];
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
        assert_eq!(vec![1], response.count);
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
        assert_eq!(vec![1], response.count);
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
        assert_eq!(vec![2], response.count);
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
        assert_eq!(vec![2], response.count);
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
        assert_eq!(vec![1], response.count);
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
        assert_eq!(vec![1], response.count);
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
        assert_eq!(vec![2], response.count);
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
        assert_eq!(vec![2], response.count);
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
        assert_eq!(vec![2], response.count);
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
        assert_eq!(vec![2], response.count);
    }

    #[test]
    #[should_panic(expected = "unexpected undefined order error for min")]
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
        assert_eq!(vec![2], response.count);
    }

    #[test]
    #[should_panic(expected = "unexpected undefined order error for min")]
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
        assert_eq!(vec![2], response.count);
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
        assert_eq!(vec![2], response.count);
    }

    #[test]
    fn select_f32_1d_1ax() {
        // Arrange
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Float32;
        request_data.axis = ReductionAxes::Multi(vec![0]);
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        // Act
        let response = Select::execute(&request_data, data).unwrap();
        // Assert (check that axis value is ignored)
        let expected: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
        assert_eq!(expected.as_bytes(), response.body);
        assert_eq!(8, response.body.len());
        assert_eq!(models::DType::Float32, response.dtype);
        assert_eq!(vec![2], response.shape);
        assert_eq!(vec![2], response.count);
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
        assert_eq!(vec![2], response.count);
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
        assert_eq!(vec![2], response.count);
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
        assert_eq!(vec![2], response.count);
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
        assert_eq!(vec![1], response.count);
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
        assert_eq!(vec![2], response.count);
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
        assert_eq!(vec![2], response.count);
    }

    /// Test helper for converting response bytes back into a type
    /// that's easier to assert on
    fn vec_from_bytes<T: zerocopy::AsBytes + zerocopy::FromBytes + Clone>(data: &Bytes) -> Vec<T> {
        let mut data = data.to_vec();
        let data = data.as_mut_slice();
        let layout = zerocopy::LayoutVerified::<_, [T]>::new_slice(&mut data[..]).unwrap();
        layout.into_mut_slice().to_vec()
    }

    #[test]
    fn sum_u32_1d_axis_0() {
        // Arrange
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Uint32;
        request_data.shape = Some(vec![2, 4]);
        request_data.axis = ReductionAxes::One(0);
        let data: Vec<u32> = vec![1, 2, 3, 4, 5, 6, 7, 8];
        // Act
        let response = Sum::execute(&request_data, data.as_bytes().into()).unwrap();
        let result = vec_from_bytes::<u32>(&response.body);
        // Assert
        let arr = ndarray::Array::from_shape_vec((2, 4), data).unwrap();
        let expected = arr.sum_axis(Axis(0)).to_vec();
        assert_eq!(result, expected);
        assert_eq!(models::DType::Uint32, response.dtype);
        assert_eq!(16, response.body.len()); // 4 bytes in a u32
        assert_eq!(vec![4], response.shape);
        assert_eq!(vec![2, 2, 2, 2], response.count);
    }

    #[test]
    fn sum_u32_1d_axis_1_missing() {
        // Arrange
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Uint32;
        request_data.shape = Some(vec![2, 4]);
        request_data.axis = ReductionAxes::One(1);
        request_data.missing = Some(Missing::MissingValue(0.into()));
        let data: Vec<u32> = vec![0, 2, 3, 4, 5, 6, 7, 8];
        // Act
        let response = Sum::execute(&request_data, data.as_bytes().into()).unwrap();
        let result = vec_from_bytes::<u32>(&response.body);
        // Assert
        let arr = ndarray::Array::from_shape_vec((2, 4), data).unwrap();
        let expected = arr.sum_axis(Axis(1)).to_vec();
        assert_eq!(result, expected);
        assert_eq!(models::DType::Uint32, response.dtype);
        assert_eq!(8, response.body.len()); // 4 bytes in a u32
        assert_eq!(vec![2], response.shape);
        assert_eq!(vec![3, 4], response.count); // Expect a lower count due to 'missing' value
    }

    #[test]
    fn sum_f64_1d_axis_1_missing() {
        // Arrange
        let mut request_data = test_utils::get_test_request_data();
        request_data.dtype = models::DType::Float64;
        request_data.shape = Some(vec![2, 2, 2]);
        request_data.axis = ReductionAxes::One(1);
        request_data.missing = Some(Missing::MissingValue(0.into()));
        let data: Vec<f64> = vec![0.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        // Act
        let response = Sum::execute(&request_data, data.as_bytes().into()).unwrap();
        let result = vec_from_bytes::<f64>(&response.body);
        let result = ndarray::Array::from_shape_vec((2, 2), result).unwrap();
        // Assert
        let arr = ndarray::Array::from_shape_vec((2, 2, 2), data).unwrap();
        let expected = arr.sum_axis(Axis(1));
        assert_eq!(result, expected);
        assert_eq!(models::DType::Float64, response.dtype);
        assert_eq!(32, response.body.len()); // 8 bytes in a f64
        assert_eq!(vec![2, 2], response.shape);
        assert_eq!(vec![1, 2, 2, 2], response.count); // Expect a lower count due to 'missing' value
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

    #[test]
    #[should_panic(expected = "assertion failed: axis.index() < self.ndim()")]
    fn sum_multi_axis_2d_wrong_axis() {
        let array = ndarray::Array::from_shape_vec((2, 2), (0..4).collect())
            .unwrap()
            .into_dyn();
        let axes = vec![2];
        let _ = sum_array_multi_axis(array.view(), &axes, None, &None);
    }

    #[test]
    fn sum_multi_axis_2d_2ax() {
        let array = ndarray::Array::from_shape_vec((2, 2), (0..4).collect())
            .unwrap()
            .into_dyn();
        let axes = vec![0, 1];
        let (sum, count, shape) = sum_array_multi_axis(array.view(), &axes, None, &None);
        assert_eq!(sum, vec![6]);
        assert_eq!(count, vec![4]);
        assert_eq!(shape, Vec::<usize>::new());
    }

    #[test]
    fn sum_multi_axis_2d_2ax_missing() {
        let array = ndarray::Array::from_shape_vec((2, 2), (0..4).collect())
            .unwrap()
            .into_dyn();
        let axes = vec![0, 1];
        let missing = Missing::MissingValue(1);
        let (sum, count, shape) = sum_array_multi_axis(array.view(), &axes, Some(missing), &None);
        assert_eq!(sum, vec![5]);
        assert_eq!(count, vec![3]);
        assert_eq!(shape, Vec::<usize>::new());
    }

    #[test]
    fn sum_multi_axis_2d_no_ax_some_missing() {
        // Arrange
        let axes = vec![];
        let missing = Some(Missing::ValidMax(2));
        let arr = ndarray::Array::from_shape_vec((2, 3), (0..6).collect())
            .unwrap()
            .into_dyn();
        // Act
        let (result, counts, shape) = sum_array_multi_axis(arr.view(), &axes, missing, &None);
        // Assert - reduction should just return original array
        assert_eq!(result, arr.iter().copied().collect::<Vec<i64>>());
        assert_eq!(counts, vec![1, 1, 1, 0, 0, 0]);
        assert_eq!(shape, arr.shape());
    }

    #[test]
    fn sum_multi_axis_2d_no_ax_some_missing_f_order() {
        // Arrange
        let axes = vec![];
        let missing = Some(Missing::ValidMax(2));
        let arr = ndarray::Array::from_shape_vec((2, 3), (0..6).collect())
            .unwrap()
            .into_dyn();
        // Act
        let (result, counts, shape) =
            sum_array_multi_axis(arr.view(), &axes, missing, &Some(Order::F));
        // Assert - reduction should return transposed version of
        // original array (due to F ordering)
        assert_eq!(result, arr.t().iter().copied().collect::<Vec<i64>>());
        assert_eq!(counts, vec![1, 0, 1, 0, 1, 0]);
        assert_eq!(shape, arr.shape());
    }

    #[test]
    fn sum_multi_axis_4d_1ax() {
        let array = ndarray::Array::from_shape_vec((2, 3, 2, 1), (0..12).collect())
            .unwrap()
            .into_dyn();
        let axes = vec![2];
        let (sum, count, shape) = sum_array_multi_axis(array.view(), &axes, None, &None);
        assert_eq!(sum, vec![1, 5, 9, 13, 17, 21]);
        assert_eq!(count, vec![2, 2, 2, 2, 2, 2]);
        assert_eq!(shape, vec![2, 3, 1]);
    }

    #[test]
    fn sum_multi_axis_4d_3ax() {
        let array = ndarray::Array::from_shape_vec((2, 3, 2, 1), (0..12).collect())
            .unwrap()
            .into_dyn();
        let axes = vec![0, 1, 3];
        let (sum, count, shape) = sum_array_multi_axis(array.view(), &axes, None, &None);
        assert_eq!(sum, vec![30, 36]);
        assert_eq!(count, vec![6, 6]);
        assert_eq!(shape, vec![2]);
    }

    #[test]
    #[should_panic(expected = "assertion failed: axis.index() < self.ndim()")]
    fn min_multi_axis_2d_wrong_axis() {
        let array = ndarray::Array::from_shape_vec((2, 2), (0..4).collect())
            .unwrap()
            .into_dyn();
        let axes = vec![2];
        let _ = min_array_multi_axis(array.view(), &axes, None, &None);
    }

    #[test]
    fn min_multi_axis_2d_2ax() {
        // Arrrange
        let axes = vec![0, 1];
        let missing = None;
        let arr = ndarray::Array::from_shape_vec((2, 3), (0..6).collect())
            .unwrap()
            .into_dyn();
        // Act
        let (result, counts, shape) = min_array_multi_axis(arr.view(), &axes, missing, &None);
        // Assert
        assert_eq!(result, vec![0]);
        assert_eq!(counts, vec![6]);
        assert_eq!(shape, Vec::<usize>::new());
    }

    #[test]
    fn min_multi_axis_2d_no_ax_some_missing() {
        // Arrange
        let axes = vec![];
        let missing = Some(Missing::ValidMax(2));
        let arr = ndarray::Array::from_shape_vec((2, 3), (0..6).collect())
            .unwrap()
            .into_dyn();
        // Act
        let (result, counts, shape) = min_array_multi_axis(arr.view(), &axes, missing, &None);
        // Assert - reduction should just return original array
        assert_eq!(result, arr.iter().copied().collect::<Vec<i64>>());
        assert_eq!(counts, vec![1, 1, 1, 0, 0, 0]);
        assert_eq!(shape, arr.shape());
    }

    #[test]
    fn min_multi_axis_2d_no_ax_some_missing_f_order() {
        // Arrange
        let axes = vec![];
        let missing = Some(Missing::ValidMax(2));
        let arr = ndarray::Array::from_shape_vec((2, 3), (0..6).collect())
            .unwrap()
            .into_dyn();
        // Act
        let (result, counts, shape) =
            min_array_multi_axis(arr.view(), &axes, missing, &Some(Order::F));
        // Assert - reduction should return transposed version of
        // original array (due to F ordering)
        assert_eq!(result, arr.t().iter().copied().collect::<Vec<i64>>());
        assert_eq!(counts, vec![1, 0, 1, 0, 1, 0]);
        assert_eq!(shape, arr.shape());
    }

    #[test]
    fn min_multi_axis_2d_1ax_missing() {
        // Arrange
        let axes = vec![1];
        let missing = Missing::MissingValue(0);
        let arr = ndarray::Array::from_shape_vec((2, 3), (0..6).collect())
            .unwrap()
            .into_dyn();
        // Act
        let (result, counts, shape) = min_array_multi_axis(arr.view(), &axes, Some(missing), &None);
        // Assert
        assert_eq!(result, vec![1, 3]);
        assert_eq!(counts, vec![2, 3]);
        assert_eq!(shape, vec![2]);
    }

    #[test]
    fn min_multi_axis_4d_3ax_missing() {
        let arr = ndarray::Array::from_shape_vec((2, 3, 2, 1), (0..12).collect())
            .unwrap()
            .into_dyn();
        let axes = vec![0, 1, 3];
        let missing = Missing::MissingValue(1);
        let (result, counts, shape) = min_array_multi_axis(arr.view(), &axes, Some(missing), &None);

        assert_eq!(result, vec![0, 3]);
        assert_eq!(counts, vec![6, 5]);
        assert_eq!(shape, vec![2]);
    }

    #[test]
    #[should_panic(expected = "assertion failed: axis.index() < self.ndim()")]
    fn max_multi_axis_2d_wrong_axis() {
        // Arrange
        let array = ndarray::Array::from_shape_vec((2, 2), (0..4).collect())
            .unwrap()
            .into_dyn();
        let axes = vec![2];
        // Act
        let _ = max_array_multi_axis(array.view(), &axes, None, &None);
    }

    #[test]
    fn max_multi_axis_2d_2ax() {
        // Arrange
        let axes = vec![0, 1];
        let missing = None;
        let arr = ndarray::Array::from_shape_vec((2, 3), (0..6).collect())
            .unwrap()
            .into_dyn();
        // Act
        let (result, counts, shape) = max_array_multi_axis(arr.view(), &axes, missing, &None);
        // Assert
        assert_eq!(result, vec![5]);
        assert_eq!(counts, vec![6]);
        assert_eq!(shape, Vec::<usize>::new());
    }

    #[test]
    fn max_multi_axis_2d_no_ax_some_missing() {
        // Arrange
        let axes = vec![];
        let missing = Some(Missing::ValidMax(2));
        let arr = ndarray::Array::from_shape_vec((2, 3), (0..6).collect())
            .unwrap()
            .into_dyn();
        // Act
        let (result, counts, shape) = max_array_multi_axis(arr.view(), &axes, missing, &None);
        // Assert - reduction should just return original array
        assert_eq!(result, arr.iter().copied().collect::<Vec<i64>>());
        assert_eq!(counts, vec![1, 1, 1, 0, 0, 0]);
        assert_eq!(shape, arr.shape());
    }

    #[test]
    fn max_multi_axis_2d_no_ax_some_missing_f_order() {
        // Arrange
        let axes = vec![];
        let missing = Some(Missing::ValidMax(2));
        let arr = ndarray::Array::from_shape_vec((2, 3), (0..6).collect())
            .unwrap()
            .into_dyn();
        // Act
        let (result, counts, shape) =
            max_array_multi_axis(arr.view(), &axes, missing, &Some(Order::F));
        // Assert - reduction should return transposed version of
        // original array (due to F ordering)
        assert_eq!(result, arr.t().iter().copied().collect::<Vec<i64>>());
        assert_eq!(counts, vec![1, 0, 1, 0, 1, 0]);
        assert_eq!(shape, arr.shape());
    }

    #[test]
    fn max_multi_axis_2d_1ax_missing() {
        // Arrange
        let axes = vec![1];
        let missing = Missing::MissingValue(0);
        let arr = ndarray::Array::from_shape_vec((2, 3), (0..6).collect())
            .unwrap()
            .into_dyn();
        // Act
        let (result, counts, shape) = max_array_multi_axis(arr.view(), &axes, Some(missing), &None);
        // Assert
        assert_eq!(result, vec![2, 5]);
        assert_eq!(counts, vec![2, 3]);
        assert_eq!(shape, vec![2]);
    }

    #[test]
    fn max_multi_axis_4d_3ax_missing() {
        // Arrange
        let arr = ndarray::Array::from_shape_vec((2, 3, 2, 1), (0..12).collect())
            .unwrap()
            .into_dyn();
        let axes = vec![0, 1, 3];
        let missing = Missing::MissingValue(10);
        // Act
        let (result, counts, shape) = max_array_multi_axis(arr.view(), &axes, Some(missing), &None);
        // Assert
        assert_eq!(result, vec![8, 11]);
        assert_eq!(counts, vec![5, 6]);
        assert_eq!(shape, vec![2]);
    }

    #[test]
    #[should_panic(expected = "assertion failed: axis.index() < self.ndim()")]
    fn count_multi_axis_2d_wrong_axis() {
        // Arrange
        let array = ndarray::Array::from_shape_vec((2, 2), (0..4).collect())
            .unwrap()
            .into_dyn();
        let axes = vec![2];
        // Act
        let _ = count_array_multi_axis(array.view(), &axes, None);
    }

    #[test]
    fn count_multi_axis_2d_2ax() {
        // Arrange
        let axes = vec![0, 1];
        let missing = None;
        let arr = ndarray::Array::from_shape_vec((2, 3), (0..6).collect())
            .unwrap()
            .into_dyn();
        // Act
        let (counts, shape) = count_array_multi_axis(arr.view(), &axes, missing);
        // Assert
        assert_eq!(counts, vec![6]);
        assert_eq!(shape, Vec::<usize>::new());
    }

    #[test]
    fn count_multi_axis_2d_no_ax() {
        // Arrange
        let axes = vec![];
        let missing = None;
        let arr = ndarray::Array::from_shape_vec((2, 3), (0..6).collect())
            .unwrap()
            .into_dyn();
        // Act
        let (counts, shape) = count_array_multi_axis(arr.view(), &axes, missing);
        // Assert
        assert_eq!(counts, vec![1, 1, 1, 1, 1, 1]);
        assert_eq!(shape, arr.shape().to_vec());
    }

    #[test]
    fn count_multi_axis_2d_1ax_missing() {
        // Arrange
        let axes = vec![1];
        let missing = Missing::MissingValue(0);
        let arr = ndarray::Array::from_shape_vec((2, 3), (0..6).collect())
            .unwrap()
            .into_dyn();
        // Act
        let (counts, shape) = count_array_multi_axis(arr.view(), &axes, Some(missing));
        // Assert
        assert_eq!(counts, vec![2, 3]);
        assert_eq!(shape, vec![2]);
    }

    #[test]
    fn count_multi_axis_4d_3ax_multi_missing() {
        // Arrange
        let arr = ndarray::Array::from_shape_vec((2, 3, 2, 1), (0..12).collect())
            .unwrap()
            .into_dyn();
        let axes = vec![0, 1, 3];
        let missing = Missing::MissingValues(vec![9, 10, 11]);
        // Act
        let (counts, shape) = count_array_multi_axis(arr.view(), &axes, Some(missing));
        // Assert
        assert_eq!(counts, vec![5, 4]);
        assert_eq!(shape, vec![2]);
    }

    #[test]
    fn count_multi_axis_4d_3ax_missing() {
        // Arrange
        let arr = ndarray::Array::from_shape_vec((2, 3, 2, 1), (0..12).collect())
            .unwrap()
            .into_dyn();
        let axes = vec![0, 1, 3];
        let missing = Missing::MissingValue(10);
        // Act
        let (counts, shape) = count_array_multi_axis(arr.view(), &axes, Some(missing));
        // Assert
        assert_eq!(counts, vec![5, 6]);
        assert_eq!(shape, vec![2]);
    }
}
