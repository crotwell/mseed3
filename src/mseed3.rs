// mod mseed3

use byteorder::{LittleEndian, WriteBytesExt};
use chrono::prelude::*;
use chrono::Utc;
use serde_json;
use std::convert::TryInto;
use std::fmt;
use std::fmt::Formatter;
use std::io::prelude::*;
use std::io::{ BufWriter};
use std::string::FromUtf8Error;
use thiserror::Error;
use crc::{Crc, CRC_32_ISCSI};

pub const CASTAGNOLI: Crc<u32> = Crc::<u32>::new(&CRC_32_ISCSI);

/// read a single little endian 64 bit float (8 bytes) and reset input
fn read_le_f64(input: &mut &[u8]) -> f64 {
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<f64>());
    *input = rest;
    f64::from_le_bytes(int_bytes.try_into().unwrap())
}

/// read a single little endian 32 bit float (4 bytes) and reset input
fn read_le_u32(input: &mut &[u8]) -> u32 {
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<u32>());
    *input = rest;
    u32::from_le_bytes(int_bytes.try_into().unwrap())
}

/// read a single little endian 16 bit int (2 bytes) and reset input
fn read_le_u16(input: &mut &[u8]) -> u16 {
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<u16>());
    *input = rest;
    u16::from_le_bytes(int_bytes.try_into().unwrap())
}

/// Size in bytes of the fixed header. This does not include the identifier, extra headers, or data.
pub const FIXED_HEADER_SIZE: usize = 40;

pub const CRC_OFFSET: usize = 28;

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
            DataEncoding::TEXT => write!(f, "Text, UTF-8 allowed, use ASCII for maximum portability, no structure defined"),
            DataEncoding::INT16 => write!(f, "16-bit integer (two’s complement), little endian byte order"),
            DataEncoding::INT32 => write!(f, "32-bit integer (two’s complement), little endian byte order"),
            DataEncoding::FLOAT32 => write!(f, "32-bit floats (IEEE float), little endian byte order"),
            DataEncoding::FLOAT64 => write!(f, "64-bit floats (IEEE double), little endian byte order"),
            DataEncoding::STEIM1 => write!(f, "Steim-1 integer compression, big endian byte order"),
            DataEncoding::STEIM2 => write!(f, "Steim-2 integer compression, big endian byte order"),
            DataEncoding::STEIM3 => write!(f, "Steim-3 integer compression, big endian (not in common use in archives)"),
            DataEncoding::OPAQUE => write!(f, "Opaque data - only for use in special scenarios, not intended for archiving"),
            DataEncoding::UNKNOWN(val) => write!(f, "Unknown encoding: {}", val),
        }
    }
}

/// The fixed section of the header. Does not contain the identifier, extra headers, or data.
#[derive(Debug, Clone)]
pub struct MSeed3Header {
    pub record_indicator: String,
    pub format_version: u8,
    pub flags: u8,
    pub nanosecond: u32,
    pub year: u16,
    pub day_of_year: u16,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub encoding: DataEncoding,
    pub sample_rate_period: f64,
    pub num_samples: u32,
    pub crc: u32,
    pub publication_version: u8,
    identifier_length: u8,
    extra_headers_length: u16,
    data_length: u32,
}

impl MSeed3Header {
    /// First two bytes of a miniseed3 header must be `MS`
    pub const REC_IND: [u8; 2] = [b'M', b'S'];
    /// The header field representing the length of the identifier. Note that this is the value
    /// at the time the record was created. If the identifier was changed this value may be
    /// wrong and will be recalculated on write.
    pub fn raw_identifier_length(&self) -> u8 {
        self.identifier_length
    }
    /// The header field representing the length of the extra headers string. Note that this is the value
    /// at the time the record was created. If the extra headers have been changed this value may be
    /// wrong and will be recalculated on write.
    pub fn raw_extra_headers_length(&self) -> u16 {
        self.extra_headers_length
    }
    /// The header field representing the length of the timeseries data. Note that this is the value
    /// at the time the record was created. If the data was changed this value may be
    /// wrong and will be recalculated on write.
    pub fn raw_data_length(&self) -> u32 {
        self.data_length
    }

    pub fn from_bytes(buffer: &[u8]) -> Result<MSeed3Header, MSeedError> {
        print!("read_mseed3_buf...");
        assert_eq!(&buffer[0..2], "MS".as_bytes());
        if buffer[0] != MSeed3Header::REC_IND[0] || buffer[1] != MSeed3Header::REC_IND[1] {
            return Err(MSeedError::BadRecordIndicator(buffer[0], buffer[1]));
        }
        let record_indicator = String::from("MS");
        let format_version = buffer[2];
        let flags = buffer[3];
        // skip M, S, format, flags
        let (_, mut header_bytes) = buffer.split_at(4);
        let nanosecond = read_le_u32(&mut header_bytes);
        let year = read_le_u16(&mut header_bytes);
        let day_of_year = read_le_u16(&mut header_bytes);
        let hour = buffer[12];
        let minute = buffer[13];
        let second = buffer[14];
        let encoding = DataEncoding::from_int(buffer[15]);
        let _ = read_le_u32(&mut header_bytes); // skip hour-encoding
        let sample_rate_period = read_le_f64(&mut header_bytes);
        let num_samples = read_le_u32(&mut header_bytes);
        let crc = read_le_u32(&mut header_bytes);
        let publication_version = buffer[32];
        let identifier_length = buffer[33];
        let _ = read_le_u16(&mut header_bytes); // skip pub ver and id len
        let extra_headers_length = read_le_u16(&mut header_bytes);
        let data_length = read_le_u32(&mut header_bytes);
        let ms3_header = MSeed3Header {
            record_indicator,
            format_version,
            flags,
            nanosecond,
            year,
            day_of_year,
            hour,
            minute,
            second,
            encoding,
            sample_rate_period,
            num_samples,
            crc,
            publication_version,
            identifier_length,
            extra_headers_length,
            data_length,
        };
        Ok(ms3_header)
    }

    pub fn write_to<W>(&self, buf: &mut BufWriter<W>) -> Result<(), MSeedError>
    where
        W: std::io::Write,
    {
        buf.write_all(&MSeed3Header::REC_IND)?;
        buf.write_all(&[self.format_version, self.flags])?;
        buf.write_u32::<LittleEndian>(self.nanosecond)?;
        buf.write_u16::<LittleEndian>(self.year)?;
        buf.write_u16::<LittleEndian>(self.day_of_year)?;
        buf.write_all(&[self.hour, self.minute, self.second, self.encoding.value()])?;
        buf.write_f64::<LittleEndian>(self.sample_rate_period)?;
        buf.write_u32::<LittleEndian>(self.num_samples)?;
        buf.write_u32::<LittleEndian>(self.crc)?;
        buf.write_all(&[self.publication_version, self.identifier_length])?;
        buf.write_u16::<LittleEndian>(self.extra_headers_length)?;
        buf.write_u32::<LittleEndian>(self.data_length)?;
        Ok(())
    }

    pub fn get_start_as_iso(&self) -> String {
        let start = Utc
            .yo(self.year as i32, self.day_of_year as u32)
            .and_hms_nano(
                self.hour as u32,
                self.minute as u32,
                self.second as u32,
                self.nanosecond,
            );

        //        start.format("%Y-%jT%H:%M:%S%.9fZ").to_string()
        start.format("%Y-%m-%dT%H:%M:%S%.9fZ").to_string()
    }

    pub fn crc_hex_string(&self) -> String {
        //        format!("{:#010X}", self.crc)
        format!("{:#X}", self.crc)
    }

    pub fn get_record_size(&self) -> u32 {
        FIXED_HEADER_SIZE as u32
            + self.identifier_length as u32
            + self.extra_headers_length as u32
            + self.data_length
    }
}

#[derive(Debug, Clone)]
pub enum ExtraHeaders {
    Raw(String),
    Parsed(serde_json::Value),
}

#[derive(Debug, Clone)]
pub enum EncodedTimeseries {
    Raw(Vec<u8>),
    Int16(Vec<i16>),
    Int32(Vec<i32>),
    Float32(Vec<f32>),
    Float64(Vec<f64>),
    Steim1(Vec<u8>),
    Steim2(Vec<u8>),
    Steim3(Vec<u8>),
    Opaque(Vec<u8>),
}

impl EncodedTimeseries {
    pub fn write_to<W>(&self, buf: &mut BufWriter<W>) -> Result<(), MSeedError>
    where
        W: std::io::Write,
    {
        match self {
            EncodedTimeseries::Raw(v) => {
                buf.write_all(v)?;
                Ok(())
            }
            EncodedTimeseries::Int16(v) => {
                for &el in v {
                    buf.write_i16::<LittleEndian>(el)?;
                }
                Ok(())
            }
            EncodedTimeseries::Int32(v) => {
                for &el in v {
                    buf.write_i32::<LittleEndian>(el)?;
                }
                Ok(())
            }
            EncodedTimeseries::Float32(v) => {
                for &el in v {
                    buf.write_f32::<LittleEndian>(el)?;
                }
                Ok(())
            }
            EncodedTimeseries::Float64(v) => {
                for &el in v {
                    buf.write_f64::<LittleEndian>(el)?;
                }
                Ok(())
            }
            EncodedTimeseries::Steim1(v) => {
                buf.write_all(v)?;
                Ok(())
            }
            EncodedTimeseries::Steim2(v) => {
                buf.write_all(v)?;
                Ok(())
            }
            EncodedTimeseries::Steim3(v) => {
                buf.write_all(v)?;
                Ok(())
            }
            EncodedTimeseries::Opaque(v) => {
                buf.write_all(v)?;
                Ok(())
            }
        }
    }

    pub fn byte_len(&self) -> u32 {
        match self {
            EncodedTimeseries::Raw(v) => v.len() as u32,
            EncodedTimeseries::Int16(v) => 2 * v.len() as u32,
            EncodedTimeseries::Int32(v) => 4 * v.len() as u32,
            EncodedTimeseries::Float32(v) => 4 * v.len() as u32,
            EncodedTimeseries::Float64(v) => 8 * v.len() as u32,
            EncodedTimeseries::Steim1(v) => v.len() as u32,
            EncodedTimeseries::Steim2(v) => v.len() as u32,
            EncodedTimeseries::Steim3(v) => v.len() as u32,
            EncodedTimeseries::Opaque(v) => v.len() as u32,
        }
    }
    /// Reconciles the number of samples in the header with the size of the EncodedTimeseries.
    /// For the primitive types, Int16, Int32, Float32 and Float64 the value is calculated from
    /// the length of the array. For the remaining, the passed in header num_samples is
    /// return as it is assumed to be correct.
    pub fn reconcile_num_samples(&self, header_num_sample: u32) -> u32 {
        match self {
            EncodedTimeseries::Int16(v) => v.len() as u32,
            EncodedTimeseries::Int32(v) => v.len() as u32,
            EncodedTimeseries::Float32(v) => v.len() as u32,
            EncodedTimeseries::Float64(v) => v.len() as u32,
            EncodedTimeseries::Raw(_) => header_num_sample,
            EncodedTimeseries::Steim1(_) => header_num_sample,
            EncodedTimeseries::Steim2(_) => header_num_sample,
            EncodedTimeseries::Steim3(_) => header_num_sample,
            EncodedTimeseries::Opaque(_) => header_num_sample,
        }
    }
}

impl fmt::Display for EncodedTimeseries {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            EncodedTimeseries::Raw(v) => {
                write!(f, "Raw bytes, {} bytes", v.len())
            }
            EncodedTimeseries::Int16(v) => {
                write!(f, "Int16, {} samples", v.len())
            }
            EncodedTimeseries::Int32(v) => {
                write!(f, "Int32, {} samples", v.len())
            }
            EncodedTimeseries::Float32(v) => {
                write!(f, "Float32, {} samples", v.len())
            }
            EncodedTimeseries::Float64(v) => {
                write!(f, "Float64, {} samples", v.len())
            }
            EncodedTimeseries::Steim1(v) => {
                write!(f, "Steim1, {} bytes", v.len())
            }
            EncodedTimeseries::Steim2(v) => {
                write!(f, "Steim2, {} bytes", v.len())
            }
            EncodedTimeseries::Steim3(v) => {
                write!(f, "Steim3, {} bytes", v.len())
            }
            EncodedTimeseries::Opaque(v) => {
                write!(f, "Opaque, {} bytes", v.len())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct MSeed3Record {
    pub header: MSeed3Header,
    pub identifier: String,
    pub extra_headers: ExtraHeaders,
    pub encoded_data: EncodedTimeseries,
}

impl MSeed3Record {
    pub fn new(
        header: MSeed3Header,
        identifier: String,
        extra_headers: ExtraHeaders,
        encoded_data: EncodedTimeseries,
    ) -> MSeed3Record {
        let mut header = header;
        header.identifier_length = identifier.len() as u8;
        match &extra_headers {
            ExtraHeaders::Raw(v) => header.extra_headers_length = v.len() as u16,
            _ => header.extra_headers_length = 0,
        }
        header.data_length = encoded_data.byte_len();

        MSeed3Record {
            header,
            identifier,
            extra_headers,
            encoded_data,
        }
    }

    pub fn from_reader<R: BufRead>(buf_reader: &mut R) -> Result<MSeed3Record, MSeedError> {
        let mut buffer = [0; FIXED_HEADER_SIZE];
        let _ = buf_reader.by_ref().take(FIXED_HEADER_SIZE as u64).read(&mut buffer)?;
        let mut header = MSeed3Header::from_bytes(&buffer)?;
        let mut buffer = Vec::new();
        let _ = buf_reader
            .by_ref()
            .take(header.identifier_length as u64)
            .read_to_end(&mut buffer)?;
        let id_result = String::from_utf8(buffer);
        let identifier = match id_result {
            Ok(id) => id,
            Err(e) => return Err(MSeedError::FromUtf8Error(e)),
        };
        let extra_headers_str: String;
        if header.extra_headers_length > 2 {
            let mut json_reader = buf_reader.by_ref().take(header.extra_headers_length as u64);
            let mut buffer = Vec::new();
            let _ = json_reader.read_to_end(&mut buffer)?;
            extra_headers_str = String::from_utf8(buffer)?;
        } else {
            extra_headers_str = String::from("{}");
        }
        let expected_data_length = match header.encoding {
            DataEncoding::INT16 => 2*header.num_samples,
            DataEncoding::INT32 => 4*header.num_samples,
            DataEncoding::FLOAT32 => 4*header.num_samples,
            DataEncoding::FLOAT64 => 8*header.num_samples,
            _ => header.data_length,
        };
        if header.data_length != expected_data_length {
            return Err(MSeedError::DataLength(expected_data_length, header.num_samples, header.encoding.value(), header.data_length));
        }

        let mut encoded_data = Vec::new();
        let _ = buf_reader
            .by_ref()
            .take(header.data_length as u64)
            .read_to_end(&mut encoded_data)?;
        let encoded_data = EncodedTimeseries::Raw(encoded_data);
        header.num_samples = encoded_data.reconcile_num_samples(header.num_samples);
        Ok(MSeed3Record {
            header,
            identifier,
            extra_headers: ExtraHeaders::Raw(extra_headers_str),
            encoded_data,
        })
    }

    /// Writes the record, after calculating the CRC. The returned tuple contains the number
    /// of bytes written and the CRC value.
    /// This does recalculate the identifier length, extra headers length and data length headers.
    /// The number of samples is sanity checked against the data, but trusts the header in cases
    /// of compressed or opaque data.
    pub fn write_to<W>(&mut self, buf: &mut BufWriter<W>) -> Result<(u32, u32), MSeedError>
        where
            W: std::io::Write,
    {
        self.header.crc = 0;
        let mut out = Vec::new();
        {
            let mut inner_buf = BufWriter::new(&mut out);
            self.write_to_wocrc(&mut inner_buf)?;
            inner_buf.flush()?;
        }
        let crc = CASTAGNOLI.checksum(&out);
        self.header.crc = crc;
        buf.write_all(&out[0..CRC_OFFSET])?;
        buf.write_u32::<LittleEndian>(crc)?;
        buf.write_all(&out[(CRC_OFFSET+4)..])?;
        Ok((out.len() as u32, crc))
    }

    /// Writes the record to the given buffer without checking, calculating or setting the header CRC field.
    /// This does recalculate the identifier length, extra headers length and data length headers.
    /// The number of samples is sanity checked against the data, but trusts the header in cases
    /// of compressed or opaque data.
    pub fn write_to_wocrc<W>(&mut self, buf: &mut BufWriter<W>) -> Result<(), MSeedError>
    where
        W: std::io::Write,
    {
        let id_bytes = self.identifier.as_bytes();
        self.header.identifier_length = id_bytes.len() as u8;
        self.header.data_length = self.encoded_data.byte_len();
        self.header.num_samples = self.encoded_data.reconcile_num_samples(self.header.num_samples);

        let mut eh_bytes = Vec::new();
        match &self.extra_headers {
            ExtraHeaders::Parsed(eh) => eh_bytes.write_all(eh.to_string().as_bytes())?,
            ExtraHeaders::Raw(s) => eh_bytes.write_all(s.as_bytes())?,
        };
        if eh_bytes.len() > 2 {
            self.header.extra_headers_length = eh_bytes.len() as u16;
        } else {
            self.header.extra_headers_length = 0;
        }
        self.header.write_to(buf)?;
        buf.write_all(id_bytes)?;
        if eh_bytes.len() > 2 {
            // don't write bytes for empty object, e.g. `{}`
            buf.write_all(&eh_bytes)?;
        }
        self.encoded_data.write_to(buf)?;
        buf.flush()?;
        Ok(())
    }

    pub fn parse_extra_headers(&mut self) -> Result<(), MSeedError> {
        match &mut self.extra_headers {
            ExtraHeaders::Parsed(_) => Ok(()),
            ExtraHeaders::Raw(eh_str) => {
                let eh_json = serde_json::from_str(eh_str)?;
                let eh_parsed = ExtraHeaders::Parsed(eh_json);
                self.extra_headers = eh_parsed;
                Ok(())
            }
        }
    }

    pub fn parsed_json(&mut self) -> Result<&serde_json::Value, MSeedError> {
        self.parse_extra_headers()?;
        if let ExtraHeaders::Parsed(eh) = &self.extra_headers {
            return Ok(eh);
        }
        Err(MSeedError::ExtraHeaderParse(String::from(
            "unable to parse extra headers",
        )))
    }

    pub fn get_record_size(&self) -> u32 {
        self.header.get_record_size()
    }
}

impl fmt::Display for MSeed3Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // FDSN:CO_HODGE_00_L_H_Z, version 4, 477 bytes (format: 3)
        //          start time: 2019,187,03:19:53.000000
        //   number of samples: 255
        //    sample rate (Hz): 1
        //               flags: [00000000] 8 bits
        //                 CRC: 0x8926FFDF
        // extra header length: 31 bytes
        // data payload length: 384 bytes
        //    payload encoding: STEIM-2 integer compression (val: 11)
        //       extra headers:
        //             "FDSN": {
        //               "Time": {
        //                 "Quality": 0
        //               }
        //             }

        let encode_name = self.encoding.to_string();
        let lines = [
            format!(
                "version ${}, ${} bytes (format: ${})\n",
                self.publication_version,
                self.get_record_size(),
                self.format_version
            ),
            format!("             start time: ${}\n", self.get_start_as_iso()),
            format!("      number of samples: ${}\n", self.num_samples),
            format!("       sample rate (Hz): ${}\n", self.sample_rate_period),
            format!("                  flags: [${:#08b}] 8 bits\n", self.flags),
            format!("                    CRC: ${}\n", self.crc_hex_string()),
            format!(
                "    extra header length: ${} bytes\n",
                self.extra_headers_length
            ),
            format!("    data payload length: ${} bytes\n", self.data_length),
            format!(
                "       payload encoding: ${encode_name} (val: ${encoding})",
                encode_name = encode_name,
                encoding = self.encoding
            ),
        ];
        let line = "";
        for l in lines {
            format!("{}{}", line, l);
        }
        write!(f, "{}", line)
    }
}

impl fmt::Display for MSeed3Record {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "  {}, {}", self.identifier, self.header)
    }
}

#[derive(Error, Debug)]
pub enum MSeedError {
    #[error("IO Error")]
    IOError(#[from] std::io::Error),
    #[error("Text not UTF8")]
    FromUtf8Error(#[from] FromUtf8Error),
    #[error("cannot parse extra headers")]
    JsonError(#[from] serde_json::Error),
    #[error("MSeed3 header must start with MS, (77, 83)  but was `{0}{1}`")]
    BadRecordIndicator(u8, u8),
    #[error("MSeed3 extra header must be object  but was `{0}`")]
    ExtraHeaderNotObject(serde_json::Value),
    #[error("MSeed3 extra header parse: `{0}`")]
    ExtraHeaderParse(String),
    #[error("Unknown data encoding: `{0}`")]
    UnknownEncoding(u8),
    #[error("Expected {0} bytes for {1} samples as encoding type {2} but header has data_length={3} bytes.",)]
    DataLength(u32, u32, u8, u32 ),
    #[error("MSeed3 error: `{0}`")]
    Unknown(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_u32_buf() {
        let buf: [u8; 5] = [1, 0, 0, 0, 5];
        let mut header_bytes = &buf[0..5];
        let nanosecond = read_le_u32(&mut header_bytes);
        assert_eq!(1, nanosecond);
        assert_eq!(header_bytes[0], 5);
    }

    #[test]
    fn read_f64_buf() {
        let buf: [u8; 8] = [0, 0, 0, 0, 0, 0, 0xf0, 0x3f];
        let mut header_bytes = &buf[0..8];
        let nanosecond = read_le_f64(&mut header_bytes);
        assert_eq!(1.0 as f64, nanosecond);
    }

    fn get_dummy_header() -> [u8; 64] {
        // 00000000  4d 53 03 04 00 00 00 00  dc 07 01 00 00 00 00 01  |MS..............|
        // 00000010  00 00 00 00 00 00 f0 3f  f4 01 00 00 89 73 2b 64  |.......?.....s+d|
        // 00000020  01 14 00 00 e8 03 00 00  58 46 44 53 4e 3a 58 58  |........XFDSN:XX|
        // 00000030  5f 54 45 53 54 5f 5f 4c  5f 48 5f 5a 00 00 02 00  |_TEST__L_H_Z....|

        // XFDSN:XX_TEST__L_H_Z, version 1, 1060 bytes (format: 3)
        //  start time: 2012,001,00:00:00.000000
        //      number of samples: 500
        //    sample rate (Hz): 1
        //               flags: [00100000] 8 bits
        //                      [Bit 2] Clock locked
        //                 CRC: 0x642B7389
        // extra header length: 0 bytes
        // data payload length: 1000 bytes
        //    payload encoding: 16-bit integer (val: 1)

        let buf: [u8; 64] = [
            0x4d, 0x53, 0x03, 0x04, 0x00, 0x00, 0x00, 0x00, 0xdc, 0x07, 0x01, 0x00, 0x00, 0x00,
            0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0x3f, 0xf4, 0x01, 0x00, 0x00,
            0x89, 0x73, 0x2b, 0x64, 0x01, 0x14, 0x00, 0x00, 0xe8, 0x03, 0x00, 0x00, 0x58, 0x46,
            0x44, 0x53, 0x4e, 0x3a, 0x58, 0x58, 0x5f, 0x54, 0x45, 0x53, 0x54, 0x5f, 0x5f, 0x4c,
            0x5f, 0x48, 0x5f, 0x5a, 0x00, 0x00, 0x02, 0x00,
        ];
        buf
    }

    #[test]
    fn read_header_sin_int16() {
        let buf = get_dummy_header();
        print!("read_header_sin_int16...");
        let head = MSeed3Header::from_bytes(&buf).unwrap();
        assert_eq!(head.record_indicator, "MS");
        assert_eq!(head.format_version, 3);
        assert_eq!(head.flags, 4);
        assert_eq!(head.nanosecond, 0);
        assert_eq!(head.year, 2012);
        assert_eq!(head.day_of_year, 1);
        assert_eq!(head.hour, 0);
        assert_eq!(head.minute, 0);
        assert_eq!(head.second, 0);
        assert_eq!(head.encoding.value(), 1);
        assert_eq!(head.sample_rate_period, 1.0 as f64);
        assert_eq!(head.num_samples, 500);
        assert_eq!(head.crc, 0x642B7389);
        assert_eq!(head.publication_version, 1 as u8);
        assert_eq!(
            head.identifier_length,
            String::from("XFDSN:XX_TEST__L_H_Z").len() as u8
        );
        assert_eq!(head.extra_headers_length, 0 as u16);
        assert_eq!(head.data_length, 1000);
        print!("{}", head);
    }

    #[test]
    fn read_header_round_trip() {
        let buf = &get_dummy_header()[0..FIXED_HEADER_SIZE];
        let head = MSeed3Header::from_bytes(buf).unwrap();
        let mut out = Vec::new();
        {
            let mut buf_writer = BufWriter::new(&mut out);
            head.write_to(&mut buf_writer).unwrap();
            buf_writer.flush().unwrap();
        }
        assert_eq!(out, buf);
        assert_eq!(out[0..2], MSeed3Header::REC_IND);
        assert_eq!(buf[0..2], MSeed3Header::REC_IND);
    }

    #[test]
    fn record_round_trip() {
        let buf = &get_dummy_header()[0..FIXED_HEADER_SIZE];
        let identifier =
            String::from_utf8(get_dummy_header()[FIXED_HEADER_SIZE..64].to_owned()).unwrap();

        let mut head = MSeed3Header::from_bytes(buf).unwrap();
        let original_head = MSeed3Header::from_bytes(buf).unwrap();
        head.identifier_length = identifier.len() as u8;
        let dummy_eh = String::from("");
        head.extra_headers_length = dummy_eh.len() as u16;
        let extra_headers = ExtraHeaders::Raw(dummy_eh);
        let dummy_data = vec![0, -1, 2, -3, 4, -5];
        head.data_length = (dummy_data.len() as u32 * 4) as u32;
        head.num_samples = dummy_data.len() as u32;
        head.encoding = DataEncoding::INT32;
        let encoded_data = EncodedTimeseries::Int32(dummy_data);
        let mut rec = MSeed3Record::new(head, identifier, extra_headers, encoded_data);
        let mut out = Vec::new();
        let bytes_written: u32;
        let crc_written: u32;
        {
            let mut buf_writer = BufWriter::new(&mut out);
            let t = rec.write_to(&mut buf_writer).unwrap();
            bytes_written = t.0;
            crc_written = t.1;
            buf_writer.flush().unwrap();
        }
        assert_eq!(rec.get_record_size(), out.len() as u32);
        assert_eq!(bytes_written, out.len() as u32);
    }
}
