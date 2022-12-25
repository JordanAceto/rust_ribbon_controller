use stm32l0xx_hal::{
    adc::{Adc, Ready},
    gpio::{
        gpioa::{PA0, PA4, PA5, PA7, PA9},
        Analog, Output, PushPull,
    },
    pac::{Peripherals, ADC, SPI1},
    prelude::*,
    rcc::Config,
    spi::{NoMiso, Spi, MODE_0},
};

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

    /// SPI peripheral for writing to onboard DAC
    spi: Spi<SPI1, (PA5<Analog>, NoMiso, PA7<Analog>)>,
    /// SPI chip select pin
    nss: PA4<Output<PushPull>>,
}

/// The channels of the MCP4822 DAC are represented here.
pub enum Mcp4822Channel {
    A,
    B,
}

/// `channel.value()` is the integer value of the MCP4822 channel.
impl Mcp4822Channel {
    fn value(&self) -> u8 {
        match self {
            Mcp4822Channel::A => 0,
            Mcp4822Channel::B => 1,
        }
    }
}

impl Board {
    /// `Board::init()` is the board with all peripherals initialized.
    pub fn init() -> Self {
        // general peripheral housekeeping
        let dp = Peripherals::take().unwrap();
        let mut rcc = dp.RCC.freeze(Config::hsi16());
        let gpioa = dp.GPIOA.split(&mut rcc);

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

        Self {
            adc,
            adc_pin,
            gate_pin,
            spi,
            nss,
        }
    }

    /// `board.get_raw_adc()` is the current value of the ADC.
    pub fn get_raw_adc(&mut self) -> u16 {
        self.adc.read(&mut self.adc_pin).unwrap()
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
        let high_word = (((val_u12 & DAC_MAX) >> 8) as u8) | (channel.value() << 7) | 0b00110000;

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

// TODO: I'd like to make the board a singleton
