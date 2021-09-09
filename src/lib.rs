//! A library for reading and writing miniseed3.
//!
//! See the specification at <https://github.com/iris-edu/miniSEED3> or
//! and <https://miniseed3.readthedocs.io/en/latest/> for now, or
//! <https://docs.fdsn.org/projects/miniSEED3> once approved by FDSN.
//!
//! # Example
//!
//! Create a record from data in memory:
//!
//! ```
//! # use mseed3::MSeedError;
//! # use std::io::Write;
//! # fn main() -> Result<(), MSeedError> {
//! # use chrono::{DateTime, Utc};
//! # use mseed3::{DataEncoding, EncodedTimeseries, ExtraHeaders, MSeedError};
//! let start = "2014-11-28T12:00:09Z".parse::<DateTime<Utc>>()?;
//! let timeseries = vec![0, 1, -1, 5, 3, -5, 10, -1, 1, 0];
//! let num_samples = timeseries.len();
//! let encoded_data = EncodedTimeseries::Int32(timeseries);
//! let header = mseed3::MSeed3Header::new(start, DataEncoding::INT32, 10.0, num_samples);
//! let identifier = String::from("FDSN:CO_BIRD_00_H_H_Z");
//! let extra_headers = ExtraHeaders::Raw(String::from("{}"));
//! let record = mseed3::MSeed3Record::new(header, identifier, extra_headers, encoded_data);
//! # Ok(())
//! # }
//! ```
//!
//! and then print it
//!
//!
//! ```
//! # use mseed3::MSeedError;
//! # use std::io::Write;
//! # fn main() -> Result<(), MSeedError> {
//! # use chrono::{DateTime, Utc};
//! # use mseed3::{DataEncoding, EncodedTimeseries, ExtraHeaders, MSeedError};
//! # let start = "2014-11-28T12:00:09Z".parse::<DateTime<Utc>>()?;
//! # let timeseries = vec![0, 1, -1, 5, 3, -5, 10, -1, 1, 0];
//! # let num_samples = timeseries.len();
//! # let encoded_data = EncodedTimeseries::Int32(timeseries);
//! # let header = mseed3::MSeed3Header::new(start, DataEncoding::INT32, 10.0, num_samples);
//! # let identifier = String::from("FDSN:CO_BIRD_00_H_H_Z");
//! # let extra_headers = ExtraHeaders::Raw(String::from("{}"));
//! # let mut record = mseed3::MSeed3Record::new(header, identifier, extra_headers, encoded_data);
//! println!("{}", record);
//! # Ok(())
//! # }
//! ```
//! ```text
//! Record:
//!   FDSN:CO_BIRD_00_H_H_Z, version 0, 101 bytes (format: 3)
//!              start time: 2014-11-28T12:00:09.000000000Z
//!       number of samples: 10
//!        sample rate (Hz): 10
//!                   flags: [0b00000000] 8 bits
//!                     CRC: 0x29ED3283
//!     extra header length: 0 bytes
//!     data payload length: 40 bytes
//!        payload encoding: 32-bit integer (two’s complement), little endian byte order (val: 32-bit integer (two’s complement), little endian byte order)
//! ```
//! write out to a file:
//! ```
//! # use mseed3::MSeedError;
//! # use std::io::Write;
//! # fn main() -> Result<(), MSeedError> {
//! # use chrono::{DateTime, Utc};
//! # use mseed3::{DataEncoding, EncodedTimeseries, ExtraHeaders, MSeedError};
//! # let start = "2014-11-28T12:00:09Z".parse::<DateTime<Utc>>()?;
//! # let timeseries = vec![0, 1, -1, 5, 3, -5, 10, -1, 1, 0];
//! # let num_samples = timeseries.len();
//! # let encoded_data = EncodedTimeseries::Int32(timeseries);
//! # let header = mseed3::MSeed3Header::new(start, DataEncoding::INT32, 10.0, num_samples);
//! # let identifier = String::from("FDSN:CO_BIRD_00_H_H_Z");
//! # let extra_headers = ExtraHeaders::Raw(String::from("{}"));
//! # let mut record = mseed3::MSeed3Record::new(header, identifier, extra_headers, encoded_data);
//!
//!     let outfile = std::fs::File::create("simple.ms3")?;
//!     let mut buf_writer = std::io::BufWriter::new(outfile);
//!     record.write_to(&mut buf_writer)?; // writing a record mut's the header to fix crc, and the byte lengths
//!     buf_writer.flush()?;
//!
//! # Ok(())
//! # }
//! ```
//! read it back in and print
//! ```
//! # use mseed3::MSeedError;
//! # use std::io::Write;
//! # fn main() -> Result<(), MSeedError> {
//! # use chrono::{DateTime, Utc};
//! # use mseed3::{DataEncoding, EncodedTimeseries, ExtraHeaders, MSeedError};
//! # let start = "2014-11-28T12:00:09Z".parse::<DateTime<Utc>>()?;
//! # let timeseries = vec![0, 1, -1, 5, 3, -5, 10, -1, 1, 0];
//! # let num_samples = timeseries.len();
//! # let encoded_data = EncodedTimeseries::Int32(timeseries);
//! # let header = mseed3::MSeed3Header::new(start, DataEncoding::INT32, 10.0, num_samples);
//! # let identifier = String::from("FDSN:CO_BIRD_00_H_H_Z");
//! # let extra_headers = ExtraHeaders::Raw(String::from("{}"));
//! # let mut record = mseed3::MSeed3Record::new(header, identifier, extra_headers, encoded_data);
//!
//!    # let outfile = std::fs::File::create("simple.ms3")?;
//!    # let mut buf_writer = std::io::BufWriter::new(outfile);
//!    # record.write_to(&mut buf_writer)?; // writing a record mut's the header to fix crc, and the byte lengths
//!    # buf_writer.flush()?;
//!
//!    let my_mseed3_file = std::fs::File::open("simple.ms3").unwrap();
//!    let mut buf_reader = std::io::BufReader::new(my_mseed3_file);
//!    let records = mseed3::read_mseed3(&mut buf_reader)?;
//!    let first_record = records.first().unwrap();
//!    print!("Read back in: \n{}", first_record);
//!
//! # Ok(())
//! # }
//! ```
//! ```text
//! Read back in:
//!   FDSN:CO_BIRD_00_H_H_Z, version 0, 101 bytes (format: 3)
//!              start time: 2014-11-28T12:00:09.000000000Z
//!       number of samples: 10
//!        sample rate (Hz): 10
//!                   flags: [0b00000000] 8 bits
//!                     CRC: 0x29ED3283
//!     extra header length: 0 bytes
//!     data payload length: 40 bytes
//!        payload encoding: 32-bit integer (two’s complement), little endian byte order (val: 32-bit integer (two’s complement), little endian byte order)
//!```
//!
//!

mod data_encoding;
mod encoded_timeseries;
mod fdsn_source_identifier;
mod header;
mod mseed_error;
mod record;

use std::io::BufRead;

pub use self::data_encoding::DataEncoding;
pub use self::encoded_timeseries::EncodedTimeseries;
pub use self::fdsn_source_identifier::FdsnSourceIdentifier;
pub use self::header::{MSeed3Header, FIXED_HEADER_SIZE};
pub use self::mseed_error::MSeedError;
pub use self::record::{ExtraHeaders, MSeed3Record, CASTAGNOLI};

/// Read miniseed3 records from a BufReader.
///
/// #Example
///
/// ```
/// # use mseed3::MSeedError;
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
