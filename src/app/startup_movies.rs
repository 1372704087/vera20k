//! Process-start movies: the EA/Westwood logo on every launch and the YR intro
//! on the first one.
//!
//! Order and addresses (gamemd.exe, read statically):
//! - `Init_Game` `0x0052BA60` mounts the bulk MIX set (`0x530460` at
//!   `0x0052C59C`). Only when that mount reports success (`0x0052C5A1..A3`
//!   `test al,al`/`jne`) does it call `Play_Movie` `0x005BED40` with the
//!   `EA_WWLOGO` literal at `0x825F20` (`0x0052C5E2..0x0052C5EE`), and it
//!   presents the startup splash `0x005312A0` at `0x0052C5F3`, after the logo.
//! - From `0x0052CA65` (after `Load_Game_Rules` `0x0052CD70`) it calls
//!   `0x0052CB50`, which tests `[0xA8E960] & 4` and plays `Play_Movie` with the
//!   `A00_F00E` literal at `0x82618C`, so the intro follows the splash.
//! - WinMain reads the one-shot flag with `ReadBool` (`0x005295F0`) on section
//!   `Intro` (`0x8241C0`) and key `Play` (`0x840BE8`), default `1`
//!   (`0x006BDF64..0x006BDF89`), and while the bit is set writes `Play=no`
//!   (`WriteBool` `0x00529560` at `0x006BDFFD`) and saves the INI (`0x474430`
//!   at `0x006BE018`) before the game run function `0x48CCC0` is entered
//!   (`0x6BE041`/`0x6BE061`). The launch that plays the intro is therefore the
//!   one that records `no`; VERA consumes the key at launch for the same reason
//!   (`persistence::intro_play`).
//!
//! The order is therefore: logo, splash, rules load under the splash, intro,
//! shell. VERA keeps that order without native's modal `Play_Movie`: the window
//! goes up on black, the logo is played by the frame loop, and the splash is
//! presented only once the logo has ended, so its hold still covers the rest of
//! the initialization and nothing ever blocks the event loop. Native's own
//! window is likewise black until its logo starts, and a modal play cannot be
//! ported here: draining window messages from a winit callback re-enters the
//! window procedure the event loop is using, and simply blocking leaves the
//! window unanswered for the movie's whole length.
//!
//! Both plays are full-screen: the screen clears to black, the Bink soundtrack
//! plays at VoiceVolume, and the shared full-screen audio hold
//! (`Audio__PauseForMovie` `0x005BF580`) pauses the stream players until the
//! play ends. A bare Escape release ends the playing movie (`0x005BED40`'s only
//! input).

use std::sync::Arc;

use anyhow::{Context, Result};

use super::{App, AppState};
use crate::app::frontend::fullscreen_movie::surface_instance;
use crate::app::frontend::shell_pass::{ShellComposition, TexturedDraw, encode_shell_pass};
use crate::app::movie_playback::MoviePlayback;
use crate::render::bink_movie::BinkMovieSurface;
use crate::ui::game_screen::GameScreen;

/// `0x0052C5E2..0x0052C5EE`: the EA/Westwood logo, every launch, right before
/// the splash.
const LOGO_MOVIE: &str = "EA_WWLOGO";
/// `0x0052CA65` -> `0x0052CB50` (name `0x82618C`): the YR intro, first launch
/// only, after the splash.
const INTRO_MOVIE: &str = "A00_F00E";

/// Which process-start movie a slot holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartupMovieSlot {
    /// Before the splash: `Init_Game`'s `EA_WWLOGO`.
    Logo,
    /// After the splash hold: the first-run `A00_F00E`.
    Intro,
}

impl StartupMovieSlot {
    const fn movie(self) -> &'static str {
        match self {
            Self::Logo => LOGO_MOVIE,
            Self::Intro => INTRO_MOVIE,
        }
    }
}

/// What a startup movie shows on one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartupMovieFrame {
    /// The playing movie's frame.
    Movie,
    /// Nothing is left to play: this frame is the closing clear.
    Ended,
}

/// One process-start movie and the audio hold it owns.
pub(crate) struct StartupMovie {
    slot: StartupMovieSlot,
    playing: Option<MoviePlayback>,
    /// Whether this play holds the shared full-screen audio hold.
    audio_held: bool,
}

impl StartupMovie {
    /// `None` when the movie must not play at all, so the frame loop skips the
    /// slot entirely. The logo plays on every launch; the intro only when
    /// WinMain's `[0xA8E960] & 4` was set from `[Intro] Play=`, which the
    /// profile read consumed.
    pub(crate) fn armed(play: bool, slot: StartupMovieSlot) -> Option<Self> {
        play.then_some(Self {
            slot,
            playing: None,
            audio_held: false,
        })
    }

    pub(crate) fn playing(&self) -> Option<&MoviePlayback> {
        self.playing.as_ref()
    }

    /// Resolve and start the movie. A missing or undecodable movie ends the
    /// play silently, as `Play_Movie` returns without a visible change.
    fn start(&mut self, state: &mut AppState) -> bool {
        let file_name = format!("{}.BIK", self.slot.movie());
        let bytes = state.process_assets.manager().and_then(|assets| {
            assets
                .get_with_source_ref(&file_name)
                .map(|(bytes, source)| (Arc::<[u8]>::from(bytes), source.to_string()))
        });
        let Some((bytes, source)) = bytes else {
            log::info!("Startup movie: {file_name} is not installed");
            return false;
        };
        let surface = match BinkMovieSurface::from_bytes(
            &state.renderer.gpu,
            &state.renderer.batch_renderer,
            bytes,
            source,
            false,
            wgpu::FilterMode::Linear,
        ) {
            Ok(surface) => surface,
            Err(err) => {
                log::warn!("Startup movie Bink error {file_name}: {err:#}");
                return false;
            }
        };
        log::info!("Startup movie: playing {file_name}");
        let wall_ms = crate::app::match_runtime::frame_pacer::wall_clock_ms();
        state.audio.set_movie_audio_held(true, wall_ms);
        self.audio_held = true;
        let voice_volume = state
            .persistence
            .options_profile
            .voice_volume
            .clamp(0.0, 1.0);
        self.playing = Some(MoviePlayback::new(
            surface,
            state.audio.movie_mixer(),
            voice_volume,
        ));
        true
    }

    /// Release the shared audio hold once the movie has ended.
    pub(crate) fn release_audio(&mut self, state: &mut AppState) {
        if std::mem::take(&mut self.audio_held) {
            let wall_ms = crate::app::match_runtime::frame_pacer::wall_clock_ms();
            state.audio.retire_movie_audio(wall_ms);
        }
    }

    /// Advance by wall time and say what this frame draws.
    pub(crate) fn advance(&mut self, state: &mut AppState) -> Result<StartupMovieFrame> {
        if self.playing.is_none() && !self.start(state) {
            return Ok(StartupMovieFrame::Ended);
        }
        let voice_volume = state
            .persistence
            .options_profile
            .voice_volume
            .clamp(0.0, 1.0);
        let ended = {
            let playback = self.playing.as_mut().expect("started startup movie");
            // Process-start movies run before the shell exists: native plays
            // them to their end whatever the window focus is doing, so focus
            // loss does not pause them here.
            playback.set_active(true);
            playback.set_voice_volume(voice_volume);
            match playback.step(&state.renderer.gpu) {
                Ok(ended) => ended,
                Err(err) => {
                    log::warn!("Startup movie playback failed: {err:#}");
                    true
                }
            }
        };
        if !ended {
            return Ok(StartupMovieFrame::Movie);
        }
        self.playing = None;
        self.release_audio(state);
        Ok(StartupMovieFrame::Ended)
    }

    /// Play_Movie's only input (`0x005BED40`): a bare Escape release ends the
    /// movie that is playing.
    pub(crate) fn abort(&mut self) {
        if let Some(playback) = self.playing.as_mut() {
            playback.abort();
        }
    }
}

/// Present one black frame, so the startup window never shows an unpainted
/// surface while the process initializes (native's window is black until its
/// logo starts).
#[cfg(not(all(feature = "wasm", target_arch = "wasm32", target_os = "unknown")))]
pub(crate) fn present_startup_black(
    gpu: &crate::render::gpu::GpuContext,
    presenter: &crate::render::shell_surface_present::ShellSurfacePresenter,
) -> Result<bool> {
    let Some(output) = gpu
        .acquire_frame()
        .context("acquire startup black surface")?
    else {
        return Ok(false);
    };
    let source_view = presenter.source_render_view();
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Startup black"),
        });
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Startup black composition"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &source_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
    presenter.encode_present(&mut encoder, &output.texture);
    gpu.queue.submit(std::iter::once(encoder.finish()));
    output.present();
    Ok(true)
}

impl App {
    /// One frame of the slot's movie, if it is armed. Returns whether it owned
    /// this frame's presentation.
    pub(super) fn service_startup_movie(
        state: &mut AppState,
        slot: StartupMovieSlot,
    ) -> Result<bool> {
        let armed = match slot {
            StartupMovieSlot::Logo => state.frontend.startup_logo.is_some(),
            StartupMovieSlot::Intro => state.frontend.startup_intro.is_some(),
        };
        if !armed {
            return Ok(false);
        }
        // Anything that takes the display ends process-start playback: a
        // client-selected launch, or a loading screen.
        if state.frontend.screen != GameScreen::MainMenu {
            Self::finish_startup_movie(state, slot);
            return Ok(false);
        }
        let Some(output) = state
            .renderer
            .gpu
            .acquire_frame()
            .map_err(|e| anyhow::anyhow!("Surface texture: {}", e))?
        else {
            Self::follow_window_size(state);
            return Ok(true);
        };
        let view: wgpu::TextureView = output.texture.create_view(&Default::default());
        let mut encoder =
            state
                .renderer
                .gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Startup movie"),
                });
        let mut movie = Self::take_startup_movie(state, slot).expect("armed startup movie");
        match movie.advance(state)? {
            StartupMovieFrame::Movie => {
                Self::render_startup_movie(
                    state,
                    movie.playing().expect("playing movie"),
                    &mut encoder,
                    &output.texture,
                );
                *Self::startup_movie_slot(state, slot) = Some(movie);
            }
            // Play_Movie's closing clear: the splash or the shell paints from
            // the next frame.
            StartupMovieFrame::Ended => {
                crate::app::frontend::movies_credits_render::render_fullscreen_black(
                    state,
                    &mut encoder,
                    &output.texture,
                );
            }
        }
        state
            .renderer
            .gpu
            .queue
            .submit(std::iter::once(encoder.finish()));
        output.present();
        // wgpu 27 requires released surface references before reconfiguration.
        drop(view);
        state.platform.window.request_redraw();
        Ok(true)
    }

    fn startup_movie_slot(
        state: &mut AppState,
        slot: StartupMovieSlot,
    ) -> &mut Option<StartupMovie> {
        match slot {
            StartupMovieSlot::Logo => &mut state.frontend.startup_logo,
            StartupMovieSlot::Intro => &mut state.frontend.startup_intro,
        }
    }

    fn take_startup_movie(state: &mut AppState, slot: StartupMovieSlot) -> Option<StartupMovie> {
        Self::startup_movie_slot(state, slot).take()
    }

    /// Drop a slot's movie and the audio hold it owns.
    pub(super) fn finish_startup_movie(state: &mut AppState, slot: StartupMovieSlot) {
        let Some(mut movie) = Self::take_startup_movie(state, slot) else {
            return;
        };
        movie.release_audio(state);
    }

    /// Paint the playing movie on black, without a cursor.
    fn render_startup_movie(
        state: &AppState,
        playback: &MoviePlayback,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Texture,
    ) {
        let surface = playback.surface();
        let draws = [TexturedDraw {
            texture: surface.batch_texture(),
            instances: vec![surface_instance(
                surface,
                state.renderer.gpu.config.width as i32,
                state.renderer.gpu.config.height as i32,
            )],
        }];
        encode_shell_pass(
            state,
            encoder,
            destination,
            "Startup movie",
            ShellComposition {
                draws: &draws,
                ..ShellComposition::default()
            },
        );
    }

    /// A bare Escape release skips the playing movie.
    pub(super) fn startup_movie_key(state: &mut AppState) {
        for slot in [StartupMovieSlot::Logo, StartupMovieSlot::Intro] {
            if let Some(movie) = Self::startup_movie_slot(state, slot).as_mut() {
                movie.abort();
            }
        }
        state.platform.window.request_redraw();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_slot_plays_only_when_armed() {
        assert!(StartupMovie::armed(false, StartupMovieSlot::Logo).is_none());
        assert!(StartupMovie::armed(true, StartupMovieSlot::Logo).is_some());
        assert!(StartupMovie::armed(false, StartupMovieSlot::Intro).is_none());
        assert!(StartupMovie::armed(true, StartupMovieSlot::Intro).is_some());
    }

    /// Both names must resolve through the production archive mount, which is
    /// where `Init_Game`'s literals and `Play_Movie`'s `.BIK` suffix meet the
    /// retail MOVIES set.
    #[test]
    fn the_retail_install_carries_both_startup_movies() {
        let Some((_, assets)) = crate::rules::retail_ini_fixture::retail_assets() else {
            return;
        };
        for slot in [StartupMovieSlot::Logo, StartupMovieSlot::Intro] {
            let file_name = format!("{}.BIK", slot.movie());
            assert!(
                assets.get_with_source_ref(&file_name).is_some(),
                "{file_name} is not installed"
            );
        }
    }
}
