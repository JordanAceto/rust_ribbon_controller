// cargo flash --chip stm32l031f4px --release

#![no_std]
#![no_main]

mod board;
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
/// value via DAC and gate via GPIO pin.
///
/// One channel of the DAC gets a quantized version of the ribbon, and the other
/// channel gets the smooth original ribbon value.
#[entry]
fn main() -> ! {
    let mut board = Board::init();

    let mut ribbon = RibbonController::new();

    let mut quantizer = Quantizer::new();

    loop {
        let raw_adc_val = board.get_raw_adc() as usize;
        ribbon.poll(raw_adc_val);

        let ribbon_val = ribbon.value() as u16;
        let quantized_ribbon = quantizer.convert(ribbon_val);

        // shift the 16-bit signals for the 12-bit DAC
        board.mcp4822_write(quantized_ribbon >> 4, Mcp4822Channel::A);
        board.mcp4822_write(ribbon_val >> 4, Mcp4822Channel::B);

        board.set_gate(ribbon.gate());
    }
}
