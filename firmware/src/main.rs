#![no_std]
#![no_main]

mod board;
mod midi_transmitter;
mod ui;

use synth_utils::{glide_processor, quantizer, ribbon_controller};

use crate::{
    board::{AdcPin, Board, Dac8162Channel},
    ui::{PitchMode, UiState},
};

use midi_convert::midi_types::MidiMessage;

use panic_halt as _;

use cortex_m_rt::entry;

const FAST_RIBBON_SAMPLE_RATE: u32 = board::TIM2_FREQ_HZ;
const OUTPUT_UPDATE_SAMPLE_RATE: u32 = board::TIM15_FREQ_HZ;

const MAIN_RIBBON_PIN: AdcPin = AdcPin::PA1;
const MOD_RIBBON_PIN: AdcPin = AdcPin::PA2;

const RIBBON_BUFF_CAPACITY: usize =
    ribbon_controller::sample_rate_to_capacity(FAST_RIBBON_SAMPLE_RATE);

// about 2 1/2 octaves of range, lowest note is F and highest note is C
const MAIN_RIBBON_NUM_SEMITONES: f32 = 32.0_f32;
const MAIN_RIBBON_MAX_VOUT: f32 = MAIN_RIBBON_NUM_SEMITONES / 12.0_f32;
const LOWEST_MIDI_NOTE: u8 = 5;

#[entry]
fn main() -> ! {
    let mut board = Board::init();
    let mut ui = UiState::new();

    // main ribbon for playing notes
    let mut main_ribbon = ribbon_controller::RibbonController::<RIBBON_BUFF_CAPACITY>::new(
        FAST_RIBBON_SAMPLE_RATE as f32,
        19_876.0_f32, // end-to-end resistance of the softpot as measured
        10_000.0_f32, // resistance of the series resistor going to vref
        1E6,          // pullup resistor from the wiper to the positive voltage refererence
    );

    // smaller aux ribbon which acts like a mod-wheel
    let mut mod_ribbon = ribbon_controller::RibbonController::<RIBBON_BUFF_CAPACITY>::new(
        FAST_RIBBON_SAMPLE_RATE as f32,
        10_271.0_f32, // end-to-end resistance of the softpot as measured
        10_000.0_f32, // resistance of the series resistor going to vref
        1E6,          // pullup resistor from the wiper to the positive voltage refererence
    );

    // quantizer for converting the raw ribbon reading to 1v/oct analog steps
    let mut ribbon_quantizer = quantizer::Quantizer::new();
    // second quantizer for re-converting prior to calculating midi note and pitch bend
    let mut midi_quantizer = quantizer::Quantizer::new();

    let mut glide = glide_processor::GlideProcessor::new(OUTPUT_UPDATE_SAMPLE_RATE as f32);

    // used in ASSIST pitch mode
    let mut offset_when_finger_pressed_down: f32 = 0.0_f32;

    let mut midi = midi_transmitter::MidiTransmitter::new();
    // keep track of conversions so we don't write mode MIDI data than needed if nothing changed
    let mut last_midi_note_sent = 0;
    let mut last_pitch_bend = 0.0_f32;

    // small delay to allow the ribbon voltage to settle before beginning
    board.delay_ms(100);

    ui.update(&mut board);

    loop {
        // slow timer for updating UI, reading pots and such
        if board.get_tim6_timeout() {
            ui.update(&mut board);
            glide.set_time(ui.glide_time());
        }

        // fast timer for polling the ribbon
        if board.get_tim2_timeout() {
            main_ribbon.poll(board.read_adc(MAIN_RIBBON_PIN));
            mod_ribbon.poll(board.read_adc(MOD_RIBBON_PIN));
        }

        // timer to update analog and MIDI outputs
        if board.get_tim15_timeout() {
            // expand the ribbon signal to 1volt/octave range
            let mut one_v_per_oct_ribbon = ribbon_to_dac8162_1v_per_oct(main_ribbon.value());

            let quantized_ribbon = ribbon_quantizer.convert(one_v_per_oct_ribbon);

            let finger_just_pressed = main_ribbon.finger_just_pressed();
            let finger_just_released = main_ribbon.finger_just_released();

            let pitch_mode = ui.pitch_mode();

            // the main ribbon can be one of three modes
            match pitch_mode {
                // hard-quantize and smooth modes are simple to calculate
                PitchMode::HardQuantize => {
                    one_v_per_oct_ribbon = quantized_ribbon.stairstep;
                }
                PitchMode::Smooth => {
                    let fudge_factor = quantizer::HALF_SEMITONE_WIDTH;
                    one_v_per_oct_ribbon -= fudge_factor;
                }
                // assist mode has more going on
                PitchMode::Assist => {
                    if finger_just_pressed {
                        // When the user first presses down after having lifted their finger record the offset between the
                        // finger position and the center of the note. We'll use this offset to make sure that it plays
                        // a nice in-tune note at first-press.
                        offset_when_finger_pressed_down = quantized_ribbon.fraction;

                        // use the stairstep for the first press for a nice in-tune note
                        one_v_per_oct_ribbon = quantized_ribbon.stairstep;
                    } else {
                        // The user is continuing to press the ribbon and maybe sliding around, use the smooth val but
                        // remove the offset
                        one_v_per_oct_ribbon -= offset_when_finger_pressed_down;
                    }
                }
            };

            let ribbon_with_portamento = glide.process(one_v_per_oct_ribbon);

            // set the analog outputs
            board.dac8162_set_vout(ribbon_with_portamento, Dac8162Channel::A);

            // scale the mod wheel ribbon for 5v range
            board.dac8162_set_vout(
                mod_ribbon.value() * board::DAC8162_MAX_VOUT,
                Dac8162Channel::B,
            );
            board.set_gate(main_ribbon.finger_is_pressing());

            // the extra quarter step helps keep things in-tune
            let midi_conversion =
                midi_quantizer.convert(one_v_per_oct_ribbon + quantizer::HALF_SEMITONE_WIDTH);
            let this_midi_note = midi_conversion.note_num + LOWEST_MIDI_NOTE;
            // MIDI pitch bend is usually set to 2 semitones, the extra divide-by-two avoids overshooting
            let this_pitch_bend = midi_conversion.fraction / (quantizer::SEMITONE_WIDTH * 2.0_f32);

            let midi_channel = board.read_midi_ch_switch();

            // Each round there may be zero or more MIDI messages sent:
            //
            // * a note-on message if the user just pressed the ribbon or if they slid into a new note
            // * one or two note-off messages if the user just released the ribbon or if they slid into a new note
            // * a pitch bend message if the user is pressing the ribbon and the value has changed since last time
            if finger_just_pressed {
                midi.push(MidiMessage::NoteOn(
                    midi_channel.into(),
                    this_midi_note.into(),
                    127.into(),
                ));
            } else if main_ribbon.finger_is_pressing() && this_midi_note != last_midi_note_sent {
                midi.push(MidiMessage::NoteOn(
                    midi_channel.into(),
                    this_midi_note.into(),
                    127.into(),
                ));

                midi.push(MidiMessage::NoteOff(
                    midi_channel.into(),
                    last_midi_note_sent.into(),
                    0.into(),
                ));
            } else if finger_just_released {
                midi.push(MidiMessage::NoteOff(
                    midi_channel.into(),
                    this_midi_note.into(),
                    0.into(),
                ));
                if this_midi_note != last_midi_note_sent {
                    midi.push(MidiMessage::NoteOff(
                        midi_channel.into(),
                        last_midi_note_sent.into(),
                        0.into(),
                    ));
                }
            }
            last_midi_note_sent = this_midi_note;

            if last_pitch_bend != this_pitch_bend {
                midi.push(MidiMessage::PitchBendChange(
                    midi_channel.into(),
                    this_pitch_bend.into(),
                ));
                last_pitch_bend = this_pitch_bend;
            }

            // send any MIDI messages, the queue might be empty but that is fine
            midi.send_queue(&mut board);
        }
    }
}

/// `ribbon_to_dac8164_1v_per_oct(r)` is the ribbon value in `[0.0, 1.0]` scaled to 1 volt per octave
fn ribbon_to_dac8162_1v_per_oct(ribb: f32) -> f32 {
    ribb * MAIN_RIBBON_MAX_VOUT
}
