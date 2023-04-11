use crate::board::Board;

use heapless::Vec;
use midi_convert::{midi_types::MidiMessage, MidiRenderSlice};

const MAX_NUM_MESSAGES_IN_QUEUE: usize = 16;

// MIDI messages may have a variable length, but the ones we care about are no more than 3 bytes long
const MAX_BYTES_PER_MSG: usize = 3;

const BYTE_BUFF_LEN: usize = MAX_NUM_MESSAGES_IN_QUEUE * MAX_BYTES_PER_MSG;

/// A very basic MIDI transmitter is represented here.
pub struct MidiTransmitter {
    msg_queue: Vec<MidiMessage, MAX_NUM_MESSAGES_IN_QUEUE>,
    byte_buffer: [u8; BYTE_BUFF_LEN],
}

impl MidiTransmitter {
    /// `MidiTransmitter::new()` is a new midi transmitter
    ///
    /// # Arguments:
    ///
    /// * `channel` - The MIDI channel to use
    pub fn new() -> Self {
        Self {
            msg_queue: Vec::new(),
            byte_buffer: [0; BYTE_BUFF_LEN],
        }
    }

    /// `mt.push(m)` pushes the MIDI message `m` onto the message queue
    pub fn push(&mut self, msg: MidiMessage) {
        self.msg_queue.push(msg).ok();
    }

    /// `mt.send_queue(b)` sends all MIDI messages currently in the queue via the board serial port
    pub fn send_queue(&mut self, board: &mut Board) {
        let mut i = 0;
        for msg in &self.msg_queue {
            msg.render_slice(&mut self.byte_buffer[i..(i + msg.len())]);
            i += msg.len();
        }
        board.serial_write_all(&self.byte_buffer[..i]);
        self.msg_queue.clear();
    }
}
