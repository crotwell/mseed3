use std::fs;
use mseed3;
use mseed3::mseed3::MSeedError;
use serde_json;
use serde_json::{Value};

#[test]
fn test_ref_data()  -> Result<(), MSeedError> {
    let base_name_list = [ "ascii", "detectiononly", "sinusoid-FDSN-All",
        "sinusoid-FDSN-Other", "sinusoid-TQ-TC-ED",
        "sinusoid-float32", "sinusoid-float64", "sinusoid-int32", "sinusoid-int16",
        "sinusoid-steim1", "sinusoid-steim2"
    ];
    for base_name in base_name_list {
        let ms3_filename = format!("tests/reference-data/reference-{}.xseed", base_name);
        let json_filename = format!("tests/reference-data/reference-{}.json", base_name);
        let mut records: Vec<mseed3::mseed3::MSeed3Record> = mseed3::mseed3::read_mseed3(&ms3_filename)?;
        let json: Value = read_ref_json(&json_filename)?;
        let &first;
        match records.first() {
            Some(msr) => first = msr,
            None => return Err(MSeedError::Unknown(format!("no records in file")))
        };
        assert_eq!(first.identifier, json["SID"]);
        assert_eq!(first.header.get_size(), json["RecordLength"].as_u64().unwrap() as u32);
        assert_eq!(first.header.format_version, json["FormatVersion"]);
        assert_eq!(first.header.flags, json["Flags"]["RawUInt8"]);
        assert_eq!(first.header.get_start_as_iso(), json["StartTime"].as_str().unwrap());
        assert_eq!(first.header.encoding, json["EncodingFormat"]);
        assert_eq!(first.header.sample_rate_period, json["SampleRate"]);
        assert_eq!(first.header.num_samples, json["SampleCount"]);
        assert_eq!(first.header.crc_hex_string(), json["CRC"].as_str().unwrap());
        assert_eq!(first.header.publication_version, json["PublicationVersion"]);
        assert_eq!(first.header.extra_headers_length, json["ExtraLength"]);
        assert_eq!(first.header.data_length, json["DataLength"]);
    }
    Ok(())
}


pub fn read_ref_json(filename: &str) -> serde_json::Result<Value> {

    let contents = fs::read_to_string(filename)
        .expect("Something went wrong reading the file");

    serde_json::from_str(&contents)
}