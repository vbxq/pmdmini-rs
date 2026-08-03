// SPDX-License-Identifier: AGPL-3.0-only

use crate::fb::{Framebuffer, HEIGHT, WIDTH};
use crate::levels::LevelAnalyzer;
use crate::monitor::{self, View};
use crate::native_files::MemoryFiles;
use crate::palette::PALETTE;
use crate::spectrum::SpectrumAnalyzer;
use crate::visualizer::ScopeHistory;
use pmd_core::{ChannelSnapshot, Player, CHANNEL_COUNT};

const CAPTURE_FPS: f32 = crate::MONITOR_FRAMES_PER_SECOND;

pub fn write_screenshot(path: &str) -> Result<(), String> {
    let snapshots = [ChannelSnapshot::default(); CHANNEL_COUNT];
    let spectrum = [0.0f32; crate::spectrum::BAR_COUNT];
    let peaks = [0.0f32; crate::spectrum::BAR_COUNT];
    let levels = [0.0f32; crate::levels::COLUMN_COUNT];

    let view = View {
        snapshots: &snapshots,
        spectrum: &spectrum,
        peaks: &peaks,
        levels: &levels,
        level_peaks: &levels,
        file_name: Some("VOYAGE~1.M"),
        title: "",
        composer: "",
        arranger: "",
        memo: "",
        muted: &[false; CHANNEL_COUNT],
        list: None,
        mode: "SINGLE",
        message: "",
        playing: true,
        elapsed: 0.0,
        clock: 0,
        loop_field: monitor::format_loops(0, None),
        volume: 0,
        cpu: crate::cpu::MEASURING,
        analyser_rate: 22_050.0,
    };

    let mut fb = Framebuffer::new();
    monitor::render(&mut fb, &view);
    write_png(path, &fb)
}

pub struct SongCapture<'a> {
    pub song: &'a std::path::Path,

    pub output: &'a str,

    pub at_seconds: f32,

    pub muted: [bool; CHANNEL_COUNT],

    pub list: Vec<String>,

    pub list_cursor: usize,

    pub loop_target: Option<u32>,

    pub message: String,
}

pub fn write_song(capture: &SongCapture) -> Result<(), String> {
    let rate = crate::DEFAULT_SAMPLE_RATE;
    let (name, main, files) = MemoryFiles::from_main_path(capture.song)?;
    let mut player = Player::new(rate);
    player
        .load(&main, &files)
        .map_err(|error| format!("{}: {error:?}", capture.song.display()))?;
    let metadata = player.metadata().clone();

    let mut scope = ScopeHistory::default();
    let mut spectrum = SpectrumAnalyzer::default();
    let mut meters = LevelAnalyzer::default();
    let dt = 1.0 / CAPTURE_FPS;
    let block_frames = (rate as f32 * dt) as usize;
    let mut audio = vec![0.0f32; block_frames * 2];
    let mut decimated = Vec::with_capacity(block_frames / crate::ANALYSER_DECIMATION + 1);

    let frames = ((capture.at_seconds.max(0.0) * CAPTURE_FPS) as usize).max(1);
    let mut snapshots = player.channel_snapshots();
    let mut last_peaks = pmd_core::VoicePeaks::default();
    for _ in 0..frames {
        player.render(&mut audio);
        decimated.clear();

        let scale = 1.0 / (2 * crate::ANALYSER_DECIMATION) as f32;
        for group in audio.chunks_exact(2 * crate::ANALYSER_DECIMATION) {
            decimated.push(group.iter().sum::<f32>() * scale);
        }
        scope.push_samples(&decimated);
        snapshots = player.channel_snapshots();
        let peaks = player.voice_peaks();
        last_peaks = peaks;
        spectrum.update(scope.analysis_window(), analyser_rate(rate), dt);
        meters.update(&peaks, &capture.muted, dt);
    }

    if std::env::var_os("PMDMINI_CAPTURE_DEBUG").is_some() {
        let window = scope.analysis_window();
        let peak = window.iter().fold(0.0f32, |acc, s| acc.max(s.abs()));
        let bars = spectrum.bars.iter().fold(0.0f32, |acc, b| acc.max(*b));
        let meters_max = meters.values().iter().fold(0.0f32, |acc, v| acc.max(*v));
        eprintln!(
            "DEBUG window={} peak={peak:.4} spectrum_max={bars:.3} level_max={meters_max:.3}",
            window.len()
        );
        eprintln!("DEBUG bars={:?}", &spectrum.bars[..12]);
        eprintln!("DEBUG levels={:?}", meters.values());
        eprintln!("DEBUG last_peaks={:?}", last_peaks);
        eprintln!(
            "DEBUG title={:?} composer={:?} arranger={:?} memo={:?}",
            metadata.title, metadata.composer, metadata.arranger, metadata.comment
        );
    }

    let position = player.position_samples();
    let status = player.playback_status();
    let entries: Vec<crate::playlist::Entry> = capture
        .list
        .iter()
        .map(|path| crate::playlist::Entry::from_path(path))
        .collect();
    let list = (!entries.is_empty()).then(|| monitor::ListView {
        entries: &entries,
        cursor: capture.list_cursor.min(entries.len().saturating_sub(1)),
        playing: Some(0),
    });
    let view = View {
        snapshots: &snapshots,
        spectrum: &spectrum.bars,
        peaks: &spectrum.peaks,
        levels: meters.values(),
        level_peaks: meters.peaks(),
        file_name: Some(&name),
        title: metadata.title.as_deref().unwrap_or(""),
        composer: metadata.composer.as_deref().unwrap_or(""),
        arranger: metadata.arranger.as_deref().unwrap_or(""),
        memo: metadata.comment.as_deref().unwrap_or(""),
        muted: &capture.muted,
        list: list.as_ref(),
        mode: "REP.ALL",
        message: &capture.message,
        playing: true,
        elapsed: position as f32 / rate as f32,
        clock: position / 256,
        loop_field: monitor::format_loops(status.loops_completed, capture.loop_target),
        volume: 255,
        cpu: "021-008-4210",
        analyser_rate: analyser_rate(rate),
    };

    let mut fb = Framebuffer::new();
    monitor::render(&mut fb, &view);
    write_png(capture.output, &fb)
}

fn analyser_rate(rate: u32) -> f32 {
    rate as f32 / crate::ANALYSER_DECIMATION as f32
}

pub fn write_png(path: &str, fb: &Framebuffer) -> Result<(), String> {
    let rgba = fb.to_rgba(&PALETTE);
    let mut rgb = Vec::with_capacity(WIDTH * HEIGHT * 3);
    for pixel in rgba.chunks_exact(4) {
        rgb.extend_from_slice(&pixel[..3]);
    }

    let file = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
    let writer = std::io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, WIDTH as u32, HEIGHT as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| format!("{path}: {e}"))?;
    writer
        .write_image_data(&rgb)
        .map_err(|e| format!("{path}: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_screenshot_is_written_at_native_resolution_with_no_scaling() {
        let dir = std::env::temp_dir().join("pmdmini-capture-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("shot.png");
        let path_str = path.to_str().expect("utf-8 path");

        write_screenshot(path_str).expect("screenshot written");

        let bytes = std::fs::read(&path).expect("read back");

        assert_eq!(&bytes[1..4], b"PNG");
        let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        assert_eq!((width, height), (640, 400));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_missing_song_is_reported_rather_than_panicking() {
        let error = write_song(&SongCapture {
            song: std::path::Path::new("/nonexistent/NOPE.M"),
            output: "/dev/null",
            at_seconds: 1.0,
            muted: [false; CHANNEL_COUNT],
            list: Vec::new(),
            list_cursor: 0,
            loop_target: None,
            message: String::new(),
        })
        .expect_err("a missing file cannot be captured");
        assert!(!error.is_empty());
    }
}
