use serde::Deserialize;

#[cfg(target_endian = "big")]
pub const NATIVE_BYTE_ORDER: ByteOrder = ByteOrder::Big;

#[cfg(target_endian = "little")]
pub const NATIVE_BYTE_ORDER: ByteOrder = ByteOrder::Little;

#[cfg(target_endian = "big")]
pub const NON_NATIVE_BYTE_ORDER: ByteOrder = ByteOrder::Little;

#[cfg(target_endian = "little")]
pub const NON_NATIVE_BYTE_ORDER: ByteOrder = ByteOrder::Big;

/// Byte order / endianness.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ByteOrder {
    /// Big Endian
    Big,
    /// Little Endian
    Little,
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json;

    #[test]
    fn test_native_byte_order() {
        assert_ne!(NATIVE_BYTE_ORDER, NON_NATIVE_BYTE_ORDER);
    }

    #[test]
    fn test_deserialise() {
        let little: ByteOrder = serde_json::from_str(r#""little""#).unwrap();
        assert_eq!(ByteOrder::Little, little);
        let big: ByteOrder = serde_json::from_str(r#""big""#).unwrap();
        assert_eq!(ByteOrder::Big, big);
    }
}
