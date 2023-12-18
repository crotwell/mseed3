use mseed3;
use mseed3::{MSeed3Record, MSeedError};
use serde_json;
use serde_json::Value;
use std::fs;
use std::fs::File;
use std::io::BufWriter;
use std::io::{BufReader, Write};
use std::path::Path;

#[test]
fn test_ref_data() -> Result<(), MSeedError> {
    let base_name_list = [
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
        "text",
    ];
    for base_name in base_name_list {
        let ms3_filename = format!("tests/reference-data/reference-{}.mseed3", base_name);
        println!("work on {}", ms3_filename);
        assert!(
            Path::new(&ms3_filename).exists(),
            "Reference data missing, download from https://github.com/FDSN/miniSEED3"
        );
        let file = File::open(&ms3_filename)?;
        let mut buf_reader = BufReader::new(file);
        //let records: Vec<mseed3::MSeed3Record> =
        //    mseed3::read_mseed3(&mut buf_reader)?;
        let json_filename = format!("tests/reference-data/reference-{}.json", base_name);
        println!("read json: {}", json_filename);
        let json_arr: Value = read_ref_json(&json_filename)?;
        let json: &Value = &json_arr[0];


        let unparsed = mseed3::UnparsedMSeed3Record::from_reader(&mut buf_reader)?;
        assert_eq!(
            unparsed.header.crc_hex_string(),
            json["CRC"].as_str().unwrap()
        );
        println!("unparsed extra headers: {}", unparsed.extra_headers);
        let file = File::open(&ms3_filename)?;
        let mut buf_reader = BufReader::new(file); // reopen to read as record
        let first: MSeed3Record = mseed3::MSeed3Record::from_reader(&mut buf_reader)?;
        assert_eq!(first.identifier.to_string(), json["SID"].as_str().unwrap());
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
        assert_eq!(first.header.get_sample_rate_hertz(), json["SampleRate"]);
        assert_eq!(first.header.num_samples, json["SampleCount"]);
        assert_eq!(first.header.crc_hex_string(), json["CRC"].as_str().unwrap());
        assert_eq!(first.header.publication_version, json["PublicationVersion"]);
        assert_eq!(first.header.raw_extra_headers_length(), json["ExtraLength"]);
        assert_eq!(first.header.raw_data_length(), json["DataLength"]);
        // use unparsed to check CRC as json keys are reordered with deserialize-serialize cycle
        let bytes_written: u32;
        let crc_written: u32;
        let mut out = Vec::new();
        {
            let mut buf_writer = BufWriter::new(&mut out);
            let t = unparsed.write_to(&mut buf_writer).unwrap();
            bytes_written = t.0;
            crc_written = t.1;
            buf_writer.flush()?;
        }
        assert_eq!(unparsed.header.crc, crc_written);
        assert_eq!(out.len() as u32, bytes_written);
        assert_eq!(
            unparsed.header.crc_hex_string(),
            json["CRC"].as_str().unwrap(),
            "first: {}, written: {:#0X},  json: {}",
            unparsed.header.crc_hex_string(),
            crc_written,
            json["CRC"].as_str().unwrap()
        );
    }
    Ok(())
}

pub fn read_ref_json(filename: &str) -> serde_json::Result<Value> {
    let contents = fs::read_to_string(filename).expect("Something went wrong reading the file");

    serde_json::from_str(&contents)
}
