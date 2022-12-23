use crate::board;
use heapless::HistoryBuffer;

/// A synthesizer ribbon controller is represented here.
///
/// Users play the ribbon by sliding their finger up and down a resistive track wired as a voltage divider.
///
/// We can detect when a user is pressing the ribbon, and where along the track they are pressing with this module.
///
/// The position of the user's finger on the ribbon is represented as an integer. The farther to the right the user is
/// pressing the larger the number. The position value is retained even when the user lifts their finger off of the
/// ribbon, similar to a sample-and-hold system. Some averaging is done to smooth out the raw readings and reduce the
/// influence of spurious inputs.
///
/// Whether or not the user is pressing on the ribbon is represented as a boolean "gate" signal. The gate is considered
/// high (true) when the ribbon is being pressed and low (false) when no one is touching the ribbon.
///
/// The position value and gate signals are then typically used as control signals for other modules, such as
/// oscillators, filters, and amplifiers.
///
/// # Inputs
///
/// * Samples are fed into the ribbon controller
///
/// # Outputs
///
/// * The average of the most recent samples representing the position of the user's finger on the ribbon
///
/// * Boolean gate which is `true` if the user is pressing their finger on the ribbon, else `false`
pub struct RibbonController {
    /// The current position value of the ribbon
    current_val: usize,

    /// The current gate value of the ribbon
    current_gate: bool,

    /// An internal buffer for storing and averaging samples as they come in via the `poll` method
    buff: HistoryBuffer<usize, BUFFER_CAPACITY>,
}

impl RibbonController {
    /// `Ribbon::new()` is a new Ribbon.
    pub fn new() -> Self {
        Self {
            current_val: 0,
            current_gate: false,
            buff: HistoryBuffer::new(),
        }
    }

    /// `rib.poll(raw_adc_value)` updates the ribbon controller by polling the raw ADC signal.
    ///
    /// # Arguments
    ///
    /// * `raw_adc_value` - the raw ADC signal to poll, represents the finger position on the ribbon
    ///
    /// The ribbon must be updated periodically. It is expected that a constant stream of ADC samples will be fed into
    /// the ribbon by calling this method. The position value and gate signals of the ribbon are updated by polling.
    pub fn poll(&mut self, raw_adc_value: usize) {
        let user_is_pressing_ribbon = raw_adc_value <= FINGER_PRESS_HIGH_BOUNDARY;

        if user_is_pressing_ribbon {
            self.buff.write(raw_adc_value);

            if MIN_VALID_SAMPLES_FOR_AVG <= self.buff.len() {
                let num_to_take = self.buff.len() - NUM_MOST_RECENT_SAMPLES_TO_IGNORE;

                // take the average of the most recent samples, minus a few of the very most recent ones
                self.current_val =
                    self.buff.oldest_ordered().take(num_to_take).sum::<usize>() / num_to_take;

                self.current_gate = true;
            }
        } else {
            // the user is not pressing on the ribbon, clear the buffer but hold on to the last valid `current_val`
            self.buff.clear();
            self.current_gate = false;
        }
    }

    /// `rib.value()` is the current position value of the ribbon.
    ///
    /// If the user's finger is not pressing on the ribbon, the last valid value before they lifted their finger is
    /// returned.
    pub fn value(&self) -> usize {
        self.current_val
    }

    /// `rib.gate()` is the current state of the ribbon gate.
    ///
    /// `true` if a finger is pressing on the ribbon and enough ADC samples have been polled to generate a stable
    /// reading, `false` otherwise.
    pub fn gate(&self) -> bool {
        self.current_gate
    }
}

/// Samples below this value indicate that there is a finger pressed down on the ribbon.
///
/// The exact value depends on the resistor chosen that connects the top of the ribbon to the positive voltage
/// reference. Derived emprically through experimentation to find values that feel right to the user.
const FINGER_PRESS_HIGH_BOUNDARY: usize = board::ADC_MAX - board::ADC_MAX / 90;

/// The capacity of the internal ribbon sample buffer.
const BUFFER_CAPACITY: usize = 64;

/// The minimum number of samples required to calculate an average in the internal sample buffer.
///
/// Must be less than or equal to than the buffer capacity.
const MIN_VALID_SAMPLES_FOR_AVG: usize = 32;

/// The number of the most recently added samples to ignore when calculating the average of the internal sample buffer.
///
/// The purpose is to avoid including spurious readings in the average.
/// Must be less than the minimum number of samples needed to calculate an average.
const NUM_MOST_RECENT_SAMPLES_TO_IGNORE: usize = 8;
