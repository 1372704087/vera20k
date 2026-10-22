//! Optional rodio music output.
//!
//! Theme catalog and lifecycle ownership live in `audio::theme`. This module
//! owns only the physical device/player plus independent gain projections.

use std::num::NonZero;

use rodio::buffer::SamplesBuffer;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player};

use crate::audio::arbiter::combine_linear;
use crate::audio::sfx::native_volume_amplitude;
use crate::audio::theme::{MusicOutputState, PreparedTrack};
use crate::rules::sound_ini::VOLUME_SCALE;
use crate::util::native_x87::X87Chop53;

/// Engine default for the score (music) volume.
pub const DEFAULT_SCORE_VOLUME: f64 = 0.4;

/// The Theme stream's DirectSound amplitude.
///
/// gamemd-derived: the Theme stream is a channel of the shared pool
/// (`StreamPlayer` ctor `0x00407860` -> `0x004036C0`), so
/// `DSoundBuffer::CombineInterps @ 0x00402220` multiplies its linears as
/// `(a * b) >> 14` and `DSoundBuffer__ApplyVolumePanFrequency @ 0x0040A6D0`
/// converts the product once through the DirectSound table, exactly as for
/// effects. The ctor clears groups `+0x90..+0x9C` (`0x004078D1..0x004078F3`);
/// `ThemeClass__ReadINI` links only `+0x94` = Score `[0x0087E744]`
/// (`0x00720743`) and `+0x9C` = `[0x0087E754]` (`0x00720751`), which only
/// `AudioSystem__Init` writes (`0x00406B92`), so it is unity here. The
/// effects master `[0x0087E758]` never reaches this stream.
///
/// `own` is the stream's own `+0x10` interp. Natively one interp carries both
/// Theme's fade (`0x004080C0`) and the radar-movie pre-fade/restore
/// (`0x00408390` / `0x004083D0`); VERA keeps them as two linears and
/// multiplies them. RESIDUAL: trigger, a radar movie starting during a Theme
/// fade; effect, the shared native interp would retarget the fade instead of
/// multiplying it; frequency, rare (a campaign transmission during a song
/// change); downstream risk, none (presentation only).
///
/// Score: `OptionsClass::Set_Score_Volume @ 0x005FA4A0` targets
/// `ftol(volume * 16384.0)` (`0x005FA4C3..0x005FA4D6`).
fn effective_music_volume(
    score_volume: f64,
    stream_linear: i32,
    theme_scale: f64,
    focus_output_scale: f64,
) -> f32 {
    let theme_linear = (theme_scale.clamp(0.0, 1.0) * f64::from(VOLUME_SCALE)) as i32;
    let own = combine_linear(theme_linear, stream_linear);
    let score = X87Chop53::ftol_f64_low_masked(score_volume * f64::from(VOLUME_SCALE));
    native_volume_amplitude(combine_linear(own, score)) * focus_output_scale as f32
}

pub struct MusicPlayer {
    /// rodio mixer device sink — must be kept alive or all audio stops.
    _device: MixerDeviceSink,
    current_player: Option<Player>,
    volume: f64,
    /// The radar-movie stream interp (`0x00408390` / `0x004083D0`).
    stream_linear: i32,
    theme_scale: f64,
    focus_output_scale: f64,
    paused: bool,
}

impl MusicPlayer {
    /// Create a physical music output. Theme construction never depends on
    /// this succeeding.
    pub fn new() -> Option<Self> {
        let device = DeviceSinkBuilder::open_default_sink()
            .map_err(|error| log::error!("Failed to initialize music audio: {error}"))
            .ok()?;
        Some(Self {
            _device: device,
            current_player: None,
            volume: DEFAULT_SCORE_VOLUME,
            stream_linear: VOLUME_SCALE,
            theme_scale: 1.0,
            focus_output_scale: 1.0,
            paused: false,
        })
    }

    pub(crate) fn state(&self) -> MusicOutputState {
        match self.current_player.as_ref() {
            Some(player) if player.empty() => MusicOutputState::Finished,
            Some(_) => MusicOutputState::Playing,
            None => MusicOutputState::Idle,
        }
    }

    pub(crate) fn submit(&mut self, prepared: PreparedTrack) -> bool {
        let Some(channels) = NonZero::new(2u16) else {
            return false;
        };
        let Some(rate) = NonZero::new(prepared.sample_rate) else {
            return false;
        };
        if let Some(player) = self.current_player.take() {
            player.stop();
        }
        let source = SamplesBuffer::new(channels, rate, prepared.samples);
        let player = Player::connect_new(self._device.mixer());
        player.set_volume(self.effective_volume());
        player.append(source);
        if self.paused {
            player.pause();
        }
        log::info!("Playing prepared music track: {}", prepared.stem);
        self.current_player = Some(player);
        true
    }

    pub fn stop(&mut self) {
        if let Some(player) = self.current_player.take() {
            player.stop();
        }
    }

    /// Play_Movie `0x005BED40` blocks and pauses every stream player
    /// (`0x00408270`) and resumes them afterwards (`0x00408230`), so the
    /// theme continues from where it stopped rather than restarting.
    pub fn pause_output(&mut self) {
        self.paused = true;
        if let Some(player) = self.current_player.as_ref() {
            player.pause();
        }
    }

    pub fn resume_output(&mut self) {
        self.paused = false;
        if let Some(player) = self.current_player.as_ref() {
            player.play();
        }
    }

    /// The music device mixer, shared by streams that must play alongside
    /// the theme output (the shell movie soundtrack).
    pub fn mixer(&self) -> &rodio::mixer::Mixer {
        self._device.mixer()
    }

    pub(crate) fn discard_finished(&mut self) {
        if self.state() == MusicOutputState::Finished {
            self.current_player = None;
        }
    }

    pub fn set_volume(&mut self, volume: f64) {
        self.volume = volume.clamp(0.0, 1.0);
        self.apply_effective_volume();
    }

    pub(crate) fn set_stream_gain(&mut self, linear: i32) {
        self.stream_linear = linear;
        self.apply_effective_volume();
    }

    pub(crate) fn set_theme_scale(&mut self, scale: f64) {
        self.theme_scale = scale.clamp(0.0, 1.0);
        self.apply_effective_volume();
    }

    /// Gate the primary music output on the application-activation edge.
    ///
    /// gamemd-derived: `WM_ACTIVATEAPP` at `0x007778AC` reaches primary
    /// Stop/restore through `0x00407020`/`0x00407040`; secondary cursors keep
    /// advancing.
    pub fn set_focus_output_active(&mut self, active: bool) {
        self.focus_output_scale = if active { 1.0 } else { 0.0 };
        self.apply_effective_volume();
    }

    fn effective_volume(&self) -> f32 {
        effective_music_volume(
            self.volume,
            self.stream_linear,
            self.theme_scale,
            self.focus_output_scale,
        )
    }

    fn apply_effective_volume(&self) {
        if let Some(player) = self.current_player.as_ref() {
            player.set_volume(self.effective_volume());
        }
    }

    pub fn volume(&self) -> f64 {
        self.volume
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db(amplitude: f32) -> f64 {
        20.0 * f64::from(amplitude).log10()
    }

    /// Default ScoreVolume 0.4 is index 39 of the table at `0x00816380`
    /// (-13.58 dB), not the linear 0.4 (-7.96 dB) of a plain gain.
    #[test]
    fn score_volume_goes_through_the_directsound_table() {
        let default = effective_music_volume(0.4, VOLUME_SCALE, 1.0, 1.0);
        assert!((db(default) - -13.58).abs() < 0.01, "{}", db(default));
        assert_eq!(effective_music_volume(1.0, VOLUME_SCALE, 1.0, 1.0), 1.0);
        // Theme's half-way fade joins the linear product before the table.
        let half = effective_music_volume(1.0, VOLUME_SCALE, 0.5, 1.0);
        assert!((db(half) - -10.0).abs() < 0.01, "{}", db(half));
        assert_eq!(effective_music_volume(0.4, VOLUME_SCALE, 1.0, 0.0), 0.0);
    }
}
