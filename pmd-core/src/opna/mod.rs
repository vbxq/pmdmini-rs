// SPDX-License-Identifier: AGPL-3.0-only

#[allow(dead_code)]
mod operator;

#[allow(dead_code)]
mod channel;
#[allow(dead_code)]
mod envelope;
#[allow(dead_code)]
mod lfo;
#[allow(dead_code)]
mod ssg;

#[allow(dead_code)]
mod rhythm;

#[allow(dead_code)]
mod adpcm;

#[allow(dead_code)]
mod chip;
#[allow(dead_code)]
mod fm;
#[allow(dead_code)]
mod registers;
#[allow(dead_code)]
mod renderer;
mod replay;
#[allow(dead_code)]
mod resampler;
pub(crate) mod tables;
mod trace;

pub use renderer::ChipRenderer;
pub use replay::RegisterReplay;
pub use trace::{
    compare_register_traces, parse_trace_csv, RegisterWrite, TraceError, TraceMismatch,
};
