//! A library for reading and writing miniseed3.
//!
//! See the specification at <https://github.com/iris-edu/miniSEED3> or
//! and <https://miniseed3.readthedocs.io/en/latest/> for now, or
//! <https://docs.fdsn.org/projects/miniSEED3> once approved by FDSN

mod header;
mod data_encoding;
mod encoded_timeseries;
mod record;
mod mseed_error;

use std::io::BufRead;

pub use self::header::{MSeed3Header, FIXED_HEADER_SIZE};
pub use self::encoded_timeseries::EncodedTimeseries;
pub use self::data_encoding::DataEncoding;
pub use self::record::{CASTAGNOLI, MSeed3Record, ExtraHeaders};
pub use self::mseed_error::MSeedError;

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
