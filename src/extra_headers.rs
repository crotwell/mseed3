use crate::mseed_error::MSeedError;
use serde::{Serialize, Deserialize};
use serde_json;
use serde_json::map::Map;
use serde_json::Value;
use std::fmt;
use std::str::FromStr;

pub const FDSN_EXTRA_HEADERS: &str = "FDSN";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExtraHeaders {
    pub root: Map<String, Value>,
}

impl ExtraHeaders {
    pub fn new() -> ExtraHeaders {
        ExtraHeaders {
            root: serde_json::map::Map::new(),
        }
    }

    pub fn create_fdsn_headers(&mut self) -> Result<&mut Map<String, Value>, MSeedError> {
        if !self.root.contains_key(FDSN_EXTRA_HEADERS) {
            let fdsn = serde_json::map::Map::new();
            self.root.insert(
                FDSN_EXTRA_HEADERS.to_string(),
                serde_json::Value::Object(fdsn),
            );
        }
        match self.root.get_mut(FDSN_EXTRA_HEADERS) {
            Some(fdsn) => match fdsn.as_object_mut() {
                Some(fdsn_obj) => Ok(fdsn_obj),
                None => Err(MSeedError::ExtraHeaderParse(String::from(
                    "value for key=FDSN is not object in json",
                ))),
            },
            None => Err(MSeedError::ExtraHeaderParse(String::from(
                "value for key=FDSN is not object in json",
            ))),
        }
    }

    pub fn validate(&self) -> Result<(), MSeedError> {
        // make sure if FDSN is in extra headers, its value is a json Object
        match &self.root.get(FDSN_EXTRA_HEADERS) {
            Some(fdsn_obj) => match fdsn_obj.as_object() {
                Some(_) => Ok(()),
                None => Err(MSeedError::ExtraHeaderParse(String::from(
                    "value for key=FDSN is not object in json",
                ))),
            },
            None => Ok(()),
        }
    }
}

impl From<Map<String, Value>> for ExtraHeaders {
    fn from(m: Map<String, Value>) -> Self {
        ExtraHeaders {  root: m }
    }
}

impl FromStr for ExtraHeaders {
    type Err = MSeedError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(ExtraHeaders { root: parse_to_map(s)?, })
    }
}

pub fn parse_to_map(s: &str) -> Result<Map<String, Value>, MSeedError> {
    let v: Value = serde_json::from_str(s)?;
    match v {
        Value::Object(map) => Ok(map),
        _ => Err(MSeedError::ExtraHeaderNotObject(v)),
    }
}

impl fmt::Display for ExtraHeaders {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{{")?;
        for (key, value) in self.root.iter() {
            write!(f, "\"{}\":{}", key, value)?;
        }
        write!(f, "}}")
    }
}
