use crate::board;

/// A quantizer which converts smooth inputs into stairsteps is represented here.
///
/// Quantizers are used in musical systems to force smoothly changing signals to to take on discrete note values so
/// that the musician can more easily play in-tune.
pub struct Quantizer {
    /// The cached last conversion
    last_conversion: u16,
}

impl Quantizer {
    /// `Quantizer::new()` is a new quantizer.
    pub fn new() -> Self {
        Self { last_conversion: 0 }
    }

    /// `qn.convert(val)` is the quantized version of the input value.
    ///
    /// # Arguments
    ///
    /// * `val` - the value to quantize
    pub fn convert(&mut self, val: u16) -> u16 {
        // center the last val in the middle of its bucket so we can check if the new val is close or far to the center
        let last_centered_conversion = self.last_conversion + HALF_BUCKET_WIDTH;

        // check how far the new val is from the center of the last conversion
        let abs_diff = if val < last_centered_conversion {
            last_centered_conversion - val
        } else {
            val - last_centered_conversion
        };

        // only register a new conversion if the input is far enough away from the last one
        if (HALF_BUCKET_WIDTH + HYSTERESIS) < abs_diff {
            self.last_conversion = (val / BUCKET_WIDTH) * BUCKET_WIDTH;
        }

        self.last_conversion
    }
}

/// The number of octaves that the quantizer can handle.
const NUM_OCTAVES: u16 = 2;

/// The number of semitones the quantizer can handle.
///
/// The +1 is so you end at an octave instead of a major-7
const NUM_SEMITONES: u16 = NUM_OCTAVES * 12 + 1;

/// The width of each bucket for the semitones.
const BUCKET_WIDTH: u16 = board::ADC_MAX / NUM_SEMITONES;

/// 1/2 bucket width
const HALF_BUCKET_WIDTH: u16 = BUCKET_WIDTH / 2;

/// Hysteresis provides some noise immunity and prevents oscillations near transition regions.
///
/// Derived empirically, can be adjusted after testing the hardware
const HYSTERESIS: u16 = BUCKET_WIDTH / 10;
