// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use crossbeam_queue::ArrayQueue;
use pmd_core::Player;

use crate::telemetry::{SharedTelemetry, TelemetryFrame};

pub const AUDIO_BLOCK_FRAMES: usize = 1024;
const AUDIO_BLOCK_SAMPLES: usize = AUDIO_BLOCK_FRAMES * 2;
const AUDIO_QUEUE_BLOCKS: usize = 32;
const AUDIO_TARGET_BLOCKS: usize = 24;

const SEEK_CHUNK_FRAMES: usize = 4_096;
pub const SCOPE_DECIMATION: usize = crate::ANALYSER_DECIMATION;
pub const SCOPE_POINTS: usize = AUDIO_BLOCK_FRAMES / SCOPE_DECIMATION;

#[derive(Clone, Copy)]
struct AudioBlock {
    samples: [f32; AUDIO_BLOCK_SAMPLES],
}

impl Default for AudioBlock {
    fn default() -> Self {
        Self {
            samples: [0.0; AUDIO_BLOCK_SAMPLES],
        }
    }
}

#[derive(Clone, Copy)]
struct ScopeBlock {
    samples: [f32; SCOPE_POINTS],
}

impl ScopeBlock {
    fn from_audio(block: &AudioBlock) -> Self {
        let mut scope = Self {
            samples: [0.0; SCOPE_POINTS],
        };

        let scale = 1.0 / (2 * SCOPE_DECIMATION) as f32;
        for (point, sample) in scope.samples.iter_mut().enumerate() {
            let mut total = 0.0;
            for step in 0..SCOPE_DECIMATION {
                let offset = (point * SCOPE_DECIMATION + step) * 2;
                total += block.samples[offset] + block.samples[offset + 1];
            }
            *sample = total * scale;
        }
        scope
    }
}

#[derive(Clone)]
struct CallbackTransport {
    queue: Arc<ArrayQueue<AudioBlock>>,
    playing: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    volume: Arc<AtomicU32>,
    underruns: Arc<AtomicU64>,
    peak_left: Arc<AtomicU32>,
    peak_right: Arc<AtomicU32>,
    callback_calls: Arc<AtomicU64>,
}

struct CallbackState {
    block: AudioBlock,
    cursor: usize,
    generation: u64,
}

impl CallbackState {
    fn new() -> Self {
        Self {
            block: AudioBlock::default(),
            cursor: AUDIO_BLOCK_SAMPLES,
            generation: 0,
        }
    }

    fn next_stereo(
        &mut self,
        queue: &ArrayQueue<AudioBlock>,
        underruns: &AtomicU64,
        generation: &AtomicU64,
    ) -> [f32; 2] {
        let current_generation = generation.load(Ordering::Acquire);
        if current_generation != self.generation {
            self.block = AudioBlock::default();
            self.cursor = AUDIO_BLOCK_SAMPLES;
            self.generation = current_generation;
        }
        if self.cursor >= AUDIO_BLOCK_SAMPLES {
            if let Some(block) = queue.pop() {
                self.block = block;
            } else {
                self.block = AudioBlock::default();
                underruns.fetch_add(1, Ordering::Relaxed);
            }
            self.cursor = 0;
        }
        let left = self.block.samples[self.cursor];
        let right = self.block.samples[self.cursor + 1];
        self.cursor += 2;
        [left, right]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioOutputError(String);

impl std::fmt::Display for AudioOutputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for AudioOutputError {}

pub struct AudioOutput {
    queue: Arc<ArrayQueue<AudioBlock>>,

    #[allow(dead_code)]
    stream: cpal::Stream,
    playing: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    volume: Arc<AtomicU32>,
    position_samples: Arc<AtomicU64>,
    worker_stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
    sample_rate: u32,
    channels: usize,
    underruns: Arc<AtomicU64>,
    peak_left: Arc<AtomicU32>,
    peak_right: Arc<AtomicU32>,
    scope_queue: Arc<ArrayQueue<ScopeBlock>>,
    callback_calls: Arc<AtomicU64>,
    seek_target: Arc<AtomicU64>,
    seek_pending: Arc<AtomicBool>,
    render_nanos: Arc<AtomicU64>,
    render_frames: Arc<AtomicU64>,
}

impl AudioOutput {
    pub fn open() -> Result<Self, AudioOutputError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| AudioOutputError(String::from("no default output device")))?;
        let supported = device
            .default_output_config()
            .map_err(|error| AudioOutputError(format!("default output config: {error}")))?;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let channels = usize::from(config.channels);
        if channels == 0 {
            return Err(AudioOutputError(String::from(
                "output device reported zero channels",
            )));
        }

        let queue = Arc::new(ArrayQueue::new(AUDIO_QUEUE_BLOCKS));
        let playing = Arc::new(AtomicBool::new(false));
        let generation = Arc::new(AtomicU64::new(0));
        let volume = Arc::new(AtomicU32::new(1.0f32.to_bits()));
        let position_samples = Arc::new(AtomicU64::new(0));
        let worker_stop = Arc::new(AtomicBool::new(false));
        let underruns = Arc::new(AtomicU64::new(0));
        let peak_left = Arc::new(AtomicU32::new(0));
        let peak_right = Arc::new(AtomicU32::new(0));
        let scope_queue = Arc::new(ArrayQueue::new(AUDIO_QUEUE_BLOCKS));
        let callback_calls = Arc::new(AtomicU64::new(0));
        let seek_target = Arc::new(AtomicU64::new(0));
        let seek_pending = Arc::new(AtomicBool::new(false));
        let render_nanos = Arc::new(AtomicU64::new(0));
        let render_frames = Arc::new(AtomicU64::new(0));
        let transport = CallbackTransport {
            queue: Arc::clone(&queue),
            playing: Arc::clone(&playing),
            generation: Arc::clone(&generation),
            volume: Arc::clone(&volume),
            underruns: Arc::clone(&underruns),
            peak_left: Arc::clone(&peak_left),
            peak_right: Arc::clone(&peak_right),
            callback_calls: Arc::clone(&callback_calls),
        };
        let stream = build_stream(&device, &config, sample_format, channels, transport)
            .map_err(|error| AudioOutputError(format!("build output stream: {error}")))?;
        stream
            .play()
            .map_err(|error| AudioOutputError(format!("start output stream: {error}")))?;

        Ok(Self {
            queue,
            stream,
            playing,
            generation,
            volume,
            position_samples,
            worker_stop,
            worker: None,
            sample_rate: config.sample_rate,
            channels,
            underruns,
            peak_left,
            peak_right,
            scope_queue,
            callback_calls,
            seek_target,
            seek_pending,
            render_nanos,
            render_frames,
        })
    }

    pub fn request_seek(&self, target_samples: u64) {
        self.seek_target.store(target_samples, Ordering::Release);
        self.seek_pending.store(true, Ordering::Release);
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn start_worker(
        &mut self,
        player: Arc<Mutex<Player>>,
        telemetry: SharedTelemetry,
    ) -> Result<(), AudioOutputError> {
        if self.worker.is_some() {
            return Ok(());
        }
        let queue = Arc::clone(&self.queue);
        let playing = Arc::clone(&self.playing);
        let generation = Arc::clone(&self.generation);
        let position_samples = Arc::clone(&self.position_samples);
        let scope_queue = Arc::clone(&self.scope_queue);
        let stop = Arc::clone(&self.worker_stop);
        let seek_target = Arc::clone(&self.seek_target);
        let seek_pending = Arc::clone(&self.seek_pending);
        let render_nanos = Arc::clone(&self.render_nanos);
        let render_frames = Arc::clone(&self.render_frames);
        self.worker = Some(
            thread::Builder::new()
                .name(String::from("pmdmini-audio-producer"))
                .spawn(move || {
                    let mut seek_scratch: Vec<f32> = Vec::new();
                    while !stop.load(Ordering::Acquire) {
                        if seek_pending.swap(false, Ordering::AcqRel) {
                            let target = seek_target.load(Ordering::Acquire);
                            generation.fetch_add(1, Ordering::AcqRel);
                            while queue.pop().is_some() {}
                            while scope_queue.pop().is_some() {}
                            let mut player = player
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            if player.position_samples() > target {
                                player.reset();
                            }
                            seek_scratch.resize(SEEK_CHUNK_FRAMES * 2, 0.0);
                            while player.position_samples() < target {
                                let remaining = target - player.position_samples();
                                let frames = remaining.min(SEEK_CHUNK_FRAMES as u64) as usize;
                                player.render(&mut seek_scratch[..frames * 2]);
                                if stop.load(Ordering::Acquire) {
                                    break;
                                }
                            }
                            position_samples.store(player.position_samples(), Ordering::Release);
                            telemetry.publish(TelemetryFrame {
                                snapshots: player.channel_snapshots(),
                                position_samples: player.position_samples(),
                                status: player.playback_status(),
                                peaks: player.voice_peaks(),
                                sequence: 0,
                            });
                            continue;
                        }
                        if !playing.load(Ordering::Acquire) {
                            thread::sleep(Duration::from_millis(10));
                            continue;
                        }
                        if queue.len() >= AUDIO_TARGET_BLOCKS {
                            thread::sleep(Duration::from_millis(10));
                            continue;
                        }

                        let mut block = AudioBlock::default();
                        let render_generation = generation.load(Ordering::Acquire);
                        let (decoded_position, snapshots, status, peaks) = {
                            let mut player = player
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());

                            let started = std::time::Instant::now();
                            player.render(&mut block.samples);
                            render_nanos.fetch_add(
                                started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
                                Ordering::Relaxed,
                            );
                            render_frames.fetch_add(AUDIO_BLOCK_FRAMES as u64, Ordering::Relaxed);
                            (
                                player.position_samples(),
                                player.channel_snapshots(),
                                player.playback_status(),
                                player.voice_peaks(),
                            )
                        };

                        if playing.load(Ordering::Acquire)
                            && generation.load(Ordering::Acquire) == render_generation
                        {
                            position_samples.store(decoded_position, Ordering::Release);
                            telemetry.publish(TelemetryFrame {
                                snapshots,
                                position_samples: decoded_position,
                                status,
                                peaks,
                                sequence: 0,
                            });
                            let scope = ScopeBlock::from_audio(&block);
                            if scope_queue.push(scope).is_err() {
                                let _ = scope_queue.pop();
                                let _ = scope_queue.push(scope);
                            }
                            let _ = queue.push(block);
                        }
                    }
                })
                .map_err(|error| AudioOutputError(format!("start audio producer: {error}")))?,
        );
        Ok(())
    }

    pub fn set_playing(&self, playing: bool) {
        self.playing.store(playing, Ordering::Release);
    }

    pub fn set_volume(&self, volume: f32) {
        let volume = if volume.is_finite() {
            volume.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.volume.store(volume.to_bits(), Ordering::Release);
    }

    pub fn clear(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.position_samples.store(0, Ordering::Release);
        while self.queue.pop().is_some() {}
        while self.scope_queue.pop().is_some() {}
    }

    pub fn position_samples(&self) -> u64 {
        self.position_samples.load(Ordering::Acquire)
    }

    pub fn take_peak(&self) -> [f32; 2] {
        [
            f32::from_bits(self.peak_left.swap(0, Ordering::Relaxed)),
            f32::from_bits(self.peak_right.swap(0, Ordering::Relaxed)),
        ]
    }

    pub fn take_scope(&self, output: &mut Vec<f32>) {
        output.clear();
        while let Some(block) = self.scope_queue.pop() {
            output.extend_from_slice(&block.samples);
        }
    }

    pub fn buffered_blocks(&self) -> usize {
        self.queue.len()
    }

    pub fn underruns(&self) -> u64 {
        self.underruns.load(Ordering::Relaxed)
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    pub fn callback_calls(&self) -> u64 {
        self.callback_calls.load(Ordering::Relaxed)
    }

    pub fn take_render_cost(&self) -> (f64, u64) {
        let nanos = self.render_nanos.swap(0, Ordering::Relaxed);
        let frames = self.render_frames.swap(0, Ordering::Relaxed);
        (nanos as f64 / 1.0e9, frames)
    }
}

impl Drop for AudioOutput {
    fn drop(&mut self) {
        self.playing.store(false, Ordering::Release);
        self.worker_stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn build_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_format: SampleFormat,
    channels: usize,
    transport: CallbackTransport,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    match sample_format {
        SampleFormat::I8 => build_typed_stream::<i8>(device, config, channels, transport.clone()),
        SampleFormat::I16 => build_typed_stream::<i16>(device, config, channels, transport.clone()),
        SampleFormat::I24 => {
            build_typed_stream::<cpal::I24>(device, config, channels, transport.clone())
        }
        SampleFormat::I32 => build_typed_stream::<i32>(device, config, channels, transport.clone()),
        SampleFormat::I64 => build_typed_stream::<i64>(device, config, channels, transport.clone()),
        SampleFormat::U8 => build_typed_stream::<u8>(device, config, channels, transport.clone()),
        SampleFormat::U16 => build_typed_stream::<u16>(device, config, channels, transport.clone()),
        SampleFormat::U24 => {
            build_typed_stream::<cpal::U24>(device, config, channels, transport.clone())
        }
        SampleFormat::U32 => build_typed_stream::<u32>(device, config, channels, transport.clone()),
        SampleFormat::U64 => build_typed_stream::<u64>(device, config, channels, transport.clone()),
        SampleFormat::F32 => build_typed_stream::<f32>(device, config, channels, transport.clone()),
        SampleFormat::F64 => build_typed_stream::<f64>(device, config, channels, transport),
        _ => Err(cpal::BuildStreamError::StreamConfigNotSupported),
    }
}

fn build_typed_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    transport: CallbackTransport,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: SizedSample + FromSample<f32>,
{
    let error_callback = |error| eprintln!("pmdmini audio stream error: {error}");
    let mut callback = CallbackState::new();
    let CallbackTransport {
        queue,
        playing,
        generation,
        volume,
        underruns,
        peak_left,
        peak_right,
        callback_calls,
    } = transport;
    device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            if playing.load(Ordering::Acquire) {
                callback_calls.fetch_add(1, Ordering::Relaxed);
            }
            if !playing.load(Ordering::Acquire) {
                for sample in data {
                    *sample = T::from_sample(0.0);
                }
                return;
            }
            let gain = f32::from_bits(volume.load(Ordering::Acquire));
            for frame in data.chunks_mut(channels) {
                let [left, right] = callback.next_stereo(&queue, &underruns, &generation);
                let left = left * gain;
                let right = right * gain;
                peak_left.fetch_max(left.abs().to_bits(), Ordering::Relaxed);
                peak_right.fetch_max(right.abs().to_bits(), Ordering::Relaxed);
                let center = (left + right) * 0.5;
                for (channel, sample) in frame.iter_mut().enumerate() {
                    let value = if channel == 0 {
                        left
                    } else if channel == 1 {
                        right
                    } else {
                        center
                    };
                    *sample = T::from_sample(value);
                }
            }
        },
        error_callback,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::{AudioBlock, AudioOutput, CallbackState, AUDIO_BLOCK_FRAMES, AUDIO_QUEUE_BLOCKS};
    use crossbeam_queue::ArrayQueue;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[test]
    fn queue_capacity_is_fixed_and_non_blocking() {
        let queue = ArrayQueue::new(AUDIO_QUEUE_BLOCKS);
        for _ in 0..AUDIO_QUEUE_BLOCKS {
            assert!(queue.push(AudioBlock::default()).is_ok());
        }
        assert!(queue.push(AudioBlock::default()).is_err());
        assert_eq!(queue.len(), AUDIO_QUEUE_BLOCKS);
        assert!(queue.pop().is_some());
        assert_eq!(queue.len(), AUDIO_QUEUE_BLOCKS - 1);
        assert_eq!(AUDIO_BLOCK_FRAMES, 1024);
    }

    #[test]
    fn callback_drains_blocks_and_counts_only_real_misses() {
        let queue = ArrayQueue::new(1);
        let mut block = AudioBlock::default();
        block.samples[0] = 0.25;
        block.samples[1] = -0.5;
        assert!(queue.push(block).is_ok(), "one block fits");
        let underruns = AtomicU64::new(0);
        let generation = AtomicU64::new(0);
        let mut callback = CallbackState::new();

        assert_eq!(
            callback.next_stereo(&queue, &underruns, &generation),
            [0.25, -0.5]
        );
        assert_eq!(underruns.load(Ordering::Relaxed), 0);

        callback.cursor = super::AUDIO_BLOCK_SAMPLES;
        assert_eq!(
            callback.next_stereo(&queue, &underruns, &generation),
            [0.0, 0.0]
        );
        assert_eq!(underruns.load(Ordering::Relaxed), 1);
    }

    #[test]
    #[ignore = "requires a native output device and PMDMINI_NATIVE_TEST_SONG"]
    fn native_output_refills_and_drains_a_real_song() {
        let path = std::env::var_os("PMDMINI_NATIVE_TEST_SONG")
            .expect("set PMDMINI_NATIVE_TEST_SONG to a local .M/.M2/.MZ file");
        let path = std::path::PathBuf::from(path);
        let (_, main, files) = crate::native_files::MemoryFiles::from_main_path(&path)
            .expect("load native song and companion files");
        let mut output = AudioOutput::open().expect("open the native output device");
        let player = Arc::new(Mutex::new(pmd_core::Player::new(output.sample_rate())));
        player
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .load(&main, &files)
            .expect("load the native song");
        let telemetry =
            crate::telemetry::SharedTelemetry::new(crate::telemetry::TelemetryFrame::new(
                player
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .channel_snapshots(),
            ));
        output
            .start_worker(Arc::clone(&player), telemetry)
            .expect("start the native audio producer");
        output.set_playing(true);

        let initial_underruns = output.underruns();
        for _ in 0..20 {
            if output.buffered_blocks() >= super::AUDIO_TARGET_BLOCKS {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let initial_blocks = output.buffered_blocks();
        assert!(initial_blocks > 0, "native producer did not queue audio");
        let initial_callbacks = output.callback_calls();
        std::thread::sleep(Duration::from_millis(40));
        assert!(
            output.callback_calls() > initial_callbacks,
            "native callback did not run while audio was playing"
        );
        assert!(output.buffered_blocks() <= super::AUDIO_TARGET_BLOCKS);
        assert_eq!(
            output.underruns(),
            initial_underruns,
            "output underrun while queued audio was available"
        );

        for _ in 0..8 {
            std::thread::sleep(Duration::from_millis(40));
        }
        assert_eq!(
            output.underruns(),
            initial_underruns,
            "output underrun during sustained refill"
        );

        output.set_playing(false);
        output.clear();
        assert_eq!(output.buffered_blocks(), 0);
    }
}
