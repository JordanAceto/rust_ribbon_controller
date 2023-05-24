use stm32l4xx_hal::{
    adc::{SampleTime, Sequence, ADC},
    delay::Delay,
    device::SPI1,
    gpio::{Alternate, Input, Output, Pin, PullUp, PushPull, H8, L8},
    hal::spi::{Mode, Phase, Polarity},
    pac::{ADC1, DMA1, TIM15, TIM2, TIM6, USART1},
    prelude::*,
    rcc::{ClockSecuritySystem, CrystalBypass},
    serial,
    spi::Spi,
    timer::Timer,
};

// type aliases for complex types so clippy doesn't complain, pins are as required by the physical PCB layout
type MidiChannelSwitch = (
    Pin<Input<PullUp>, H8, 'A', 8>,  // 1s
    Pin<Input<PullUp>, H8, 'A', 12>, // 2s
    Pin<Input<PullUp>, H8, 'A', 11>, // 4s
    Pin<Input<PullUp>, L8, 'B', 0>,  // 8s
);
type SpiBus = Spi<
    SPI1,
    (
        Pin<Alternate<PushPull, 5>, L8, 'B', 3>,
        Pin<Alternate<PushPull, 5>, L8, 'B', 4>,
        Pin<Alternate<PushPull, 5>, L8, 'B', 5>,
    ),
>;

/// The physical board structure is represented here
pub struct Board {
    // USART for MIDI
    _midi_tx: serial::Tx<USART1>,
    _midi_rx: serial::Rx<USART1>,

    // SPI for DAC
    spi: SpiBus,
    nss: Pin<Output<PushPull>, H8, 'A', 15>, // manual chip select

    // general purpose delay
    delay: Delay,

    // 2 pins for the 3-position QUANTIZE MODE switch
    mode_switch: (
        Pin<Input<PullUp>, L8, 'A', 3>,
        Pin<Input<PullUp>, L8, 'A', 4>,
    ),

    // 4 pins for the rotary MIDI channel select switch
    midi_ch_switch: MidiChannelSwitch,

    // ribbon gate output
    gate_pin: Pin<Output<PushPull>, L8, 'A', 5>,
}

impl Board {
    /// `Board::init()` is the board structure with all peripherals initialized.
    pub fn init() -> Self {
        ////////////////////////////////////////////////////////////////////////
        //
        // general peripheral housekeeping, core peripherals and clocks
        //
        ////////////////////////////////////////////////////////////////////////
        let cp = cortex_m::Peripherals::take().unwrap();
        let dp = stm32l4xx_hal::pac::Peripherals::take().unwrap();
        let mut flash = dp.FLASH.constrain();
        let mut rcc = dp.RCC.constrain();
        let mut pwr = dp.PWR.constrain(&mut rcc.apb1r1);

        let clocks = rcc
            .cfgr
            .lse(CrystalBypass::Disable, ClockSecuritySystem::Disable) // LSE auto trims the HSI
            .sysclk(SYST_CLK_FREQ_MHZ.MHz())
            .pclk1(SYST_CLK_FREQ_MHZ.MHz())
            .pclk2(SYST_CLK_FREQ_MHZ.MHz())
            .freeze(&mut flash.acr, &mut pwr);

        let mut gpioa = dp.GPIOA.split(&mut rcc.ahb2);
        let mut gpiob = dp.GPIOB.split(&mut rcc.ahb2);

        let dma_channels = dp.DMA1.split(&mut rcc.ahb1);

        let mut delay = Delay::new(cp.SYST, clocks);

        ////////////////////////////////////////////////////////////////////////
        //
        // Gate pin
        //
        ////////////////////////////////////////////////////////////////////////
        let gate_pin = gpioa.pa5.into_push_pull_output_in_state(
            &mut gpioa.moder,
            &mut gpioa.otyper,
            PinState::Low,
        );

        ////////////////////////////////////////////////////////////////////////
        //
        // ADC
        //
        ////////////////////////////////////////////////////////////////////////

        // configure DMA1 to transfer ADC readings to the buffer
        let mut dma1_ch1 = dma_channels.1;
        unsafe {
            dma1_ch1.set_peripheral_address(&dp.ADC1.dr as *const _ as u32, false);
            dma1_ch1.set_memory_address(ADC_DMA_BUFF.as_ptr() as u32, true);
        }
        dma1_ch1.set_transfer_length(NUM_ADC_DMA_SIGNALS as u16);
        unsafe {
            (*DMA1::ptr()).ccr1.modify(|_, w| {
                w.msize()
                    .bits16()
                    .psize()
                    .bits16()
                    .minc()
                    .enabled()
                    .circ()
                    .enabled()
                    .en()
                    .set_bit()
            });
        }

        // configure the ADC
        let mut adc1 = ADC::new(
            dp.ADC1,
            dp.ADC_COMMON,
            &mut rcc.ahb2,
            &mut rcc.ccipr,
            &mut delay,
        );

        // gpio pins which are configured as analog inputs on the physical PCB
        let mut adc_pins = (
            gpioa.pa0.into_analog(&mut gpioa.moder, &mut gpioa.pupdr),
            gpioa.pa1.into_analog(&mut gpioa.moder, &mut gpioa.pupdr),
            gpioa.pa2.into_analog(&mut gpioa.moder, &mut gpioa.pupdr),
        );
        adc1.configure_sequence(&mut adc_pins.0, Sequence::One, SampleTime::Cycles640_5);
        adc1.configure_sequence(&mut adc_pins.1, Sequence::Two, SampleTime::Cycles640_5);
        adc1.configure_sequence(&mut adc_pins.2, Sequence::Three, SampleTime::Cycles640_5);

        unsafe {
            // configure hardware oversampler for 16 bit resolution
            (*ADC1::ptr()).cfgr2.modify(|_, w| {
                w.ovss()
                    .bits(0b0001) // shift right by 1
                    .ovsr()
                    .bits(0b100) // oversample 32x
                    .rovse()
                    .set_bit()
            });
            // enable continuous DMA mode
            (*ADC1::ptr())
                .cfgr
                .modify(|_, w| w.dmacfg().set_bit().dmaen().set_bit().cont().set_bit());
        }

        dma1_ch1.start();
        adc1.start_conversion();

        ////////////////////////////////////////////////////////////////////////
        //
        // TIMx periodic timers
        //
        ////////////////////////////////////////////////////////////////////////
        let _tim2 = Timer::tim2(dp.TIM2, TIM2_FREQ_HZ.Hz(), clocks, &mut rcc.apb1r1);

        let _tim6 = Timer::tim6(dp.TIM6, TIM6_FREQ_HZ.Hz(), clocks, &mut rcc.apb1r1);

        let _tim15 = Timer::tim15(dp.TIM15, TIM15_FREQ_HZ.Hz(), clocks, &mut rcc.apb2);

        ////////////////////////////////////////////////////////////////////////
        //
        // USART
        //
        ////////////////////////////////////////////////////////////////////////

        // configure DMA1 to transmit bytes via the UART
        let mut dma1_ch4 = dma_channels.4;
        unsafe {
            dma1_ch4.set_peripheral_address(&dp.USART1.tdr as *const _ as u32, false);
            dma1_ch4.set_memory_address(MIDI_USART_DMA_BUFF.as_ptr() as u32, true);
        }
        unsafe {
            (*DMA1::ptr()).ccr4.modify(|_, w| {
                w.pl()
                    .high()
                    .msize()
                    .bits8()
                    .psize()
                    .bits8()
                    .minc()
                    .enabled()
                    .dir()
                    .from_memory()
            });
            // map DMA channel 4 to UART tx
            (*DMA1::ptr()).cselr.modify(|_, w| w.c4s().bits(0b0010));
        }

        let tx_pin = gpioa
            .pa9
            .into_alternate(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrh);
        let rx_pin =
            gpioa
                .pa10
                .into_alternate(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrh);

        let usart = serial::Serial::usart1(
            dp.USART1,
            (tx_pin, rx_pin),
            serial::Config::default().baudrate(MIDI_BAUD_RATE_HZ.bps()),
            clocks,
            &mut rcc.apb2,
        );
        let (tx, rx) = usart.split();

        ////////////////////////////////////////////////////////////////////////
        //
        // SPI DAC
        //
        ////////////////////////////////////////////////////////////////////////
        let sck = gpiob
            .pb3
            .into_alternate(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl);
        let sdi = gpiob
            .pb4
            .into_alternate(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl);
        let sdo = gpiob
            .pb5
            .into_alternate(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl);

        let mut nss = gpioa.pa15.into_push_pull_output_in_state(
            &mut gpioa.moder,
            &mut gpioa.otyper,
            PinState::High,
        );

        let mut spi = Spi::spi1(
            dp.SPI1,
            (sck, sdi, sdo),
            Mode {
                phase: Phase::CaptureOnFirstTransition,
                polarity: Polarity::IdleHigh,
            },
            SPI_CLK_FREQ_MHZ.MHz(),
            clocks,
            &mut rcc.apb2,
        );

        // brief delay before initializing DAC
        delay.delay_ms(100_u32);

        // reset DAC
        nss.set_low();
        spi.write(&[0b0010_1000, 0, 0b0000_0001]).unwrap();
        nss.set_high();

        delay.delay_ms(10_u32);

        // enable DAC8162 vref (defaults to gain = 2 when enabled)
        nss.set_low();
        spi.write(&[0b0011_1000, 0, 0b0000_0001]).unwrap();
        nss.set_high();

        delay.delay_ms(10_u32);

        // set DAC gain to 1
        nss.set_low();
        spi.write(&[0b0000_0010, 0, 0b0000_0011]).unwrap();
        nss.set_high();

        delay.delay_ms(10_u32);

        ////////////////////////////////////////////////////////////////////////
        //
        // 3-way Mode switch
        //
        ////////////////////////////////////////////////////////////////////////
        let mode_switch = (
            gpioa
                .pa3
                .into_pull_up_input(&mut gpioa.moder, &mut gpioa.pupdr),
            gpioa
                .pa4
                .into_pull_up_input(&mut gpioa.moder, &mut gpioa.pupdr),
        );

        ////////////////////////////////////////////////////////////////////////
        //
        // MIDI channel select switch
        //
        ////////////////////////////////////////////////////////////////////////
        let midi_ch_switch = (
            gpioa
                .pa8
                .into_pull_up_input(&mut gpioa.moder, &mut gpioa.pupdr),
            gpioa
                .pa12
                .into_pull_up_input(&mut gpioa.moder, &mut gpioa.pupdr),
            gpioa
                .pa11
                .into_pull_up_input(&mut gpioa.moder, &mut gpioa.pupdr),
            gpiob
                .pb0
                .into_pull_up_input(&mut gpiob.moder, &mut gpiob.pupdr),
        );

        // delay to allow the analog voltages to settle
        delay.delay_ms(500_u32);

        Self {
            _midi_tx: tx,
            _midi_rx: rx,
            spi,
            nss,
            delay,
            mode_switch,
            midi_ch_switch,
            gate_pin,
        }
    }

    /// `board.read_adc(p)` is the digitized analog value on pin `p` in the range `[0.0, +1.0]`
    pub fn read_adc(&mut self, pin: AdcPin) -> f32 {
        // the values are already stored in the buffer via DMA
        unsafe { adc_fs_to_normalized_fl(ADC_DMA_BUFF[pin as usize]) }
    }

    /// `board.dac8162_set_vout(v, c)` writes the voltage `v` to channel `c` of the onboard DAC.
    ///
    /// # Arguments
    ///
    /// * `v_out` - The analog voltage to write, clamped to `[0.0, DAC8162_MAX_VOLTS]`
    ///
    /// * `channel` - The enumerated DAC channel to write to
    pub fn dac8162_set_vout(&mut self, v_out: f32, channel: Dac8162Channel) {
        let v_out = v_out.max(0.0_f32).min(DAC8162_MAX_VOUT);

        let val_u14 = (v_out * DAC8162_COUNTS_PER_VOLT) as u16;
        // move the value out of DB0 and DB1
        let val_u14 = val_u14 << 2;
        // split it into bytes
        let low_byte = (val_u14 & 0xFF) as u8;
        let mid_byte = (val_u14 >> 8) as u8;
        let high_byte = channel as u8 | 0b0001_1000; // write to channel and update output

        self.spi_write(&[high_byte, mid_byte, low_byte]);
    }

    /// `board.read_mode_switch()` is the enumerated state of the 3-way mode switch.
    pub fn read_mode_switch(&self) -> Switch3wayState {
        // The physical switch on the PCB is a SPDT on-off-on switch which grounds
        // either PB6, PB7, or neither pins depending on the position.
        match (self.mode_switch.0.is_low(), self.mode_switch.1.is_low()) {
            (true, false) => Switch3wayState::Up,
            (false, false) => Switch3wayState::Middle,
            _ => Switch3wayState::Down, // should only happen with (false, true) but catch unlikely (true, true) as well
                                        // (true, true) means that something is wrong with the switch, but the show must go on
        }
    }

    /// `board.read_midi_mode_switch()` is the value of the rotary MIDI channel switch in `[0..15]`
    pub fn read_midi_ch_switch(&self) -> u8 {
        // the physical switch on the pcb is a Nidec SD-1011 coded rotary switch. This kind of switch represents a
        // binary number on 4 GPIO pins as you turn it
        [
            self.midi_ch_switch.3.is_low(), // msb
            self.midi_ch_switch.2.is_low(),
            self.midi_ch_switch.1.is_low(),
            self.midi_ch_switch.0.is_low(), // lsb
        ]
        .iter()
        .fold(0_u8, |acc, x| (acc << 1_u8) + (*x as u8))
    }

    /// `board.serial_write_all(bs)` writes all bytes `bs` via the serial port
    ///
    /// # Requires
    ///
    /// * `bytes` is no greater than `MIDI_TX_BUFF_LEN` in length
    pub fn serial_write_all(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }

        // use DMA to send the bytes
        unsafe {
            MIDI_USART_DMA_BUFF[..bytes.len()].copy_from_slice(bytes);

            while (*USART1::ptr()).isr.read().tc().bit_is_clear() {
                // wait for any ongoing transfer to complete
            }

            // disable DMA
            (*DMA1::ptr()).ccr4.modify(|_, w| w.en().disabled());
            // set the length for the data transfer
            (*DMA1::ptr())
                .cndtr4
                .write(|w| w.ndt().bits(bytes.len() as u16));
            // clear the transfer complete flag
            (*USART1::ptr()).icr.write(|w| w.tccf().set_bit());
            // enable DMA to start the transfer
            (*DMA1::ptr()).ccr4.modify(|_, w| w.en().enabled());
        }
    }

    /// `board.spi_write(words)` writes the words via SPI.
    fn spi_write(&mut self, words: &[u8]) {
        self.nss.set_low();
        self.spi.write(words).unwrap();
        self.nss.set_high();
    }

    /// `board.set_gate(val)` sets the state of the gate pin to `val`.
    pub fn set_gate(&mut self, val: bool) {
        self.gate_pin.set_state(PinState::from(val));
    }

    /// `board.delay_ms(ms)` causes the board to busy-wait for `ms` milliseconds
    pub fn delay_ms(&mut self, ms: u32) {
        self.delay.delay_ms(ms);
    }

    /// board.get_tim2_timeout()` is true iff timer TIM2 has timed out, self clearing.
    pub fn get_tim2_timeout(&self) -> bool {
        unsafe {
            if (*TIM2::ptr()).sr.read().uif().bit() {
                (*TIM2::ptr()).sr.modify(|_, w| w.uif().clear());
                true
            } else {
                false
            }
        }
    }

    /// board.get_tim6_timeout()` is true iff timer TIM6 has timed out, self clearing.
    pub fn get_tim6_timeout(&self) -> bool {
        unsafe {
            if (*TIM6::ptr()).sr.read().uif().bit() {
                (*TIM6::ptr()).sr.modify(|_, w| w.uif().clear());
                true
            } else {
                false
            }
        }
    }

    /// board.get_tim15_timeout()` is true iff timer TIM6 has timed out, self clearing.
    pub fn get_tim15_timeout(&self) -> bool {
        unsafe {
            if (*TIM15::ptr()).sr.read().uif().bit() {
                (*TIM15::ptr()).sr.modify(|_, w| w.uif().clear());
                true
            } else {
                false
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
//
// Public constants
//
////////////////////////////////////////////////////////////////////////////////

/// The frequenct of the main system clock
pub const SYST_CLK_FREQ_MHZ: u32 = 80;

/// The frequency for periodic timer TIM2
pub const TIM2_FREQ_HZ: u32 = 1_000;

/// The frequency for periodic timer TIM6
pub const TIM6_FREQ_HZ: u32 = 30;

/// The frequency for periodic timer TIM15
pub const TIM15_FREQ_HZ: u32 = 300;

/// The SPI clock frequency to use
const SPI_CLK_FREQ_MHZ: u32 = 10;

/// The maximum value that can be produced by the Analog to Digital Converters.
pub const ADC_MAX: u16 = 0xFFF0;

/// The maximum value that can be written to the onboard Digital to Analog Converter.
pub const DAC8162_MAX_COUNT: u16 = (1 << 14) - 1;

/// The maximum analog voltage that the DAC can produce after onboard amplification
pub const DAC8162_MAX_VOUT: f32 = 5.0_f32;

/// The baud rate required for MIDI communication
pub const MIDI_BAUD_RATE_HZ: u32 = 31_250;

////////////////////////////////////////////////////////////////////////////////
//
// Private constants and static variables
//
////////////////////////////////////////////////////////////////////////////////

/// The number of DAC counts for 1 volt output
const DAC8162_COUNTS_PER_VOLT: f32 = DAC8162_MAX_COUNT as f32 / DAC8162_MAX_VOUT;

/// ADC readings are stored in a static array via DMA
const NUM_ADC_DMA_SIGNALS: usize = 3;
static mut ADC_DMA_BUFF: [u16; NUM_ADC_DMA_SIGNALS] = [0; NUM_ADC_DMA_SIGNALS];

const MIDI_TX_BUFF_LEN: usize = 16;
static mut MIDI_USART_DMA_BUFF: [u8; MIDI_TX_BUFF_LEN] = [0; MIDI_TX_BUFF_LEN];

////////////////////////////////////////////////////////////////////////////////
//
// Private helper functions
//
////////////////////////////////////////////////////////////////////////////////

/// `adc_fs_to_normalized_fl(v)` is the integer adc value normalized to [0.0, +1.0]
///
/// If the input value would overflow the output range it is clamped.
fn adc_fs_to_normalized_fl(val: u16) -> f32 {
    let val = val.min(ADC_MAX); // don't need to clamp negative values, it's already unsigned

    (val as f32) / (ADC_MAX as f32)
}

////////////////////////////////////////////////////////////////////////////////
//
// Public enums
//
////////////////////////////////////////////////////////////////////////////////

/// Pins which may be read by the ADC are represented here
#[derive(Clone, Copy)]
pub enum AdcPin {
    PA0 = 0,
    PA1 = 1,
    PA2 = 2,
}

/// Channels of the onboard DAC are represented here
#[derive(Clone, Copy)]
pub enum Dac8162Channel {
    A = 0b000,
    B = 0b001,
}

/// Valid states of a 3-way switch are represented here
#[derive(Clone, Copy)]
pub enum Switch3wayState {
    Up,
    Middle,
    Down,
}
