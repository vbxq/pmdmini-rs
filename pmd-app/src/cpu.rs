// SPDX-License-Identifier: AGPL-3.0-only

const WINDOW_SECONDS: f64 = 0.5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CpuReading {
    pub synth_permille: u32,

    pub draw_tenths_ms: u32,

    pub power_ksamples: u32,
}

impl CpuReading {
    pub fn text(self) -> String {
        format!(
            "{:03}-{:03}-{:04}",
            self.synth_permille.min(999),
            self.draw_tenths_ms.min(999),
            self.power_ksamples.min(9_999)
        )
    }
}

pub const MEASURING: &str = "888-888-8888";

#[derive(Debug, Default)]
pub struct CpuMeter {
    window_start: Option<f64>,
    synth_cpu_seconds: f64,
    synth_frames: u64,
    draw_seconds: f64,
    draw_frames: u32,

    reading: Option<CpuReading>,

    power_ksamples: u32,
}

impl CpuMeter {
    pub fn add_synthesis(&mut self, cpu_seconds: f64, frames: u64) {
        if cpu_seconds.is_finite() && cpu_seconds > 0.0 {
            self.synth_cpu_seconds += cpu_seconds;
            self.synth_frames += frames;
        }
    }

    pub fn add_draw(&mut self, seconds: f64) {
        if seconds.is_finite() && seconds >= 0.0 {
            self.draw_seconds += seconds;
            self.draw_frames += 1;
        }
    }

    pub fn tick(&mut self, now: f64, sample_rate: u32) -> Option<CpuReading> {
        let start = *self.window_start.get_or_insert(now);
        if now - start < WINDOW_SECONDS {
            return self.reading;
        }

        let audio_seconds = if sample_rate == 0 {
            0.0
        } else {
            self.synth_frames as f64 / f64::from(sample_rate)
        };
        let synth_permille = if audio_seconds > 0.0 {
            (self.synth_cpu_seconds / audio_seconds * 1_000.0).round() as u32
        } else {
            0
        };
        let draw_tenths_ms = if self.draw_frames > 0 {
            (self.draw_seconds / f64::from(self.draw_frames) * 10_000.0).round() as u32
        } else {
            0
        };
        if self.synth_cpu_seconds > 0.0 && self.synth_frames > 0 {
            let per_second = self.synth_frames as f64 / self.synth_cpu_seconds;
            self.power_ksamples = (per_second / 1_000.0).round() as u32;
        }

        self.reading = Some(CpuReading {
            synth_permille,
            draw_tenths_ms,
            power_ksamples: self.power_ksamples,
        });
        self.window_start = Some(now);
        self.synth_cpu_seconds = 0.0;
        self.synth_frames = 0;
        self.draw_seconds = 0.0;
        self.draw_frames = 0;
        self.reading
    }

    pub fn text(&self) -> String {
        self.reading
            .map_or_else(|| String::from(MEASURING), CpuReading::text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_placeholder_shows_only_until_the_first_window_closes() {
        let mut meter = CpuMeter::default();
        assert_eq!(meter.text(), MEASURING);
        meter.add_draw(0.002);
        assert!(meter.tick(0.0, 44_100).is_none(), "the window just opened");
        assert_eq!(meter.text(), MEASURING);

        assert!(meter.tick(1.0, 44_100).is_some());
        assert_ne!(meter.text(), MEASURING, "a closed window must publish");
    }

    #[test]
    fn the_reading_fits_the_reference_template() {
        let text = CpuReading {
            synth_permille: 21,
            draw_tenths_ms: 8,
            power_ksamples: 4_210,
        }
        .text();
        assert_eq!(text, "021-008-4210");
        assert_eq!(text.len(), MEASURING.len());
    }

    #[test]
    fn groups_saturate_instead_of_overflowing_their_cells() {
        let text = CpuReading {
            synth_permille: 12_000,
            draw_tenths_ms: 5_000,
            power_ksamples: 1_000_000,
        }
        .text();
        assert_eq!(text, "999-999-9999");
    }

    #[test]
    fn synthesis_load_is_cpu_time_against_the_audio_it_produced() {
        let mut meter = CpuMeter::default();
        meter.tick(0.0, 44_100);

        meter.add_synthesis(0.02, 44_100);
        let reading = meter.tick(1.0, 44_100).expect("window closed");
        assert_eq!(reading.synth_permille, 20);
        assert_eq!(reading.power_ksamples, 2_205);
    }

    #[test]
    fn measured_power_survives_a_window_with_no_playback() {
        let mut meter = CpuMeter::default();
        meter.tick(0.0, 44_100);
        meter.add_synthesis(0.02, 44_100);
        let playing = meter.tick(1.0, 44_100).expect("window closed");
        meter.add_draw(0.001);
        let idle = meter.tick(2.0, 44_100).expect("window closed");
        assert_eq!(idle.synth_permille, 0, "nothing was synthesised");
        assert_eq!(
            idle.power_ksamples, playing.power_ksamples,
            "the machine's power is not unknown again just because it went idle"
        );
    }

    #[test]
    fn a_zero_sample_rate_reports_zero_load_rather_than_dividing_by_it() {
        let mut meter = CpuMeter::default();
        meter.tick(0.0, 0);
        meter.add_synthesis(0.5, 1_000);
        let reading = meter.tick(1.0, 0).expect("window closed");
        assert_eq!(reading.synth_permille, 0);
    }
}
