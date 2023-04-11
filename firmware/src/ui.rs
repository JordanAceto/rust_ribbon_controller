use crate::board::{AdcPin, Board, Switch3wayState};

/// The user interface is represented here (i.e. the front panel pots and switches that the user interacts with)
pub struct UiState {
    pitch_mode: PitchMode,
    glide_level: f32,
    glide_time: f32,
}

/// There are three modes for the ribbon pitch information
#[derive(Clone, Copy)]
pub enum PitchMode {
    HardQuantize,
    Assist,
    Smooth,
}

impl UiState {
    /// `UiState::new()` is a new UI state initialized to default values.
    pub fn new() -> Self {
        Self {
            pitch_mode: PitchMode::Smooth,
            glide_level: 0.0_f32,
            glide_time: 0.0_f32,
        }
    }

    /// `ui.update()` updates the UI state by reading and storing the panel control user inputs.
    ///
    /// It is required to periodically call this function to updat the state of the UI controls. Since these controls
    /// are manually adjusted by the user, they don't need to be updated very fast, just fast enough that they don't
    /// feel sluggish to the user.
    pub fn update(&mut self, board: &mut Board) {
        self.pitch_mode = match board.read_mode_switch() {
            Switch3wayState::Up => PitchMode::HardQuantize,
            Switch3wayState::Middle => PitchMode::Assist,
            Switch3wayState::Down => PitchMode::Smooth,
        };

        self.glide_level = board.read_adc(AdcPin::PA0);

        // bend the glide signal so the control feels nicer
        self.glide_time = bend_glide_ctl(self.glide_level);
    }

    /// `ui.glide_time()` is the current value of the front panel glide control knob as a time
    pub fn glide_time(&self) -> f32 {
        self.glide_time
    }

    /// `ui.pitch_mode()` is the current enumerated pitch mode, as set by the panel mount switch
    pub fn pitch_mode(&self) -> PitchMode {
        self.pitch_mode
    }
}

/// `bend_glide_ctl(v)` is value `v` scaled for a more natural feeling glide control
///
/// The physical glide control is a linear potentiometer, but it feels better for the user if the taper of the control
/// is tweaked some.
///
/// # Arguments:
///
/// * `val` - the value to scale, must be in `[0.0, 1.0]`
fn bend_glide_ctl(val: f32) -> f32 {
    val * val * 3.
}
