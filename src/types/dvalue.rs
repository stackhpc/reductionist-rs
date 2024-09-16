//! Data value representing a value of any [DType](crate::models::DType)

use crate::error::ActiveStorageError;

/// A value of any DType.
/// This is an alias of the Number type from serde_json, which is an enum that can represent
/// integers and floating point numbers.
/// The number type is an enum over i64, u64 and f64, with the additional constraint that floating
/// point numbers must be finite (not positive or negative infinity or NaN).
pub type DValue = serde_json::Number;

/// Try to convert a DValue to an i64.
fn as_i64(value: &DValue) -> Result<i64, ActiveStorageError> {
    value
        .as_i64()
        .ok_or(ActiveStorageError::IncompatibleMissing(value.clone()))
}

/// Try to convert a DValue to a u64.
fn as_u64(value: &DValue) -> Result<u64, ActiveStorageError> {
    value
        .as_u64()
        .ok_or(ActiveStorageError::IncompatibleMissing(value.clone()))
}

/// Try to convert a DValue to an f64.
fn as_f64(value: &DValue) -> Result<f64, ActiveStorageError> {
    value
        .as_f64()
        .ok_or(ActiveStorageError::IncompatibleMissing(value.clone()))
}

/// Attempt to convert from a [DValue] to specific numeric type.
// This trait exists because we can't implement TryFrom<DValue> for numeric types because the trait
// and type are in external crates.
pub trait TryFromDValue: Sized {
    /// Try to convert from a [DValue] to a numeric type.
    fn try_from_dvalue(value: DValue) -> Result<Self, ActiveStorageError>;
}

// Implement the TryFromDValue trait for all supported numeric data types.

impl TryFromDValue for i32 {
    fn try_from_dvalue(value: DValue) -> Result<Self, ActiveStorageError> {
        Self::try_from(as_i64(&value)?).map_err(|_| ActiveStorageError::IncompatibleMissing(value))
    }
}

impl TryFromDValue for i64 {
    fn try_from_dvalue(value: DValue) -> Result<Self, ActiveStorageError> {
        as_i64(&value)
    }
}

impl TryFromDValue for u32 {
    fn try_from_dvalue(value: DValue) -> Result<Self, ActiveStorageError> {
        Self::try_from(as_u64(&value)?).map_err(|_| ActiveStorageError::IncompatibleMissing(value))
    }
}

impl TryFromDValue for u64 {
    fn try_from_dvalue(value: DValue) -> Result<Self, ActiveStorageError> {
        as_u64(&value)
    }
}

impl TryFromDValue for f32 {
    fn try_from_dvalue(value: DValue) -> Result<Self, ActiveStorageError> {
        // If the number is too large to be represented as an f32 this cast returns infinity.
        let float = as_f64(&value)? as f32;
        if float.is_finite() {
            Ok(float)
        } else {
            Err(ActiveStorageError::IncompatibleMissing(value))
        }
    }
}

impl TryFromDValue for f64 {
    fn try_from_dvalue(value: DValue) -> Result<Self, ActiveStorageError> {
        as_f64(&value)
    }
}

#[cfg(test)]
mod tests {
    use num_traits::Float;

    use super::*;

    #[test]
    fn test_dvalue_is_finite() {
        assert!(DValue::from_f64(f64::infinity()).is_none());
    }

    #[test]
    fn test_dvalue_is_not_nan() {
        assert!(DValue::from_f64(f64::nan()).is_none());
    }

    #[test]
    fn test_try_from_dvalue_i32() {
        let result = i32::try_from_dvalue(42.into()).unwrap();
        assert_eq!(42, result);
    }

    #[test]
    #[should_panic(expected = "IncompatibleMissing(Number(2147483648))")]
    fn test_try_from_dvalue_i32_too_large() {
        i32::try_from_dvalue((i32::max_value() as i64 + 1).into()).unwrap();
    }

    #[test]
    #[should_panic(expected = "IncompatibleMissing(Number(-2147483649))")]
    fn test_try_from_dvalue_i32_too_negative() {
        i32::try_from_dvalue((i32::min_value() as i64 - 1).into()).unwrap();
    }

    #[test]
    fn test_try_from_dvalue_i64() {
        let result = i64::try_from_dvalue((-42).into()).unwrap();
        assert_eq!(-42, result);
    }

    #[test]
    #[should_panic(expected = "IncompatibleMissing(Number(9223372036854775808))")]
    fn test_try_from_dvalue_i64_too_large() {
        i64::try_from_dvalue((i64::max_value() as u64 + 1).into()).unwrap();
    }

    #[test]
    #[should_panic(expected = "IncompatibleMissing(Number(1.0))")]
    fn test_try_from_dvalue_i64_float() {
        i64::try_from_dvalue(DValue::from_f64(1.0).unwrap()).unwrap();
    }

    #[test]
    fn test_try_from_dvalue_u32() {
        let result = u32::try_from_dvalue(42.into()).unwrap();
        assert_eq!(42, result);
    }

    #[test]
    #[should_panic(expected = "IncompatibleMissing(Number(4294967296))")]
    fn test_try_from_dvalue_u32_too_large() {
        u32::try_from_dvalue((u32::max_value() as u64 + 1).into()).unwrap();
    }

    #[test]
    #[should_panic(expected = "IncompatibleMissing(Number(-1))")]
    fn test_try_from_dvalue_u32_negative() {
        u32::try_from_dvalue((-1).into()).unwrap();
    }

    #[test]
    fn test_try_from_dvalue_u64() {
        let result = u64::try_from_dvalue(42.into()).unwrap();
        assert_eq!(42, result);
    }

    #[test]
    #[should_panic(expected = "IncompatibleMissing(Number(-1))")]
    fn test_try_from_dvalue_u64_negative() {
        u64::try_from_dvalue((-1).into()).unwrap();
    }

    #[test]
    #[should_panic(expected = "IncompatibleMissing(Number(-1.0))")]
    fn test_try_from_dvalue_u64_float() {
        u64::try_from_dvalue(DValue::from_f64(-1.0).unwrap()).unwrap();
    }

    #[test]
    fn test_try_from_dvalue_f32() {
        let result = f32::try_from_dvalue(DValue::from_f64(42.0).unwrap()).unwrap();
        assert_eq!(42.0, result);
    }

    #[test]
    fn test_try_from_dvalue_f32_int() {
        let result = f32::try_from_dvalue(42_u64.into()).unwrap();
        assert_eq!(42.0, result);
    }

    #[test]
    #[should_panic(expected = "IncompatibleMissing(Number(6.805646932770577e38))")]
    fn test_try_from_dvalue_f32_too_large() {
        f32::try_from_dvalue(DValue::from_f64((f32::max_value() as f64) * 2.0).unwrap()).unwrap();
    }

    #[test]
    fn test_try_from_dvalue_f64() {
        let result = f64::try_from_dvalue(DValue::from_f64(-42.0).unwrap()).unwrap();
        assert_eq!(-42.0, result);
    }

    #[test]
    fn test_try_from_dvalue_f64_int() {
        let result = f64::try_from_dvalue(42_u64.into()).unwrap();
        assert_eq!(42.0, result);
    }
}
