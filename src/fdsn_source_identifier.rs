use crate::MSeedError;
use lazy_static::lazy_static;
use regex::{Captures, Regex};
use std::fmt;

lazy_static! {
    static ref PARSE_FDSN_REGEX: Regex = Regex::new(
        r"(?x)^
            FDSN:                      # prefix
            (?P<net>[A-Z0-9]{1,8})_    # network, 1-8 chars
            (?P<sta>[-A-Z0-9]{1,8})_   # station, 1-8 chars with dash
            (?P<loc>[-A-Z0-9]{0,8})_   # location, 0-8 chars with dash
            (?P<band>[A-Z0-9]*)_       # band, optional, usually single char
            (?P<source>[A-Z0-9]+)_     # source, one or more, usually single char
            (?P<subsource>[A-Z0-9]*)$  # subsource, optional, usually single char
"
    )
    .unwrap();
}

pub const PREFIX: &str = "FDSN:";

#[derive(Debug, Clone)]
pub struct FdsnSourceIdentifier {
    pub network: String,
    pub station: String,
    pub location: String,
    pub band: String,
    pub source: String,
    pub subsource: String,
}

impl FdsnSourceIdentifier {
    pub fn calc_len(&self) -> u8 {
        (10 + self.network.len()
            + self.station.len()
            + self.location.len()
            + self.band.len()
            + self.source.len()
            + self.subsource.len()) as u8
    }
    /// Returns a byte slice of this identifier.
    pub fn as_bytes(&self) -> Vec<u8> {
        Vec::from(self.to_string().as_bytes())
    }
    pub fn from_utf8(vec: Vec<u8>) -> Result<FdsnSourceIdentifier, MSeedError> {
        let text = String::from_utf8(vec)?;
        FdsnSourceIdentifier::parse(&text)
    }

    pub fn parse(id: &str) -> Result<FdsnSourceIdentifier, MSeedError> {
        let sid = match PARSE_FDSN_REGEX.captures(id) {
            Some(captures) => FdsnSourceIdentifier {
                network: capture_named(&captures, "net", id)?,
                station: capture_named(&captures, "sta", id)?,
                location: capture_named(&captures, "loc", id)?,
                band: capture_named(&captures, "band", id)?,
                source: capture_named(&captures, "source", id)?,
                subsource: capture_named(&captures, "subsource", id)?,
            },
            None => {
                return Err(MSeedError::IdentifierParse(
                    id.to_string(),
                    String::from("all"),
                ))
            }
        };
        Ok(sid)
    }

    pub fn create_fake_channel() -> FdsnSourceIdentifier {
        FdsnSourceIdentifier {
            network: String::from("XX"),
            station: String::from("STA"),
            location: String::from("00"),
            band: String::from("B"),
            source: String::from("H"),
            subsource: String::from("Z"),
        }
    }
}

impl fmt::Display for FdsnSourceIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}{}_{}_{}_{}_{}_{}",
            PREFIX,
            self.network,
            self.station,
            self.location,
            self.band,
            self.source,
            self.subsource
        )
    }
}

fn capture_named(captures: &Captures, name: &str, id: &str) -> Result<String, MSeedError> {
    match captures.name(name) {
        Some(s) => Ok(s.as_str().to_string()),
        None => Err(MSeedError::IdentifierParse(
            id.to_string(),
            name.to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Error;

    #[test]
    fn init_regex() -> Result<(), MSeedError> {
        let id = "FDSN:IU_ABCD_00_B_H_Z";
        assert!(PARSE_FDSN_REGEX.is_match(&id));
        let sid = FdsnSourceIdentifier::parse(&id)?;
        assert_eq!("IU", sid.network);
        assert_eq!("ABCD", sid.station);
        assert_eq!("00", sid.location);
        assert_eq!("B", sid.band);
        assert_eq!("H", sid.source);
        assert_eq!("Z", sid.subsource);
        assert_eq!(id, sid.to_string());
        Ok(())
    }

    #[test]
    fn calc_len() -> Result<(), MSeedError> {
        let id = String::from("FDSN:IU_COLA_00_B_H_Z");
        let sid = FdsnSourceIdentifier::parse(&id)?;
        assert_eq!(id.len() as u8, sid.calc_len());
        Ok(())
    }
}
