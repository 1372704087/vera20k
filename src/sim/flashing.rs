//! A Techno's flash countdown (`TechnoClass+0xF0`). The elite promotion
//! (`EliteFlashTimer`, `0x006FA0DC`), a building's Flash (vt+0x148,
//! `0x00456E00`, from a C4 plant) and team script action 50 (`0x006E9D0A`)
//! seed it; the shared late TechnoAI slot steps it (`0x004CC770`, called at
//! `0x006FAC4D`). Presentation reads its phase bit; there is no separate
//! flash clock.

/// The signed native flash dword `TechnoClass+0xF0`, owned by this module.
/// Each writer's value is kept with all its bits. `+0xF4` is a parity cache
/// for drawing, not a separate gameplay timer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Flashing {
    frames: i32,
}

impl Flashing {
    /// The countdown's remaining frames; bit 1 is the drawn phase.
    pub(crate) fn frames(&self) -> i32 {
        self.frames
    }

    /// Seed the countdown, as each native writer stores its operand.
    pub(crate) fn start(&mut self, frames: i32) {
        self.frames = frames;
    }

    /// Countdown `0x004CC770`: DEC runs for every nonzero signed dword,
    /// negative values included; zero is idle. Executed signed controls are
    /// in tools/spatial_oracle/team_creation (elite_flash rows). Returns
    /// whether it counted.
    pub(crate) fn tick(&mut self) -> bool {
        if self.frames == 0 {
            return false;
        }
        self.frames = self.frames.wrapping_sub(1);
        true
    }
}
