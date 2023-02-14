use crate::board::Board;

/// A very basic MIDI transmitter is represented here.
pub struct MidiTransmitter {
    channel: u8,
}

/// A few common MIDI messages
#[derive(Clone, Copy)]
pub enum MidiMessage {
    NoteOn = 0x90,
    NoteOff = 0x80,
    PitchBend = 0xE0,
}

impl MidiTransmitter {
    /// `MidiTransmitter::new(ch)` is a new midi transmitter set to channel `ch`
    ///
    /// # Arguments:
    ///
    /// * `channel` - The MIDI channel to use
    pub fn new(channel: u8) -> Self {
        Self { channel }
    }

    /// `midi.send_command(brd, b1, b2, b3)` sends a 3 byte MIDI payload
    ///
    /// # Arguments:
    ///
    /// * `board` - Reference to the board structure used to transmit the MIDI data
    ///
    /// * `b1..b3` - The 3 byte MIDI payload to send  
    fn send_command(&self, board: &mut Board, byte_1: u8, byte_2: u8, byte_3: u8) {
        board.serial_write(byte_1 | self.channel);
        board.serial_write(byte_2);
        board.serial_write(byte_3);
    }

    /// `midi.note_on(board, note)` turns the specified note on at max velocity
    ///
    /// # Arguments:
    ///
    /// * `board` - Reference to the board structure used to transmit the MIDI data
    ///
    /// * `note` - The MIDI note to turn on
    pub fn note_on(&self, board: &mut Board, note: u8) {
        self.send_command(board, MidiMessage::NoteOn as u8, note, MAX_VELOCITY);
    }

    /// `midi.note_on(board, note)` turns the specified note off and turns velocity to minimum
    ///
    /// # Arguments:
    ///
    /// * `board` - Reference to the board structure used to transmit the MIDI data
    ///
    /// * `note` - The MIDI note to turn off
    pub fn note_off(&self, board: &mut Board, note: u8) {
        self.send_command(board, MidiMessage::NoteOff as u8, note, MIN_VELOCITY);
    }

    /// `midi.pitch_bend(board, pb)` sends the 14 bit MIDI pitch bend message
    ///
    /// # Arguments:
    ///
    /// * `board` - Reference to the board structure used to transmit the MIDI data
    ///
    /// * `pb_u14` - The 14 bit pitch bend message to send
    pub fn pitch_bend(&self, board: &mut Board, pb_u14: u16) {
        self.send_command(
            board,
            MidiMessage::PitchBend as u8,
            (pb_u14 & 0x7F) as u8,      // pitch bend LSB
            (pb_u14 >> 7 & 0x7F) as u8, // pitch bend MSB
        );
    }
}

/// The center value for pitch bend messages, represents zero pitch bend
pub const PITCH_BEND_CENTER: u16 = 1 << 13;

/// The full scale value of pitch bend in one direction, pitch beng goes up and down by this amount from the PITCH_BEND_CENTER
pub const PITCH_BEND_FULL_SCALE: u16 = PITCH_BEND_CENTER;

/// The maximum value for a velocity message
const MAX_VELOCITY: u8 = (1 << 7) - 1;

/// The minumum value for a velocity message
const MIN_VELOCITY: u8 = 0x00;
