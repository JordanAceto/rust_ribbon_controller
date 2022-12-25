// cargo flash --chip stm32l031f4px --release

#![no_std]
#![no_main]

mod board;
mod ribbon_controller;

use crate::board::{Board, Mcp4822Channel};
use crate::ribbon_controller::RibbonController;

use panic_halt as _;

use cortex_m_rt::entry;

/// Do a simple demo of the hardware to test the ribbon controller
///
/// Simply read the raw analog ribbon signal and then write the processed ribbon
/// value via DAC and gate via GPIO pin.
#[entry]
fn main() -> ! {
    let mut board = Board::init();

    let mut ribbon = RibbonController::new();

    loop {
        let raw_adc_val = board.get_raw_adc() as usize;
        ribbon.poll(raw_adc_val);

        // shift the 16 bit ribbon signal to fit the 12 bit DAC
        let ribbon_val = (ribbon.value() >> 4) as u16;

        board.mcp4822_write(ribbon_val, Mcp4822Channel::A);
        board.mcp4822_write(ribbon_val, Mcp4822Channel::B);

        board.set_gate(ribbon.gate());
    }
}
