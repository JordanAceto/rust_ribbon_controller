// cargo flash --chip stm32l031f4px --release

#![no_std]
#![no_main]

mod board;
mod midi_note_converter;
mod midi_transmitter;
mod mode_switch;
mod ribbon_controller;

use crate::board::{Board, Mcp4822Channel};
use crate::midi_note_converter::MidiNoteConverter;
use crate::mode_switch::ModeSwitch;
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
    let midi = midi_transmitter::MidiTransmitter::new(1);
    let mut midi_converter = MidiNoteConverter::new();
    let mode_switch = ModeSwitch::new();

    let mut last_midi_note: u8 = 0;

    let mut offset_when_finger_pressed_down: i32 = 0;

    loop {
        let raw_adc_val = board.get_raw_adc() as usize;
        ribbon.poll(raw_adc_val);

        let smooth_ribbon = ribbon.value() as u16;

        // shift the 16-bit signals for the 12-bit DAC
        board.mcp4822_write(smooth_ribbon >> 4, Mcp4822Channel::A);
        board.mcp4822_write(smooth_ribbon >> 4, Mcp4822Channel::B);

        let mode = mode_switch.read(&board);

        if ribbon.gate() {
            let this_midi_note;
            let this_pitch_bend;

            match mode {
                mode_switch::Mode::HardQuantize => {
                    let conversion = midi_converter.convert(smooth_ribbon);

                    this_midi_note = conversion.note;
                    this_pitch_bend = midi_transmitter::PITCH_BEND_CENTER;
                }
                mode_switch::Mode::Smooth => {
                    // A small fudge factor helps keep smooth mode in tune with hard-quantize mode at the low and high ends
                    let fudge_factor = 850;
                    let conversion = midi_converter.convert(smooth_ribbon - fudge_factor);

                    this_midi_note = conversion.note;
                    this_pitch_bend = conversion.pitch_bend;
                }
                mode_switch::Mode::Assist => {
                    if ribbon.rising_gate() {
                        // When the user first presses down after having lifted their finger note the offset between the
                        // finger position and the center of the note. We'll use this offset to make sure that it plays
                        // a nice in-tune note at first-press.
                        let initial_conversion = midi_converter.convert(smooth_ribbon);

                        offset_when_finger_pressed_down =
                            initial_conversion.offset_from_note_center;

                        this_midi_note = initial_conversion.note;
                        this_pitch_bend = midi_transmitter::PITCH_BEND_CENTER;
                    } else {
                        // The user is continuing to press the ribbon and maybe sliding around

                        // Use the offset from when the finger was first pressed down to recenter the signal
                        let offset_ribbon =
                            offset_and_clamp(smooth_ribbon, offset_when_finger_pressed_down);

                        // take a new conversion with the offset from when they first pressed down, this makes it so the
                        // initial press is in-tune and then they can slide around from there
                        let offset_conversion = midi_converter.convert(offset_ribbon);

                        this_midi_note = offset_conversion.note;
                        this_pitch_bend = offset_conversion.pitch_bend;
                    }
                }
            };

            midi.note_on(&mut board, this_midi_note);
            midi.pitch_bend(&mut board, this_pitch_bend);

            // Kill any lingering notes that have since changed
            if last_midi_note != this_midi_note {
                midi.note_off(&mut board, last_midi_note);
            }
            last_midi_note = this_midi_note;
        }

        if ribbon.falling_gate() {
            // The user just lifted their finger off the ribbon, make sure to kill any active note, but leave pitch bend alone
            midi.note_off(&mut board, last_midi_note);
        }

        board.set_gate(ribbon.gate());
    }
}
