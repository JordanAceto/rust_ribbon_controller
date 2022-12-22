// cargo flash --chip stm32l031f4px --release

#![no_std]
#![no_main]

mod board;
mod ribbon_controller;

use crate::board::Board;
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
        let ribbon_val = ribbon.value();

        // TODO: we need to shift the signals to get them in the right range
        // eventually this should be changed so that we don't need shifts
        board.set_dac((ribbon_val >> 2) as u16);
        board.set_gate(ribbon.gate());
    }
}
