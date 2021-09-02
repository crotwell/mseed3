use serde_json;
use chrono::prelude::*;
use chrono::Utc;
use std::convert::TryInto;
use std::fmt;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::string::FromUtf8Error;

const BUFFER_SIZE: usize = 256;

fn read_le_f64(input: &mut &[u8]) -> f64 {
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<f64>());
    *input = rest;
    print!("in read_le_f64");
    for n in 0..8 {
        print!("{}", int_bytes[n]);
    }
    f64::from_le_bytes(int_bytes.try_into().unwrap())
}
fn read_le_u32(input: &mut &[u8]) -> u32 {
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<u32>());
    *input = rest;
    u32::from_le_bytes(int_bytes.try_into().unwrap())
}
fn read_le_u16(input: &mut &[u8]) -> u16 {
    let (int_bytes, rest) = input.split_at(std::mem::size_of::<u16>());
    *input = rest;
    u16::from_le_bytes(int_bytes.try_into().unwrap())
}
pub fn read_mseed3(file_name: &str) -> Result<MSeed3Header, MSeedError> {
    let file = File::open(&file_name)?;
    let mut buf_reader = BufReader::new(file);

    let mut buffer = [0; BUFFER_SIZE];

    let _ = buf_reader.by_ref().take(40).read(&mut buffer)?;
    return MSeed3Header::from_bytes(&buffer);
}

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
    pub encoding: u8,
    pub sample_rate_period: f64,
    pub num_samples: u32,
    pub crc: u32,
    pub publication_version: u8,
    pub identifier_length: u8,
    pub extra_headers_length: u16,
    pub data_length: u32,
}

impl MSeed3Header {
    pub fn from_bytes(buffer: &[u8]) -> Result<MSeed3Header, MSeedError> {
        print!("read_mseed3_buf...");
        assert_eq!(&buffer[0..2], "MS".as_bytes());
        if buffer[0] != b'M' || buffer[1] != b'S' {
            return Err(MSeedError::BadRecordIndicator);
        }
        // skip M, S, format, flags
        let (_, mut header_bytes) = buffer.split_at(4);
        let nanosecond = read_le_u32(&mut header_bytes);
        let year = read_le_u16(&mut header_bytes);
        let day_of_year = read_le_u16(&mut header_bytes);
        let _ = read_le_u32(&mut header_bytes); // skip hour-encoding
        let sample_rate_period = read_le_f64(&mut header_bytes);
        let num_samples = read_le_u32(&mut header_bytes);
        let crc = read_le_u32(&mut header_bytes);
        let _ = read_le_u16(&mut header_bytes); // skip pub ver and id len
        let extra_headers_length = read_le_u16(&mut header_bytes);
        let data_length = read_le_u32(&mut header_bytes);
        let ms3_header = MSeed3Header {
            record_indicator: String::from("MS"),
            format_version: buffer[2],
            flags: buffer[3],
            nanosecond,
            year,
            day_of_year,
            hour: buffer[12],
            minute: buffer[13],
            second: buffer[14],
            encoding: buffer[15],
            sample_rate_period,
            num_samples,
            crc,
            publication_version: buffer[32],
            identifier_length: buffer[33],
            extra_headers_length,
            data_length,
        };
        return Ok(ms3_header);
    }

    pub fn get_start_as_iso(&self) -> String {
        let start = Utc.yo(self.year as i32,
                           self.day_of_year as u32)
            .and_hms_nano(self.hour as u32,
                          self.minute as u32,
                          self.second as u32,
                          self.nanosecond);
        start.format("%Y-%jT%H:%M:%S%.9f").to_string()
    }

    pub fn crc_hex_string(&self) -> String {
        format!("{:#010x}", self.crc)
    }

    pub fn get_size(&self) -> u32 {
        self.identifier_length as u32
            +self.extra_headers_length as u32
            +self.data_length
    }
}

pub enum ExtraHeaders {
    Raw(String),
    Parsed(serde_json::Map<String, serde_json::Value>)
}

pub struct MSeed3Record {
    pub header: MSeed3Header,
    pub identifier: String,
    pub extra_headers: ExtraHeaders,
    pub encoded_data: Vec<u8>,
}

impl MSeed3Record {
    pub fn from_bytes<R: BufRead>(buf_reader: &mut R) -> Result<MSeed3Record, MSeedError> {
        let mut buffer = [0; BUFFER_SIZE];
        let _ = buf_reader.by_ref().take(40).read(&mut buffer)?;
        let header = MSeed3Header::from_bytes(&buffer)?;
        let mut buffer = Vec::new();
        let _ = buf_reader
            .by_ref()
            .take(header.identifier_length as u64)
            .read_to_end(&mut buffer)?;
        let identifier = String::from_utf8(buffer)?;
        let extra_headers_str: String;
        if header.extra_headers_length > 2 {
            let mut json_reader = buf_reader.by_ref().take(header.extra_headers_length as u64);
            let mut buffer = Vec::new();
            let _ = json_reader.read_to_end(&mut buffer)?;
            extra_headers_str = String::from_utf8(buffer)?;
        } else {
            extra_headers_str = String::from("{}");
        }

        let mut encoded_data = Vec::new();
        let _ = buf_reader
            .by_ref()
            .take(header.data_length as u64)
            .read_to_end(&mut encoded_data)?;
        let encoded_data = encoded_data;
        return Ok(MSeed3Record {
            header,
            identifier,
            extra_headers: ExtraHeaders::Raw(extra_headers_str),
            encoded_data,
        });
    }

    pub fn parsed_json(&mut self) -> Result<&serde_json::Map<String, serde_json::Value>, MSeedError> {
        if let ExtraHeaders::Raw(eh_str) = &self.extra_headers {
                let v: serde_json::Value = serde_json::from_str(&eh_str)?;
                let eh = match v {
                    serde_json::Value::Object(m) => m,
                    _ => return Err(MSeedError::JsonError),
                };
            self.extra_headers = ExtraHeaders::Parsed(eh);
            }
        if let ExtraHeaders::Parsed(eh) = &self.extra_headers {
            return Ok(&eh);
        }
        Err(MSeedError::JsonError)
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

            let encode_name = match self.encoding {
                10 => "STEIM-1 integer compression",
                11 => "STEIM-2 integer compression",
                _ => "unknown",
            };
        let lines = [
            format!("version ${}, ${} bytes (format: ${})\n", self.publication_version, self.get_size()+self.data_length, self.format_version ),
            format!("             start time: ${}\n", self.get_start_as_iso()),
            format!("      number of samples: ${}\n", self.num_samples),
            format!("       sample rate (Hz): ${}\n", self.sample_rate_period),
            format!("                  flags: [${:#08b}] 8 bits\n", self.flags),
            format!("                    CRC: ${}\n", self.crc_hex_string()),
            format!("    extra header length: ${} bytes\n", self.extra_headers_length),
            format!("    data payload length: ${} bytes\n", self.data_length),
            format!("       payload encoding: ${encode_name} (val: ${encoding})",encode_name=encode_name,encoding=self.encoding)
        ];
        let line = "";
        for l in lines {
            format!("{}{}",line, l);
        }
        write!(f, "{}", line)
    }
}


impl fmt::Display for MSeed3Record {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "  {}, {}", self.identifier, self.header )
    }
}

#[derive(Debug)]
pub enum MSeedError {
    IOError,
    FromUtf8Error,
    BadRecordIndicator,
    JsonError,
}

impl std::error::Error for MSeedError {}

impl fmt::Display for MSeedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MSeedError::IOError => write!(f, "IO Error"),
            MSeedError::JsonError => write!(f, "JSON Error"),
            MSeedError::BadRecordIndicator => write!(f, "Header must start with MS"),
            MSeedError::FromUtf8Error => write!(f, "UTF8 Error"),
        }
    }
}

impl From<std::io::Error> for MSeedError {
    fn from(err: std::io::Error) -> MSeedError {
        MSeedError::IOError
    }
}
impl From<serde_json::Error> for MSeedError {
    fn from(err: serde_json::Error) -> MSeedError {
        MSeedError::JsonError
    }
}
impl From<FromUtf8Error> for MSeedError {
    fn from(err: FromUtf8Error) -> MSeedError {
        MSeedError::FromUtf8Error
    }
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

    #[test]
    fn read_header_sin_int16() {
        print!("read_header_sin_int16...");
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
        assert_eq!(head.encoding, 1);
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
}
