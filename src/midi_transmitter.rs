use crate::board::Board;

/// A very basic MIDI transmitter is represented here.
pub struct MidiTransmitter {
    channel: u8,
}

/// A few common MIDI messages
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
        self.send_command(board, MidiMessage::NoteOn as u8, note, MAX_MESSAGE);
    }

    /// `midi.note_on(board, note)` turns the specified note off and turns velocity to minimum
    ///
    /// # Arguments:
    ///
    /// * `board` - Reference to the board structure used to transmit the MIDI data
    ///
    /// * `note` - The MIDI note to turn off
    pub fn note_off(&self, board: &mut Board, note: u8) {
        self.send_command(board, MidiMessage::NoteOff as u8, note, 0);
    }

    /// `midi.pitch_bend(board, pb)` sends the 14 bit MIDI pitch bend message
    ///
    /// # Arguments:
    ///
    /// * `board` - Reference to the board structure used to transmit the MIDI data
    ///
    /// * `pb_u14` - The 14 bit pitch bend message to send
    ///
    /// Explanation for the cheeky OR-1 in the LSB: On some MIDI devices if the incoming pitch bend is EXACTLY centered
    /// then they are free to update the pitch bend with their own logic. But if it is off by even one, then they use the
    /// incoming pitch bend value exactly.
    /// If we send exactly centered pitch bend, sometimes the device we are controlling goes out of tune because it
    /// "remembers" a trailing pitch bend from before. Always sending a pitch bend message that is ever so slightly off
    /// center prevents this. This was only tested using an Arturia Keystep 37 as a MIDI device. Other devices may have
    /// different behavior.
    pub fn pitch_bend(&self, board: &mut Board, pb_u14: u16) {
        self.send_command(
            board,
            MidiMessage::PitchBend as u8,
            ((pb_u14 as u8) & MAX_MESSAGE) | 1,  // pitch bend LSB
            ((pb_u14 >> 7) as u8) & MAX_MESSAGE, // pitch bend MSB
        );
    }
}
/// The full scale value of pitch bend in one direction, pitch bend goes up and down by this amount from the center
pub const PITCH_BEND_FULL_SCALE: u16 = 1 << 13;

/// The center value for pitch bend messages, represents zero pitch bend
pub const PITCH_BEND_CENTER: u16 = PITCH_BEND_FULL_SCALE;

/// The maximum value for a 7 bit MIDI message
const MAX_MESSAGE: u8 = (1 << 7) - 1;
