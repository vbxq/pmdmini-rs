// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::{Arc, Mutex};

use pmd_core::{ChannelSnapshot, PlaybackStatus, VoicePeaks, CHANNEL_COUNT};

#[derive(Clone, Copy)]
pub struct TelemetryFrame {
    pub snapshots: [ChannelSnapshot; CHANNEL_COUNT],

    pub position_samples: u64,

    pub status: PlaybackStatus,

    pub peaks: VoicePeaks,

    pub sequence: u64,
}

impl TelemetryFrame {
    pub fn new(snapshots: [ChannelSnapshot; CHANNEL_COUNT]) -> Self {
        Self {
            snapshots,
            position_samples: 0,
            status: PlaybackStatus::default(),
            peaks: VoicePeaks::default(),
            sequence: 0,
        }
    }
}

#[derive(Clone)]
pub struct SharedTelemetry {
    slot: Arc<Mutex<TelemetryFrame>>,
}

impl SharedTelemetry {
    pub fn new(initial: TelemetryFrame) -> Self {
        Self {
            slot: Arc::new(Mutex::new(initial)),
        }
    }

    pub fn publish(&self, mut frame: TelemetryFrame) {
        let Ok(mut slot) = self.slot.try_lock() else {
            return;
        };
        frame.sequence = slot.sequence.wrapping_add(1);
        *slot = frame;
    }

    pub fn read_into(&self, destination: &mut TelemetryFrame) -> bool {
        let Ok(slot) = self.slot.try_lock() else {
            return false;
        };
        if slot.sequence == destination.sequence {
            return false;
        }
        *destination = *slot;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{SharedTelemetry, TelemetryFrame};
    use pmd_core::Player;

    fn frame() -> TelemetryFrame {
        TelemetryFrame::new(Player::new(44_100).channel_snapshots())
    }

    #[test]
    fn published_frames_are_visible_exactly_once() {
        let telemetry = SharedTelemetry::new(frame());
        let mut mine = frame();
        assert!(
            !telemetry.read_into(&mut mine),
            "the initial sequence is already current"
        );

        let mut published = frame();
        published.position_samples = 4_096;
        telemetry.publish(published);

        assert!(telemetry.read_into(&mut mine), "a new frame is visible");
        assert_eq!(mine.position_samples, 4_096);
        assert_eq!(mine.sequence, 1);
        assert!(
            !telemetry.read_into(&mut mine),
            "the same frame is not reported twice"
        );
    }

    #[test]
    fn neither_side_blocks_while_the_other_holds_the_slot() {
        let telemetry = SharedTelemetry::new(frame());
        let guard = telemetry.slot.lock().expect("uncontended lock");

        let mut mine = frame();
        assert!(!telemetry.read_into(&mut mine), "reads never wait");
        let mut published = frame();
        published.position_samples = 99;
        telemetry.publish(published);
        assert_eq!(guard.position_samples, 0, "the drop left the slot intact");
    }
}
