use crate::board;
use crate::midi_transmitter;

/// A module for converting integers into MIDI notes is represented here.
///
/// Both the MIDI note number and pitch bend value necessary to hit the desired output pitch are available.
/// Some hysteresis is applied to prevent chattering around note boundaries.
pub struct MidiNoteConverter {
    /// The cached last note value
    last_conversion: u16,
}

/// A MIDI note conversion is represented here.
///
/// This conversion provides the MIDI note number as well as the pitch bend value
pub struct Conversion {
    pub note: u8,
    pub pitch_bend: u16,
    pub offset_from_note_center: i32,
}

impl MidiNoteConverter {
    /// `MidiNoteConverter::new()` is a new MIDI note converter.
    pub fn new() -> Self {
        Self { last_conversion: 0 }
    }

    /// `mnc.convert(val)` is the input value converted to MIDI note and pitch bend.
    ///
    /// # Arguments
    ///
    /// * `raw_value` - the value to convert to MIDI
    pub fn convert(&mut self, raw_value: u16) -> Conversion {
        // center the last val in the middle of its bucket so we can check if the new val is close or far to the center
        let last_centered_conversion = self.last_conversion + HALF_BUCKET_WIDTH;

        // // check how far the new val is from the center of the last conversion
        let abs_diff = if raw_value < last_centered_conversion {
            last_centered_conversion - raw_value
        } else {
            raw_value - last_centered_conversion
        };

        // only register a new conversion if the input is far enough away from the last one
        if (HALF_BUCKET_WIDTH + HYSTERESIS) < abs_diff {
            self.last_conversion = (raw_value / BUCKET_WIDTH) * BUCKET_WIDTH;
        }

        // in [0..NUM_SEMITONES - 1]
        let this_note = self.last_conversion / BUCKET_WIDTH;

        // the same range as the raw input, but quantized to a stairstep
        let this_quantized_value = this_note * BUCKET_WIDTH;

        // positive or negative distance between the raw input and the center of the note
        let offset_from_note_center = raw_value as i32 - this_quantized_value as i32;

        // in (-1.0, +1.0)
        let fractional_note = (offset_from_note_center as f32) / (BUCKET_WIDTH as f32);

        // The extra divide by 2 is because most pitch bend is 2 semitones, we only want one semitone.
        // The order of casts is important, we might be negative at some points, but the final value is unsigned.
        let pitch_bend = (((fractional_note / 2.) * midi_transmitter::PITCH_BEND_FULL_SCALE as f32)
            + midi_transmitter::PITCH_BEND_CENTER as f32) as u16;

        Conversion {
            note: this_note as u8 + LOWEST_MIDI_NOTE,
            pitch_bend,
            offset_from_note_center,
        }
    }
}

/// The lowest possible MIDI note that can be generated
pub const LOWEST_MIDI_NOTE: u8 = 24;

/// The number of octaves that the quantizer can handle.
pub const NUM_OCTAVES: u16 = 2;

/// The number of semitones the quantizer can handle.
///
/// The +1 is so you end at an octave instead of a major-7
pub const NUM_SEMITONES: u16 = NUM_OCTAVES * 12 + 1;

/// The width of each bucket for the semitones.
pub const BUCKET_WIDTH: u16 = board::ADC_MAX / NUM_SEMITONES;

/// 1/2 bucket width
pub const HALF_BUCKET_WIDTH: u16 = BUCKET_WIDTH / 2;

/// Hysteresis provides some noise immunity and prevents oscillations near transition regions.
///
/// Derived empirically, can be adjusted after testing the hardware
const HYSTERESIS: u16 = BUCKET_WIDTH / 10;
