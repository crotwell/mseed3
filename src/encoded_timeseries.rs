use byteorder::{LittleEndian, WriteBytesExt};
use serde::{Serialize, Deserialize};
use std::fmt;
use std::fmt::Formatter;
use std::io::prelude::*;
use std::io::BufWriter;

use crate::mseed_error::MSeedError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum EncodedTimeseries {
    Raw(Vec<u8>),
    Int16(Vec<i16>),
    Int32(Vec<i32>),
    Float32(Vec<f32>),
    Float64(Vec<f64>),
    Steim1(Vec<u8>),
    Steim2(Vec<u8>),
    Steim3(Vec<u8>),
    Opaque(Vec<u8>),
}

impl EncodedTimeseries {
    pub fn write_to<W>(&self, buf: &mut BufWriter<W>) -> Result<(), MSeedError>
    where
        W: std::io::Write,
    {
        match self {
            EncodedTimeseries::Raw(v) => {
                buf.write_all(v)?;
                Ok(())
            }
            EncodedTimeseries::Int16(v) => {
                for &el in v {
                    buf.write_i16::<LittleEndian>(el)?;
                }
                Ok(())
            }
            EncodedTimeseries::Int32(v) => {
                for &el in v {
                    buf.write_i32::<LittleEndian>(el)?;
                }
                Ok(())
            }
            EncodedTimeseries::Float32(v) => {
                for &el in v {
                    buf.write_f32::<LittleEndian>(el)?;
                }
                Ok(())
            }
            EncodedTimeseries::Float64(v) => {
                for &el in v {
                    buf.write_f64::<LittleEndian>(el)?;
                }
                Ok(())
            }
            EncodedTimeseries::Steim1(v) => {
                buf.write_all(v)?;
                Ok(())
            }
            EncodedTimeseries::Steim2(v) => {
                buf.write_all(v)?;
                Ok(())
            }
            EncodedTimeseries::Steim3(v) => {
                buf.write_all(v)?;
                Ok(())
            }
            EncodedTimeseries::Opaque(v) => {
                buf.write_all(v)?;
                Ok(())
            }
        }
    }

    pub fn byte_len(&self) -> u32 {
        match self {
            EncodedTimeseries::Raw(v) => v.len() as u32,
            EncodedTimeseries::Int16(v) => 2 * v.len() as u32,
            EncodedTimeseries::Int32(v) => 4 * v.len() as u32,
            EncodedTimeseries::Float32(v) => 4 * v.len() as u32,
            EncodedTimeseries::Float64(v) => 8 * v.len() as u32,
            EncodedTimeseries::Steim1(v) => v.len() as u32,
            EncodedTimeseries::Steim2(v) => v.len() as u32,
            EncodedTimeseries::Steim3(v) => v.len() as u32,
            EncodedTimeseries::Opaque(v) => v.len() as u32,
        }
    }
    /// Reconciles the number of samples in the header with the size of the EncodedTimeseries.
    /// For the primitive types, Int16, Int32, Float32 and Float64 the value is calculated from
    /// the length of the array. For the remaining, the passed in header num_samples is
    /// return as it is assumed to be correct.
    pub fn reconcile_num_samples(&self, header_num_sample: u32) -> u32 {
        match self {
            EncodedTimeseries::Int16(v) => v.len() as u32,
            EncodedTimeseries::Int32(v) => v.len() as u32,
            EncodedTimeseries::Float32(v) => v.len() as u32,
            EncodedTimeseries::Float64(v) => v.len() as u32,
            EncodedTimeseries::Raw(_) => header_num_sample,
            EncodedTimeseries::Steim1(_) => header_num_sample,
            EncodedTimeseries::Steim2(_) => header_num_sample,
            EncodedTimeseries::Steim3(_) => header_num_sample,
            EncodedTimeseries::Opaque(_) => header_num_sample,
        }
    }
}

impl fmt::Display for EncodedTimeseries {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            EncodedTimeseries::Raw(v) => {
                write!(f, "Raw bytes, {} bytes", v.len())
            }
            EncodedTimeseries::Int16(v) => {
                write!(f, "Int16, {} samples", v.len())
            }
            EncodedTimeseries::Int32(v) => {
                write!(f, "Int32, {} samples", v.len())
            }
            EncodedTimeseries::Float32(v) => {
                write!(f, "Float32, {} samples", v.len())
            }
            EncodedTimeseries::Float64(v) => {
                write!(f, "Float64, {} samples", v.len())
            }
            EncodedTimeseries::Steim1(v) => {
                write!(f, "Steim1, {} bytes", v.len())
            }
            EncodedTimeseries::Steim2(v) => {
                write!(f, "Steim2, {} bytes", v.len())
            }
            EncodedTimeseries::Steim3(v) => {
                write!(f, "Steim3, {} bytes", v.len())
            }
            EncodedTimeseries::Opaque(v) => {
                write!(f, "Opaque, {} bytes", v.len())
            }
        }
    }
}
