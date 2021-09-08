
use std::fmt;
use std::fmt::Formatter;

/// Known data compression codes.
/// ```text
/// 0   Text, UTF-8 allowed, use ASCII for maximum portability, no structure defined
/// 1   16-bit integer (two’s complement), little endian byte order
/// 3   32-bit integer (two’s complement), little endian byte order
/// 4   32-bit floats (IEEE float), little endian byte order
/// 5   64-bit floats (IEEE double), little endian byte order
/// 10  Steim-1 integer compression, big endian byte order
/// 11  Steim-2 integer compression, big endian byte order
/// 19  Steim-3 integer compression, big endian (not in common use in archives)
/// 100 Opaque data - only for use in special scenarios, not intended for archiving
/// ```
#[derive(Debug, Clone)]
pub enum DataEncoding {
    TEXT,
    INT16,
    INT32,
    FLOAT32,
    FLOAT64,
    STEIM1,
    STEIM2,
    STEIM3,
    OPAQUE,
    UNKNOWN(u8),
}

impl DataEncoding {
    /// Creates a DataEncoding based on the input integer
    pub fn from_int(val: u8) -> DataEncoding {
        match val {
            0 => DataEncoding::TEXT,
            1 => DataEncoding::INT16,
            3 => DataEncoding::INT32,
            4 => DataEncoding::FLOAT32,
            5 => DataEncoding::FLOAT64,
            10 => DataEncoding::STEIM1,
            11 => DataEncoding::STEIM2,
            19 => DataEncoding::STEIM3,
            100 => DataEncoding::OPAQUE,
            _ => DataEncoding::UNKNOWN(val),
        }
    }
    /// The integer value, as a u8, of the encoding
    pub fn value(&self) -> u8 {
        match &self {
            DataEncoding::TEXT => 0,
            DataEncoding::INT16 => 1,
            DataEncoding::INT32 => 3,
            DataEncoding::FLOAT32 => 4,
            DataEncoding::FLOAT64 => 5,
            DataEncoding::STEIM1 => 10,
            DataEncoding::STEIM2 => 11,
            DataEncoding::STEIM3 => 19,
            DataEncoding::OPAQUE => 100,
            DataEncoding::UNKNOWN(val) => *val,
        }
    }
}

impl fmt::Display for DataEncoding {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            DataEncoding::TEXT => write!(
                f,
                "Text, UTF-8 allowed, use ASCII for maximum portability, no structure defined"
            ),
            DataEncoding::INT16 => write!(
                f,
                "16-bit integer (two’s complement), little endian byte order"
            ),
            DataEncoding::INT32 => write!(
                f,
                "32-bit integer (two’s complement), little endian byte order"
            ),
            DataEncoding::FLOAT32 => {
                write!(f, "32-bit floats (IEEE float), little endian byte order")
            }
            DataEncoding::FLOAT64 => {
                write!(f, "64-bit floats (IEEE double), little endian byte order")
            }
            DataEncoding::STEIM1 => write!(f, "Steim-1 integer compression, big endian byte order"),
            DataEncoding::STEIM2 => write!(f, "Steim-2 integer compression, big endian byte order"),
            DataEncoding::STEIM3 => write!(
                f,
                "Steim-3 integer compression, big endian (not in common use in archives)"
            ),
            DataEncoding::OPAQUE => write!(
                f,
                "Opaque data - only for use in special scenarios, not intended for archiving"
            ),
            DataEncoding::UNKNOWN(val) => write!(f, "Unknown encoding: {}", val),
        }
    }
}
