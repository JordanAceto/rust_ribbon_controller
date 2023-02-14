// cargo flash --chip stm32l031f4px --release

#![no_std]
#![no_main]

mod board;
mod midi_note_converter;
mod midi_transmitter;
mod ribbon_controller;

use crate::board::{Board, Mcp4822Channel};
use crate::midi_note_converter::MidiNoteConverter;
use crate::ribbon_controller::RibbonController;

use panic_halt as _;

use cortex_m_rt::entry;

/// `offset_and_clamp(v, o)` applies the offset `o` to value `v` and clamps between zero and the ADC max
fn offset_and_clamp(initial_val: u16, offset: i32) -> u16 {
    // apply the offset, result might be positive or negative
    let signed_offset_val = initial_val as i32 - offset;

    // clamp and return
    if signed_offset_val < 0 {
        0
    } else if (board::ADC_MAX as i32) < signed_offset_val {
        board::ADC_MAX as u16
    } else {
        signed_offset_val as u16
    }
}

/// Do a simple demo of the hardware to test the ribbon controller
///
/// Reads the raw analog ribbon signal and then writes the processed ribbon
/// value via DAC, MIDI, and GPIO gate pin.
#[entry]
fn main() -> ! {
    let mut board = Board::init();
    let mut ribbon = RibbonController::new();
    let midi = midi_transmitter::MidiTransmitter::new(0);
    let mut midi_converter = MidiNoteConverter::new();

    let mut last_midi_note: u8 = 0;

    let mut offset_when_finger_pressed_down: i32 = 0;

    loop {
        let raw_adc_val = board.get_raw_adc() as usize;
        ribbon.poll(raw_adc_val);

        let smooth_ribbon = ribbon.value() as u16;

        // shift the 16-bit signals for the 12-bit DAC
        board.mcp4822_write(smooth_ribbon >> 4, Mcp4822Channel::A);
        board.mcp4822_write(smooth_ribbon >> 4, Mcp4822Channel::B);

        if ribbon.gate() {
            if ribbon.rising_gate() {
                // When the user first presses down after having lifted their finger
                // note the offset between the finger position and the center of the note.
                // We'll use this offset to make sure that it plays a nice in-tune note at first-press.
                let initial_conversion = midi_converter.convert(smooth_ribbon);
                offset_when_finger_pressed_down = initial_conversion.offset_from_note_center;
            } else {
                // The user is continuing to press the ribbon

                // Use the offset from when the finger was first pressed down to recenter the signal
                let offset_ribbon =
                    offset_and_clamp(smooth_ribbon, offset_when_finger_pressed_down);

                // A small positve offset helps to avoid bad behavior at the bottom of the ribbon, otherwise it can get
                // stuck and never get down to the lowest note. Adjusted to taste.
                let fudge_factor = 750;
                let offset_conversion = midi_converter.convert(offset_ribbon + fudge_factor);

                midi.note_on(&mut board, offset_conversion.note);
                midi.pitch_bend(&mut board, offset_conversion.pitch_bend);

                // Kill any lingering notes that have since changed
                if last_midi_note != offset_conversion.note {
                    midi.note_off(&mut board, last_midi_note);
                }

                last_midi_note = offset_conversion.note;
            }

            board.set_gate(true);
        }

        if ribbon.falling_gate() {
            // The user just lifted their finger off the ribbon, make sure to kill any active note, but leave pitch bend alone
            midi.note_off(&mut board, last_midi_note);

            board.set_gate(false);
        }
    }
}
