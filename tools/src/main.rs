// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::{HashMap, VecDeque};
use std::env;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use pmd_core::opna::{compare_register_traces, parse_trace_csv, ChipRenderer, RegisterWrite};
use pmd_core::{FileProvider, Player};

const OPNA_CLOCK_HZ: u32 = 7_987_200;

const PMD_PSG_VOLUME_NEG18: i32 = 23_253;
const DEFAULT_REGISTER_WAIT_NS: u64 = 30_000;

#[derive(Debug)]
struct Options {
    trace_path: String,
    reference_path: String,
    rate: u32,
    frames: Option<u64>,
    interpolation: bool,
    output_path: Option<String>,
    initial_count2: u64,
}

#[derive(Clone, Copy, Debug)]
struct ScheduledWrite {
    frame: u64,
    write: RegisterWrite,
}

#[derive(Debug)]
struct RegisterWait {
    rate: u32,
    fractional: u64,
}

impl RegisterWait {
    fn new(rate: u32, initial_count2: u64) -> Self {
        Self {
            rate,
            fractional: initial_count2,
        }
    }

    fn frames_before_write(&mut self) -> u32 {
        let wait_count = DEFAULT_REGISTER_WAIT_NS * u64::from(self.rate) / 1_000_000;
        let mut frames = wait_count / 1_000;
        self.fractional += wait_count % 1_000;
        if self.fractional > 1_000 {
            frames += 1;
            self.fractional -= 1_000;
        }
        frames as u32
    }
}

fn main() {
    match run() {
        Ok(()) => {}
        Err(message) => {
            eprintln!("pmd-tools: {message}");
            process::exit(1);
        }
    }
}

fn run() -> Result<(), String> {
    if env::args().nth(1).as_deref() == Some("--compare-reference") {
        return run_compare_player(env::args().skip(2));
    }
    if env::args().nth(1).as_deref() == Some("compare-trace") {
        return run_compare_trace(env::args().skip(2));
    }
    if env::args().nth(1).as_deref() == Some("dump-sequencer") {
        return run_dump_sequencer(env::args().skip(2));
    }
    if env::args().nth(1).as_deref() == Some("compare-player") {
        return run_compare_player(env::args().skip(2));
    }
    if env::args().nth(1).as_deref() == Some("compare-reference")
        && env::args().skip(2).any(|argument| argument == "--input")
    {
        return run_compare_player(env::args().skip(2));
    }
    let options = parse_options(env::args().skip(1))?;
    let trace_bytes = fs::read(&options.trace_path)
        .map_err(|error| format!("cannot read trace {}: {error}", options.trace_path))?;
    let writes = parse_trace_csv(&trace_bytes)
        .map_err(|error| format!("invalid trace at line {}", error.line))?;
    if writes.is_empty() {
        return Err("trace contains no register writes".to_owned());
    }

    let reference = fs::read(&options.reference_path)
        .map_err(|error| format!("cannot read reference {}: {error}", options.reference_path))?;
    if reference.len() % 4 != 0 {
        return Err("reference PCM length is not a whole number of stereo frames".to_owned());
    }

    let scheduled = schedule_writes(&writes, options.rate)?;
    let trace_frames = scheduled
        .last()
        .map_or(1, |event| event.frame.saturating_add(1));
    let reference_frames = (reference.len() / 4) as u64;
    let requested_frames = options.frames.unwrap_or(trace_frames);
    let frame_count = requested_frames.min(reference_frames);
    if frame_count == 0 {
        return Err("requested comparison has no reference frames".to_owned());
    }

    let mut renderer = ChipRenderer::new(OPNA_CLOCK_HZ, options.rate, options.interpolation)
        .ok_or_else(|| format!("invalid output rate {}", options.rate))?;
    renderer.set_psg_volume(PMD_PSG_VOLUME_NEG18);
    let mut register_wait = RegisterWait::new(options.rate, options.initial_count2);
    let mut wait_queue = VecDeque::new();

    let mut rendered_pcm = options
        .output_path
        .as_ref()
        .map(|_| Vec::with_capacity(frame_count as usize * 4));
    let mut next_event = 0;
    let mut mismatch_count = 0u64;
    let mut max_abs_error = 0i32;
    let mut first_mismatch = None;

    for frame in 0..frame_count {
        while next_event < scheduled.len() && scheduled[next_event].frame <= frame {
            let event = scheduled[next_event];

            if event.write.address == 0x29 && event.write.value == 0x83 {
                wait_queue.clear();
            }
            let wait_frames = register_wait.frames_before_write();
            for _ in 0..wait_frames {
                let sample = render_one(&mut renderer);
                wait_queue.push_back(sample);
            }
            renderer.write(event.write.address, event.write.value);
            next_event += 1;
        }

        let generated = wait_queue
            .pop_front()
            .unwrap_or_else(|| render_one(&mut renderer));
        let actual = [clamp_i16(generated[0]), clamp_i16(generated[1])];
        let expected = [
            read_i16(&reference, frame, 0),
            read_i16(&reference, frame, 1),
        ];

        if let Some(output) = rendered_pcm.as_mut() {
            for sample in actual {
                output.extend_from_slice(&sample.to_le_bytes());
            }
        }

        for channel in 0..2 {
            let error = i32::from(actual[channel]) - i32::from(expected[channel]);
            let absolute = error.unsigned_abs().min(i32::MAX as u32) as i32;
            max_abs_error = max_abs_error.max(absolute);
            if error != 0 {
                mismatch_count += 1;
                if first_mismatch.is_none() {
                    first_mismatch = Some((frame, channel, expected[channel], actual[channel]));
                }
            }
        }
    }

    if let (Some(path), Some(output)) = (&options.output_path, rendered_pcm) {
        fs::write(path, output).map_err(|error| format!("cannot write {path}: {error}"))?;
    }

    println!(
        "writes={} trace_frames={} compared_frames={} mismatches={} max_abs_error={}",
        writes.len(),
        trace_frames,
        frame_count,
        mismatch_count,
        max_abs_error
    );
    if let Some((frame, channel, expected, actual)) = first_mismatch {
        println!(
            "first_mismatch=frame:{frame},channel:{channel},expected:{expected},actual:{actual}"
        );
        return Err("reference comparison failed".to_owned());
    }
    Ok(())
}

#[derive(Debug)]
struct DumpSequencerOptions {
    input_path: PathBuf,
    trace_path: PathBuf,
    ticks: u64,
}

#[derive(Debug)]
struct ComparePlayerOptions {
    input_path: PathBuf,
    reference_path: PathBuf,
    rate: u32,
    frames: Option<u64>,
    output_path: Option<PathBuf>,
}

struct DirectoryFiles {
    files: HashMap<String, Vec<u8>>,
}

impl DirectoryFiles {
    fn from_parent(input_path: &Path) -> Result<Self, String> {
        let parent = input_path.parent().unwrap_or_else(|| Path::new("."));
        let mut files = HashMap::new();
        let entries = fs::read_dir(parent).map_err(|error| {
            format!("cannot read input directory {}: {error}", parent.display())
        })?;
        for entry in entries {
            let entry =
                entry.map_err(|error| format!("cannot inspect input directory: {error}"))?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let bytes = fs::read(&path)
                .map_err(|error| format!("cannot read companion {}: {error}", path.display()))?;
            files.insert(name.to_ascii_uppercase(), bytes);
        }
        Ok(Self { files })
    }
}

impl FileProvider for DirectoryFiles {
    fn get(&self, name: &str) -> Option<&[u8]> {
        self.files
            .get(&name.to_ascii_uppercase())
            .map(Vec::as_slice)
    }
}

fn run_dump_sequencer<I>(args: I) -> Result<(), String>
where
    I: IntoIterator,
    I::Item: Into<String>,
{
    let options = parse_dump_sequencer_options(args)?;
    let main = fs::read(&options.input_path).map_err(|error| {
        format!(
            "cannot read input {}: {error}",
            options.input_path.display()
        )
    })?;
    let files = DirectoryFiles::from_parent(&options.input_path)?;
    let mut player = Player::new(44_100);
    player
        .load(&main, &files)
        .map_err(|error| format!("cannot load {}: {error:?}", options.input_path.display()))?;

    let mut trace = String::from("# time_us,tick,address,value\n# sequencer-only\n");
    let mut rows = 0usize;
    let mut ticks_run = 0u64;
    for _ in 0..options.ticks {
        let status = player
            .tick()
            .map_err(|error| format!("sequencer error: {error:?}"))?;
        ticks_run = ticks_run.saturating_add(1);
        let mut writes = Vec::new();
        player.take_sequencer_trace(&mut writes);
        for write in writes {
            writeln!(
                trace,
                "{},{},0x{:03x},0x{:02x}",
                write.time_us, write.tick, write.address, write.value
            )
            .map_err(|_| "cannot format trace row".to_owned())?;
            rows += 1;
        }
        if status.ended {
            break;
        }
    }
    fs::write(&options.trace_path, trace).map_err(|error| {
        format!(
            "cannot write trace {}: {error}",
            options.trace_path.display()
        )
    })?;
    println!(
        "ticks={} rows={} trace={}",
        ticks_run,
        rows,
        options.trace_path.display()
    );
    Ok(())
}

fn run_compare_player<I>(args: I) -> Result<(), String>
where
    I: IntoIterator,
    I::Item: Into<String>,
{
    let options = parse_compare_player_options(args)?;
    let main = fs::read(&options.input_path).map_err(|error| {
        format!(
            "cannot read input {}: {error}",
            options.input_path.display()
        )
    })?;
    let files = DirectoryFiles::from_parent(&options.input_path)?;
    let mut player = Player::new(options.rate);
    player
        .load(&main, &files)
        .map_err(|error| format!("cannot load {}: {error:?}", options.input_path.display()))?;

    let reference = fs::read(&options.reference_path).map_err(|error| {
        format!(
            "cannot read reference {}: {error}",
            options.reference_path.display()
        )
    })?;
    if reference.len() % 4 != 0 {
        return Err("reference PCM length is not a whole number of stereo frames".to_owned());
    }
    let reference_frames = (reference.len() / 4) as u64;
    let frame_count = options
        .frames
        .unwrap_or(reference_frames)
        .min(reference_frames);
    if frame_count == 0 {
        return Err("requested comparison has no reference frames".to_owned());
    }

    const CHUNK_FRAMES: usize = 4096;
    let mut rendered = vec![0.0f32; CHUNK_FRAMES * 2];
    let mut mismatches = 0u64;
    let mut max_abs_error = 0i32;
    let mut first_mismatch = None;
    let mut rendered_pcm = options
        .output_path
        .as_ref()
        .map(|_| Vec::with_capacity(frame_count as usize * 4));
    let mut base = 0u64;
    while base < frame_count {
        let count = (frame_count - base).min(CHUNK_FRAMES as u64) as usize;
        player.render(&mut rendered[..count * 2]);
        for frame in 0..count {
            let absolute_frame = base + frame as u64;
            let expected = [
                read_i16(&reference, absolute_frame, 0),
                read_i16(&reference, absolute_frame, 1),
            ];
            let actual = [
                float_to_i16(rendered[frame * 2]),
                float_to_i16(rendered[frame * 2 + 1]),
            ];
            if let Some(output) = rendered_pcm.as_mut() {
                for sample in actual {
                    output.extend_from_slice(&sample.to_le_bytes());
                }
            }
            for channel in 0..2 {
                let error = i32::from(actual[channel]) - i32::from(expected[channel]);
                let absolute = error.unsigned_abs().min(i32::MAX as u32) as i32;
                max_abs_error = max_abs_error.max(absolute);
                if error != 0 {
                    mismatches += 1;
                    if first_mismatch.is_none() {
                        first_mismatch =
                            Some((absolute_frame, channel, expected[channel], actual[channel]));
                    }
                }
            }
        }
        base += count as u64;
    }

    if let (Some(path), Some(output)) = (&options.output_path, rendered_pcm) {
        fs::write(path, output)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    }

    println!(
        "frames={} mismatches={} max_abs_error={} player_position={}",
        frame_count,
        mismatches,
        max_abs_error,
        player.position_samples()
    );
    if let Some((frame, channel, expected, actual)) = first_mismatch {
        println!(
            "first_mismatch=frame:{frame},channel:{channel},expected:{expected},actual:{actual}"
        );
        return Err("Player PCM comparison failed".to_owned());
    }
    Ok(())
}

fn parse_compare_player_options<I>(args: I) -> Result<ComparePlayerOptions, String>
where
    I: IntoIterator,
    I::Item: Into<String>,
{
    let mut input_path = None;
    let mut reference_path = None;
    let mut rate = None;
    let mut frames = None;
    let mut output_path = None;
    let mut args = args.into_iter().map(Into::into);
    while let Some(argument) = args.next() {
        let value = |name: &str, args: &mut dyn Iterator<Item = String>| {
            args.next()
                .ok_or_else(|| format!("missing value for {name}"))
        };
        match argument.as_str() {
            "--input" => input_path = Some(PathBuf::from(value("--input", &mut args)?)),
            "--reference" => reference_path = Some(PathBuf::from(value("--reference", &mut args)?)),
            "--rate" => {
                let text = value("--rate", &mut args)?;
                let parsed = text
                    .parse::<u32>()
                    .map_err(|_| format!("invalid rate {text}"))?;
                if parsed == 0 {
                    return Err("rate must not be zero".to_owned());
                }
                rate = Some(parsed);
            }
            "--frames" => {
                let text = value("--frames", &mut args)?;
                frames = Some(
                    text.parse::<u64>()
                        .map_err(|_| format!("invalid frame count {text}"))?,
                );
            }
            "--output" => {
                output_path = Some(PathBuf::from(value("--output", &mut args)?));
            }
            "--help" => return Err(usage()),
            unknown => return Err(format!("unknown argument {unknown}\n{}", usage())),
        }
    }
    Ok(ComparePlayerOptions {
        input_path: input_path.ok_or_else(usage)?,
        reference_path: reference_path.ok_or_else(usage)?,
        rate: rate.ok_or_else(usage)?,
        frames,
        output_path,
    })
}

fn parse_dump_sequencer_options<I>(args: I) -> Result<DumpSequencerOptions, String>
where
    I: IntoIterator,
    I::Item: Into<String>,
{
    let mut input_path = None;
    let mut trace_path = None;
    let mut ticks = 128;
    let mut args = args.into_iter().map(Into::into);
    while let Some(argument) = args.next() {
        let value = |name: &str, args: &mut dyn Iterator<Item = String>| {
            args.next()
                .ok_or_else(|| format!("missing value for {name}"))
        };
        match argument.as_str() {
            "--input" => input_path = Some(PathBuf::from(value("--input", &mut args)?)),
            "--trace" => trace_path = Some(PathBuf::from(value("--trace", &mut args)?)),
            "--ticks" => {
                let text = value("--ticks", &mut args)?;
                ticks = text
                    .parse::<u64>()
                    .map_err(|_| format!("invalid tick count {text}"))?;
            }
            "--help" => return Err(usage()),
            unknown => return Err(format!("unknown argument {unknown}\n{}", usage())),
        }
    }
    Ok(DumpSequencerOptions {
        input_path: input_path.ok_or_else(usage)?,
        trace_path: trace_path.ok_or_else(usage)?,
        ticks,
    })
}

fn run_compare_trace<I>(args: I) -> Result<(), String>
where
    I: IntoIterator,
    I::Item: Into<String>,
{
    let mut expected_path = None;
    let mut actual_path = None;
    let mut args = args.into_iter().map(Into::into);
    while let Some(argument) = args.next() {
        let value = |name: &str, args: &mut dyn Iterator<Item = String>| {
            args.next()
                .ok_or_else(|| format!("missing value for {name}"))
        };
        match argument.as_str() {
            "--expected" => expected_path = Some(value("--expected", &mut args)?),
            "--actual" => actual_path = Some(value("--actual", &mut args)?),
            "--help" => return Err(usage()),
            unknown => return Err(format!("unknown argument {unknown}\n{}", usage())),
        }
    }

    let expected_path = expected_path.ok_or_else(usage)?;
    let actual_path = actual_path.ok_or_else(usage)?;
    let expected_bytes = fs::read(&expected_path)
        .map_err(|error| format!("cannot read expected trace {expected_path}: {error}"))?;
    let actual_bytes = fs::read(&actual_path)
        .map_err(|error| format!("cannot read actual trace {actual_path}: {error}"))?;
    let expected = parse_trace_csv(&expected_bytes)
        .map_err(|error| format!("invalid expected trace at line {}", error.line))?;
    let actual = parse_trace_csv(&actual_bytes)
        .map_err(|error| format!("invalid actual trace at line {}", error.line))?;

    match compare_register_traces(&expected, &actual) {
        Ok(()) => {
            println!(
                "expected_rows={} actual_rows={} mismatches=0",
                expected.len(),
                actual.len()
            );
            Ok(())
        }
        Err(mismatch) => {
            println!(
                "expected_rows={} actual_rows={} first_mismatch={}",
                expected.len(),
                actual.len(),
                mismatch.index
            );
            Err("register traces differ".to_owned())
        }
    }
}

fn parse_options<I>(args: I) -> Result<Options, String>
where
    I: IntoIterator,
    I::Item: Into<String>,
{
    let mut trace_path = None;
    let mut reference_path = None;
    let mut rate = None;
    let mut frames = None;
    let mut interpolation = false;
    let mut output_path = None;
    let mut initial_count2 = 0;
    let mut args = args.into_iter().map(Into::into);

    let command = args.next().ok_or_else(usage)?;
    if command != "compare-reference" {
        return Err(usage());
    }

    while let Some(argument) = args.next() {
        let value = |name: &str, args: &mut dyn Iterator<Item = String>| {
            args.next()
                .ok_or_else(|| format!("missing value for {name}"))
        };
        match argument.as_str() {
            "--trace" => trace_path = Some(value("--trace", &mut args)?),
            "--reference" => reference_path = Some(value("--reference", &mut args)?),
            "--rate" => {
                let text = value("--rate", &mut args)?;
                let parsed = text
                    .parse::<u32>()
                    .map_err(|_| format!("invalid rate {text}"))?;
                if parsed == 0 {
                    return Err("rate must not be zero".to_owned());
                }
                rate = Some(parsed);
            }
            "--frames" => {
                let text = value("--frames", &mut args)?;
                frames = Some(
                    text.parse::<u64>()
                        .map_err(|_| format!("invalid frame count {text}"))?,
                );
            }
            "--interpolation" => interpolation = true,
            "--output" => output_path = Some(value("--output", &mut args)?),
            "--initial-count2" => {
                let text = value("--initial-count2", &mut args)?;
                initial_count2 = text
                    .parse::<u64>()
                    .map_err(|_| format!("invalid count2 remainder {text}"))?;
                if initial_count2 > 1_000 {
                    return Err("initial count2 remainder must be at most 1000".to_owned());
                }
            }
            "--help" => return Err(usage()),
            unknown => return Err(format!("unknown argument {unknown}\n{}", usage())),
        }
    }

    Ok(Options {
        trace_path: trace_path.ok_or_else(usage)?,
        reference_path: reference_path.ok_or_else(usage)?,
        rate: rate.ok_or_else(usage)?,
        frames,
        interpolation,
        output_path,
        initial_count2,
    })
}

fn usage() -> String {
    "usage: pmd-tools compare-reference --trace TRACE.csv --reference REF.pcm16le --rate HZ [--frames N] [--interpolation] [--output OUT.pcm16le] [--initial-count2 N]\n       pmd-tools compare-trace --expected CPP.csv --actual RUST.csv\n       pmd-tools dump-sequencer --input SONG.M --trace RUST.csv [--ticks N]\n       pmd-tools compare-reference --input SONG.M --reference REF.pcm16le --rate HZ [--frames N] [--output OUT.pcm16le]\n       pmd-tools compare-player --input SONG.M --reference REF.pcm16le --rate HZ [--frames N] [--output OUT.pcm16le]\n       pmd-tools --compare-reference --input SONG.M --reference REF.pcm16le --rate HZ [--frames N] [--output OUT.pcm16le]"
        .to_owned()
}

fn schedule_writes(writes: &[RegisterWrite], rate: u32) -> Result<Vec<ScheduledWrite>, String> {
    let mut scheduled = Vec::with_capacity(writes.len());
    let mut previous_time = 0u64;
    let mut index = 0;

    while index < writes.len() {
        let time = writes[index].time_us;
        if time < previous_time {
            return Err("trace timestamps are not monotonic".to_owned());
        }

        let frame = time
            .checked_mul(u64::from(rate))
            .ok_or_else(|| "trace timestamp overflows frame conversion".to_owned())?
            / 1_000_000;

        while index < writes.len() && writes[index].time_us == time {
            scheduled.push(ScheduledWrite {
                frame,
                write: writes[index],
            });
            index += 1;
        }
        previous_time = time;
    }
    Ok(scheduled)
}

fn read_i16(bytes: &[u8], frame: u64, channel: usize) -> i16 {
    let offset = (frame as usize * 4) + channel * 2;
    i16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn float_to_i16(value: f32) -> i16 {
    (value * 32_768.0).clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16
}

fn render_one(renderer: &mut ChipRenderer) -> [i32; 2] {
    let mut output = [[0i32; 2]; 1];
    renderer.render(&mut output);
    output[0]
}

fn clamp_i16(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

#[cfg(test)]
mod tests {
    use super::{float_to_i16, run_compare_player, schedule_writes, DirectoryFiles};
    use pmd_core::opna::RegisterWrite;
    use pmd_core::Player;
    use std::fs;
    use std::path::Path;

    #[test]
    fn schedule_uses_absolute_timestamp_floor() {
        let writes = [
            RegisterWrite {
                time_us: 0,
                tick: 0,
                address: 0,
                value: 0,
            },
            RegisterWrite {
                time_us: 18_462,
                tick: 1,
                address: 1,
                value: 1,
            },
            RegisterWrite {
                time_us: 32_110,
                tick: 2,
                address: 2,
                value: 2,
            },
        ];

        let scheduled = schedule_writes(&writes, 44_100).expect("timestamps are valid");
        assert_eq!(
            scheduled
                .iter()
                .map(|event| event.frame)
                .collect::<Vec<_>>(),
            [0, 814, 1_416]
        );
    }

    #[test]
    fn compare_reference_accepts_exact_pcm_and_rejects_a_mismatch() {
        let directory = std::env::temp_dir().join(format!(
            "pmd-tools-compare-reference-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("create synthetic compare directory");
        let input = directory.join("SONG.M");
        let reference = directory.join("reference.pcm16le");
        let main = [0x01, 0x18, 0x00];
        fs::write(&input, main).expect("write synthetic PMD");

        let files = DirectoryFiles::from_parent(&input).expect("read synthetic directory");
        let mut player = Player::new(44_100);
        player.load(&main, &files).expect("load synthetic PMD");
        let mut rendered = [0.0f32; 8];
        player.render(&mut rendered);
        let mut expected = Vec::with_capacity(rendered.len() * 2);
        for sample in rendered {
            expected.extend_from_slice(&float_to_i16(sample).to_le_bytes());
        }
        fs::write(&reference, &expected).expect("write exact reference");

        let arguments = compare_arguments(&input, &reference);
        run_compare_player(arguments.clone()).expect("exact reference must pass");

        let mut mismatch = expected;
        mismatch[0] = mismatch[0].wrapping_add(1);
        fs::write(&reference, mismatch).expect("write mismatching reference");
        assert!(run_compare_player(arguments).is_err());

        fs::remove_dir_all(&directory).expect("remove synthetic compare directory");
    }

    fn compare_arguments(input: &Path, reference: &Path) -> Vec<String> {
        vec![
            String::from("--input"),
            input.display().to_string(),
            String::from("--reference"),
            reference.display().to_string(),
            String::from("--rate"),
            String::from("44100"),
            String::from("--frames"),
            String::from("4"),
        ]
    }
}
