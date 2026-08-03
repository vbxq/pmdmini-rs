// SPDX-License-Identifier: AGPL-3.0-only

pub struct FrameStats {
    #[cfg(not(target_arch = "wasm32"))]
    frame_start: Option<std::time::Instant>,
    window_start: f64,
    frames: u32,
    accumulated_ms: f32,
    window_max_ms: f32,

    pub fps: f32,

    pub mean_ms: f32,

    pub max_ms: f32,

    pub total_frames: u64,
    log_enabled: bool,
}

impl Default for FrameStats {
    fn default() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            frame_start: None,
            window_start: f64::NEG_INFINITY,
            frames: 0,
            accumulated_ms: 0.0,
            window_max_ms: 0.0,
            fps: 0.0,
            mean_ms: 0.0,
            max_ms: 0.0,
            total_frames: 0,
            log_enabled: log_enabled(),
        }
    }
}

impl FrameStats {
    pub fn begin_frame(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.frame_start = Some(std::time::Instant::now());
        }
    }

    pub fn end_frame(&mut self, time: f64) -> Option<String> {
        let elapsed_ms = self.frame_ms();
        self.frames += 1;
        self.total_frames += 1;
        self.accumulated_ms += elapsed_ms;
        self.window_max_ms = self.window_max_ms.max(elapsed_ms);

        if self.window_start == f64::NEG_INFINITY {
            self.window_start = time;
            return None;
        }
        let window = time - self.window_start;
        if window < 1.0 {
            return None;
        }

        self.fps = self.frames as f32 / window as f32;
        self.mean_ms = self.accumulated_ms / self.frames as f32;
        self.max_ms = self.window_max_ms;
        let line = self.log_enabled.then(|| {
            format!(
                "PERF fps={:.1} ui_mean_ms={:.2} ui_max_ms={:.2} frames={}",
                self.fps, self.mean_ms, self.max_ms, self.total_frames
            )
        });
        self.window_start = time;
        self.frames = 0;
        self.accumulated_ms = 0.0;
        self.window_max_ms = 0.0;
        line
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn frame_ms(&mut self) -> f32 {
        self.frame_start
            .take()
            .map(|start| start.elapsed().as_secs_f32() * 1000.0)
            .unwrap_or(0.0)
    }

    #[cfg(target_arch = "wasm32")]
    fn frame_ms(&mut self) -> f32 {
        0.0
    }
}

pub struct FrameLimiter {
    #[cfg(not(target_arch = "wasm32"))]
    interval: std::time::Duration,
    #[cfg(not(target_arch = "wasm32"))]
    next_frame: Option<std::time::Instant>,
}

impl FrameLimiter {
    pub fn new(frames_per_second: f32) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let hertz = frames_per_second.clamp(1.0, 240.0);
            Self {
                interval: std::time::Duration::from_secs_f32(1.0 / hertz),
                next_frame: None,
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = frames_per_second;
            Self {}
        }
    }

    pub fn set_rate(&mut self, frames_per_second: f32) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let hertz = frames_per_second.clamp(1.0, 240.0);
            self.interval = std::time::Duration::from_secs_f32(1.0 / hertz);
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = frames_per_second;
        }
    }

    pub fn pace(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let now = std::time::Instant::now();
            if let Some(next_frame) = self.next_frame {
                if now < next_frame {
                    std::thread::sleep(next_frame - now);
                }
            }

            self.next_frame = Some(std::time::Instant::now() + self.interval);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn log_enabled() -> bool {
    std::env::var_os("PMDMINI_PERF_LOG").is_some()
}

#[cfg(target_arch = "wasm32")]
fn log_enabled() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::{FrameLimiter, FrameStats};

    #[test]
    fn the_limiter_paces_consecutive_frames() {
        let mut limiter = FrameLimiter::new(200.0);
        limiter.set_rate(50.0);
        limiter.pace();
        let start = std::time::Instant::now();
        limiter.pace();
        limiter.pace();
        let elapsed = start.elapsed();
        assert!(
            elapsed >= std::time::Duration::from_millis(36),
            "two paced frames must take at least two intervals, took {elapsed:?}"
        );
    }

    #[test]
    fn statistics_close_once_per_second_window() {
        let mut stats = FrameStats::default();
        assert!(stats.end_frame(0.0).is_none(), "first frame opens a window");
        assert_eq!(stats.fps, 0.0, "no window has closed yet");
        for step in 1..=30 {
            stats.end_frame(f64::from(step) / 30.0);
        }
        assert_eq!(stats.total_frames, 31);
        assert!(
            (stats.fps - 31.0).abs() < 0.01,
            "one closed second publishes its own frame count, got {}",
            stats.fps
        );
        assert_eq!(stats.frames, 0, "the window restarts empty");
    }
}
