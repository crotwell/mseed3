//! A library for reading and writing miniseed3.
//!
//! See the specification at <https://github.com/iris-edu/miniSEED3> or
//! <https://docs.fdsn.org/projects/miniSEED3> once approved by FDSN
mod mseed3;
use std::io::{BufRead};

pub use self::mseed3::{MSeed3Record, MSeedError, MSeed3Header, DataEncoding, ExtraHeaders, EncodedTimeseries, FIXED_HEADER_SIZE};

/// Read miniseed3 records from a BufReader.
///
/// #Example
///
/// ```
/// use mseed3::MSeedError;
/// # fn main() -> Result<(), MSeedError> {
/// # let my_mseed3_file = std::fs::File::open("tests/reference-data/reference-sinusoid-int32.xseed").unwrap();
/// let mut buf_reader = std::io::BufReader::new(my_mseed3_file);
/// let records = mseed3::read_mseed3(&mut buf_reader);
/// # Ok(())
/// # }
/// ```
///
pub fn read_mseed3<R: BufRead>(buf_reader: &mut R) -> Result<Vec<MSeed3Record>, MSeedError> {
    let mut records: Vec<MSeed3Record> = Vec::new();
    while !buf_reader.fill_buf()?.is_empty() {
        let result = MSeed3Record::from_reader(&mut buf_reader.by_ref());
        match result {
            Ok(rec) => {
                records.push(rec);
            }
            Err(e) => return Err(e),
        }
    }
    Ok(records)
}