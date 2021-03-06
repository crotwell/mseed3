use byteorder::{LittleEndian, WriteBytesExt};
use chrono::prelude::*;
use chrono::Utc;
use crc::{Crc, CRC_32_ISCSI};
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::map::Map;
use serde_json::Value;
use std::fmt;
use std::io::prelude::*;
use std::io::BufWriter;

use crate::data_encoding::DataEncoding;
use crate::encoded_timeseries::EncodedTimeseries;
use crate::fdsn_source_identifier::{FdsnSourceIdentifier, SourceIdentifier};
use crate::header::{MSeed3Header, CRC_OFFSET, FIXED_HEADER_SIZE};
use crate::mseed_error::MSeedError;
use std::convert::TryFrom;

pub const CASTAGNOLI: Crc<u32> = Crc::<u32>::new(&CRC_32_ISCSI);
pub const FDSN_EXTRA_HEADERS: &str = "FDSN";

/// Miniseed3 record, consisting of a fixed header, a string identifier, json extra headers and
/// encoded timeseries points. The Unparsed leaves the extra headers as a string, see
/// MSeed3Record for a similar struct with extra headers parsed with serde to a Map.
///
/// See the spec at
/// <https://miniseed3.readthedocs.io> now or at
/// <https://docs.fdsn.org/projects/miniSEED3>.

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UnparsedMSeed3Record {
    pub header: MSeed3Header,
    pub identifier: SourceIdentifier,
    pub extra_headers: String,
    pub encoded_data: EncodedTimeseries,
}

impl UnparsedMSeed3Record {
    /// Read a single record record from the BufRead
    pub fn from_reader<R: BufRead>(buf_reader: &mut R) -> Result<UnparsedMSeed3Record, MSeedError> {
        let mut buffer = [0; FIXED_HEADER_SIZE];
        let _ = buf_reader
            .by_ref()
            .take(FIXED_HEADER_SIZE as u64)
            .read(&mut buffer)?;
        let mut header = MSeed3Header::try_from(&buffer)?;
        // set crc field to zero for crc calculation, header has already read value
        buffer[CRC_OFFSET] = 0;
        buffer[CRC_OFFSET + 1] = 0;
        buffer[CRC_OFFSET + 2] = 0;
        buffer[CRC_OFFSET + 3] = 0;
        let mut digest = CASTAGNOLI.digest();
        digest.update(&buffer);

        let mut buffer = Vec::new();
        let _ = buf_reader
            .by_ref()
            .take(header.raw_identifier_length() as u64)
            .read_to_end(&mut buffer)?;
        digest.update(&buffer);
        let identifier = SourceIdentifier::try_from(buffer)?;
        let extra_headers: String;
        let mut json_reader = buf_reader
            .by_ref()
            .take(header.raw_extra_headers_length() as u64);
        let mut buffer = Vec::new();
        let _ = json_reader.read_to_end(&mut buffer)?;
        digest.update(&buffer);
        if header.raw_extra_headers_length() > 2 {
            extra_headers = String::from_utf8(buffer)?;
        } else {
            extra_headers = String::from("{}");
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
        if crc_calc != header.crc {
            return Err(MSeedError::CrcInvalid(crc_calc, header.crc));
        }
        let encoded_data = EncodedTimeseries::Raw(encoded_data);
        header.num_samples = encoded_data.reconcile_num_samples(header.num_samples);
        Ok(UnparsedMSeed3Record {
            header,
            identifier,
            extra_headers,
            encoded_data,
        })
    }

    /// Writes the record, after calculating the CRC. The returned tuple contains the number
    /// of bytes written and the CRC value.
    /// This does recalculate the identifier length, extra headers length and data length headers.
    /// The number of samples is sanity checked against the data, but trusts the header in cases
    /// of compressed or opaque data.
    pub fn write_to<W>(&self, buf: &mut BufWriter<W>) -> Result<(u32, u32), MSeedError>
    where
        W: std::io::Write,
    {
        let mut out = Vec::new();
        {
            let mut inner_buf = BufWriter::new(&mut out);
            self.write_to_wocrc(&mut inner_buf)?;
            inner_buf.flush()?;
        }
        let crc = CASTAGNOLI.checksum(&out);
        buf.write_all(&out[0..CRC_OFFSET])?;
        buf.write_u32::<LittleEndian>(crc)?;
        buf.write_all(&out[(CRC_OFFSET + 4)..])?;
        Ok((out.len() as u32, crc))
    }

    /// Writes the record to the given buffer with zero in place of the header CRC field.
    /// This also recalculates the identifier length, extra headers length and data length headers.
    /// The number of samples is sanity checked against the data, but trusts the header in cases
    /// of compressed or opaque data.
    pub fn write_to_wocrc<W>(&self, buf: &mut BufWriter<W>) -> Result<(), MSeedError>
    where
        W: std::io::Write,
    {
        let id_bytes = self.identifier.as_bytes();
        let identifier_length = id_bytes.len() as u8;
        let data_length = self.encoded_data.byte_len();
        let num_samples = self
            .encoded_data
            .reconcile_num_samples(self.header.num_samples);

        let eh_bytes = self.extra_headers.as_bytes();
        let extra_headers_length;
        if eh_bytes.len() > 2 {
            extra_headers_length = eh_bytes.len() as u16;
        } else {
            extra_headers_length = 0;
        }
        let mut mod_header = self.header.clone();
        mod_header.crc = 0;
        mod_header.recalculated_lengths(
            identifier_length,
            extra_headers_length,
            data_length,
            num_samples,
        );
        mod_header.write_to(buf)?;
        buf.write_all(&id_bytes)?;
        if eh_bytes.len() > 2 {
            // don't write bytes for empty object, e.g. `{}`
            buf.write_all(eh_bytes)?;
        }
        self.encoded_data.write_to(buf)?;
        buf.flush()?;
        Ok(())
    }
}

pub fn parse_headers(raw_rec: UnparsedMSeed3Record) -> Result<MSeed3Record, MSeedError> {
    let v: Value = serde_json::from_str(&raw_rec.extra_headers)?;
    let eh_json = match v {
        Value::Object(map) => Ok(map),
        _ => Err(MSeedError::ExtraHeaderNotObject(v)),
    };
    Ok(MSeed3Record {
        header: raw_rec.header,
        identifier: raw_rec.identifier,
        encoded_data: raw_rec.encoded_data,
        extra_headers: eh_json?,
    })
}

pub fn pack_headers(rec: MSeed3Record) -> Result<UnparsedMSeed3Record, MSeedError> {
    let identifier = rec.identifier;
    let eh_str = serde_json::Value::Object(rec.extra_headers).to_string();
    let eh_bytes = eh_str.as_bytes();
    let extra_headers_length;
    if eh_bytes.len() > 2 {
        extra_headers_length = eh_bytes.len() as u16;
    } else {
        extra_headers_length = 0;
    }
    let mut header = rec.header.clone();
    header.crc = 0;
    let data_length = rec.encoded_data.byte_len();
    let num_samples = rec.encoded_data.reconcile_num_samples(header.num_samples);
    header.recalculated_lengths(
        identifier.to_string().len() as u8,
        extra_headers_length,
        data_length,
        num_samples,
    );
    Ok(UnparsedMSeed3Record {
        header,
        identifier,
        encoded_data: rec.encoded_data,
        extra_headers: eh_str,
    })
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MSeed3Record {
    pub header: MSeed3Header,
    pub identifier: SourceIdentifier,
    pub extra_headers: Map<String, Value>,
    pub encoded_data: EncodedTimeseries,
}

impl MSeed3Record {
    /// Create new miniseed3 Record. The header's fields are reconciled with the other inputs, so
    /// for example in the case where the data is a primitive, uncompressed type like Int32,
    /// num_samples will be calculated and set from the length of the array and so a 0 can be passed
    /// as the last argument. However, in the case of compressed data, the number of samples cannot
    /// be determined and so needs to be passed in.
    ///
    /// #Example
    ///
    /// ```
    /// # use mseed3::MSeedError;
    /// # use mseed3::FdsnSourceIdentifier;
    /// # fn main() -> Result<(), MSeedError> {
    /// use chrono::{DateTime, Utc};
    /// use mseed3::{DataEncoding, EncodedTimeseries, MSeedError, SourceIdentifier};
    /// let start = "2014-11-28T12:00:09Z".parse::<DateTime<Utc>>()?;
    /// let timeseries = vec![0, 1, -1, 5, 3, -5, 10, -1, 1, 0];
    /// let num_samples = timeseries.len();
    /// let encoded_data = EncodedTimeseries::Int32(timeseries);
    /// let header = mseed3::MSeed3Header::new(start, DataEncoding::INT32, 10.0, num_samples);
    /// let identifier = SourceIdentifier::from("FDSN:CO_BIRD_00_H_H_Z");
    /// let extra_headers = None;
    /// let record = mseed3::MSeed3Record::new(header, identifier, extra_headers, encoded_data);
    /// # Ok(())
    /// # }
    ///
    /// ```
    pub fn new(
        header: MSeed3Header,
        identifier: SourceIdentifier,
        extra_headers: Option<Map<String, Value>>,
        encoded_data: EncodedTimeseries,
    ) -> MSeed3Record {
        let mut header = header;
        // set identifier_length, extra_header_length and data_length based on inputs
        let extra_headers_length = 0; // this is expensive to calc, as must turn json into string
        header.recalculated_lengths(
            identifier.calc_len(),
            extra_headers_length,
            encoded_data.byte_len(),
            encoded_data.reconcile_num_samples(header.num_samples),
        );
        let extra_headers = match extra_headers {
            Some(eh) => eh,
            None => Map::new(),
        };

        MSeed3Record {
            header,
            identifier,
            extra_headers,
            encoded_data,
        }
    }

    /// Create a record with the given start time and sample rate from a Vec of f32 floats
    pub fn from_floats(
        start: DateTime<Utc>,
        sample_rate_period: f64,
        data: Vec<f32>,
    ) -> MSeed3Record {
        let header =
            MSeed3Header::new(start, DataEncoding::FLOAT32, sample_rate_period, data.len());
        MSeed3Record::new(
            header,
            SourceIdentifier::Fdsn(FdsnSourceIdentifier::create_fake_channel()),
            None,
            EncodedTimeseries::Float32(data),
        )
    }

    /// Create a record with the given start time and sample rate from a Vec of i32 integers
    pub fn from_ints(
        start: DateTime<Utc>,
        sample_rate_period: f64,
        data: Vec<i32>,
    ) -> MSeed3Record {
        let header = MSeed3Header::new(start, DataEncoding::INT32, sample_rate_period, data.len());
        MSeed3Record::new(
            header,
            SourceIdentifier::Fdsn(FdsnSourceIdentifier::create_fake_channel()),
            None,
            EncodedTimeseries::Int32(data),
        )
    }

    /// Read a single record record from the BufRead
    pub fn from_reader<R: BufRead>(buf_reader: &mut R) -> Result<MSeed3Record, MSeedError> {
        parse_headers(UnparsedMSeed3Record::from_reader(buf_reader)?)
    }

    /// Writes the record, after calculating the CRC. The returned tuple contains the number
    /// of bytes written and the CRC value.
    /// This does recalculate the identifier length, extra headers length and data length headers.
    /// The number of samples is sanity checked against the data, but trusts the header in cases
    /// of compressed or opaque data.
    ///
    /// Note to self, maybe better to pack record to UnparsedMSeed3Record and then write that???
    pub fn write_to<W>(&self, buf: &mut BufWriter<W>) -> Result<(u32, u32), MSeedError>
    where
        W: std::io::Write,
    {
        let mut header = self.header.clone();
        header.crc = 0;
        let mut out = Vec::new();
        {
            let mut inner_buf = BufWriter::new(&mut out);
            self.write_to_wocrc(&mut inner_buf)?;
            inner_buf.flush()?;
        }
        let crc = CASTAGNOLI.checksum(&out);
        buf.write_all(&out[0..CRC_OFFSET])?;
        buf.write_u32::<LittleEndian>(crc)?;
        buf.write_all(&out[(CRC_OFFSET + 4)..])?;
        Ok((out.len() as u32, crc))
    }

    /// Writes the record to the given buffer without checking, calculating or setting the header CRC field.
    /// This does recalculate the identifier length, extra headers length and data length headers.
    /// The number of samples is sanity checked against the data, but trusts the header in cases
    /// of compressed or opaque data.
    pub fn write_to_wocrc<W>(&self, buf: &mut BufWriter<W>) -> Result<(), MSeedError>
    where
        W: std::io::Write,
    {
        let id_bytes = self.identifier.as_bytes();
        let identifier_length = id_bytes.len() as u8;
        let data_length = self.encoded_data.byte_len();
        let num_samples = self
            .encoded_data
            .reconcile_num_samples(self.header.num_samples);

        let eh_str = serde_json::Value::Object(self.extra_headers.clone()).to_string();
        let eh_bytes = eh_str.as_bytes();
        let extra_headers_length;
        if eh_bytes.len() > 2 {
            extra_headers_length = eh_bytes.len() as u16;
        } else {
            extra_headers_length = 0;
        }
        let mut mod_header = self.header.clone();
        mod_header.recalculated_lengths(
            identifier_length,
            extra_headers_length,
            data_length,
            num_samples,
        );
        mod_header.write_to(buf)?;
        buf.write_all(&id_bytes)?;
        if eh_bytes.len() > 2 {
            // don't write bytes for empty object, e.g. `{}`
            buf.write_all(eh_bytes)?;
            println!("write eh bytes: {}", &eh_str);
        }
        self.encoded_data.write_to(buf)?;
        buf.flush()?;
        Ok(())
    }

    pub fn get_record_size(&self) -> u32 {
        self.header.get_record_size()
    }

    pub fn get_fdsn_headers(&self) -> Option<&Map<String, Value>> {
        match &self.extra_headers[FDSN_EXTRA_HEADERS] {
            Value::Object(map) => Some(map),
            _ => None,
        }
    }

    pub fn mut_fdsn_headers(&mut self) -> &mut Map<String, Value> {
        self.create_fdsn_headers();
        let fdsn = self.extra_headers.get_mut(FDSN_EXTRA_HEADERS).unwrap();
        match fdsn {
            Value::Object(map) => map,
            _ => panic!("fdsn extra headers are not map"),
        }
    }
    pub fn create_fdsn_headers(&mut self) {
        match &self.extra_headers[FDSN_EXTRA_HEADERS] {
            Value::Object(_map) => (),
            _ => self.extra_headers[FDSN_EXTRA_HEADERS] = json!({}),
        }
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
    fn record_round_trip() -> Result<(), MSeedError> {
        let buf = &get_dummy_header()[0..FIXED_HEADER_SIZE];

        let mut head = MSeed3Header::try_from(buf).unwrap();

        let identifier_bytes = get_dummy_header()
            [FIXED_HEADER_SIZE..(FIXED_HEADER_SIZE + head.raw_identifier_length() as usize)]
            .to_owned();
        let identifier_length = identifier_bytes.len() as u8;
        let identifier = SourceIdentifier::try_from(identifier_bytes)?;
        let extra_headers_length = 0;
        let dummy_data = vec![0, -1, 2, -3, 4, -5];
        let data_length = (dummy_data.len() as u32 * 4) as u32;
        let num_samples = dummy_data.len() as u32;
        head.recalculated_lengths(
            identifier_length,
            extra_headers_length,
            data_length,
            num_samples,
        );
        head.encoding = DataEncoding::INT32;
        let encoded_data = EncodedTimeseries::Int32(dummy_data);
        let rec = MSeed3Record::new(head, identifier, None, encoded_data);
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
        println!("crc is {:#0X}", crc_written);
        assert_eq!(0xEB08F2A9, crc_written);
        Ok(())
    }

    // copy from header.rs
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
}
