
use byteorder::{LittleEndian, WriteBytesExt};
use crc::{Crc, CRC_32_ISCSI};
use serde_json;
use std::fmt;
use std::io::prelude::*;
use std::io::BufWriter;

use crate::data_encoding::DataEncoding;
use crate::encoded_timeseries::EncodedTimeseries;
use crate::header::{MSeed3Header, FIXED_HEADER_SIZE, CRC_OFFSET};
use crate::mseed_error::MSeedError;

pub const CASTAGNOLI: Crc<u32> = Crc::<u32>::new(&CRC_32_ISCSI);


#[derive(Debug, Clone)]
pub enum ExtraHeaders {
    Raw(String),
    Parsed(serde_json::Value),
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
        // set identifier_length, extra_header_length and data_length based on inputs
        let extra_headers_length = match &extra_headers {
            ExtraHeaders::Raw(v) =>  v.len() as u16,
            _ =>  0,
        };
        header.recalculated_lengths(identifier.len() as u8, extra_headers_length, encoded_data.byte_len(), encoded_data.reconcile_num_samples(header.num_samples));

        MSeed3Record {
            header,
            identifier,
            extra_headers,
            encoded_data,
        }
    }

    /// Read a single record record from the BufRead
    pub fn from_reader<R: BufRead>(buf_reader: &mut R) -> Result<MSeed3Record, MSeedError> {
        let mut buffer = [0; FIXED_HEADER_SIZE];
        let _ = buf_reader
            .by_ref()
            .take(FIXED_HEADER_SIZE as u64)
            .read(&mut buffer)?;
        let mut header = MSeed3Header::from_bytes(&buffer)?;
        // set crc field to zero for crc calculation, header has already read value
        buffer[CRC_OFFSET] = 0;
        buffer[CRC_OFFSET+1] = 0;
        buffer[CRC_OFFSET+2] = 0;
        buffer[CRC_OFFSET+3] = 0;
        let mut digest = CASTAGNOLI.digest();
        digest.update(&buffer);

        let mut buffer = Vec::new();
        let _ = buf_reader
            .by_ref()
            .take(header.raw_identifier_length() as u64)
            .read_to_end(&mut buffer)?;
        digest.update(&buffer);
        let id_result = String::from_utf8(buffer);
        let identifier = match id_result {
            Ok(id) => id,
            Err(e) => return Err(MSeedError::FromUtf8Error(e)),
        };
        let extra_headers_str: String;
        let mut json_reader = buf_reader.by_ref().take(header.raw_extra_headers_length() as u64);
        let mut buffer = Vec::new();
        let _ = json_reader.read_to_end(&mut buffer)?;
        digest.update(&buffer);
        if header.raw_extra_headers_length() > 2 {
            extra_headers_str = String::from_utf8(buffer)?;
        } else {
            extra_headers_str = String::from("{}");
        }
        let expected_data_length = match header.encoding {
            DataEncoding::INT16 => 2 * header.num_samples,
            DataEncoding::INT32 => 4 * header.num_samples,
            DataEncoding::FLOAT32 => 4 * header.num_samples,
            DataEncoding::FLOAT64 => 8 * header.num_samples,
            _ => header.raw_data_length(),
        };
        if header.raw_data_length() != expected_data_length {
            return Err(MSeedError::DataLength(
                expected_data_length,
                header.num_samples,
                header.encoding.value(),
                header.raw_data_length(),
            ));
        }

        let mut encoded_data = Vec::new();
        let _ = buf_reader
            .by_ref()
            .take(header.raw_data_length() as u64)
            .read_to_end(&mut encoded_data)?;
        digest.update(&encoded_data);
        let crc_calc = digest.finalize();
        if crc_calc != header.crc { return Err(MSeedError::CrcInvalid(crc_calc, header.crc));}
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
        buf.write_all(&out[(CRC_OFFSET + 4)..])?;
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
        let identifier_length = id_bytes.len() as u8;
        let data_length = self.encoded_data.byte_len();
        let num_samples = self
            .encoded_data
            .reconcile_num_samples(self.header.num_samples);

        let mut eh_bytes = Vec::new();
        match &self.extra_headers {
            ExtraHeaders::Parsed(eh) => eh_bytes.write_all(eh.to_string().as_bytes())?,
            ExtraHeaders::Raw(s) => eh_bytes.write_all(s.as_bytes())?,
        };
        let extra_headers_length;
        if eh_bytes.len() > 2 {
            extra_headers_length = eh_bytes.len() as u16;
        } else {
            extra_headers_length = 0;
        }
        self.header.recalculated_lengths(identifier_length, extra_headers_length, data_length, num_samples);
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

impl fmt::Display for MSeed3Record {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "  {}, {}", self.identifier, self.header)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_round_trip() {
        let buf = &get_dummy_header()[0..FIXED_HEADER_SIZE];
        let identifier =
            String::from_utf8(get_dummy_header()[FIXED_HEADER_SIZE..64].to_owned()).unwrap();

        let mut head = MSeed3Header::from_bytes(buf).unwrap();
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