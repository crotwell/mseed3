use mseed3::MSeedError;
use std::io::Write;

#[test]
fn lib_test() -> Result<(), MSeedError> {
    use chrono::{DateTime, Utc};
    use mseed3::{DataEncoding, EncodedTimeseries, ExtraHeaders, SourceIdentifier};
    let start = "2014-11-28T12:00:09Z".parse::<DateTime<Utc>>()?;
    let timeseries = vec![0, 1, -1, 5, 3, -5, 10, -1, 1, 0];
    let num_samples = timeseries.len();
    let encoded_data = EncodedTimeseries::Int32(timeseries);
    let header = mseed3::MSeed3Header::new(start, DataEncoding::INT32, 10.0, num_samples);
    let identifier = SourceIdentifier::from("FDSN:CO_BIRD_00_H_H_Z");
    let extra_headers = ExtraHeaders::Raw(String::from("{}"));
    let mut record = mseed3::MSeed3Record::new(header, identifier, extra_headers, encoded_data);

    let outfile = std::fs::File::create("simple.ms3")?;
    let mut buf_writer = std::io::BufWriter::new(outfile);
    record.write_to(&mut buf_writer)?; // writing a record mut's the header to fix crc, and the byte lengths
    buf_writer.flush()?;

    println!("Record: \n{}", record);

    let my_mseed3_file = std::fs::File::open("simple.ms3").unwrap();
    let mut buf_reader = std::io::BufReader::new(my_mseed3_file);
    let records = mseed3::read_mseed3(&mut buf_reader)?;
    let first_record = records.first().unwrap();
    print!("Read back in: \n{}", first_record);

    Ok(())
}
