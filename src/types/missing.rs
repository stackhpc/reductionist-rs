//! Missing data descriptors
//!
//! Arrays can contain missing data which should be ignored during computation. There are multiple
//! ways to describe the missing data. Currently we support:
//!
//! * A single missing value
//! * Multiple missing values
//! * A valid minimum value
//! * A valid maximum value
//! * A valid range of values

use serde::{Deserialize, Serialize};
use validator::ValidationError;

use crate::error::ActiveStorageError;
use crate::models::DType;
use crate::operation::Element;
use crate::types::dvalue::TryFromDValue;
use crate::types::DValue;

/// Missing data
///
/// This enum can represent all known descriptions of missing data used in NetCDF4 files.
/// It is generic over the type of missing data values. We use this in two ways:
/// 1. T = [DValue], used in the API to represent values of any supported
///    [DType].
/// 2. T = a primitive numeric type (i32, u64, f32, etc.), used in numeric operations when we know
///    the DType of the values.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Missing<T> {
    /// A single missing value
    MissingValue(T),
    /// Multple missing values
    MissingValues(Vec<T>),
    /// Valid minimum
    ValidMin(T),
    /// Valid maxiumum
    ValidMax(T),
    /// Valid range
    ValidRange(T, T),
}

impl Missing<DValue> {
    /// Validate a [`Missing<DValue>`](crate::types::Missing) object for a given
    /// [DType].
    pub fn validate(&self, dtype: DType) -> Result<(), ValidationError> {
        match dtype {
            DType::Int32 => Missing::<i32>::validate_dvalue(self),
            DType::Int64 => Missing::<i64>::validate_dvalue(self),
            DType::Uint32 => Missing::<u32>::validate_dvalue(self),
            DType::Uint64 => Missing::<u64>::validate_dvalue(self),
            DType::Float32 => Missing::<f32>::validate_dvalue(self),
            DType::Float64 => Missing::<f64>::validate_dvalue(self),
        }
    }
}

impl<T: PartialOrd + Serialize + TryFromDValue> Missing<T> {
    /// Validate a [`Missing<DValue>`](crate::types::Missing) for Missing<T> where T is a supported
    /// primitive numeric type.
    fn validate_dvalue(missing: &Missing<DValue>) -> Result<(), ValidationError> {
        // Perform a conversion to the primitive based type.
        let missing_primitive = Self::try_from(missing).map_err(|err| {
            let mut error = ValidationError::new("Missing data descriptor is invalid");
            error.add_param("error".into(), &err.to_string());
            error
        })?;
        // Validate min + max for valid ranges.
        if let Missing::ValidRange(min, max) = missing_primitive {
            if min >= max {
                let mut error =
                    ValidationError::new("Missing data valid range min must be less than max");
                error.add_param("min".into(), &min);
                error.add_param("max".into(), &max);
                return Err(error);
            };
        };
        Ok(())
    }
}

// Implement TryFrom<&Missing<DValue>> for Missing<T>.
// This allows us to convert from a Missing type based on the enum DValue type to a numeric type.
impl<T: TryFromDValue> TryFrom<&Missing<DValue>> for Missing<T> {
    type Error = ActiveStorageError;

    fn try_from(missing: &Missing<DValue>) -> Result<Self, Self::Error> {
        let result = match missing {
            Missing::MissingValue(value) => {
                Missing::<T>::MissingValue(T::try_from_dvalue(value.clone())?)
            }
            Missing::MissingValues(values) => {
                // Map to Results, then use ? on the collected Vec to fail if any is Err.
                let values = values
                    .iter()
                    .map(|value| T::try_from_dvalue(value.clone()))
                    .collect::<Result<Vec<T>, _>>()?;
                Missing::<T>::MissingValues(values)
            }
            Missing::ValidMin(min) => Missing::<T>::ValidMin(T::try_from_dvalue(min.clone())?),
            Missing::ValidMax(max) => Missing::<T>::ValidMax(T::try_from_dvalue(max.clone())?),
            Missing::ValidRange(min, max) => Missing::<T>::ValidRange(
                T::try_from_dvalue(min.clone())?,
                T::try_from_dvalue(max.clone())?,
            ),
        };
        Ok(result)
    }
}

impl<T: Element> Missing<T> {
    /// Filter function to check whether the provided value is a 'missing' value
    pub fn is_missing(&self, x: &T) -> bool {
        match self {
            Missing::MissingValue(value) => x == value,
            Missing::MissingValues(values) => values.contains(x),
            Missing::ValidMin(min) => x < min,
            Missing::ValidMax(max) => x > max,
            Missing::ValidRange(min, max) => x < min || x > max,
        }
    }
}

#[cfg(test)]
mod tests {
    use num_traits::Float;

    use super::*;

    #[test]
    fn test_try_from_missing_value() {
        let result = Missing::<i32>::try_from(&Missing::<DValue>::MissingValue(42.into())).unwrap();
        assert_eq!(Missing::<i32>::MissingValue(42), result);
    }

    #[test]
    fn test_validate_i32() {
        Missing::<DValue>::MissingValue(42.into())
            .validate(DType::Int32)
            .unwrap();
    }

    #[test]
    #[should_panic(expected = "Incompatible value -1 for missing")]
    fn test_validate_u32_negative() {
        Missing::<DValue>::MissingValue((-1).into())
            .validate(DType::Uint32)
            .unwrap();
    }

    #[test]
    #[should_panic(expected = "Missing data valid range min must be less than max")]
    fn test_validate_f32_range_min_gt_max() {
        Missing::<DValue>::ValidRange(
            DValue::from_f64(42.0).unwrap(),
            DValue::from_f64(-42.0).unwrap(),
        )
        .validate(DType::Float32)
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "Missing data valid range min must be less than max")]
    fn test_validate_f32_range_min_eq_max() {
        Missing::<DValue>::ValidRange(
            DValue::from_f64(42.0).unwrap(),
            DValue::from_f64(42.0).unwrap(),
        )
        .validate(DType::Float32)
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "IncompatibleMissing(Number(2147483648))")]
    fn test_try_from_missing_value_too_large() {
        Missing::<i32>::try_from(&Missing::<DValue>::MissingValue(
            (i32::max_value() as i64 + 1).into(),
        ))
        .unwrap();
    }

    #[test]
    fn test_try_from_missing_values() {
        let result = Missing::<i64>::try_from(&Missing::<DValue>::MissingValues(vec![
            42.into(),
            (-1).into(),
        ]))
        .unwrap();
        assert_eq!(Missing::<i64>::MissingValues(vec![42, -1]), result);
    }

    #[test]
    #[should_panic(expected = "IncompatibleMissing(Number(9223372036854775808))")]
    fn test_try_from_missing_values_too_large() {
        Missing::<i64>::try_from(&Missing::<DValue>::MissingValues(vec![
            (i64::max_value() as u64 + 1).into(),
            (-1).into(),
        ]))
        .unwrap();
    }

    #[test]
    fn test_try_from_valid_min() {
        let result = Missing::<u32>::try_from(&Missing::<DValue>::ValidMin(42.into())).unwrap();
        assert_eq!(Missing::<u32>::ValidMin(42), result);
    }

    #[test]
    #[should_panic(expected = "IncompatibleMissing(Number(4294967296))")]
    fn test_try_from_valid_min_too_large() {
        Missing::<u32>::try_from(&Missing::<DValue>::ValidMin(
            (u32::max_value() as u64 + 1).into(),
        ))
        .unwrap();
    }

    #[test]
    fn test_try_from_valid_max() {
        let result = Missing::<u64>::try_from(&Missing::<DValue>::ValidMax(42.into())).unwrap();
        assert_eq!(Missing::<u64>::ValidMax(42), result);
    }

    #[test]
    #[should_panic(expected = "IncompatibleMissing(Number(-1))")]
    fn test_try_from_valid_max_negative() {
        Missing::<u64>::try_from(&Missing::<DValue>::ValidMax((-1).into())).unwrap();
    }

    #[test]
    fn test_try_from_valid_range() {
        let result = Missing::<f32>::try_from(&Missing::<DValue>::ValidRange(
            DValue::from_f64(-42.0).unwrap(),
            DValue::from_f64(42.0).unwrap(),
        ))
        .unwrap();
        assert_eq!(Missing::<f32>::ValidRange(-42.0, 42.0), result);
    }

    #[test]
    #[should_panic(expected = "IncompatibleMissing(Number(6.805646932770577e38))")]
    fn test_try_from_valid_range_too_large() {
        Missing::<f32>::try_from(&Missing::<DValue>::ValidRange(
            DValue::from_f64((f32::max_value() as f64) * 2.0).unwrap(),
            DValue::from_f64(42.0).unwrap(),
        ))
        .unwrap();
    }

    #[test]
    fn test_is_missing_value() {
        let missing = Missing::MissingValue(1);
        assert!(!missing.is_missing(&0));
        assert!(missing.is_missing(&1));
        assert!(!missing.is_missing(&2));
    }

    #[test]
    fn test_is_missing_values() {
        let missing = Missing::MissingValues(vec![1, 2]);
        assert!(!missing.is_missing(&0));
        assert!(missing.is_missing(&1));
        assert!(missing.is_missing(&2));
        assert!(!missing.is_missing(&3));
    }

    #[test]
    fn test_is_missing_valid_min() {
        let missing = Missing::ValidMin(1);
        assert!(missing.is_missing(&0));
        assert!(!missing.is_missing(&1));
        assert!(!missing.is_missing(&2));
    }

    #[test]
    fn test_is_missing_valid_max() {
        let missing = Missing::ValidMax(1);
        assert!(!missing.is_missing(&0));
        assert!(!missing.is_missing(&1));
        assert!(missing.is_missing(&2));
    }

    #[test]
    fn test_is_missing_valid_range() {
        let missing = Missing::ValidRange(1, 2);
        assert!(missing.is_missing(&0));
        assert!(!missing.is_missing(&1));
        assert!(!missing.is_missing(&2));
        assert!(missing.is_missing(&3));
    }
}
