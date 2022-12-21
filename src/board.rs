use stm32l0xx_hal::{
    adc::{Adc, Ready},
    gpio::{
        gpioa::{PA0, PA1, PA6},
        Analog, Output, PushPull,
    },
    pac::{Peripherals, TIM2},
    prelude::*,
    pwm::{Assigned, Pwm, Timer, C2},
    rcc::Config,
};

/// The maximum value that can be produced by the Analog to Digital Converters.
pub const ADC_MAX: usize = u16::MAX as usize;

/// The physical board hardware structure is represented here.
///
/// The board consists of digital and analog peripherals.
pub struct Board {
    /// The analog to digital converter
    adc: Adc<Ready>,

    /// The pin used to read analog signals via the ADC
    adc_pin: PA0<Analog>,

    /// PWM DAC as a test until I add an external DAC
    pwm: Pwm<TIM2, C2, Assigned<PA1<Analog>>>,

    /// GPIO pin used as a gate ouptut
    gate_pin: PA6<Output<PushPull>>,
}

impl Board {
    /// `Board::init()` is the board with all peripherals initialized.
    pub fn init() -> Self {
        let dp = Peripherals::take().unwrap();

        // use internal HSI oscillator as clock
        let mut rcc = dp.RCC.freeze(Config::hsi16());

        let gpioa = dp.GPIOA.split(&mut rcc);

        let gate_pin = gpioa.pa6.into_push_pull_output();

        // TODO: investigate hardware oversampling/averaging, I've done this in
        // C++ before
        let adc = dp.ADC.constrain(&mut rcc);
        let adc_pin = gpioa.pa0.into_analog();

        // TODO: eventually change this to be a SPI DAC or something
        let pwm = Timer::new(dp.TIM2, 1_000.Hz(), &mut rcc);
        let mut pwm = pwm.channel2.assign(gpioa.pa1);
        pwm.enable();

        Self {
            adc,
            adc_pin,
            pwm,
            gate_pin,
        }
    }

    /// `board.get_raw_adc()` is the current value of the ADC.
    pub fn get_raw_adc(&mut self) -> u16 {
        self.adc.read(&mut self.adc_pin).unwrap()
    }

    /// `board.set_dac(val)` sets the DAC to the given value.
    pub fn set_dac(&mut self, val: u16) {
        self.pwm.set_duty(val);
    }

    /// `board.set_gate(val)` sets the state of the gate GPIO pin to `val`.
    pub fn set_gate(&mut self, val: bool) {
        match val {
            true => self.gate_pin.set_high().unwrap(),
            false => self.gate_pin.set_low().unwrap(),
        }
    }
}

// TODO: I'd like to make the board a singleton
