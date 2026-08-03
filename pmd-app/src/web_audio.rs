// SPDX-License-Identifier: AGPL-3.0-only

use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{AudioContext, AudioContextOptions, AudioScheduledSourceNode, GainNode};

use pmd_core::Player;

const CHUNK_FRAMES: usize = 2_048;
const AHEAD_SECONDS: f64 = 0.40;
const START_GUARD_SECONDS: f64 = 0.05;

struct ScheduledSource {
    end_time: f64,
    source: AudioScheduledSourceNode,
}

pub struct WebAudioOutput {
    context: AudioContext,
    gain: GainNode,
    sample_rate: u32,
    next_start: f64,
    running: bool,
    sources: Vec<ScheduledSource>,

    interleaved: Vec<f32>,
    left: Vec<f32>,
    right: Vec<f32>,
}

impl WebAudioOutput {
    pub fn new(sample_rate: u32) -> Result<Self, JsValue> {
        let options = AudioContextOptions::new();
        options.set_sample_rate(sample_rate as f32);
        let context = AudioContext::new_with_context_options(&options)?;
        let actual_rate = context.sample_rate().round() as u32;
        if actual_rate != sample_rate {
            let _ = context.close();
            return Err(JsValue::from_str(&format!(
                "requested {sample_rate} Hz, browser created {actual_rate} Hz"
            )));
        }

        let gain = context.create_gain()?;
        gain.gain().set_value(1.0);
        gain.connect_with_audio_node(&context.destination())?;

        Ok(Self {
            context,
            gain,
            sample_rate: actual_rate,
            next_start: 0.0,
            running: false,
            sources: Vec::new(),
            interleaved: Vec::with_capacity(CHUNK_FRAMES * 2),
            left: Vec::with_capacity(CHUNK_FRAMES),
            right: Vec::with_capacity(CHUNK_FRAMES),
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn resume(&mut self) -> Result<(), JsValue> {
        if self.running {
            return Ok(());
        }
        let promise = self.context.resume()?;
        observe_promise(promise, "AudioContext.resume");
        self.running = true;
        Ok(())
    }

    pub fn suspend(&mut self) {
        if !self.running {
            return;
        }
        if let Ok(promise) = self.context.suspend() {
            observe_promise(promise, "AudioContext.suspend");
        }
        self.running = false;
    }

    pub fn set_volume(&self, volume: f32) {
        self.gain.gain().set_value(volume.clamp(0.0, 1.0));
    }

    pub fn reset(&mut self) {
        let now = self.context.current_time();
        for scheduled in self.sources.drain(..) {
            let _ = scheduled.source.stop();

            let _ = scheduled.source.disconnect();
        }
        self.next_start = now;
        self.suspend();
    }

    pub fn schedule(
        &mut self,
        player: &mut Player,
        scope_output: &mut Vec<f32>,
    ) -> Result<(), JsValue> {
        scope_output.clear();
        let now = self.context.current_time();
        self.sources.retain(|scheduled| {
            if scheduled.end_time <= now {
                let _ = scheduled.source.disconnect();
                false
            } else {
                true
            }
        });
        if self.next_start < now + START_GUARD_SECONDS {
            self.next_start = now + START_GUARD_SECONDS;
        }

        while self.next_start < now + AHEAD_SECONDS {
            self.interleaved.resize(CHUNK_FRAMES * 2, 0.0);
            player.render(&mut self.interleaved);
            append_scope_points(&self.interleaved, scope_output);
            self.left.clear();
            self.right.clear();
            for frame in self.interleaved.chunks_exact(2) {
                self.left.push(frame[0]);
                self.right.push(frame[1]);
            }

            let buffer =
                self.context
                    .create_buffer(2, CHUNK_FRAMES as u32, self.sample_rate as f32)?;
            buffer.copy_to_channel(&self.left, 0)?;
            buffer.copy_to_channel(&self.right, 1)?;

            let start_time = self
                .next_start
                .max(self.context.current_time() + START_GUARD_SECONDS);
            let source = self.context.create_buffer_source()?;
            source.set_buffer(Some(&buffer));
            source.connect_with_audio_node(&self.gain)?;
            let source: AudioScheduledSourceNode = source.unchecked_into();
            source.start_with_when(start_time)?;

            let end_time = start_time + CHUNK_FRAMES as f64 / self.sample_rate as f64;
            self.sources.push(ScheduledSource { end_time, source });
            self.next_start = end_time;
        }
        Ok(())
    }
}

fn append_scope_points(interleaved: &[f32], output: &mut Vec<f32>) {
    const DECIMATION: usize = 2;
    let scale = 1.0 / (2 * DECIMATION) as f32;
    let points = interleaved.len() / (2 * DECIMATION);
    for point in 0..points {
        let mut total = 0.0;
        for step in 0..DECIMATION {
            let offset = (point * DECIMATION + step) * 2;
            total += interleaved[offset] + interleaved[offset + 1];
        }
        output.push(total * scale);
    }
}

fn observe_promise(promise: js_sys::Promise, operation: &'static str) {
    spawn_local(async move {
        if let Err(error) = JsFuture::from(promise).await {
            log::error!("{operation} failed: {error:?}");
        }
    });
}
