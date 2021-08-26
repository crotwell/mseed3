use serde_json;

pub struct MSeed3Header {
    record_indicator: String,
    format_version: u8,
    flags: u8,
    nanosecond: u32,
    year: u16,
    day_of_year: u16,
    hour: u8,
    minute: u8,
    second: u8,
    encoding: u8,
    sample_rate_period: i64,
    num_samples: u32,
    crc: u32,
    publication_version: u8,
    identifier_length: u8,
    extra_headers_length: u16,
    data_length: u32,
}

pub struct MSeed3Record {
    header: MSeed3Header,
    identifier: String,
    extra_headers: serde_json::Map<String, serde_json::Value>,
    data: Vec<u8>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
