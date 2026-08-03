// SPDX-License-Identifier: AGPL-3.0-only

use pmd_core::{VoicePeaks, CHANNEL_COUNT};

pub const COLUMN_COUNT: usize = 17;

pub const RHYTHM_FIRST: usize = 9;

pub const ADPCM_COLUMN: usize = 15;

pub const EXTENSION_COLUMN: usize = 16;

pub const fn column_channel(column: usize) -> Option<usize> {
    match column {
        0..=8 => Some(column),
        RHYTHM_FIRST..=14 => Some(10),
        ADPCM_COLUMN | EXTENSION_COLUMN => Some(9),
        _ => None,
    }
}

const FLOOR_DB: f32 = -48.0;

const RELEASE_SECONDS: f32 = 0.22;

const PEAK_HOLD_SECONDS: f32 = 0.5;

const PEAK_FALL_PER_SECOND: f32 = 0.9;

#[derive(Debug)]
pub struct LevelAnalyzer {
    values: [f32; COLUMN_COUNT],

    peaks: [f32; COLUMN_COUNT],

    holds: [f32; COLUMN_COUNT],
}

impl Default for LevelAnalyzer {
    fn default() -> Self {
        Self {
            values: [0.0; COLUMN_COUNT],
            peaks: [0.0; COLUMN_COUNT],
            holds: [0.0; COLUMN_COUNT],
        }
    }
}

impl LevelAnalyzer {
    pub fn values(&self) -> &[f32; COLUMN_COUNT] {
        &self.values
    }

    pub fn peaks(&self) -> &[f32; COLUMN_COUNT] {
        &self.peaks
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn update(&mut self, peaks: &VoicePeaks, muted: &[bool; CHANNEL_COUNT], dt: f32) {
        let dt = dt.clamp(0.0, 0.25);
        let release = decay(dt, RELEASE_SECONDS);
        for column in 0..COLUMN_COUNT {
            let (share, channel) = self.target(column, peaks);
            let target = decibel_height(share);
            let value = &mut self.values[column];
            if muted[channel] {
                *value = 0.0;
                self.peaks[column] = 0.0;
                self.holds[column] = 0.0;
                continue;
            }
            *value = if target >= *value {
                target
            } else {
                target + (*value - target) * release
            };

            let peak = &mut self.peaks[column];
            if *value >= *peak {
                *peak = *value;
                self.holds[column] = PEAK_HOLD_SECONDS;
            } else if self.holds[column] > 0.0 {
                self.holds[column] -= dt;
            } else {
                *peak = (*peak - PEAK_FALL_PER_SECOND * dt).max(*value);
            }
        }
    }

    fn target(&self, column: usize, peaks: &VoicePeaks) -> (f32, usize) {
        match column {
            0..=5 => (peaks.fm[column], column),
            6..=8 => (peaks.ssg[column - 6], column),
            RHYTHM_FIRST..=14 => (peaks.rhythm[column - RHYTHM_FIRST], 10),
            ADPCM_COLUMN => (peaks.adpcm, 9),
            _ => (peaks.extension, 9),
        }
    }
}

fn decibel_height(share: f32) -> f32 {
    if share <= 0.0 {
        return 0.0;
    }
    let decibels = 20.0 * share.log10();
    ((decibels - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0)
}

fn decay(dt: f32, tau: f32) -> f32 {
    (-dt / tau).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: f32 = 1.0 / 30.0;
    const OPEN: [bool; CHANNEL_COUNT] = [false; CHANNEL_COUNT];

    fn silent() -> VoicePeaks {
        VoicePeaks::default()
    }

    #[test]
    fn the_column_count_matches_the_reference_groups() {
        assert_eq!(COLUMN_COUNT, 6 + 3 + 6 + 1 + 1);
        assert_eq!(RHYTHM_FIRST, 9);
        assert_eq!(EXTENSION_COLUMN, COLUMN_COUNT - 1);
    }

    #[test]
    fn every_column_reads_the_voice_it_names() {
        let mut meter = LevelAnalyzer::default();
        let mut peaks = silent();
        peaks.fm[2] = 0.5;
        peaks.ssg[1] = 0.6;
        peaks.rhythm[3] = 0.7;
        peaks.adpcm = 0.8;
        peaks.extension = 0.9;
        meter.update(&peaks, &OPEN, FRAME);

        let bars = meter.values();
        let expect = |share: f32| super::decibel_height(share);
        assert!((bars[2] - expect(0.5)).abs() < 0.001, "FM3");
        assert!((bars[7] - expect(0.6)).abs() < 0.001, "SSG2");
        assert!((bars[RHYTHM_FIRST + 3] - expect(0.7)).abs() < 0.001, "HH");
        assert!((bars[ADPCM_COLUMN] - expect(0.8)).abs() < 0.001, "ADP");
        assert!((bars[EXTENSION_COLUMN] - expect(0.9)).abs() < 0.001, "Ex");

        assert!(bars[2] < bars[7] && bars[7] < bars[ADPCM_COLUMN]);

        assert_eq!(bars[0], 0.0);
        assert_eq!(bars[RHYTHM_FIRST], 0.0);
    }

    #[test]
    fn a_bar_rises_on_the_frame_its_note_is_struck() {
        let mut meter = LevelAnalyzer::default();
        let mut peaks = silent();
        peaks.fm[0] = 1.0;
        meter.update(&peaks, &OPEN, FRAME);
        assert!(
            meter.values()[0] > 0.99,
            "attack must be immediate, got {}",
            meter.values()[0]
        );
    }

    #[test]
    fn a_released_bar_falls_gradually_rather_than_snapping_to_zero() {
        let mut meter = LevelAnalyzer::default();
        let mut peaks = silent();
        peaks.fm[0] = 1.0;
        meter.update(&peaks, &OPEN, FRAME);
        meter.update(&silent(), &OPEN, FRAME);
        let after_one_frame = meter.values()[0];
        assert!(
            after_one_frame > 0.5 && after_one_frame < 1.0,
            "one frame of release should leave a readable tail, got {after_one_frame}"
        );
        for _ in 0..60 {
            meter.update(&silent(), &OPEN, FRAME);
        }
        assert!(meter.values()[0] < 0.01, "two seconds must reach the floor");
    }

    #[test]
    fn muting_a_channel_takes_its_bar_to_zero_on_the_same_frame() {
        let mut meter = LevelAnalyzer::default();
        let mut peaks = silent();
        peaks.fm[3] = 1.0;
        meter.update(&peaks, &OPEN, FRAME);
        assert!(meter.values()[3] > 0.9);

        let mut muted = OPEN;
        muted[3] = true;
        meter.update(&peaks, &muted, FRAME);
        assert_eq!(meter.values()[3], 0.0, "a mute must not decay, it must cut");
        assert_eq!(meter.peaks()[3], 0.0, "the peak marker goes with it");
    }

    #[test]
    fn muting_the_rhythm_channel_clears_all_six_of_its_columns() {
        let mut meter = LevelAnalyzer::default();
        let peaks = VoicePeaks {
            rhythm: [1.0; 6],
            ..silent()
        };
        meter.update(&peaks, &OPEN, FRAME);
        assert!(meter.values()[RHYTHM_FIRST..=14].iter().all(|v| *v > 0.5));

        let mut muted = OPEN;
        muted[10] = true;
        meter.update(&peaks, &muted, FRAME);
        assert!(meter.values()[RHYTHM_FIRST..=14].iter().all(|v| *v == 0.0));
    }

    #[test]
    fn muting_the_pcm_row_clears_both_of_its_columns() {
        let mut meter = LevelAnalyzer::default();
        let peaks = VoicePeaks {
            adpcm: 1.0,
            extension: 1.0,
            ..silent()
        };
        meter.update(&peaks, &OPEN, FRAME);
        assert!(meter.values()[ADPCM_COLUMN] > 0.9);

        let mut muted = OPEN;
        muted[9] = true;
        meter.update(&peaks, &muted, FRAME);
        assert_eq!(meter.values()[ADPCM_COLUMN], 0.0);
        assert_eq!(meter.values()[EXTENSION_COLUMN], 0.0);
    }

    #[test]
    fn a_peak_marker_holds_before_it_slides() {
        let mut meter = LevelAnalyzer::default();
        let mut peaks = silent();
        peaks.fm[0] = 1.0;
        meter.update(&peaks, &OPEN, FRAME);

        for _ in 0..6 {
            meter.update(&silent(), &OPEN, FRAME);
        }
        assert!(meter.peaks()[0] > 0.99, "the peak must hold, not track");

        for _ in 0..60 {
            meter.update(&silent(), &OPEN, FRAME);
        }
        assert!(meter.peaks()[0] < 0.2, "the peak must eventually fall");
    }

    #[test]
    fn the_scale_is_decibels_so_ordinary_music_fills_the_bars() {
        assert!(decibel_height(0.0) == 0.0, "silence is the floor");
        assert!(decibel_height(1.0) > 0.99, "full scale is the top");
        let ordinary = decibel_height(0.09);
        assert!(
            (0.45..0.85).contains(&ordinary),
            "a 9 % share should read mid-scale, got {ordinary}"
        );
    }

    #[test]
    fn the_display_moves_when_the_music_does() {
        let mut meter = LevelAnalyzer::default();
        let mut loud = silent();
        loud.fm = [0.9, 0.2, 0.7, 0.1, 0.4, 0.8];
        meter.update(&loud, &OPEN, FRAME);
        let first = *meter.values();

        let mut quiet = silent();
        quiet.fm = [0.1, 0.8, 0.2, 0.6, 0.9, 0.3];
        for _ in 0..12 {
            meter.update(&quiet, &OPEN, FRAME);
        }
        let second = *meter.values();
        let moved = first
            .iter()
            .zip(&second)
            .filter(|(a, b)| (*a - *b).abs() > 0.1)
            .count();
        assert!(moved >= 4, "only {moved} bars moved between two mixes");
    }
}
