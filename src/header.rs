use byteorder::{LittleEndian, WriteBytesExt};
use chrono::prelude::*;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::io::prelude::*;
use std::io::BufWriter;

use crate::data_encoding::DataEncoding;
use crate::mseed_error::MSeedError;

/// Size in bytes of the fixed header. This does not include the identifier, extra headers, or data.
pub const FIXED_HEADER_SIZE: usize = 40;

/// Offset to the 4-byte CRC within the header.
pub const CRC_OFFSET: usize = 28;

/// The fixed section of the header. Does not contain the identifier, extra headers, or timeseries data.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MSeed3Header {
    pub record_indicator: [u8; 2],
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

    /// Resets the values that may have changed since the record was deserialized, for example
    /// due to modifying the data or adding extra headers.
    pub fn recalculated_lengths(
        &mut self,
        identifier_length: u8,
        extra_headers_length: u16,
        data_length: u32,
        num_samples: u32,
    ) {
        self.identifier_length = identifier_length;
        self.extra_headers_length = extra_headers_length;
        self.data_length = data_length;
        self.num_samples = num_samples;
    }

    /// Create a minimal new header with time, data encoding, sample rate and number of samples.
    /// All other fields are set to sensible defaults or zero.
    pub fn new(
        start: DateTime<Utc>,
        encoding: DataEncoding,
        sample_rate_period: f64,
        num_samples: usize,
    ) -> MSeed3Header {
        let mut header = MSeed3Header {
            record_indicator: MSeed3Header::REC_IND,
            format_version: 3_u8,
            flags: 0_u8,
            nanosecond: 0,
            year: 2000,
            day_of_year: 0,
            hour: 0,
            minute: 0,
            second: 0,
            encoding,
            sample_rate_period,
            num_samples: num_samples as u32,
            crc: 0,
            publication_version: 0,
            identifier_length: 0,
            extra_headers_length: 0,
            data_length: 0,
        };
        header.set_start_from_utc(start);
        header
    }

    /// Writes a miniseed3 header to a BufWriter.
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

    /// Start time as DateTime struct.
    pub fn get_start_as_utc(&self) -> DateTime<Utc> {
        Utc.yo(self.year as i32, self.day_of_year as u32)
            .and_hms_nano(
                self.hour as u32,
                self.minute as u32,
                self.second as u32,
                self.nanosecond,
            )
    }

    pub fn set_start_from_utc(&mut self, start: DateTime<Utc>) {
        let date = start.date();
        let time = start.time();

        self.nanosecond = time.nanosecond() % 1_000_000_000;
        self.year = date.year() as u16;
        self.day_of_year = date.ordinal() as u16;
        self.hour = time.hour() as u8;
        self.minute = time.minute() as u8;
        self.second = (time.second() + time.nanosecond() / 1_000_000_000) as u8;
    }

    /// Start time as ISO8601 string
    pub fn get_start_as_iso(&self) -> String {
        let start = self.get_start_as_utc();
        //        start.format("%Y-%jT%H:%M:%S%.9fZ").to_string()
        start.format("%Y-%m-%dT%H:%M:%S%.9fZ").to_string()
    }

    /// Format CRC as a hex string, like 0x106EAFA5
    pub fn crc_hex_string(&self) -> String {
        //        format!("{:#010X}", self.crc) // I like this style as shows it is a 32 bit number
        format!("{:#0X}", self.crc) // but mseed3-utils from Chad does this
    }

    /// The size of the data record, including the identifier, extra headers and data. Note that
    /// this uses header values set on read, and so if any of these have changed, this value
    /// will be wrong.
    pub fn get_record_size(&self) -> u32 {
        FIXED_HEADER_SIZE as u32
            + self.identifier_length as u32
            + self.extra_headers_length as u32
            + self.data_length
    }
}

impl TryFrom<&[u8]> for MSeed3Header {
    type Error = MSeedError;

    /// Convert byte array to MSeed3Header, error if fewer than FIXED_HEADER_SIZE bytes
    fn try_from(buffer: &[u8]) -> Result<Self, Self::Error> {
        if buffer.len() < FIXED_HEADER_SIZE {
            return Err(MSeedError::InsufficientBytes(
                buffer.len(),
                FIXED_HEADER_SIZE,
            ));
        }
        let bufslice: &[u8; FIXED_HEADER_SIZE] = &buffer.try_into().unwrap();
        MSeed3Header::try_from(bufslice)
    }
}

impl TryFrom<&[u8; FIXED_HEADER_SIZE]> for MSeed3Header {
    type Error = MSeedError;

    /// Convert byte array to MSeed3Header, error if first bytes are not 'MS3'
    fn try_from(buffer: &[u8; FIXED_HEADER_SIZE]) -> Result<Self, Self::Error> {
        if buffer[0] != MSeed3Header::REC_IND[0] || buffer[1] != MSeed3Header::REC_IND[1] {
            return Err(MSeedError::BadRecordIndicator(buffer[0], buffer[1]));
        }
        if buffer[2] != 3 {
            return Err(MSeedError::UnknownFormatVersion(buffer[2]));
        }
        let record_indicator = MSeed3Header::REC_IND;
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

        writeln!(
            f,
            "version {}, {} bytes (format: {})",
            self.publication_version,
            self.get_record_size(),
            self.format_version
        )?;
        writeln!(f, "             start time: {}", self.get_start_as_iso())?;
        writeln!(f, "      number of samples: {}", self.num_samples)?;
        writeln!(f, "       sample rate (Hz): {}", self.sample_rate_period)?;
        writeln!(f, "                  flags: [{:#010b}] 8 bits", self.flags)?;
        writeln!(f, "                    CRC: {}", self.crc_hex_string())?;
        writeln!(
            f,
            "    extra header length: {} bytes",
            self.extra_headers_length
        )?;
        writeln!(f, "    data payload length: {} bytes", self.data_length)?;
        write!(
            f,
            "       payload encoding: {encode_name} (val: {encoding})",
            encode_name = encode_name,
            encoding = self.encoding
        )
    }
}

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
        // special check as clippy doesn't like float equals
        assert!((nanosecond - 1.0_f64).abs() < f64::EPSILON);
    }

    fn get_dummy_header() -> [u8; 64] {
        // hexdump -C -n 64 reference-sinusoid-int16.mseed3
        // 00000000  4d 53 03 04 00 00 00 00  dc 07 01 00 00 00 00 01  |MS..............|
        // 00000010  00 00 00 00 00 00 f0 3f  90 01 00 00 d6 87 d2 04  |.......?........|
        // 00000020  01 13 00 00 20 03 00 00  46 44 53 4e 3a 58 58 5f  |.... ...FDSN:XX_|
        // 00000030  54 45 53 54 5f 5f 4c 5f  48 5f 5a 00 00 02 00 04  |TEST__L_H_Z.....|

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
            0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf0, 0x3f, 0x90, 0x01, 0x00, 0x00,
            0xd6, 0x87, 0xd2, 0x04, 0x01, 0x13, 0x00, 0x00, 0x20, 0x03, 0x00, 0x00, 0x46, 0x44,
            0x53, 0x4e, 0x3a, 0x58, 0x58, 0x5f, 0x54, 0x45, 0x53, 0x54, 0x5f, 0x5f, 0x4c, 0x5f,
            0x48, 0x5f, 0x5a, 0x00, 0x00, 0x02, 0x00, 0x04,
        ];
        buf
    }

    #[test]
    fn read_header_sin_int16() {
        let dummy = get_dummy_header();
        print!("read_header_sin_int16...");
        let head = MSeed3Header::try_from(&dummy[0..40]).unwrap();
        assert_eq!(head.record_indicator, MSeed3Header::REC_IND);
        assert_eq!(head.format_version, 3);
        assert_eq!(head.flags, 4);
        assert_eq!(head.nanosecond, 0);
        assert_eq!(head.year, 2012);
        assert_eq!(head.day_of_year, 1);
        assert_eq!(head.hour, 0);
        assert_eq!(head.minute, 0);
        assert_eq!(head.second, 0);
        assert_eq!(head.encoding.value(), 1);
        // special check as clippy doesn't like float equals
        assert!((head.sample_rate_period - 1.0_f64).abs() < f64::EPSILON);
        assert_eq!(head.num_samples, 400);
        assert_eq!(head.crc_hex_string(), "0x4D287D6");
        assert_eq!(head.crc, 0x4D287D6);
        assert_eq!(head.publication_version, 1_u8);
        assert_eq!(
            head.identifier_length,
            String::from("FDSN:XX_TEST__L_H_Z").len() as u8
        );
        assert_eq!(head.extra_headers_length, 0_u16);
        assert_eq!(head.data_length, 2 * head.num_samples); // 16 bit ints
        assert_eq!(head.data_length, 800);
        print!("{}", head);
    }

    #[test]
    fn read_header_round_trip() {
        let buf = &get_dummy_header()[0..FIXED_HEADER_SIZE];
        let head = MSeed3Header::try_from(buf).unwrap();
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
    fn set_start_leap_second() {
        let buf = get_dummy_header();
        let mut header = MSeed3Header::try_from(&buf[0..FIXED_HEADER_SIZE]).unwrap();
        let start = Utc
            .ymd(2016, 12, 31)
            .and_hms_nano(23, 59, 59, 1_900_000_000);
        header.set_start_from_utc(start);
        assert_eq!(header.nanosecond, 900_000_000);
        assert_eq!(header.second, 60);
    }
}
