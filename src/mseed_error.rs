use chrono::ParseError;
use std::string::FromUtf8Error;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MSeedError {
    #[error("IO Error")]
    IOError(#[from] std::io::Error),
    #[error("Insufficient bytes, {0} < fixed header size {1}")]
    InsufficientBytes(usize, usize),
    #[error("CRC invalid for record: calc:{0:#X} header:{1:#X}")]
    CrcInvalid(u32, u32),
    #[error("Text not UTF8")]
    FromUtf8Error(#[from] FromUtf8Error),
    #[error("cannot parse extra headers")]
    JsonError(#[from] serde_json::Error),
    #[error("MSeed3 header must start with MS, (77, 83)  but was `{0}{1}`")]
    BadRecordIndicator(u8, u8),
    #[error("MSeed3 header format_version must be 3 but was `{0}`")]
    UnknownFormatVersion(u8),
    #[error("cannot parse {1} in FDSN source identifier `{0}`")]
    IdentifierParse(String, String),
    #[error("Unknown data encoding: `{0}`")]
    ExtraHeaderNotObject(serde_json::Value),
    #[error("MSeed3 extra header parse: `{0}`")]
    ExtraHeaderParse(String),
    #[error("Unknown data encoding: `{0}`")]
    UnknownEncoding(u8),
    #[error("Expected {0} bytes for {1} samples as encoding type {2} but header has data_length={3} bytes.",)]
    DataLength(u32, u32, u8, u32),
    #[error("Date parsing error: `{0}`")]
    ParseError(#[from] ParseError),
    #[error("MSeed3 compression/decompression error: `{0}`")]
    Compression(String),
    #[error("MSeed3 error: `{0}`")]
    Unknown(String),
}
