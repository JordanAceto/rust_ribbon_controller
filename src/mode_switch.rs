use crate::board::Board;

pub struct ModeSwitch {}

#[derive(PartialEq, Eq)]
pub enum Mode {
    HardQuantize,
    Assist,
    Smooth,
}

impl ModeSwitch {
    pub fn new() -> Self {
        Self {}
    }

    pub fn read(&self, board: &Board) -> Mode {
        match (board.get_mode_switch_1(), board.get_mode_switch_2()) {
            (false, true) => Mode::HardQuantize,
            (false, false) => Mode::Assist,
            _ => Mode::Smooth,
        }
    }
}
