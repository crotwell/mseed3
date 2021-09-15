use crate::mseed_error::MSeedError;
use crate::steim_frame_block::{SteimFrame, SteimFrameBlock};
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
pub fn decode_with_bias(
    b: &[u8],
    num_samples: u32,
    swap_bytes: bool,
    bias: i32,
) -> Result<Vec<i32>, MSeedError> {
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

    //System.err.println("DEBUG: number of samples: " + num_samples + ", number of frames: " + num_frames + ", byte array size: " + b.length);
    for i in 0..num_frames {
        //System.err.println("DEBUG: start of frame " + i);
        let temp_samples = extractSamples(b, i * 64)?; // returns only differences except for frame 0
        let mut first_data = 0; // d(0) is byte 0 by default
        if i == 0 {
            // special case for first frame
            last_value = bias; // assign our X(-1)
                               // x0 and xn are in 1 and 2 spots
            start = temp_samples[1]; // X(0) is byte 1 for frame 0
            end = temp_samples[2]; // X(n) is byte 2 for frame 0
            first_data = 3; // d(0) is byte 3 for frame 0
                            //System.err.println("DEBUG: frame " + i + ", bias = " + bias + ", x(0) = " + start + ", x(n) = " + end);
                            // if bias was zero, then we want the first sample to be X(0) constant
            if bias == 0 {
                last_value = start - temp_samples[3]; // X(-1) = X(0) - d(0)
            }
        }
        //System.err.print("DEBUG: ");
        for s in temp_samples {
            last_value = last_value + s;
            samples.push(last_value)
        }
        //System.err.println("DEBUG: end of frame " + i);
    } // end for each frame...
    if samples.len() != nsamp {
        return Err(MSeedError::Compression(format!(
            "Number of samples decompressed doesn't match number in header: {} != {}",
            samples.len(),
            num_samples
        )));
    }
    // ignore last sample check???
    //if (end != samples[num_samples-1]) {
    //    throw new SteimException("Last sample decompressed doesn't match value x(n) value in Steim1 record: "+samples[num_samples-1]+" != "+end);
    //}
    return Ok(samples);
}

/**
 * Abbreviated, zero-bias version of decode().
 *
 * see edu.iris.Fissures.codec.Steim1#decode(byte[],int,boolean,int)
 */
pub fn decode(b: &[u8], num_samples: u32, swap_bytes: bool) -> Result<Vec<i32>, MSeedError> {
    // zero-bias version of decode
    return decode_with_bias(b, num_samples, swap_bytes, 0);
}

/**
	* Encode the array of integer values into a Steim 1 * compressed byte frame block.
	* This algorithm will not create a byte block any greater * than 63 64-byte frames.
	* <b>frames</b> represents the number of frames to be written.
	* This number should be determined from the desired logical record length
	* <i>minus</i> the data offset from the record header (modulo 64)
	* If <b>samples</b> is exhausted before all frames are filled, the remaining frames
	* will be nulls.
	* <b>bias</b> is a value carried over from a previous data record, representing
	* X(-1)...set to 0 otherwise
	* @param samples the data points represented as signed integers
	* @param frames the number of Steim frames to use in the encoding
	* @param bias offset for use as a constant for the first difference, otherwise
	* set to 0
	* @return SteimFrameBlock containing encoded byte array
	* @throws SteimException samples array is zero size
	* @throws SteimException number of frames is not a positive value
	* @throws SteimException cannot encode more than 63 frames
	*/
pub fn encode_with_bias(
    samples: &[i32],
    frames: usize,
    bias: i32,
) -> Result<SteimFrameBlock, MSeedError> {
    return encode_with_offset(samples, frames, bias, 0);
}

pub fn encode_with_offset(
    samples: &[i32],
    frames: usize,
    bias: i32,
    offset: usize,
) -> Result<SteimFrameBlock, MSeedError> {
    if samples.len() == 0 {
        return Err(MSeedError::Compression(String::from(
            "samples array is zero size",
        )));
    }
    if frames <= 0 {
        return Err(MSeedError::Compression(String::from(
            "number of frames is not a positive value",
        )));
    }
    if frames > 63 {
        return Err(MSeedError::Compression(format!(
            "cannot encode more than 63 frames, you asked for {}",
            frames
        )));
    }
    if offset >= samples.len() {
        return Err(MSeedError::Compression(format!(
            "Offset bigger than samples array: {} >= {}",
            offset,
            samples.len()
        )));
    }
    // all encoding will be contained within a frame block
    // Steim encoding 1
    let mut frameBlock = SteimFrameBlock::new(1);
    //
    // pass through the list of samples, and pass encoded words
    // to frame block
    // end loop if we run out of samples or the frame block
    // fills up
    // .............................................................
    // first initialize the first frame with integration constant X(0)
    // and reverse integration constant X(N)
    // ...reverse integration constant may need to be changed if
    // the frameBlock fills up.
    let mut frame = SteimFrame::new();
    //
    // now begin looping over differences
    // iterator produces first sample, then differences to all remaining values
    let mut diff_iter = samples.iter().scan(0, |state, &x| {
        let d = x - *state;
        *state = x;
        Some(d)
    });

    // set first value into first 4 bytes after nibbles
    frame.set_word(
        u32::from_be_bytes(diff_iter.next().unwrap().to_be_bytes()),
        0,
        0,
    );
    // and last value after
    frame.set_word(
        u32::from_be_bytes(samples[samples.len() - 1].to_be_bytes()),
        0,
        1,
    );
    let mut frame_idx: usize = 2;
    let num_samples = 1;

    let by_four = ByFours::new(diff_iter);
    for chunk in by_four {
        frame_idx = chunk.add_to_frame(&mut frame, frame_idx);
        if frame_idx == 15 {
            // filled the frame
            frameBlock.steim_frame.push(frame);
            frame = SteimFrame::new();
            frame_idx = 0;
        }
    }
    frameBlock.num_samples = samples.len();
    return Ok(frameBlock);
}

/**
 * Abbreviated zero-bias version of encode().
 * see edu.iris.Fissures.codec.Steim1#encode(int[],int,int)
 */
pub fn encode(samples: &[i32], frames: usize) -> Result<SteimFrameBlock, MSeedError> {
    return encode_with_bias(samples, frames, 0); // zero-bias version of encode
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
fn extractSamples(bytes: &[u8], offset: usize) -> Result<Vec<i32>, MSeedError> {
    /* get nibbles */
    let nibbles = <[u8; 4]>::try_from(&bytes[offset..offset + 4]).unwrap();
    let nibbles = u32::from_be_bytes(nibbles);
    let mut temp = Vec::new(); // 4 samples * 16 longwords, can't be more
    let mut currNum = 0;
    //System.err.print ("DEBUG: ");
    for i in 1..16 {
        // i is the word number of the frame starting at 0
        //currNibble = (nibbles >>> (30 - i*2 ) ) & 0x03; // count from top to bottom each nibble in W(0)
        let currNibble = (nibbles >> (30 - i * 2)) & 0x03; // count from top to bottom each nibble in W(0)
                                                           //System.err.print("c(" + i + ")" + currNibble + ",");  // DEBUG
                                                           // Rule appears to be:
                                                           // only check for byte-swap on actual value-atoms, so a 32-bit word in of itself
                                                           // is not swapped, but two 16-bit short *values* are or a single
                                                           // 32-bit int *value* is, if the flag is set to TRUE.  8-bit values
                                                           // are naturally not swapped.
                                                           // It would seem that the W(0) word is swap-checked, though, which is confusing...
                                                           // maybe it has to do with the reference to high-order bits for c(0)
        match currNibble {
            0 => {
                //System.out.println("0 means header info");
                // only include header info if offset is 0
                // headers can only occur in the second and third 4-byte chunk, so ignore after that
                // second byte, i=1, holds first sample
                // third word, i=2, holds last sample, only used for validation
                if (offset == 0 && i == 1) {
                    let v = <[u8; 4]>::try_from(&bytes[offset..offset + 4]).unwrap();
                    let v = i32::from_be_bytes(v);
                    temp.push(v);
                    currNum += 1;
                }
            }
            1 => {
                //System.out.println("1 means 4 one byte differences");
                for n in 0..4 {
                    temp.push((bytes[offset + (i * 4) + n] as i8) as i32);
                }
                currNum += 4;
            }
            2 => {
                //System.out.println("2 means 2 two byte differences");
                for n in 0..2 {
                    let v = <[u8; 2]>::try_from(&bytes[offset..offset + 2]).unwrap();
                    let v = i16::from_be_bytes(v);
                    temp.push(v as i32);
                }
                currNum += 2;
            }
            3 => {
                //System.out.println("3 means 1 four byte difference");
                let v = <[u8; 4]>::try_from(&bytes[offset..offset + 4]).unwrap();
                let v = i32::from_be_bytes(v);
                temp.push(v);
                currNum += 1;
            }
            _ => {
                panic!("Cannot happen");
            } //System.out.println("default");
        }
    }
    return Ok(temp);
}

struct ByFours<I>
where
    I: Iterator<Item = i32>,
{
    diff_iter: I,
    prev: VecDeque<i32>,
}
impl<I> ByFours<I>
where
    I: Iterator<Item = i32>,
{
    pub fn new(mut diff_iter: I) -> ByFours<I> {
        ByFours::<I> {
            diff_iter,
            prev: VecDeque::new(),
        }
    }
}
impl<Iter> Iterator for ByFours<Iter>
where
    Iter: Iterator<Item = i32>,
{
    type Item = Steim1Word;

    fn next(&mut self) -> Option<Self::Item> {
        while (self.prev.len() < 4) {
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
            return Some(Steim1Word::Four(
                self.prev.pop_front()? as i8,
                self.prev.pop_front()? as i8,
                self.prev.pop_front()? as i8,
                0,
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

enum Steim1Word {
    Four(i8, i8, i8, i8),
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
            Steim1Word::Two(a, b) => {
                let a = a.to_be_bytes();
                let b = b.to_be_bytes();
                u32::from_be_bytes([a[0], a[1], b[0], b[1]])
            }
            Steim1Word::One(a) => u32::from_be_bytes(a.to_be_bytes()),
        };
        let nibble = match self {
            Steim1Word::Four(a, b, c, d) => 1 as u32,
            Steim1Word::Two(a, b) => 2 as u32,
            Steim1Word::One(a) => 3 as u32,
        };
        frame.set_word(word, nibble, frame_idx);
        frame_idx + 1
    }
}
