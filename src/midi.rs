use crate::board::Board;

/// A very basic MIDI transmitter is represented here.
pub struct Midi {
    channel: u8,
}

/// A few common MIDI messages
#[derive(Clone, Copy)]
pub enum MidiMessage {
    NoteOn = 0x90,
    NoteOff = 0x80,
    PitchBend = 0xE0,
    MaxVelocity = 0x7F,
    MinVelocity = 0x00,
}

impl Midi {
    /// `Midi::new(ch)` is a new midi transmitter set to channel `ch`
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
        self.send_command(
            board,
            MidiMessage::NoteOn as u8,
            note,
            MidiMessage::MaxVelocity as u8,
        );
    }

    /// `midi.note_on(board, note)` turns the specified note off and turns velocity to minimum
    ///
    /// # Arguments:
    ///
    /// * `board` - Reference to the board structure used to transmit the MIDI data
    ///
    /// * `note` - The MIDI note to turn off
    pub fn note_off(&self, board: &mut Board, note: u8) {
        self.send_command(
            board,
            MidiMessage::NoteOff as u8,
            note,
            MidiMessage::MinVelocity as u8,
        );
    }
}
