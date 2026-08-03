// SPDX-License-Identifier: AGPL-3.0-only

use core::f32::consts::PI;

pub const FFT_SIZE: usize = 1_024;

pub const BAR_COUNT: usize = crate::layout::SPECTRUM_COLUMNS;

pub const FLOOR_DB: f32 = -62.0;

const ATTACK_SECONDS: f32 = 0.012;

const RELEASE_SECONDS: f32 = 0.35;

const PEAK_HOLD_SECONDS: f32 = 0.5;

const PEAK_FALL_PER_SECOND: f32 = 0.55;

struct Transform {
    cos_table: [f32; FFT_SIZE / 2],
    sin_table: [f32; FFT_SIZE / 2],
    real: [f32; FFT_SIZE],
    imag: [f32; FFT_SIZE],
}

impl Transform {
    fn new() -> Self {
        let mut cos_table = [0.0; FFT_SIZE / 2];
        let mut sin_table = [0.0; FFT_SIZE / 2];
        for (index, (cosine, sine)) in cos_table.iter_mut().zip(&mut sin_table).enumerate() {
            let angle = -2.0 * PI * index as f32 / FFT_SIZE as f32;
            *cosine = angle.cos();
            *sine = angle.sin();
        }
        Self {
            cos_table,
            sin_table,
            real: [0.0; FFT_SIZE],
            imag: [0.0; FFT_SIZE],
        }
    }

    fn run(&mut self) {
        let n = FFT_SIZE;
        let mut target = 0usize;
        for source in 1..n {
            let mut bit = n >> 1;
            while target & bit != 0 {
                target ^= bit;
                bit >>= 1;
            }
            target |= bit;
            if source < target {
                self.real.swap(source, target);
                self.imag.swap(source, target);
            }
        }

        let mut span = 2usize;
        while span <= n {
            let half = span / 2;
            let stride = n / span;
            let mut base = 0usize;
            while base < n {
                for offset in 0..half {
                    let twiddle = offset * stride;
                    let cosine = self.cos_table[twiddle];
                    let sine = self.sin_table[twiddle];
                    let high = base + offset + half;
                    let low = base + offset;
                    let product_real = self.real[high] * cosine - self.imag[high] * sine;
                    let product_imag = self.real[high] * sine + self.imag[high] * cosine;
                    self.real[high] = self.real[low] - product_real;
                    self.imag[high] = self.imag[low] - product_imag;
                    self.real[low] += product_real;
                    self.imag[low] += product_imag;
                }
                base += span;
            }
            span <<= 1;
        }
    }
}

pub struct SpectrumAnalyzer {
    transform: Transform,
    window: [f32; FFT_SIZE],

    bins: [(usize, usize); BAR_COUNT],

    pub bars: [f32; BAR_COUNT],

    pub peaks: [f32; BAR_COUNT],

    holds: [f32; BAR_COUNT],

    mapped_rate: f32,
}

impl Default for SpectrumAnalyzer {
    fn default() -> Self {
        let mut window = [0.0; FFT_SIZE];
        for (index, weight) in window.iter_mut().enumerate() {
            *weight = 0.5 - 0.5 * (2.0 * PI * index as f32 / FFT_SIZE as f32).cos();
        }
        Self {
            transform: Transform::new(),
            window,
            bins: [(0, 0); BAR_COUNT],
            bars: [0.0; BAR_COUNT],
            peaks: [0.0; BAR_COUNT],
            holds: [0.0; BAR_COUNT],
            mapped_rate: 0.0,
        }
    }
}

impl SpectrumAnalyzer {
    pub const LOW_HZ: f32 = 136.0;

    pub const HIGH_HZ: f32 = 7_578.0;

    pub fn fraction_of_hz(hertz: f32, sample_rate: f32) -> f32 {
        let high = Self::high_hz(sample_rate);
        let ratio = (high / Self::LOW_HZ).ln();
        ((hertz.max(Self::LOW_HZ) / Self::LOW_HZ).ln() / ratio).clamp(0.0, 1.0)
    }

    fn high_hz(sample_rate: f32) -> f32 {
        let nyquist = sample_rate * 0.5;
        Self::HIGH_HZ.min(nyquist * 0.98).max(Self::LOW_HZ * 2.0)
    }

    fn remap(&mut self, sample_rate: f32) {
        let high = Self::high_hz(sample_rate);
        let bin_width = sample_rate / FFT_SIZE as f32;
        let ratio = (high / Self::LOW_HZ).ln();
        for (column, slot) in self.bins.iter_mut().enumerate() {
            let start_hz = Self::LOW_HZ * (ratio * column as f32 / BAR_COUNT as f32).exp();
            let end_hz = Self::LOW_HZ * (ratio * (column + 1) as f32 / BAR_COUNT as f32).exp();
            let first = ((start_hz / bin_width).floor() as usize).clamp(1, FFT_SIZE / 2 - 1);
            let last = ((end_hz / bin_width).ceil() as usize).clamp(first, FFT_SIZE / 2 - 1);
            *slot = (first, last);
        }
        self.mapped_rate = sample_rate;
    }

    pub fn update(&mut self, samples: &[f32], sample_rate: f32, dt: f32) {
        if sample_rate > 0.0 && (sample_rate - self.mapped_rate).abs() > 0.5 {
            self.remap(sample_rate);
        }
        let dt = dt.clamp(0.0, 0.25);
        let attack = 1.0 - (-dt / ATTACK_SECONDS).exp();
        let release = (-dt / RELEASE_SECONDS).exp();

        if samples.len() >= FFT_SIZE {
            let tail = &samples[samples.len() - FFT_SIZE..];
            for (index, sample) in tail.iter().enumerate() {
                self.transform.real[index] = sample * self.window[index];
                self.transform.imag[index] = 0.0;
            }
            self.transform.run();

            for (column, &(first, last)) in self.bins.iter().enumerate() {
                let mut power = 0.0_f32;
                for bin in first..=last {
                    let real = self.transform.real[bin];
                    let imag = self.transform.imag[bin];
                    power += real * real + imag * imag;
                }
                let peak = power.sqrt();

                let magnitude = peak * 4.0 / FFT_SIZE as f32;
                let decibels = 20.0 * (magnitude + 1.0e-7).log10();
                let level = ((decibels - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0);
                let bar = &mut self.bars[column];
                *bar = if level > *bar {
                    *bar + (level - *bar) * attack
                } else {
                    level + (*bar - level) * release
                };
            }
        } else {
            for bar in &mut self.bars {
                *bar *= release;
            }
        }

        for ((peak, hold), bar) in self.peaks.iter_mut().zip(&mut self.holds).zip(&self.bars) {
            if *bar >= *peak {
                *peak = *bar;
                *hold = PEAK_HOLD_SECONDS;
            } else if *hold > 0.0 {
                *hold -= dt;
            } else {
                *peak = (*peak - PEAK_FALL_PER_SECOND * dt).max(*bar);
            }
        }
    }

    pub fn clear(&mut self) {
        self.bars = [0.0; BAR_COUNT];
        self.peaks = [0.0; BAR_COUNT];
        self.holds = [0.0; BAR_COUNT];
    }
}

#[cfg(test)]
mod tests {
    use super::{SpectrumAnalyzer, Transform, BAR_COUNT, FFT_SIZE};
    use crate::layout as L;
    use core::f32::consts::PI;

    const FRAME: f32 = 1.0 / 30.0;

    fn tone(hz: f32, rate: f32) -> Vec<f32> {
        (0..FFT_SIZE)
            .map(|index| (2.0 * PI * hz * index as f32 / rate).sin())
            .collect()
    }

    #[test]
    fn the_transform_finds_a_pure_tone_in_its_own_bin() {
        let mut transform = Transform::new();
        let target_bin = 37;
        for index in 0..FFT_SIZE {
            transform.real[index] =
                (2.0 * PI * target_bin as f32 * index as f32 / FFT_SIZE as f32).sin();
            transform.imag[index] = 0.0;
        }
        transform.run();

        let mut best = 0usize;
        let mut best_magnitude = 0.0_f32;
        for bin in 1..FFT_SIZE / 2 {
            let magnitude = (transform.real[bin] * transform.real[bin]
                + transform.imag[bin] * transform.imag[bin])
                .sqrt();
            if magnitude > best_magnitude {
                best_magnitude = magnitude;
                best = bin;
            }
        }
        assert_eq!(best, target_bin);
        assert!(
            best_magnitude > FFT_SIZE as f32 * 0.4,
            "a full-scale sine concentrates its energy, got {best_magnitude}"
        );
    }

    #[test]
    fn there_is_one_column_per_column_of_the_drawn_matrix() {
        assert_eq!(BAR_COUNT, L::SPECTRUM_COLUMNS);

        let last = L::SPECTRUM_X + (BAR_COUNT as i32 - 1) * L::SPECTRUM_COL_STEP;
        assert!(last + L::SPECTRUM_DOT_W <= L::SPECTRUM_X + L::SPECTRUM_W);
    }

    #[test]
    fn the_ruler_labels_land_where_the_reference_prints_them() {
        let rate = 22_050.0;
        for (hertz, expected) in [
            (250.0, 396),
            (500.0, 444),
            (1_000.0, 492),
            (2_000.0, 541),
            (4_000.0, 589),
        ] {
            let fraction = SpectrumAnalyzer::fraction_of_hz(hertz, rate);
            let x = L::SPECTRUM_X + (L::SPECTRUM_W as f32 * fraction) as i32;
            assert!(
                (x - expected).abs() <= 2,
                "{hertz} Hz maps to x={x}, measured at {expected}"
            );
        }
    }

    #[test]
    fn a_tone_lights_one_region_and_silence_decays() {
        let mut analyzer = SpectrumAnalyzer::default();
        let rate = 22_050.0;
        let tone_hz = 1_000.0;
        let samples = tone(tone_hz, rate);

        for _ in 0..8 {
            analyzer.update(&samples, rate, FRAME);
        }
        let lit = SpectrumAnalyzer::fraction_of_hz(tone_hz, rate);
        let column = ((lit * BAR_COUNT as f32) as usize).min(BAR_COUNT - 1);
        let loudest = analyzer
            .bars
            .iter()
            .enumerate()
            .max_by(|left, right| left.1.total_cmp(right.1))
            .map(|(index, _)| index)
            .expect("the analyzer always has columns");
        assert!(
            loudest.abs_diff(column) <= 1,
            "expected the tone near column {column}, found {loudest}"
        );

        let silence = [0.0_f32; FFT_SIZE];
        for _ in 0..200 {
            analyzer.update(&silence, rate, FRAME);
        }
        assert!(
            analyzer.bars.iter().all(|bar| *bar < 0.02),
            "silence must fall back to the noise floor"
        );
    }

    #[test]
    fn the_envelope_moves_when_the_input_does() {
        let rate = 22_050.0;
        let mut analyzer = SpectrumAnalyzer::default();
        for _ in 0..30 {
            analyzer.update(&tone(400.0, rate), rate, FRAME);
        }
        let first = analyzer.bars;
        for _ in 0..30 {
            analyzer.update(&tone(3_000.0, rate), rate, FRAME);
        }
        let second = analyzer.bars;
        let moved = first
            .iter()
            .zip(&second)
            .filter(|(a, b)| (*a - *b).abs() > 0.05)
            .count();
        assert!(
            moved >= 4,
            "only {moved} columns changed between two different inputs"
        );
    }

    #[test]
    fn attack_is_fast_and_release_is_slow() {
        let rate = 22_050.0;
        let mut analyzer = SpectrumAnalyzer::default();
        let samples = tone(1_000.0, rate);
        analyzer.update(&samples, rate, FRAME);
        let after_one_frame = analyzer.bars.iter().copied().fold(0.0_f32, f32::max);
        assert!(
            after_one_frame > 0.8,
            "one frame should already be near full height, got {after_one_frame}"
        );

        let silence = [0.0_f32; FFT_SIZE];
        analyzer.update(&silence, rate, FRAME);
        let after_release = analyzer.bars.iter().copied().fold(0.0_f32, f32::max);
        assert!(
            after_release > after_one_frame * 0.7,
            "one frame of silence must not empty the display, got {after_release}"
        );
    }

    #[test]
    fn a_peak_marker_holds_above_its_column_then_slides() {
        let rate = 22_050.0;
        let mut analyzer = SpectrumAnalyzer::default();
        let samples = tone(1_000.0, rate);
        for _ in 0..4 {
            analyzer.update(&samples, rate, FRAME);
        }
        let struck = analyzer.peaks.iter().copied().fold(0.0_f32, f32::max);
        let silence = [0.0_f32; FFT_SIZE];
        for _ in 0..12 {
            analyzer.update(&silence, rate, FRAME);
        }
        let held = analyzer.peaks.iter().copied().fold(0.0_f32, f32::max);
        assert!(
            (held - struck).abs() < 0.01,
            "the marker must hold for half a second, moved from {struck} to {held}"
        );
        for _ in 0..90 {
            analyzer.update(&silence, rate, FRAME);
        }
        let fallen = analyzer.peaks.iter().copied().fold(0.0_f32, f32::max);
        assert!(fallen < 0.05, "the marker must come down, left at {fallen}");
    }

    #[test]
    fn a_short_history_only_decays_the_display() {
        let mut analyzer = SpectrumAnalyzer {
            bars: [1.0; BAR_COUNT],
            ..SpectrumAnalyzer::default()
        };
        analyzer.update(&[0.0; 8], 22_050.0, 0.0);
        assert!(
            analyzer.bars.iter().all(|bar| (*bar - 1.0).abs() < 0.001),
            "no elapsed time means no decay"
        );

        for _ in 0..10 {
            analyzer.update(&[0.0; 8], 22_050.0, super::RELEASE_SECONDS / 10.0);
        }
        assert!(
            analyzer.bars.iter().all(|bar| *bar < 0.4),
            "one time constant should leave 1/e behind"
        );
    }
}
