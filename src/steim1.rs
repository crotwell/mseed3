use crate::mseed_error::MSeedError;
use crate::steim_frame_block::{SteimFrame, SteimFrameBlock};
use serde::{Serialize, Deserialize};
use std::collections::VecDeque;
use std::convert::TryFrom;

/**
 * Class for decoding or encoding Steim1-compressed data blocks
 * to or from an array of integer values.
 * <p>
 * Steim compression scheme Copyrighted by Dr. Joseph Steim.<p>
 * <dl>
 * <dt>Reference material found in:</dt>
 * <dd>
 * Appendix B of SEED Reference Manual, 2nd Ed., pp. 119-125
 * <i>Federation of Digital Seismic Networks, et al.</i>
 * February, 1993
 * </dd>
 * <dt>Coding concepts gleaned from code written by:</dt>
 * <dd>Guy Stewart, IRIS, 1991</dd>
 * <dd>Tom McSweeney, IRIS, 2000</dd>
 * </dl>
 *
 * @author Philip Crotwell (U South Carolina)
 * @author Robert Casey (IRIS DMC)
 * @version 10/22/2002
 */

/**
 *  Decode the indicated number of samples from the provided byte array and
 *  return an integer array of the decompressed values.  Being differencing
 *  compression, there may be an offset carried over from a previous data
 *  record.  This offset value can be placed in <b>bias</b>, otherwise leave
 *  the value as 0.
 *  @param b input byte array to be decoded
 *  @param num_samples the number of samples that can be decoded from array
 *  <b>b</b>
 *  @param swap_bytes if true, swap reverse the endian-ness of the elements of
 *  byte array <b>b</b>.
 *  @param bias the first difference value will be computed from this value.
 *  If set to 0, the method will attempt to use the X(0) constant instead.
 *  @return int array of length <b>num_samples</b>.
 *  @throws SteimException - encoded data length is not multiple of 64
 *  bytes.
 */
pub fn decode_with_bias(b: &[u8], num_samples: u32) -> Result<Vec<i32>, MSeedError> {
    // Decode Steim1 compression format from the provided byte array, which contains num_samples number
    // of samples.  swap_bytes is set to true if the value words are to be byte swapped.  bias represents
    // a previous value which acts as a starting constant for continuing differences integration.  At the
    // very start, bias is set to 0.
    if b.len() % 64 != 0 {
        return Err(MSeedError::Compression(format!(
            "encoded data length is not multiple of 64 bytes ({})",
            b.len()
        )));
    }
    let nsamp = num_samples as usize;
    let mut samples = Vec::with_capacity(nsamp);
    let num_frames = b.len() / 64;
    let mut start = 0;
    let mut end = 0;
    let mut last_value = 0;

    for i in 0..num_frames {
        let temp_samples = extract_samples(b, i * 64)?; // returns only differences except for frame 0
                                                        // d(0) is byte 0 by default
        let mut ts_itr = temp_samples.iter();
        if i == 0 {
            // special case for first frame
            start = *ts_itr.next().unwrap(); // X(0) is byte 1 for frame 0
            samples.push(start);
            last_value = start;
            end = *ts_itr.next().unwrap(); // X(n) is byte 2 for frame 0
        }
        for s in ts_itr {
            last_value = last_value + s;
            samples.push(last_value)
        }
    } // end for each frame...
    if samples.len() != nsamp {
        return Err(MSeedError::Compression(format!(
            "Number of samples decompressed doesn't match number in header: decomp: {} != {}, header",
            samples.len(),
            num_samples
        )));
    }
    assert_eq!(samples[0], start);
    assert_eq!(samples[samples.len() - 1], end);
    return Ok(samples);
}

/**
 * Abbreviated, zero-bias version of decode().
 *
 * see edu.iris.Fissures.codec.Steim1#decode(byte[],int,boolean,int)
 */
pub fn decode(b: &[u8], num_samples: u32) -> Result<Vec<i32>, MSeedError> {
    // zero-bias version of decode
    return decode_with_bias(b, num_samples);
}

/**
	* Encode the array of integer values into a Steim 1 * compressed byte frame block.
	* This algorithm will not create a byte block any greater * than 63 64-byte frames.
	* <b>frames</b> represents the maximum number of frames to be written.
	* This number should be determined from the desired logical record length
	* <i>minus</i> the data offset from the record header (modulo 64)
	* If <b>samples</b> is exhausted before all frames are filled, the remaining frames
	* will be nulls.
	* @param samples the data points represented as signed integers
	* @param frames the number of Steim frames to use in the encoding, 0 for unlimited
	* @param bias offset for use as a constant for the first difference, otherwise
	* set to 0
	* @return SteimFrameBlock containing encoded byte array
	* @throws SteimException samples array is zero size
	* @throws SteimException number of frames is not a positive value
	* @throws SteimException cannot encode more than 63 frames
	*/
pub fn encode(samples: &[i32], frames: usize) -> Result<SteimFrameBlock, MSeedError> {
    if samples.len() == 0 {
        return Err(MSeedError::Compression(String::from(
            "samples array is zero size",
        )));
    }
    // all encoding will be contained within a frame block
    // Steim encoding 1
    let mut frame_block = SteimFrameBlock::new(1);
    //
    // pass through the list of samples, and pass encoded words
    // to frame block
    // end loop if we run out of samples or the frame block
    // fills up
    // .............................................................
    // first initialize the first frame with integration constant X(0)
    // and reverse integration constant X(N)
    // ...reverse integration constant may need to be changed if
    // the frame_block fills up.
    //
    // now begin looping over differences
    // iterator produces first sample, then differences to all remaining values
    let diff_iter = samples.iter().scan(0, |state, &x| {
        let d = x - *state;
        *state = x;
        Some(d)
    });

    let mut num_samples = 0;
    let by_four = ByFours::new(diff_iter);
    let mut first_sample = true;

    'outer: loop {
        let mut frame = SteimFrame::new();
        let mut frame_idx = 0;
        for chunk in by_four {
            if first_sample {
                match chunk {
                    Steim1Word::One(v) => frame.set_word(u32::from_be_bytes(v.to_be_bytes()), 0, 0),
                    v => panic!("first sample must be one 4-byte value, but {:?}", v),
                }
                first_sample = false;
                frame_idx += 2; //skip past the last sample in second word
            } else {
                frame_idx = chunk.add_to_frame(&mut frame, frame_idx);
            }
            num_samples += chunk.num_samples();
            if frame_idx == 15 {
                // filled the frame, push a new one
                if frame_block.steim_frame.len() == frames {
                    // zero means unlimited, but len() always >=1, so ok
                    frame_block.steim_frame.push(frame);
                    break 'outer;
                }
                break;
            }
        }
        if frame_idx > 0 {
            // last partially filled the frame, push
            frame_block.steim_frame.push(frame);
        }
        break;
    }
    frame_block.num_samples = num_samples;
    assert_ne!(frame_block.steim_frame.len(), 0);
    frame_block.reverse_integration_constant(samples[num_samples - 1]);
    return Ok(frame_block);
}

/**
 * Extracts differences from the next 64 byte frame of the given compressed
 * byte array (starting at offset) and returns those differences in an int
 * array.
 * An offset of 0 means that we are at the first frame, so include the header
 * bytes in the returned int array...else, do not include the header bytes
 * in the returned array.
 * @param bytes byte array of compressed data differences
 * @param offset index to begin reading compressed bytes for decoding
 * @param swapBytes reverse the endian-ness of the compressed bytes being read
 * @return integer array of difference (and constant) values
 */
fn extract_samples(bytes: &[u8], offset: usize) -> Result<Vec<i32>, MSeedError> {
    /* get nibbles */
    let nibbles = <[u8; 4]>::try_from(&bytes[offset..offset + 4]).unwrap();
    let nibbles = u32::from_be_bytes(nibbles);
    let mut temp = Vec::new(); // 4 samples * 16 longwords, can't be more
    for i in 1..16 {
        // i is the word number of the frame starting at 0
        //curr_nibble = (nibbles >>> (30 - i*2 ) ) & 0x03; // count from top to bottom each nibble in W(0)
        let curr_nibble = (nibbles >> (32 - i * 2)) & 0x03; // count from top to bottom each nibble in W(0)
                                                            // Rule appears to be:
                                                            // only check for byte-swap on actual value-atoms, so a 32-bit word in of itself
                                                            // is not swapped, but two 16-bit short *values* are or a single
                                                            // 32-bit int *value* is, if the flag is set to TRUE.  8-bit values
                                                            // are naturally not swapped.
                                                            // It would seem that the W(0) word is swap-checked, though, which is confusing...
                                                            // maybe it has to do with the reference to high-order bits for c(0)
        let offset_idx = offset + 4 * i;
        match curr_nibble {
            0 => {
                // only include header info if offset is 0
                // headers can only occur in the second and third 4-byte chunk, so ignore after that
                // second byte, i=1, holds first sample
                // third word, i=2, holds last sample, only used for validation
                if offset == 0 && (i == 1 || i == 2) {
                    let v = <[u8; 4]>::try_from(&bytes[offset_idx..offset_idx + 4]).unwrap();
                    let v = i32::from_be_bytes(v);
                    temp.push(v);
                }
            }
            1 => {
                //"1 means 4 one byte differences");
                for n in 0..4 {
                    temp.push((bytes[offset_idx + (i * 4) + n] as i8) as i32);
                }
            }
            2 => {
                //("2 means 2 two byte differences");
                for n in 0..2 {
                    let v =
                        <[u8; 2]>::try_from(&bytes[(offset_idx + 2 * n)..(offset_idx + 2 + 2 * n)])
                            .unwrap();
                    temp.push(i16::from_be_bytes(v) as i32);
                }
            }
            3 => {
                //("3 means 1 four byte difference");
                let v = <[u8; 4]>::try_from(&bytes[offset_idx..offset_idx + 4]).unwrap();
                let v = i32::from_be_bytes(v);
                temp.push(v);
            }
            _ => {
                panic!("Cannot happen");
            }
        }
    }
    return Ok(temp);
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ByFours<I>
where
    I: Iterator<Item = i32>,
{
    diff_iter: I,
    prev: VecDeque<i32>,
    first: bool,
}
impl<I> ByFours<I>
where
    I: Iterator<Item = i32>,
{
    pub fn new(diff_iter: I) -> ByFours<I> {
        ByFours::<I> {
            diff_iter,
            prev: VecDeque::new(),
            first: true,
        }
    }
}
impl<Iter> Iterator for ByFours<Iter>
where
    Iter: Iterator<Item = i32>,
{
    type Item = Steim1Word;

    fn next(&mut self) -> Option<Self::Item> {
        if self.first {
            self.first = false;
            // first is always single 4-byte value
            return Some(Steim1Word::One(self.diff_iter.next()?));
        }
        while self.prev.len() < 4 {
            match &self.diff_iter.next() {
                Some(v) => self.prev.push_back(*v),
                None => {
                    if self.prev.len() > 0 {
                        break;
                    } else {
                        return None;
                    }
                }
            }
        }
        if self.prev.len() == 4
            && ok_i8(self.prev[0])
            && ok_i8(self.prev[1])
            && ok_i8(self.prev[2])
            && ok_i8(self.prev[3])
        {
            // four one-byte values
            return Some(Steim1Word::Four(
                self.prev.pop_front()? as i8,
                self.prev.pop_front()? as i8,
                self.prev.pop_front()? as i8,
                self.prev.pop_front()? as i8,
            ));
        } else if self.prev.len() == 3
            && ok_i8(self.prev[0])
            && ok_i8(self.prev[1])
            && ok_i8(self.prev[2])
        {
            // this case should only happen at end, so pad with 0 to encode 4 bytes
            return Some(Steim1Word::Three(
                self.prev.pop_front()? as i8,
                self.prev.pop_front()? as i8,
                self.prev.pop_front()? as i8,
            ));
        } else if self.prev.len() > 2 && ok_i16(self.prev[0]) && ok_i16(self.prev[1]) {
            // two two-byte values
            return Some(Steim1Word::Two(
                self.prev.pop_front()? as i16,
                self.prev.pop_front()? as i16,
            ));
        } else if self.prev.len() != 0 {
            // single 4-byte value
            return Some(Steim1Word::One(self.prev.pop_front()?));
        }
        None
    }
}

pub fn ok_i8(v: i32) -> bool {
    -128 <= v && v <= 127
}
pub fn ok_i16(v: i32) -> bool {
    -32768 <= v && v <= 32767
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum Steim1Word {
    Four(i8, i8, i8, i8),
    Three(i8, i8, i8),
    Two(i16, i16),
    One(i32),
}

impl Steim1Word {
    pub fn add_to_frame(&self, frame: &mut SteimFrame, frame_idx: usize) -> usize {
        let word = match self {
            Steim1Word::Four(a, b, c, d) => u32::from_be_bytes([
                a.to_be_bytes()[0],
                b.to_be_bytes()[0],
                c.to_be_bytes()[0],
                d.to_be_bytes()[0],
            ]),
            Steim1Word::Three(a, b, c) => u32::from_be_bytes([
                a.to_be_bytes()[0],
                b.to_be_bytes()[0],
                c.to_be_bytes()[0],
                0,
            ]),
            Steim1Word::Two(a, b) => {
                let a = a.to_be_bytes();
                let b = b.to_be_bytes();
                u32::from_be_bytes([a[0], a[1], b[0], b[1]])
            }
            Steim1Word::One(a) => u32::from_be_bytes(a.to_be_bytes()),
        };
        let nibble = match self {
            Steim1Word::Four(_a, _b, _c, _d) => 1 as u32,
            Steim1Word::Three(_a, _b, _c) => 1 as u32,
            Steim1Word::Two(_a, _b) => 2 as u32,
            Steim1Word::One(_a) => 3 as u32,
        };
        frame.set_word(word, nibble, frame_idx);
        frame_idx + 1
    }
    pub fn num_samples(&self) -> usize {
        match self {
            Steim1Word::Four(_a, _b, _c, _d) => 4 as usize,
            Steim1Word::Three(_a, _b, _c) => 3 as usize,
            Steim1Word::Two(_a, _b) => 2 as usize,
            Steim1Word::One(_a) => 1 as usize,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_iter() {
        let data = [1, -1, -1, -1, 20, -300, 160, -18000];
        let mut diff_iter = data.iter().scan(0, |state, &x| {
            let d = x - *state;
            *state = x;
            Some(d)
        });
        assert_eq!(diff_iter.next().unwrap(), data[0]);
        for i in 1..data.len() {
            assert_eq!(diff_iter.next().unwrap(), data[i] - data[i - 1]);
        }
    }

    #[test]
    fn by_four() -> Result<(), MSeedError> {
        let data = [1, -1, -1, -1, 20, -300, 160, -18000];
        let diff_iter = data.iter().scan(0, |state, &x| {
            let d = x - *state;
            *state = x;
            Some(d)
        });
        let mut found = 0;
        let mut byfour = ByFours::new(diff_iter);

        if let Steim1Word::One(_) = byfour.next().unwrap() {
            // first should be single
            found += 1;
            if let Steim1Word::Four(_, _, _, _) = byfour.next().unwrap() {
                // next 4 1-byte values, -1, -1, -1, 20
                found += 4;
                if let Steim1Word::Two(_, _) = byfour.next().unwrap() {
                    // next 2 2-byte values, -300, 160
                    found += 2;
                    if let Steim1Word::One(_) = byfour.next().unwrap() {
                        // then single value -18000
                        found += 1;
                    }
                }
            }
        }
        assert_eq!(found, data.len());
        return Ok(());
    }

    #[test]
    fn data_round_trip() -> Result<(), MSeedError> {
        let data = [1, -1, -1, -1, 200, -300, 16000, -18000, 20000, -40000];
        let frame_block = encode(&data, 0)?;
        assert_eq!(data.len(), frame_block.num_samples);
        assert_ne!(frame_block.steim_frame.len(), 0);
        assert_eq!(
            data[0],
            i32::from_be_bytes(frame_block.steim_frame[0].words[0].to_be_bytes())
        );
        let enc_bytes = &frame_block.get_encoded_data()?;
        assert_eq!(enc_bytes[4], 0);
        assert_eq!(enc_bytes[5], 0);
        assert_eq!(enc_bytes[6], 0);
        assert_eq!(enc_bytes[7], 1);
        let frame_data = extract_samples(&enc_bytes[0..64], 0)?;
        assert_eq!(frame_data[0], 1);
        assert_eq!(frame_data[1], -40000); // last sample
        for i in 2..frame_data.len() {
            assert_eq!(frame_data[i], data[i - 1] - data[i - 2], "i: {} ", i);
        }
        let rt_data = decode(
            &frame_block.get_encoded_data()?,
            frame_block.num_samples as u32,
        )?;
        assert_eq!(rt_data.len(), data.len());
        let mut idx = 0;
        for pair in rt_data.iter().zip(data.iter()) {
            assert_eq!(pair.0, pair.1, " index {}", idx);
            idx += 1;
        }
        Ok(())
    }
}
