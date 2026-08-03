// SPDX-License-Identifier: AGPL-3.0-only

use crate::spectrum::FFT_SIZE;
use std::collections::VecDeque;

const SCOPE_HISTORY: usize = 4_096;

pub struct ScopeHistory {
    samples: VecDeque<f32>,

    window: Vec<f32>,
}

impl Default for ScopeHistory {
    fn default() -> Self {
        Self {
            samples: VecDeque::with_capacity(SCOPE_HISTORY),
            window: Vec::with_capacity(SCOPE_HISTORY),
        }
    }
}

impl ScopeHistory {
    pub fn push_samples(&mut self, samples: &[f32]) {
        for &sample in samples {
            if self.samples.len() == SCOPE_HISTORY {
                self.samples.pop_front();
            }
            self.samples.push_back(sample.clamp(-1.0, 1.0));
        }
    }

    pub fn clear(&mut self) {
        self.samples.clear();
        self.window.clear();
    }

    #[cfg(target_arch = "wasm32")]
    pub fn peak(&self) -> f32 {
        self.samples
            .iter()
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()))
    }

    pub fn analysis_window(&mut self) -> &[f32] {
        self.window.clear();
        let available = self.samples.len().min(FFT_SIZE);
        self.window
            .extend(self.samples.iter().skip(self.samples.len() - available));
        &self.window
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_history_is_bounded_and_clamped() {
        let mut history = ScopeHistory::default();
        let input = [2.0, -2.0, 0.25];
        for _ in 0..4_000 {
            history.push_samples(&input);
        }

        assert_eq!(history.samples.len(), super::SCOPE_HISTORY);
        assert_eq!(history.samples.back(), Some(&0.25));
        assert!(
            history.samples.iter().all(|sample| sample.abs() <= 1.0),
            "samples are clamped to full scale"
        );
        assert_eq!(
            history.analysis_window().len(),
            super::FFT_SIZE,
            "a full history yields a full transform window"
        );
        history.clear();
        assert!(history.samples.is_empty());
        assert!(history.analysis_window().is_empty());
    }
}
