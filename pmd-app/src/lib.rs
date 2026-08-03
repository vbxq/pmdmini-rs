// SPDX-License-Identifier: AGPL-3.0-only

#![forbid(unsafe_code)]

use eframe::egui;
use pmd_core::{ChannelSnapshot, Metadata, Player, CHANNEL_COUNT};
use std::sync::{Arc, Mutex};

#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::{self, TryRecvError};

#[cfg(not(target_arch = "wasm32"))]
pub mod capture;
mod clock;
pub mod cpu;
pub mod fb;
pub mod font;
pub mod kanji;
pub mod layout;
pub mod levels;
pub mod monitor;
pub mod palette;
mod perf;
pub mod playlist;
pub mod settings;
mod spectrum;
mod telemetry;
mod visualizer;
use cpu::CpuMeter;
use perf::{FrameLimiter, FrameStats};
use spectrum::SpectrumAnalyzer;
use telemetry::{SharedTelemetry, TelemetryFrame};
use visualizer::ScopeHistory;

#[cfg(any(target_arch = "wasm32", test))]
mod bundle;

#[cfg(not(target_arch = "wasm32"))]
mod audio;

#[cfg(not(target_arch = "wasm32"))]
mod native_files;
#[cfg(target_arch = "wasm32")]
mod web_audio;
#[cfg(target_arch = "wasm32")]
mod web_files;

#[cfg(not(target_arch = "wasm32"))]
use audio::AudioOutput;
#[cfg(target_arch = "wasm32")]
use bundle::MemoryBundle;
#[cfg(not(target_arch = "wasm32"))]
use native_files::{LoadedSong, MemoryFiles};
#[cfg(target_arch = "wasm32")]
use web_audio::WebAudioOutput;
#[cfg(target_arch = "wasm32")]
use web_files::WebFilePicker;

#[cfg(not(target_arch = "wasm32"))]
struct NativeLoadJob {
    result: mpsc::Receiver<Result<LoadedSong, String>>,
    label: String,
    path: String,
    target: Option<usize>,
    autoplay: bool,
}

pub const DEFAULT_SAMPLE_RATE: u32 = 44_100;

const MONITOR_FRAMES_PER_SECOND: f32 = 30.0;

const IDLE_FRAMES_PER_SECOND: f32 = 12.0;

pub const ANALYSER_DECIMATION: usize = 2;

const SEEK_STEP_SECONDS: f32 = 5.0;

const SEEK_BUDGET_SECONDS: u32 = 30;

const FADE_SECONDS: f32 = 1.5;

pub const DRIVER_CREDITS: [&str; 3] = [
    "PMD 4.8        (C)M.KAJIHARA",
    "PMDWIN PORT    (C)C60",
    concat!("PMDMINI-RS ", env!("CARGO_PKG_VERSION"), " (C)VBXQ HAYDAR"),
];

pub const TAGLINE_MACHINES: &str = "for UNIX / LINUX / WINDOWS / WEBASSEMBLY";

pub const TAGLINE_PROGRAM: &str = concat!(
    "PMD MINI MONITOR Ver ",
    env!("CARGO_PKG_VERSION"),
    " (C)VBXQ HAYDAR"
);

pub struct PmdApp {
    player: Arc<Mutex<Player>>,
    loaded_name: Option<String>,
    metadata: Metadata,
    status: String,
    volume: f32,
    playing: bool,

    settings: settings::Settings,

    playlist: playlist::Playlist,

    show_list: bool,

    zoom: Option<u32>,

    fade: Option<f32>,

    meters: levels::LevelAnalyzer,
    telemetry: SharedTelemetry,
    telemetry_frame: TelemetryFrame,
    master_peak: f32,
    scope_history: ScopeHistory,
    scope_scratch: Vec<f32>,
    spectrum: SpectrumAnalyzer,
    loop_length: Option<u64>,
    last_loops: u32,
    frame_stats: FrameStats,
    frame_limiter: FrameLimiter,

    cpu_meter: CpuMeter,

    cpu_text: String,

    last_frame_time: f64,

    frame_dt: f32,

    frame_buffer: fb::Framebuffer,

    screen_texture: Option<egui::TextureHandle>,
    output_rate: u32,
    #[cfg(not(target_arch = "wasm32"))]
    audio: Option<AudioOutput>,
    #[cfg(not(target_arch = "wasm32"))]
    files: MemoryFiles,
    #[cfg(not(target_arch = "wasm32"))]
    native_load: Option<NativeLoadJob>,
    #[cfg(target_arch = "wasm32")]
    web_audio: Option<WebAudioOutput>,
    #[cfg(target_arch = "wasm32")]
    web_files: Option<WebFilePicker>,
    #[cfg(target_arch = "wasm32")]
    web_bundle: Option<MemoryBundle>,
}

impl PmdApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(7, 12, 27);
        visuals.window_fill = egui::Color32::from_rgb(9, 16, 34);
        visuals.extreme_bg_color = egui::Color32::from_rgb(4, 8, 19);
        visuals.faint_bg_color = egui::Color32::from_rgb(13, 24, 45);
        cc.egui_ctx.set_visuals(visuals);
        #[cfg(not(target_arch = "wasm32"))]
        let (mut audio, output_rate, _audio_status) = match AudioOutput::open() {
            Ok(audio) => {
                let output_rate = audio.sample_rate();
                let channels = audio.channels();
                (
                    Some(audio),
                    output_rate,
                    format!("Output ready ({channels} channels)"),
                )
            }
            Err(error) => (
                None,
                DEFAULT_SAMPLE_RATE,
                format!("Output unavailable: {error}"),
            ),
        };

        #[cfg(target_arch = "wasm32")]
        let (output_rate, _audio_status) = (
            DEFAULT_SAMPLE_RATE,
            String::from("Web Audio is initialized by the Phase 4 user gesture."),
        );

        let player = Arc::new(Mutex::new(Player::new(output_rate)));
        let initial_frame = TelemetryFrame::new(
            player
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .channel_snapshots(),
        );
        let telemetry = SharedTelemetry::new(initial_frame);
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(audio_output) = audio.as_mut() {
            if let Err(error) = audio_output.start_worker(Arc::clone(&player), telemetry.clone()) {
                eprintln!("audio producer unavailable: {error}");
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        let initial_status = String::from("Choose a PMD file to begin.");
        #[cfg(target_arch = "wasm32")]
        let (initial_status, web_files) = match WebFilePicker::new(cc.egui_ctx.clone()) {
            Ok(picker) => (String::from("Choose a PMD file to begin."), Some(picker)),
            Err(error) => (format!("Browser file input unavailable: {error:?}"), None),
        };

        let stored = settings::Settings::load();

        #[allow(unused_mut)]
        let mut app = Self {
            player,
            loaded_name: None,
            metadata: Metadata::default(),
            status: initial_status,
            volume: stored.volume,
            playing: false,
            show_list: false,
            zoom: stored.zoom,
            fade: None,
            playlist: playlist::Playlist::from_paths(stored.playlist.clone()),
            settings: stored,
            meters: levels::LevelAnalyzer::default(),
            telemetry,
            telemetry_frame: initial_frame,
            master_peak: 0.0,
            scope_history: ScopeHistory::default(),
            scope_scratch: Vec::with_capacity(4096),
            spectrum: SpectrumAnalyzer::default(),
            loop_length: None,
            last_loops: 0,
            frame_stats: FrameStats::default(),
            frame_limiter: FrameLimiter::new(MONITOR_FRAMES_PER_SECOND),
            cpu_meter: CpuMeter::default(),
            cpu_text: String::from(cpu::MEASURING),
            last_frame_time: clock::now_seconds(),
            frame_dt: 1.0 / MONITOR_FRAMES_PER_SECOND,
            frame_buffer: fb::Framebuffer::new(),
            screen_texture: None,
            output_rate,
            #[cfg(not(target_arch = "wasm32"))]
            audio,
            #[cfg(not(target_arch = "wasm32"))]
            files: MemoryFiles::default(),
            #[cfg(not(target_arch = "wasm32"))]
            native_load: None,
            #[cfg(target_arch = "wasm32")]
            web_audio: None,
            #[cfg(target_arch = "wasm32")]
            web_files,
            #[cfg(target_arch = "wasm32")]
            web_bundle: None,
        };

        app.set_audio_volume();
        app.apply_loop_count();
        app.apply_mutes();

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = std::env::var_os("PMDMINI_AUTOLOAD") {
            app.load_path(std::path::Path::new(&path));
            if app.loaded_name.is_some() && std::env::var_os("PMDMINI_AUTOPLAY").is_some() {
                app.apply_transport(TransportAction::Play);
            }
        }

        app
    }

    fn position_samples(&self) -> u64 {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.audio
                .as_ref()
                .map(AudioOutput::position_samples)
                .unwrap_or(self.telemetry_frame.position_samples)
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.telemetry_frame.position_samples
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn buffered_label(&self) -> String {
        let Some(audio) = self.audio.as_ref() else {
            return String::from("n/a");
        };
        format!(
            "{} blocks / {} underruns / {} callbacks",
            audio.buffered_blocks(),
            audio.underruns(),
            audio.callback_calls()
        )
    }

    #[cfg(target_arch = "wasm32")]
    fn buffered_label(&self) -> String {
        String::from("Web Audio / 400 ms scheduled")
    }

    #[cfg(target_arch = "wasm32")]
    fn set_audio_volume(&mut self) {
        if let Some(audio) = self.web_audio.as_mut() {
            audio.set_volume(self.volume);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn set_audio_volume(&mut self) {
        if let Some(audio) = self.audio.as_ref() {
            audio.set_volume(self.volume);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn pause_audio(&self) {
        if let Some(audio) = self.audio.as_ref() {
            audio.set_playing(false);
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn pause_audio(&mut self) {
        if let Some(audio) = self.web_audio.as_mut() {
            audio.suspend();
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn reset_audio(&self) {
        if let Some(audio) = self.audio.as_ref() {
            audio.set_playing(false);
            audio.clear();
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn reset_audio(&mut self) {
        if let Some(audio) = self.web_audio.as_mut() {
            audio.reset();
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn refill_audio(&mut self) {
        if let Some(audio) = self.audio.as_ref() {
            audio.take_scope(&mut self.scope_scratch);
            self.scope_history.push_samples(&self.scope_scratch);
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn refill_audio(&mut self) {
        if !self.playing {
            return;
        }

        if let Err(error) = self.try_refill_web_audio() {
            self.playing = false;

            self.status = format!("Web Audio failed: {error}");
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn try_refill_web_audio(&mut self) -> Result<(), String> {
        if self.web_audio.is_none() {
            let audio = WebAudioOutput::new(self.output_rate)
                .map_err(|error| format!("create AudioContext: {error:?}"))?;
            self.output_rate = audio.sample_rate();

            self.web_audio = Some(audio);
        }

        let (scheduled, snapshots, position_samples, status, cost, peaks) = {
            let audio = self
                .web_audio
                .as_mut()
                .expect("web audio is installed above");
            audio
                .resume()
                .map_err(|error| format!("resume AudioContext: {error:?}"))?;
            let mut player = self
                .player
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let before_position = player.position_samples();
            let started = clock::now_seconds();
            let scheduled = audio.schedule(&mut player, &mut self.scope_scratch);
            let cost = (
                clock::now_seconds() - started,
                player.position_samples().saturating_sub(before_position),
            );
            (
                scheduled,
                player.channel_snapshots(),
                player.position_samples(),
                player.playback_status(),
                cost,
                player.voice_peaks(),
            )
        };
        self.cpu_meter.add_synthesis(cost.0, cost.1);
        scheduled.map_err(|error| format!("schedule AudioBuffer: {error:?}"))?;
        self.scope_history.push_samples(&self.scope_scratch);

        self.publish_telemetry_with_peaks(snapshots, position_samples, status, peaks);
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    fn poll_web_files(&mut self) {
        let Some(picker) = self.web_files.as_ref() else {
            return;
        };
        let Some(result) = picker.poll() else {
            return;
        };

        self.playing = false;
        self.reset_audio();
        match result {
            Ok(bundle) => {
                let file_count = bundle.file_count();
                let main_name = bundle.main_name().to_owned();
                let main_size = bundle.main_bytes().len();
                let load_result = {
                    let mut player = self
                        .player
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    let result = player.load(bundle.main_bytes(), &bundle);
                    if result.is_ok() {
                        self.metadata = player.metadata().clone();
                    }
                    result
                };
                match load_result {
                    Ok(()) => {
                        self.loaded_name = Some(main_name.clone());

                        let songs = bundle.song_names();
                        self.playlist.extend(&songs);
                        if let Some(index) =
                            self.playlist.entries().iter().position(|entry| {
                                entry.path.eq_ignore_ascii_case(bundle.main_name())
                            })
                        {
                            self.playlist.set_playing(index);
                        }
                        self.settings.playlist = self.playlist.paths();
                        self.settings.save();
                        self.web_bundle = Some(bundle);
                        self.status = format!(
                            "Loaded {main_name} ({main_size} bytes, {file_count} file(s)). Press Play."
                        );
                    }
                    Err(error) => {
                        self.loaded_name = None;
                        self.web_bundle = None;
                        self.status = format!("PMD load failed for {main_name}: {error:?}");
                    }
                }
            }
            Err(error) => {
                self.loaded_name = None;
                self.web_bundle = None;
                self.status = format!("Browser file bundle failed: {error}");
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn open_file(&mut self) {
        self.cancel_native_load();
        self.playing = false;
        self.reset_audio();

        let mut dialog = rfd::FileDialog::new().add_filter("PMD music", &["M", "M2", "MZ"]);
        if let Some(directory) = self.settings.last_directory.as_ref() {
            dialog = dialog.set_directory(directory);
        }
        let Some(paths) = dialog.pick_files() else {
            return;
        };
        self.add_paths(&paths);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn add_paths(&mut self, paths: &[std::path::PathBuf]) {
        let mut expanded: Vec<String> = Vec::new();
        for path in paths {
            if path.is_dir() {
                let Ok(entries) = std::fs::read_dir(path) else {
                    continue;
                };
                let mut found: Vec<String> = entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| is_song_path(path))
                    .filter_map(|path| path.to_str().map(str::to_owned))
                    .collect();
                found.sort();
                expanded.extend(found);
            } else if let Some(text) = path.to_str() {
                expanded.push(text.to_owned());
            }
        }
        if expanded.is_empty() {
            self.status = String::from("No PMD files in that selection");
            return;
        }
        if let Some(directory) = paths
            .first()
            .and_then(|path| path.parent())
            .and_then(|parent| parent.to_str())
        {
            self.settings.last_directory = Some(directory.to_owned());
        }
        let start = self.playlist.len();
        let added = self.playlist.extend(&expanded);
        self.settings.playlist = self.playlist.paths();
        self.settings.save();
        self.status = format!("Added {added} file(s)");
        if added > 0 {
            self.play_index(start.min(self.playlist.len() - 1));
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn install_native_song(&mut self, loaded: LoadedSong) {
        let LoadedSong {
            name,
            directory,
            files,
            player,
            metadata,
        } = loaded;
        let file_count = files.file_count();
        let (snapshots, playback_status) = {
            let snapshots = player.channel_snapshots();
            let playback_status = player.playback_status();
            let mut current = self
                .player
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *current = player;
            (snapshots, playback_status)
        };

        self.files = files;
        self.metadata = metadata;
        self.loaded_name = Some(name);
        self.loop_length = None;
        self.last_loops = 0;
        self.fade = None;
        self.scope_history.clear();
        self.scope_scratch.clear();
        self.publish_telemetry_with_status(snapshots, 0, playback_status);
        self.status = format!("Loaded {file_count} file(s). Press Play.");

        if self.settings.last_directory.as_deref() != directory.as_deref() {
            self.settings.last_directory = directory;
            self.settings.save();
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn cancel_native_load(&mut self) {
        self.native_load.take();
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn begin_native_load(&mut self, path: &std::path::Path, target: Option<usize>, autoplay: bool) {
        self.cancel_native_load();
        self.playing = false;
        self.reset_audio();
        self.loaded_name = None;
        self.metadata = Metadata::default();
        self.files = MemoryFiles::default();
        self.fade = None;
        self.loop_length = None;
        self.last_loops = 0;
        self.scope_history.clear();
        self.scope_scratch.clear();

        let label = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| path.display().to_string());
        let path_text = path.to_string_lossy().into_owned();
        let worker_path = path.to_owned();
        let sample_rate = self.output_rate;
        let loop_count = self.settings.loop_count;
        let muted = self.settings.muted;
        let (sender, result) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name(String::from("pmdmini-file-loader"))
            .spawn(move || {
                let result = native_files::load_song(&worker_path, sample_rate, loop_count, muted);
                let _ = sender.send(result);
            });
        if let Err(error) = worker {
            self.status = format!("Could not start loading {label}: {error}");
            return;
        }

        self.status = format!("Loading {label}…");
        self.native_load = Some(NativeLoadJob {
            result,
            label,
            path: path_text,
            target,
            autoplay,
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_path(&mut self, path: &std::path::Path) {
        self.cancel_native_load();
        self.playing = false;
        self.reset_audio();
        self.loaded_name = None;
        self.metadata = Metadata::default();
        self.files = MemoryFiles::default();

        match native_files::load_song(
            path,
            self.output_rate,
            self.settings.loop_count,
            self.settings.muted,
        ) {
            Ok(loaded) => self.install_native_song(loaded),
            Err(error) => self.status = error,
        }
    }

    fn reset_position(&mut self) {
        self.reset_audio();
        self.scope_history.clear();
        self.scope_scratch.clear();
        let snapshots = {
            let mut player = self
                .player
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            player.reset();
            player.channel_snapshots()
        };
        self.publish_telemetry(snapshots, 0);
    }

    fn publish_telemetry(
        &mut self,
        snapshots: [ChannelSnapshot; CHANNEL_COUNT],
        position_samples: u64,
    ) {
        self.publish_telemetry_with_status(
            snapshots,
            position_samples,
            pmd_core::PlaybackStatus {
                loop_limit: self.settings.loop_count,
                ..pmd_core::PlaybackStatus::default()
            },
        );
    }

    #[cfg(target_arch = "wasm32")]
    fn publish_telemetry_with_peaks(
        &mut self,
        snapshots: [ChannelSnapshot; CHANNEL_COUNT],
        position_samples: u64,
        status: pmd_core::PlaybackStatus,
        peaks: pmd_core::VoicePeaks,
    ) {
        self.telemetry.publish(TelemetryFrame {
            snapshots,
            position_samples,
            status,
            peaks,
            sequence: 0,
        });
        self.telemetry.read_into(&mut self.telemetry_frame);
    }

    fn publish_telemetry_with_status(
        &mut self,
        snapshots: [ChannelSnapshot; CHANNEL_COUNT],
        position_samples: u64,
        status: pmd_core::PlaybackStatus,
    ) {
        self.telemetry.publish(TelemetryFrame {
            snapshots,
            position_samples,
            status,

            peaks: pmd_core::VoicePeaks::default(),
            sequence: 0,
        });
        self.telemetry.read_into(&mut self.telemetry_frame);
    }

    fn update_levels(&mut self) {
        self.telemetry.read_into(&mut self.telemetry_frame);
        #[cfg(not(target_arch = "wasm32"))]
        let master_peak = self
            .audio
            .as_ref()
            .map(|audio| {
                let [left, right] = audio.take_peak();
                left.max(right).clamp(0.0, 1.0)
            })
            .unwrap_or(0.0);
        #[cfg(target_arch = "wasm32")]
        let master_peak = self.scope_history.peak();
        self.master_peak = self.master_peak * 0.65 + master_peak * 0.35;

        self.meters.update(
            &self.telemetry_frame.peaks,
            &self.settings.muted,
            self.frame_dt,
        );
    }

    fn apply_transport(&mut self, action: TransportAction) {
        match action {
            TransportAction::Play => {
                if self.loaded_name.is_some() {
                    self.playing = true;
                    self.status = String::from("Playing");
                    #[cfg(not(target_arch = "wasm32"))]
                    if let Some(audio) = self.audio.as_ref() {
                        audio.set_playing(true);
                    }
                    self.refill_audio();
                }
            }
            TransportAction::Pause => {
                self.playing = false;
                self.status = String::from("Paused");
                self.pause_audio();
            }
            TransportAction::Stop => {
                #[cfg(not(target_arch = "wasm32"))]
                self.cancel_native_load();
                self.playing = false;
                self.status = String::from("Stopped at the start of the song");
                self.reset_position();
            }
            TransportAction::Rewind => self.seek_by(-SEEK_STEP_SECONDS),
            TransportAction::Forward => self.seek_by(SEEK_STEP_SECONDS),
        }
    }
}

#[derive(Clone, Copy)]
enum TransportAction {
    Play,
    Pause,
    Stop,
    Rewind,
    Forward,
}

impl PmdApp {
    fn analyser_rate(&self) -> f32 {
        self.output_rate as f32 / ANALYSER_DECIMATION as f32
    }

    fn update_analysers(&mut self) {
        let rate = self.analyser_rate();
        let dt = self.frame_dt;
        let window = self.scope_history.analysis_window();
        self.spectrum.update(window, rate, dt);
    }

    fn advance_frame_clock(&mut self) {
        let now = clock::now_seconds();
        let previous = std::mem::replace(&mut self.last_frame_time, now);
        self.frame_dt = ((now - previous) as f32).clamp(0.0, 0.25);
    }

    fn update_cpu_meter(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(audio) = self.audio.as_ref() {
            let (seconds, frames) = audio.take_render_cost();
            self.cpu_meter.add_synthesis(seconds, frames);
        }
        if self
            .cpu_meter
            .tick(clock::now_seconds(), self.output_rate)
            .is_some()
        {
            self.cpu_text = self.cpu_meter.text();
        }
    }

    fn track_loop_length(&mut self) {
        let loops = self.telemetry_frame.status.loops_completed;
        if loops >= 1 && self.loop_length.is_none() {
            let position = self.telemetry_frame.position_samples;
            if position > 0 {
                self.loop_length = Some(position);

                let seconds = (position / u64::from(self.output_rate.max(1))) as u32;
                self.playlist.record_length(seconds);
                self.settings.save();
            }
        }
        self.last_loops = loops;
    }

    fn seek_by(&mut self, delta_seconds: f32) {
        if self.loaded_name.is_none() {
            return;
        }
        let rate = self.output_rate as f32;
        let current = self.position_samples() as f32;
        let target = (current + delta_seconds * rate).max(0.0) as u64;
        self.scope_history.clear();
        self.scope_scratch.clear();
        self.spectrum.clear();

        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(audio) = self.audio.as_ref() {
                audio.request_seek(target);
                self.status = format!("Seeking to {}", format_position(target, self.output_rate));
                return;
            }
        }

        let budget = u64::from(self.output_rate) * u64::from(SEEK_BUDGET_SECONDS);
        let (snapshots, position, status) = {
            let mut player = self
                .player
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if player.position_samples() > target {
                player.reset();
            }
            let mut remaining_budget = budget;
            self.scope_scratch.resize(4_096 * 2, 0.0);
            while player.position_samples() < target && remaining_budget > 0 {
                let remaining = target - player.position_samples();
                let frames = remaining.min(4_096).min(remaining_budget) as usize;
                player.render(&mut self.scope_scratch[..frames * 2]);
                remaining_budget -= frames as u64;
            }
            (
                player.channel_snapshots(),
                player.position_samples(),
                player.playback_status(),
            )
        };
        self.scope_scratch.clear();
        self.publish_telemetry_with_status(snapshots, position, status);
        self.status = format!("Seek to {}", format_position(position, self.output_rate));
    }

    fn present_rect(&self, full: egui::Rect) -> (egui::Rect, f32) {
        let fit = (full.width() / fb::WIDTH as f32)
            .min(full.height() * 5.0 / 6.0 / fb::HEIGHT as f32)
            .max(0.05);
        let scale = match self.zoom {
            Some(zoom) => zoom as f32,
            None => {
                let whole = fit.floor().max(1.0);
                if whole / fit >= 0.86 {
                    whole
                } else {
                    fit
                }
            }
        };
        let size = egui::vec2(
            fb::WIDTH as f32 * scale,
            fb::HEIGHT as f32 * scale * 6.0 / 5.0,
        );
        let origin = full.min + ((full.size() - size) * 0.5).max(egui::Vec2::ZERO);
        (egui::Rect::from_min_size(origin, size), scale)
    }

    fn paint_monitor(&mut self, ui: &mut egui::Ui) {
        let full = ui.available_rect_before_wrap();

        let [r, g, b] = palette::PALETTE[palette::SHADOW as usize];
        ui.painter().rect_filled(
            full,
            egui::CornerRadius::ZERO,
            egui::Color32::from_rgb(r, g, b),
        );

        let elapsed = self.telemetry_frame.position_samples as f32 / self.output_rate.max(1) as f32;
        let title = self.metadata.title.clone().unwrap_or_default();
        let composer = self.metadata.composer.clone().unwrap_or_default();
        let arranger = self.metadata.arranger.clone().unwrap_or_default();
        let memo = self.metadata.comment.clone().unwrap_or_default();
        let file_name = self.loaded_name.clone();
        let list = self.show_list.then(|| monitor::ListView {
            entries: self.playlist.entries(),
            cursor: self.playlist.cursor(),
            playing: self.playlist.playing(),
        });

        let view = monitor::View {
            snapshots: &self.telemetry_frame.snapshots,
            spectrum: &self.spectrum.bars,
            peaks: &self.spectrum.peaks,
            levels: self.meters.values(),
            level_peaks: self.meters.peaks(),
            file_name: file_name.as_deref(),
            title: &title,
            composer: &composer,
            arranger: &arranger,
            memo: &memo,
            muted: &self.settings.muted,
            list: list.as_ref(),
            mode: self.settings.play_mode.label(),
            message: &self.status,
            playing: self.playing,
            elapsed,
            clock: self.telemetry_frame.position_samples / 256,
            loop_field: monitor::format_loops(self.last_loops, self.settings.loop_count),
            volume: (self.volume * 255.0) as u8,
            cpu: &self.cpu_text,
            analyser_rate: self.analyser_rate(),
        };
        let draw_started = clock::now_seconds();
        monitor::render(&mut self.frame_buffer, &view);

        let image = egui::ColorImage::from_rgba_unmultiplied(
            [fb::WIDTH, fb::HEIGHT],
            &self.frame_buffer.to_rgba(&palette::PALETTE),
        );
        match self.screen_texture.as_mut() {
            Some(texture) => texture.set(image, egui::TextureOptions::NEAREST),
            None => {
                self.screen_texture = Some(ui.ctx().load_texture(
                    "pmdmini_screen",
                    image,
                    egui::TextureOptions::NEAREST,
                ));
            }
        }

        self.cpu_meter.add_draw(clock::now_seconds() - draw_started);

        let Some(texture) = self.screen_texture.as_ref() else {
            return;
        };
        let (screen, scale) = self.present_rect(full);
        ui.painter().image(
            texture.id(),
            screen,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );

        let response = ui.interact(
            screen,
            ui.id().with("pmdmini_screen"),
            egui::Sense::click_and_drag(),
        );
        let to_frame = |pos: egui::Pos2| {
            (
                ((pos.x - screen.min.x) / scale) as i32,
                ((pos.y - screen.min.y) / (scale * 6.0 / 5.0)) as i32,
            )
        };

        if response.double_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let (fx, fy) = to_frame(pos);
                if self.show_list {
                    if let Some(row) = monitor::list_row_hit(fx, fy) {
                        self.play_list_row(row);
                    }
                }
            }
            return;
        }
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let (fx, fy) = to_frame(pos);
                let modifiers = ui.ctx().input(|input| input.modifiers);
                self.click(fx, fy, modifiers);
            }
        }
        if response.secondary_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let (fx, fy) = to_frame(pos);
                if monitor::loop_field_hit(fx, fy) {
                    self.step_loop_count(-1);
                }
            }
        }
    }

    fn click(&mut self, x: i32, y: i32, modifiers: egui::Modifiers) {
        if let Some(action) = monitor::hit_test(x, y) {
            self.dispatch(action);
        } else if let Some(level) = monitor::slider_hit(x, y) {
            self.volume = level;
            self.set_audio_volume();
            self.settings.volume = level;
            self.settings.save();
        } else if monitor::loop_field_hit(x, y) {
            self.step_loop_count(1);
        } else if monitor::mode_hit(x, y) {
            self.settings.play_mode = self.settings.play_mode.next();
            self.settings.save();
        } else if let Some(strip) = monitor::strip_hit(x, y) {
            if modifiers.command || modifiers.shift {
                self.solo_channel(strip);
            } else {
                self.toggle_mute(strip);
            }
        } else if self.show_list {
            if let Some(row) = monitor::list_row_hit(x, y) {
                let first = self.list_scroll();
                self.playlist.set_cursor(first + row);
            }
        }
    }

    fn list_scroll(&self) -> usize {
        self.playlist
            .cursor()
            .saturating_sub(layout::LIST_ROWS - 1)
            .min(self.playlist.len().saturating_sub(layout::LIST_ROWS))
    }

    fn apply_loop_count(&mut self) {
        let count = self.settings.loop_count;
        self.player
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .set_loop_count(count);
    }

    fn apply_mutes(&mut self) {
        let mut player = self
            .player
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for (channel, muted) in self.settings.muted.iter().enumerate() {
            player.set_channel_mute(channel, *muted);
        }
    }

    fn toggle_mute(&mut self, channel: usize) {
        let Some(slot) = self.settings.muted.get_mut(channel) else {
            return;
        };
        *slot = !*slot;
        let muted = *slot;
        self.player
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .set_channel_mute(channel, muted);
        self.status = format!(
            "Channel {} {}",
            channel + 1,
            if muted { "muted" } else { "on" }
        );
        self.settings.save();
    }

    fn solo_channel(&mut self, channel: usize) {
        let already_solo = self
            .settings
            .muted
            .iter()
            .enumerate()
            .all(|(index, muted)| *muted != (index == channel));
        for (index, slot) in self.settings.muted.iter_mut().enumerate() {
            *slot = !already_solo && index != channel;
        }
        self.apply_mutes();
        self.status = if already_solo {
            String::from("All channels on")
        } else {
            format!("Channel {} solo", channel + 1)
        };
        self.settings.save();
    }

    fn unmute_all(&mut self) {
        self.settings.muted = [false; CHANNEL_COUNT];
        self.apply_mutes();
        self.status = String::from("All channels on");
        self.settings.save();
    }

    fn step_loop_count(&mut self, delta: i32) {
        const MAX: u32 = 9;
        self.settings.loop_count = match (self.settings.loop_count, delta) {
            (None, d) if d > 0 => Some(1),
            (None, _) => Some(MAX),
            (Some(MAX), d) if d > 0 => None,
            (Some(1), d) if d < 0 => None,
            (Some(count), d) => Some(count.saturating_add_signed(d).clamp(1, MAX)),
        };
        self.apply_loop_count();
        self.status = match self.settings.loop_count {
            Some(count) => format!("Loop count {count}"),
            None => String::from("Loop count INFINITE"),
        };
        self.settings.save();
    }

    fn skip_track(&mut self, delta: i32) {
        let target = if delta < 0 {
            self.playlist.previous()
        } else {
            self.playlist.next()
        };
        if let Some(index) = target {
            self.play_index(index);
        }
    }

    fn play_list_row(&mut self, row: usize) {
        let index = self.list_scroll() + row;
        if index < self.playlist.len() {
            self.play_index(index);
        }
    }

    fn play_index(&mut self, index: usize) {
        let Some(entry) = self.playlist.entries().get(index).cloned() else {
            return;
        };
        self.apply_transport(TransportAction::Stop);
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.begin_native_load(std::path::Path::new(&entry.path), Some(index), true);
        }
        #[cfg(target_arch = "wasm32")]
        if self.load_entry(&entry.path) {
            self.playlist.set_playing(index);
            self.fade = None;
            self.apply_transport(TransportAction::Play);
        } else {
            self.playlist.remove(index);
            self.settings.playlist = self.playlist.paths();
            self.settings.save();
        }
    }

    fn advance_playlist(&mut self) {
        match self.playlist.next_under(self.settings.play_mode) {
            Some(index) => self.play_index(index),
            None => {
                self.apply_transport(TransportAction::Stop);
                self.status = String::from("End of playlist");
            }
        }
    }

    fn update_fade(&mut self) {
        let status = self.telemetry_frame.status;
        let reached = match self.settings.loop_count {
            Some(limit) => status.loops_completed >= limit,
            None => false,
        };
        if (reached || status.ended) && self.playing && self.fade.is_none() {
            self.fade = Some(FADE_SECONDS);
        }
        let Some(remaining) = self.fade else {
            return;
        };
        let remaining = remaining - self.frame_dt;
        if remaining <= 0.0 {
            self.fade = None;
            self.volume = self.settings.volume;
            self.set_audio_volume();
            self.advance_playlist();
            return;
        }
        self.fade = Some(remaining);
        self.volume = self.settings.volume * (remaining / FADE_SECONDS);
        self.set_audio_volume();
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        use egui::Key;
        let (keys, modifiers) = ctx.input(|input| {
            (
                input
                    .events
                    .iter()
                    .filter_map(|event| match event {
                        egui::Event::Key {
                            key,
                            pressed: true,
                            repeat: false,
                            ..
                        } => Some(*key),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
                input.modifiers,
            )
        });
        for key in keys {
            match key {
                Key::Space => {
                    if self.playing {
                        self.apply_transport(TransportAction::Pause);
                    } else {
                        self.apply_transport(TransportAction::Play);
                    }
                }
                Key::Enter => {
                    if self.show_list {
                        self.play_list_row(self.playlist.cursor() - self.list_scroll());
                    } else {
                        self.apply_transport(TransportAction::Play);
                    }
                }
                Key::ArrowUp => self.playlist.move_cursor(-1),
                Key::ArrowDown => self.playlist.move_cursor(1),
                Key::Delete | Key::Backspace => {
                    let cursor = self.playlist.cursor();
                    self.playlist.remove(cursor);
                    self.settings.playlist = self.playlist.paths();
                    self.settings.save();
                }
                Key::L => self.show_list = !self.show_list,
                Key::N => self.skip_track(1),
                Key::P => self.skip_track(-1),
                Key::M => {
                    self.settings.play_mode = self.settings.play_mode.next();
                    self.settings.save();
                }
                Key::R => self.unmute_all(),
                Key::OpenBracket => self.step_loop_count(-1),
                Key::CloseBracket => self.step_loop_count(1),
                Key::F | Key::F11 => self.toggle_fullscreen(ctx),
                Key::Plus | Key::Equals => self.step_zoom(1),

                Key::Minus if modifiers.shift || modifiers.command => self.step_zoom(-1),
                Key::Minus => self.mute_key(10),
                Key::Num0 => self.mute_key(9),
                Key::Num1 => self.mute_key(0),
                Key::Num2 => self.mute_key(1),
                Key::Num3 => self.mute_key(2),
                Key::Num4 => self.mute_key(3),
                Key::Num5 => self.mute_key(4),
                Key::Num6 => self.mute_key(5),
                Key::Num7 => self.mute_key(6),
                Key::Num8 => self.mute_key(7),
                Key::Num9 => self.mute_key(8),
                _ => {}
            }
        }
    }

    fn mute_key(&mut self, channel: usize) {
        self.toggle_mute(channel);
    }

    fn step_zoom(&mut self, delta: i32) {
        let current = self.zoom.unwrap_or(0) as i32;
        let next = current + delta;
        self.zoom = (1..=6).contains(&next).then_some(next as u32);
        self.settings.zoom = self.zoom;
        self.status = match self.zoom {
            Some(zoom) => format!("Zoom x{zoom}"),
            None => String::from("Zoom fit"),
        };
        self.settings.save();
    }

    fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        let fullscreen = ctx.input(|input| input.viewport().fullscreen.unwrap_or(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn poll_native_load(&mut self) {
        let Some(job) = self.native_load.take() else {
            return;
        };
        match job.result.try_recv() {
            Ok(Ok(loaded)) => {
                let target_path = job.path;
                let target_index = job.target.and_then(|_| {
                    self.playlist
                        .entries()
                        .iter()
                        .position(|entry| entry.path == target_path)
                });
                self.install_native_song(loaded);
                if let Some(index) = target_index {
                    self.playlist.set_playing(index);
                }
                if job.autoplay {
                    self.fade = None;
                    self.apply_transport(TransportAction::Play);
                }
            }
            Ok(Err(error)) => {
                self.loaded_name = None;
                self.metadata = Metadata::default();
                let remove_index = job.target.and_then(|_| {
                    self.playlist
                        .entries()
                        .iter()
                        .position(|entry| entry.path == job.path)
                });
                if let Some(index) = remove_index {
                    self.playlist.remove(index);
                    self.settings.playlist = self.playlist.paths();
                    self.settings.save();
                }
                self.status = format!("{}: {error}", job.label);
            }
            Err(TryRecvError::Empty) => {
                self.native_load = Some(job);
            }
            Err(TryRecvError::Disconnected) => {
                self.loaded_name = None;
                self.status = format!("{}: loader stopped", job.label);
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn poll_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<std::path::PathBuf> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| file.path.clone())
                .collect()
        });
        if !dropped.is_empty() {
            self.add_paths(&dropped);
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn load_entry(&mut self, path: &str) -> bool {
        let Some(bundle) = self.web_bundle.as_mut() else {
            self.status = String::from("No files loaded");
            return false;
        };
        if !bundle.select_main(path) {
            self.status = format!("{path} is not in the loaded bundle");
            return false;
        }
        let name = bundle.main_name().to_owned();
        let result = {
            let mut player = self
                .player
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let outcome = player.load(bundle.main_bytes(), bundle);
            if outcome.is_ok() {
                self.metadata = player.metadata().clone();
            }
            outcome
        };
        match result {
            Ok(()) => {
                self.loaded_name = Some(name.clone());
                self.status = format!("Loaded {name}");
                true
            }
            Err(error) => {
                self.status = format!("PMD load failed for {name}: {error:?}");
                false
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn request_file(&mut self) {
        self.open_file();
    }

    #[cfg(target_arch = "wasm32")]
    fn request_file(&mut self) {
        if let Some(picker) = self.web_files.as_ref() {
            picker.open();
        }
    }

    fn dispatch(&mut self, action: monitor::Action) {
        match action {
            monitor::Action::Play => self.apply_transport(TransportAction::Play),
            monitor::Action::Stop => self.apply_transport(TransportAction::Stop),
            monitor::Action::Pause => self.apply_transport(TransportAction::Pause),
            monitor::Action::Rewind => self.apply_transport(TransportAction::Rewind),
            monitor::Action::FastForward => self.apply_transport(TransportAction::Forward),
            monitor::Action::File => self.request_file(),
            monitor::Action::Previous => self.skip_track(-1),
            monitor::Action::Next => self.skip_track(1),
            monitor::Action::List => self.show_list = !self.show_list,

            monitor::Action::Fade => {}
        }
    }
}

impl eframe::App for PmdApp {
    fn on_exit(&mut self) {
        self.settings.flush(clock::now_seconds());
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let pointer_busy = ui
            .ctx()
            .input(|input| input.pointer.any_down() || input.pointer.any_released());
        self.frame_limiter
            .set_rate(if self.playing || pointer_busy {
                MONITOR_FRAMES_PER_SECOND
            } else {
                IDLE_FRAMES_PER_SECOND
            });
        if !pointer_busy {
            self.frame_limiter.pace();
        }
        self.frame_stats.begin_frame();
        self.advance_frame_clock();
        if self.playing {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs_f32(
                    1.0 / MONITOR_FRAMES_PER_SECOND,
                ));
        }
        self.handle_keys(ui.ctx());
        #[cfg(not(target_arch = "wasm32"))]
        self.poll_native_load();
        self.refill_audio();
        self.update_levels();
        self.update_analysers();
        self.track_loop_length();
        self.update_cpu_meter();
        self.update_fade();
        #[cfg(not(target_arch = "wasm32"))]
        self.poll_dropped_files(ui.ctx());
        self.settings.flush_if_due(clock::now_seconds());
        if self.settings.is_dirty() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs_f64(
                    settings::SAVE_INTERVAL_SECONDS,
                ));
        }

        #[cfg(target_arch = "wasm32")]
        self.poll_web_files();

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::BLACK))
            .show(ui, |ui| {
                self.paint_monitor(ui);
            });

        #[cfg(not(target_arch = "wasm32"))]
        if self.native_load.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(33));
        }

        let time = ui.ctx().input(|input| input.time);
        if let Some(line) = self.frame_stats.end_frame(time) {
            println!("{line} state={} {}", self.playing, self.buffered_label());
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn is_song_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            let extension = extension.to_ascii_uppercase();
            matches!(extension.as_str(), "M" | "M2" | "MZ")
        })
        .unwrap_or(false)
}

pub fn format_position(samples: u64, sample_rate: u32) -> String {
    if sample_rate == 0 {
        return String::from("00:00");
    }
    let total_seconds = samples / u64::from(sample_rate);
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes:02}:{seconds:02}")
}

#[cfg(test)]
mod tests {
    use super::{font, format_position, TAGLINE_MACHINES, TAGLINE_PROGRAM};

    #[test]
    fn position_is_formatted_from_output_samples() {
        assert_eq!(format_position(0, 44_100), "00:00");
        assert_eq!(format_position(44_100 * 61, 44_100), "01:01");
    }

    #[test]
    fn zero_rate_has_a_safe_display_value() {
        assert_eq!(format_position(123, 0), "00:00");
    }

    #[test]
    fn program_tagline_uses_the_package_version_and_fits_its_box() {
        assert_eq!(
            TAGLINE_PROGRAM,
            concat!(
                "PMD MINI MONITOR Ver ",
                env!("CARGO_PKG_VERSION"),
                " (C)VBXQ HAYDAR"
            )
        );
        assert!(TAGLINE_PROGRAM.chars().count() <= 42);
    }

    #[test]
    fn platform_tagline_fills_its_header_box() {
        assert_eq!(TAGLINE_MACHINES, "for UNIX / LINUX / WINDOWS / WEBASSEMBLY");
        assert_eq!(font::width(TAGLINE_MACHINES), 200);
        assert!(font::width(TAGLINE_MACHINES) <= 214);
    }
}
