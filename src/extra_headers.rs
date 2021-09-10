
use serde_json;
use serde_json::Value;
use serde_json::map::Map;
use std::fmt;
use crate::mseed_error::MSeedError;

pub const FDSN_EXTRA_HEADERS: &str = "FDSN";

#[derive(Debug, Clone)]
pub struct ExtraHeaders {
    raw_str: Option<String>,
    map: Map<String, Value>,
}

impl ExtraHeaders {
    pub fn new() -> ExtraHeaders {
        let map = serde_json::map::Map::new();
        ExtraHeaders {
            raw_str: None,
            map,
        }
    }

    pub fn is_parsed(&self) -> bool {
        match &self.raw_str {
            Some(_) => false,
            None => true,
        }
    }

    pub fn parse(&mut self) -> Result<(), MSeedError> {
        let raw_opt = self.raw_str.to_owned();
        self.raw_str = None;
         match raw_opt {
            Some(s) => {
                let v: Value = serde_json::from_str(&s)?;
                match v {
                    Value::Object(map) => {
                        self.map = map;
                        Ok(())
                    },
                    _ => Err(MSeedError::ExtraHeaderNotObject(v)),
                }
            },
            None => {Ok(())},
        }

    }

    pub fn get_fdsn_headers(&mut self) -> Result<&Map<String, Value>, MSeedError> {
        self.parse()?;
        if ! &self.map.contains_key(FDSN_EXTRA_HEADERS) {
            let fdsn = serde_json::map::Map::new();
            &self.map.insert(FDSN_EXTRA_HEADERS.to_string(), serde_json::Value::Object(fdsn));
        }
        let fdsn = self.map.get(FDSN_EXTRA_HEADERS).unwrap();
        match fdsn.as_object() {
            Some(fdsn_obj) => Ok(fdsn_obj),
            None => Err(MSeedError::ExtraHeaderParse(String::from("value for key=FDSN is not object in json")))
        }
    }

    pub fn validate(&mut self) -> Result<(), MSeedError> {
        self.parse()?;
        // make sure if FDSN is in extra headers, its value is a json Object
        match &self.map.get(FDSN_EXTRA_HEADERS) {
            Some(fdsn_obj) => {
                 match fdsn_obj.as_object() {
                    Some(_) => Ok(()),
                    None => Err(MSeedError::ExtraHeaderParse(String::from("value for key=FDSN is not object in json"))),
                }
            },
            None => Ok(())
        }
    }
}

impl From<String> for ExtraHeaders {
    fn from(s: String) -> Self {
        ExtraHeaders {
            raw_str: Some(s),
            map: Map::new(), // empty placeholder map
        }
    }
}

impl fmt::Display for ExtraHeaders {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.raw_str {
            Some(s) => write!(f, "{}",s),
            None => {
                write!(f, "{{")?;
                for (key, value) in self.map.iter() {
                    write!(f, "\"{}\":{}", key, value)?;
                }
                write!(f, "}}")
            }
        }
    }
}
