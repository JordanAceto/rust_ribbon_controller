use stm32l0xx_hal::{
    adc::{Adc, Ready},
    delay::Delay,
    gpio::{
        gpioa::{PA0, PA4, PA5, PA7, PA9},
        gpioc::{PC14, PC15},
        Analog, Input, Output, PullUp, PushPull,
    },
    pac::{Peripherals, ADC, SPI1},
    prelude::*,
    rcc, serial,
    spi::{NoMiso, Spi, MODE_0},
};

use nb::block;

/// The physical board hardware structure is represented here.
///
/// The board consists of digital and analog peripherals.
pub struct Board {
    /// The analog to digital converter
    adc: Adc<Ready>,

    /// The pin used to read analog signals via the ADC
    adc_pin: PA0<Analog>,

    /// GPIO pin used as a gate ouptut
    gate_pin: PA9<Output<PushPull>>,

    mode_switch_1: PC14<Input<PullUp>>,
    mode_switch_2: PC15<Input<PullUp>>,

    /// SPI peripheral for writing to onboard DAC
    spi: Spi<SPI1, (PA5<Analog>, NoMiso, PA7<Analog>)>,
    /// SPI chip select pin
    nss: PA4<Output<PushPull>>,

    /// The USART
    tx: serial::Tx<serial::USART2>,

    delay: Delay,
}

/// The channels of the MCP4822 DAC are represented here.
#[derive(Clone, Copy)]
pub enum Mcp4822Channel {
    A = 0,
    B = 1,
}

impl Board {
    /// `Board::init()` is the board with all necessary peripherals initialized.
    pub fn init() -> Self {
        // general peripheral housekeeping
        let dp = Peripherals::take().unwrap();
        let cp = cortex_m::Peripherals::take().unwrap();
        let mut rcc = dp.RCC.freeze(rcc::Config::hsi16());
        let gpioa = dp.GPIOA.split(&mut rcc);
        let gpioc = dp.GPIOC.split(&mut rcc);

        // mode switch pins
        let mode_switch_1 = gpioc.pc14.into_pull_up_input();
        let mode_switch_2 = gpioc.pc15.into_pull_up_input();

        // USART for MIDI output
        let tx_pin = gpioa.pa2;
        let rx_pin = gpioa.pa3;

        let usart = dp
            .USART2
            .usart(
                tx_pin,
                rx_pin,
                serial::Config {
                    baudrate: 31_250_u32.Bd(), // MIDI baud rate
                    wordlength: serial::WordLength::DataBits8,
                    parity: serial::Parity::ParityNone,
                    stopbits: serial::StopBits::STOP1,
                },
                &mut rcc,
            )
            .unwrap();

        let (tx, _) = usart.split();

        // ADC
        let adc = dp.ADC.constrain(&mut rcc);
        let adc_pin = gpioa.pa0.into_analog();
        // configure hardware oversampling for effective ADC resolution of 16 bits
        unsafe {
            (*ADC::ptr()).cfgr2.modify(|_, w| {
                w.ovsr().mul256();
                w.ovss().bits(4);
                w.ovse().enabled()
            });
        }

        // SPI DAC
        let mut nss = gpioa.pa4.into_push_pull_output();
        nss.set_high().unwrap();
        let sck = gpioa.pa5;
        let mosi = gpioa.pa7;

        let spi = dp
            .SPI1
            .spi((sck, NoMiso, mosi), MODE_0, 100_000.Hz(), &mut rcc);

        // GATE PIN
        let gate_pin = gpioa.pa9.into_push_pull_output();

        let delay = cp.SYST.delay(rcc.clocks);

        Self {
            adc,
            adc_pin,
            gate_pin,
            mode_switch_1,
            mode_switch_2,
            spi,
            nss,
            tx,
            delay,
        }
    }

    /// `board.sleep_ms(ms)` causes the board to busy-wait for the `ms` milliseconds
    pub fn sleep_ms(&mut self, ms: u16) {
        self.delay.delay_ms(ms);
    }

    /// `board.serial_write(val)` writes the byte `val` via the USART in a blocking fashion
    pub fn serial_write(&mut self, val: u8) {
        block!(self.tx.write(val)).ok();
    }

    /// `board.get_raw_adc()` is the current value of the ADC.
    pub fn get_raw_adc(&mut self) -> u16 {
        self.adc.read(&mut self.adc_pin).unwrap()
    }

    pub fn get_mode_switch_1(&self) -> bool {
        self.mode_switch_1.is_low().unwrap()
    }

    pub fn get_mode_switch_2(&self) -> bool {
        self.mode_switch_2.is_low().unwrap()
    }
    /// `board.mcp4822_write(val_u12, channel)` writes the 12 bit value to the given channel of the onboard MCP4822 DAC.
    ///
    /// # Arguments:
    ///
    /// * `val_u12` - The 12 bit unsigned value to write. Bits above 11 are trucated.
    ///
    /// * `channel` - The enumerated MCP4822 channel
    pub fn mcp4822_write(&mut self, val_u12: u16, channel: Mcp4822Channel) {
        let low_word = (val_u12 & 0xFF) as u8;

        // OR in the channel, GAIN=1x, and Enable Vout
        let high_word = (((val_u12 & DAC_MAX) >> 8) as u8) | ((channel as u8) << 7) | 0b00110000;

        self.nss.set_low().unwrap();
        self.spi.write(&[high_word, low_word]).unwrap();
        self.nss.set_high().unwrap();
    }

    /// `board.set_gate(val)` sets the state of the gate pin to `val`.
    pub fn set_gate(&mut self, val: bool) {
        match val {
            true => self.gate_pin.set_high().unwrap(),
            false => self.gate_pin.set_low().unwrap(),
        }
    }
}

/// The maximum value that can be produced by the Analog to Digital Converters.
pub const ADC_MAX: u16 = u16::MAX;

/// The maximum value that can be written to the onboard Digital to Analog Converter.
pub const DAC_MAX: u16 = 0x0FFF;
