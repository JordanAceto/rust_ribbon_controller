// cargo flash --chip stm32l031f4px --release

#![no_std]
#![no_main]

mod board;
mod midi;
mod quantizer;
mod ribbon_controller;

use crate::board::{Board, Mcp4822Channel};
use crate::quantizer::Quantizer;
use crate::ribbon_controller::RibbonController;

use panic_halt as _;

use cortex_m_rt::entry;

/// Do a simple demo of the hardware to test the ribbon controller
///
/// Simply read the raw analog ribbon signal and then write the processed ribbon
/// value via DAC, MIDI, and GPIO gate pin.
///
/// Both channela of the DAC get the smooth ribbon value.
///
/// A quantized version of the ribbon is sent as MIDI note on and off messages via the USART
#[entry]
fn main() -> ! {
    let mut board = Board::init();

    let mut ribbon = RibbonController::new();

    let mut quantizer = Quantizer::new();

    let midi = midi::Midi::new(0);

    let mut last_midi_note = 0;

    loop {
        let raw_adc_val = board.get_raw_adc() as usize;
        ribbon.poll(raw_adc_val);

        let smooth_ribbon = ribbon.value() as u16;
        let this_midi_note = quantizer.convert(smooth_ribbon) as u8;
        let this_gate = ribbon.gate();

        // shift the 16-bit signals for the 12-bit DAC
        board.mcp4822_write(smooth_ribbon >> 4, Mcp4822Channel::A);
        board.mcp4822_write(smooth_ribbon >> 4, Mcp4822Channel::B);

        if this_gate {
            midi.note_on(&mut board, this_midi_note);

            // make sure to turn off old notes, you might have slid into a new one
            if last_midi_note != this_midi_note {
                midi.note_off(&mut board, last_midi_note);
            }
        } else {
            midi.note_off(&mut board, this_midi_note);
            midi.note_off(&mut board, last_midi_note);
        }

        board.set_gate(this_gate);

        last_midi_note = this_midi_note;
    }
}
