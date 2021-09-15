use crate::MSeedError;

use std::io::prelude::*;

/**
 * This represents a single Steim compression frame.  It stores values
 * as an int array and keeps track of it's current position in the frame.
 */
pub struct SteimFrame {
    nibbles: u32,
    pub(crate) words: [u32; 15],
    // 16 32-byte words
}

impl SteimFrame {
    pub fn new() -> SteimFrame {
        SteimFrame {
            nibbles: 0,
            words: [0; 15],
        }
    }
    pub fn is_empty(&self) -> bool {
        self.nibbles == 0
    }
    pub fn set_word(&mut self, word: u32, nibble: u32, idx: usize) {
        self.words[idx] = word;
        assert!(idx < 16, "nubble idx must be 0..16, {}",idx);
        self.nibbles = self.nibbles + (nibble << (30 - 2 * idx as u32))
    }
}

/**
 * This class acts as a container to hold encoded bytes processed
 * by a Steim compression routine, as well as supporting information
 * relating to the data processed.
 * It also facilitates Steim notation and the formation
 * of the data frames.
 * This class stores the Steim encoding, but is ignorant of the encoding
 * process itself...it's just for self-referencing.
 * @author Robert Casey (IRIS DMC)
 * @version 12/10/2001
 */

pub struct SteimFrameBlock {
    pub num_samples: usize,           // number of samples represented
    pub steim_version: usize,         // Steim version number
    pub steim_frame: Vec<SteimFrame>, // array of frames;
}

impl SteimFrameBlock {
    // *** constructors ***

    /**
     * Create a new block of Steim frames for a particular version of Steim
     * copression.
     * Instantiate object with the number of 64-byte frames
     * that this block will contain (should connect to data
     * record header such that a proper power of 2 boundary is
     * formed for the data record) AND the version of Steim
     * compression used (1 and 2 currently)
     * the number of frames remains static...frames that are
     * not filled with data are simply full of nulls.
     * @param numFrames the number of frames in this Steim record
     * @param steim_version which version of Steim compression is being used
     * (1,2,3).
     */
    pub fn new(steim_version: usize) -> SteimFrameBlock {
        SteimFrameBlock {
            steim_version,
            num_samples: 0, // number of samples represented
            steim_frame: Vec::new(),
        }
    }

    /**
     * Return the compressed byte representation of the data for inclusion
     * in a data record.
     * @return byte array containing the encoded, compressed data
     * @throws IOException from called method(s)
     */
    pub fn get_encoded_data(&self) -> Result<Vec<u8>, MSeedError> {
        let mut encoded_data = Vec::new();
        for f in &self.steim_frame {
            encoded_data.write_all(&f.nibbles.to_be_bytes())?;
            for w in f.words {
                encoded_data.write_all(&w.to_be_bytes())?;
            }
        }
        Ok(encoded_data)
    }

    /**
     * Set the reverse integration constant X(N) explicitly to the
     * provided word value.
     * This method is typically used to reset X(N) should the compressor
     * fill the frame block before all samples have been read.
     * @param word integer value to be placed in X(N)
     */
    pub fn reverse_integration_constant(&mut self, v: i32) {
        assert_ne!(self.steim_frame.len(), 0);
        self.steim_frame[0].set_word(
            u32::from_be_bytes(v.to_be_bytes()),
            0,
            1,
        );
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_integration_constant() -> Result<(), MSeedError> {
        let mut frame_block = SteimFrameBlock::new(1);
        frame_block.steim_frame.push(SteimFrame::new());
        frame_block.reverse_integration_constant(1);
        let enc_data = frame_block.get_encoded_data()?;
        assert_eq!(enc_data[8], 0 as u8);
        assert_eq!(enc_data[9], 0 as u8);
        assert_eq!(enc_data[10], 0 as u8);
        assert_eq!(enc_data[11], 1 as u8);
        Ok(())
    }
}