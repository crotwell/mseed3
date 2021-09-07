use mseed3;
use mseed3::{MSeedError, MSeed3Record};
use serde_json;
use serde_json::Value;
use std::fs;
use std::io::{BufReader, Write};
use std::fs::File;
use byteorder::{LittleEndian, WriteBytesExt};
use std::io::{ BufWriter};

#[test]
fn test_ref_data() -> Result<(), MSeedError> {
    let base_name_list = [
        "ascii",
        "detectiononly",
        "sinusoid-FDSN-All",
        "sinusoid-FDSN-Other",
        "sinusoid-TQ-TC-ED",
        "sinusoid-float32",
        "sinusoid-float64",
        "sinusoid-int32",
        "sinusoid-int16",
        "sinusoid-steim1",
        "sinusoid-steim2",
    ];
    for base_name in base_name_list {
        let ms3_filename = format!("tests/reference-data/reference-{}.xseed", base_name);
        let file = File::open(&ms3_filename)?;
        let mut buf_reader = BufReader::new(file);
        //let records: Vec<mseed3::MSeed3Record> =
        //    mseed3::read_mseed3(&mut buf_reader)?;
        let json_filename = format!("tests/reference-data/reference-{}.json", base_name);
        let json: Value = read_ref_json(&json_filename)?;
        let mut first: MSeed3Record = mseed3::MSeed3Record::from_reader(&mut buf_reader)?;
        // match records.first() {
        //     Some(&msr) => first = msr,
        //     None => return Err(MSeedError::Unknown(format!("no records in file"))),
        // };
        assert_eq!(first.identifier, json["SID"]);
        assert_eq!(
            first.header.get_record_size(),
            json["RecordLength"].as_u64().unwrap() as u32
        );
        assert_eq!(first.header.format_version, json["FormatVersion"]);
        assert_eq!(first.header.flags, json["Flags"]["RawUInt8"]);
        assert_eq!(
            first.header.get_start_as_iso(),
            json["StartTime"].as_str().unwrap()
        );
        assert_eq!(first.header.encoding.value(), json["EncodingFormat"]);
        assert_eq!(first.header.sample_rate_period, json["SampleRate"]);
        assert_eq!(first.header.num_samples, json["SampleCount"]);
        assert_eq!(first.header.crc_hex_string(), json["CRC"].as_str().unwrap());
        assert_eq!(first.header.publication_version, json["PublicationVersion"]);
        assert_eq!(first.header.raw_extra_headers_length(), json["ExtraLength"]);
        assert_eq!(first.header.raw_data_length(), json["DataLength"]);
        let bytes_written: u32;
        let crc_written: u32;
        let mut out = Vec::new();
        {

            let mut buf_writer = BufWriter::new(&mut out);
            let t = first.write_to(&mut buf_writer).unwrap();
            bytes_written = t.0;
            crc_written = t.1;
            buf_writer.flush();
        }
        assert_eq!(first.header.crc, crc_written);
        assert_eq!(out.len() as u32, bytes_written);
        assert_eq!(first.header.crc_hex_string(), json["CRC"].as_str().unwrap());
    }
    Ok(())
}

pub fn read_ref_json(filename: &str) -> serde_json::Result<Value> {
    let contents = fs::read_to_string(filename).expect("Something went wrong reading the file");

    serde_json::from_str(&contents)
}
