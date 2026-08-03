// SPDX-License-Identifier: AGPL-3.0-only

use crate::fb::Framebuffer;
use crate::font;
use crate::layout as L;
use crate::levels;
use crate::palette as P;
use crate::spectrum::SpectrumAnalyzer;
use pmd_core::{ChannelSnapshot, VisualChannelKind, CHANNEL_COUNT, NO_NOTE};

const FIRST_OCTAVE: u8 = 1;

const WHITE_SEMITONE: [usize; 7] = [0, 2, 4, 5, 7, 9, 11];

const SEMITONE_SPAN: usize =
    (L::KEY_COUNT as usize / 7) * 12 + WHITE_SEMITONE[L::KEY_COUNT as usize % 7];

const BLACK_LEFT: [bool; 7] = [true, false, true, true, false, true, true];

pub struct View<'a> {
    pub snapshots: &'a [ChannelSnapshot; CHANNEL_COUNT],

    pub spectrum: &'a [f32],

    pub peaks: &'a [f32],

    pub levels: &'a [f32],

    pub level_peaks: &'a [f32],

    pub file_name: Option<&'a str>,

    pub title: &'a str,

    pub composer: &'a str,

    pub arranger: &'a str,

    pub memo: &'a str,

    pub muted: &'a [bool],

    pub list: Option<&'a ListView<'a>>,

    pub mode: &'a str,

    pub message: &'a str,

    pub playing: bool,

    pub elapsed: f32,

    pub clock: u64,

    pub loop_field: String,

    pub volume: u8,

    pub cpu: &'a str,

    pub analyser_rate: f32,
}

pub struct ListView<'a> {
    pub entries: &'a [crate::playlist::Entry],

    pub cursor: usize,

    pub playing: Option<usize>,
}

pub fn render(fb: &mut Framebuffer, view: &View) {
    fb.clear(P::BLACK);
    for index in 0..L::STRIP_COUNT {
        strip(
            fb,
            index,
            view.snapshots[index],
            view.muted.get(index).copied().unwrap_or(false),
        );
    }
    right_panel(fb, view);
    status_line(fb, view);
    match view.list {
        Some(list) => playlist_panel(fb, list, view.mode, view.message),
        None => metadata_band(fb, view),
    }
}

pub fn field(fb: &mut Framebuffer, x: i32, y: i32, w: i32, h: i32, label: &str, idx: u8) -> i32 {
    fb.rect_fill(x, y, 2, h - 2, idx);
    let text_x = x + 4;
    let end = font::draw(fb, text_x, y + 1, label, idx);
    fb.hline(x, x + w - 1, y + h - 1, idx);
    end
}

fn draw_right(fb: &mut Framebuffer, right: i32, y: i32, text: &str, idx: u8) {
    font::draw(fb, right - font::width(text), y, text, idx);
}

pub fn format_loops(completed: u32, target: Option<u32>) -> String {
    match target {
        None => format!("{:04}", completed.min(9_999)),
        Some(target) => format!("{:02}-{:02}", completed.min(99), target.min(99)),
    }
}

fn format_time(seconds: f32) -> String {
    let total = seconds.max(0.0);
    let minutes = (total / 60.0) as u32;
    let secs = (total % 60.0) as u32;
    let cents = ((total * 100.0) as u32) % 100;
    format!("{minutes:02}:{secs:02}.{cents:02}")
}

fn kind_name(kind: VisualChannelKind) -> &'static str {
    match kind {
        VisualChannelKind::Fm => "FM",
        VisualChannelKind::Ssg => "SSG",
        VisualChannelKind::Adpcm => "ADPCM",
        VisualChannelKind::Rhythm => "RHY",
    }
}

fn key_slot(note: u8) -> Option<usize> {
    if note == NO_NOTE {
        return None;
    }
    let semitone = usize::from(note & 0x0f);
    if semitone > 11 {
        return None;
    }
    let octave = note >> 4;
    if octave < FIRST_OCTAVE {
        return None;
    }
    let slot = usize::from(octave - FIRST_OCTAVE) * 12 + semitone;
    (slot < SEMITONE_SPAN).then_some(slot)
}

fn key_name(note: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C+", "D", "D+", "E", "F", "F+", "G", "G+", "A", "A+", "B",
    ];
    if note == NO_NOTE {
        return String::from("--");
    }
    if note & 0x0f == 0x0f {
        return String::from("S");
    }
    match key_slot(note) {
        Some(_) => format!("o{}{}", note >> 4, NAMES[usize::from(note & 0x0f)]),
        None => String::from("--"),
    }
}

fn voice_number(snapshot: ChannelSnapshot) -> u8 {
    if snapshot.voice == NO_NOTE {
        0
    } else {
        snapshot.voice
    }
}

fn semitone_to_key(semitone: usize) -> (usize, bool) {
    const WHITE_INDEX: [usize; 12] = [0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5, 6];
    const IS_BLACK: [bool; 12] = [
        false, true, false, true, false, false, true, false, true, false, true, false,
    ];
    let octave = semitone / 12;
    let degree = semitone % 12;
    (octave * 7 + WHITE_INDEX[degree], IS_BLACK[degree])
}

fn strip(fb: &mut Framebuffer, index: usize, snapshot: ChannelSnapshot, muted: bool) {
    let top = L::strip_top(index);
    let ink = if muted { P::DIM } else { P::TEXT };

    font::draw(
        fb,
        L::STRIP_KIND_X,
        top + L::STRIP_LINE1_Y,
        kind_name(snapshot.kind),
        ink,
    );
    activity_bar(fb, top, snapshot, muted);

    font::draw(fb, 0, top + L::STRIP_LINE2_Y, "TRACK、", ink);

    font::draw_tall(
        fb,
        L::STRIP_NUMBER_X,
        top,
        &format!("{:02}", voice_number(snapshot) % 100),
        ink,
    );

    let note = key_name(snapshot.note);
    let readouts: [(&str, String); 5] = [
        ("KN、", note),
        ("TN、", format!("{:03}", voice_number(snapshot))),
        ("VL、", format!("{:03}", snapshot.level)),
        ("GT、", format!("{:03}", snapshot.remaining.min(999))),
        (
            "DT、",
            format!("{:03}", snapshot.lfo.unsigned_abs().min(999)),
        ),
    ];

    const FIELD_X: [i32; 5] = [66, 107, 141, 172, 208];

    const VALUE_X: [i32; 5] = [81, 122, 156, 187, 226];
    for (slot, (label, value)) in readouts.iter().enumerate() {
        font::draw(fb, FIELD_X[slot], top + L::STRIP_LINE2_Y, label, ink);
        font::draw(fb, VALUE_X[slot], top + L::STRIP_LINE2_Y, value, ink);
    }

    font::draw(fb, L::STRIP_MASK_X, top + L::STRIP_LINE2_Y, "M、", ink);
    mask_meter(fb, top, snapshot, muted);

    keyboard(fb, top, snapshot, muted);
}

fn activity_bar(fb: &mut Framebuffer, top: i32, snapshot: ChannelSnapshot, muted: bool) {
    let y = top + L::STRIP_LINE1_Y;

    let fraction = if snapshot.active && !muted {
        f32::from(snapshot.level) / 255.0
    } else {
        0.0
    };
    let width = 6 + (fraction * (L::STRIP_BAR_W - 6) as f32) as i32;
    let ink = if muted { P::DIM } else { P::TEXT };
    fb.rect_fill(L::STRIP_BAR_X, y, width, L::STRIP_BAR_H - 1, ink);
    fb.hline(
        L::STRIP_BAR_X,
        L::STRIP_BAR_X + L::STRIP_BAR_W - 1,
        y + L::STRIP_BAR_H - 1,
        P::DIM,
    );
}

fn mask_meter(fb: &mut Framebuffer, top: i32, snapshot: ChannelSnapshot, muted: bool) {
    let y = top + L::STRIP_LINE2_Y + 2;

    let bits = if muted || !snapshot.active {
        0
    } else {
        snapshot.operator_mask
    };
    for slot in 0..L::STRIP_METER_CELLS {
        let x = L::STRIP_METER_X + slot * L::STRIP_METER_PITCH;
        let lit = bits & (1 << slot) != 0;
        fb.rect_fill(
            x,
            y,
            L::STRIP_METER_CELL_W,
            2,
            if lit { P::TEXT } else { P::DIM },
        );
    }
}

fn keyboard(fb: &mut Framebuffer, top: i32, snapshot: ChannelSnapshot, muted: bool) {
    let y = top + L::KBD_Y;

    fb.rect_fill(
        L::KBD_X,
        y,
        L::KEY_COUNT * L::KEY_W,
        L::KBD_H - 1,
        P::KEY_WHITE,
    );

    for key in 1..L::KEY_COUNT {
        let x = L::KBD_X + key * L::KEY_W - 1;
        fb.vline(x, y, y + L::KBD_H - 2, P::KEY_SHADE);
    }
    fb.hline(
        L::KBD_X,
        L::KBD_X + L::KEY_COUNT * L::KEY_W - 1,
        y + L::KBD_H - 1,
        P::KEY_SHADE,
    );

    for key in 0..L::KEY_COUNT {
        if !BLACK_LEFT[(key % 7) as usize] {
            continue;
        }
        let x = L::KBD_X + key * L::KEY_W - L::KEY_BLACK_W / 2;
        fb.rect_fill(x, y, L::KEY_BLACK_W, L::KBD_BLACK_H, P::BLACK);
    }

    if let Some(slot) = key_slot(snapshot.note) {
        if snapshot.active && !muted {
            let (white, is_black) = semitone_to_key(slot);
            if white < L::KEY_COUNT as usize {
                let x = L::KBD_X + white as i32 * L::KEY_W;
                if is_black {
                    fb.rect_fill(
                        x - L::KEY_BLACK_W / 2,
                        y,
                        L::KEY_BLACK_W,
                        L::KBD_BLACK_H,
                        P::NOTE,
                    );
                } else {
                    fb.rect_fill(x, y, L::KEY_W - 1, L::KBD_H - 1, P::NOTE);
                }
            }
        }
    }
}

fn right_panel(fb: &mut Framebuffer, view: &View) {
    logo(fb);

    font::draw(fb, L::RIGHT_X + 2, L::DRIVER_Y, "DRIVER、", P::TEXT);

    for (index, credit) in crate::DRIVER_CREDITS.iter().enumerate() {
        let ink = if index == crate::DRIVER_CREDITS.len() - 1 {
            P::TEXT
        } else {
            P::DIM
        };
        font::draw(
            fb,
            L::DRIVER_VALUE_X,
            L::DRIVER_Y + index as i32 * L::DRIVER_LINE_PITCH,
            credit,
            ink,
        );
    }

    dial(fb);
    slider(fb, view);
    transport(fb, view);
    fields(fb, view);
    cpu_field(fb, view);

    fb.hline(L::RIGHT_X, L::SCREEN_W - 1, L::SPECTRUM_RULE_Y, P::TEXT);
    spectrum(fb, view);
    level_analyzer(fb, view);
}

fn logo(fb: &mut Framebuffer) {
    let x = L::RIGHT_X;
    let y = L::LOGO_Y;
    font::draw_logo(fb, x, y, P::TEXT);

    font::draw(fb, L::TAGLINE_X, y, crate::TAGLINE_MACHINES, P::TEXT);
    font::draw(fb, L::TAGLINE_X, y + 6, crate::TAGLINE_PROGRAM, P::TEXT);

    fb.hline(x, L::SCREEN_W - 1, y + font::LOGO_H, P::TEXT);
}

fn dial(fb: &mut Framebuffer) {
    let (cx, cy, r) = (L::DIAL_CX, L::DIAL_CY, L::DIAL_R);

    for step in 0..24 {
        let angle = std::f32::consts::TAU * step as f32 / 24.0;
        let (s, c) = angle.sin_cos();
        let x0 = cx + (c * (r - 6) as f32) as i32;
        let y0 = cy + (s * (r - 6) as f32) as i32;
        let x1 = cx + (c * r as f32) as i32;
        let y1 = cy + (s * r as f32) as i32;
        fb.line(x0, y0, x1, y1, P::TEXT);
    }

    for dy in -4i32..=4 {
        let half = match dy.abs() {
            0 | 1 => 4,
            2 => 3,
            3 => 2,
            _ => 1,
        };
        fb.hline(cx - half, cx + half, cy + dy, P::DIM);
    }
}

fn slider(fb: &mut Framebuffer, view: &View) {
    let y = L::SLIDER_Y;

    for seg in 0..(L::SLIDER_W / 4) {
        let x = L::SLIDER_X + seg * 4;
        fb.rect_fill(x, y + 1, 3, 3, P::DIM);
        fb.set(x, y, P::DIM);
    }
    let filled = (f32::from(view.volume) / 255.0 * L::SLIDER_W as f32) as i32;
    for seg in 0..(filled / 4) {
        let x = L::SLIDER_X + seg * 4;
        fb.rect_fill(x, y + 1, 3, 3, P::TEXT);
        fb.set(x, y, P::TEXT);
    }
}

fn transport(fb: &mut Framebuffer, view: &View) {
    let x = L::TRANSPORT_X;
    let (y1, y2) = (L::TRANSPORT_Y, L::TRANSPORT_ROW2_Y);

    let last = buttons()
        .iter()
        .map(|(_, (bx, _, bw, _))| bx + bw)
        .max()
        .unwrap_or(x);
    let (w, h) = ((last + 4 - (x - 4)).min(L::FIELD_X - 6 - (x - 4)), 20);
    let (cx0, cy0) = (x - 4, y1 - 2);
    for (dx, dy, sx, sy) in [(0, 0, 1, 1), (w, 0, -1, 1), (0, h, 1, -1), (w, h, -1, -1)] {
        let px = cx0 + dx;
        let py = cy0 + dy;
        fb.hline(px, px + 3 * sx, py, P::TEXT);
        fb.vline(px, py, py + 3 * sy, P::TEXT);
    }

    let _ = (y1, y2);
    for (action, (bx, by, _, _)) in buttons() {
        let active = match action {
            Action::Play => view.playing,
            Action::Stop => !view.playing,
            _ => false,
        };
        button(fb, bx, by, action.label(), active);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Play,

    Stop,

    Pause,

    Fade,

    FastForward,

    Rewind,

    File,

    Previous,

    Next,

    List,
}

impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Action::Play => "PLAY",
            Action::Stop => "STOP",
            Action::Pause => "PAUSE",
            Action::Fade => "FADE",
            Action::FastForward => "FF",
            Action::Rewind => "REW",
            Action::File => "FILE",
            Action::Previous => "PREV",
            Action::Next => "NEXT",
            Action::List => "LIST",
        }
    }
}

pub fn buttons() -> Vec<(Action, (i32, i32, i32, i32))> {
    let mut out = Vec::new();
    let height = font::SMALL_H + 2;
    let mut pen = L::TRANSPORT_X;
    for action in [Action::Play, Action::Stop, Action::Pause, Action::Fade] {
        let w = font::width(action.label()) + 4;
        out.push((action, (pen, L::TRANSPORT_Y, w, height)));
        pen += w + 4;
    }
    let mut pen = L::TRANSPORT_X;

    for action in [
        Action::FastForward,
        Action::Rewind,
        Action::File,
        Action::Previous,
        Action::Next,
        Action::List,
    ] {
        let w = font::width(action.label()) + 4;
        out.push((action, (pen, L::TRANSPORT_ROW2_Y, w, height)));
        pen += w + 3;
    }
    out
}

pub fn strip_hit(x: i32, y: i32) -> Option<usize> {
    if !(L::KEYS_PANEL_X..L::KEYS_PANEL_X + L::KEYS_PANEL_W).contains(&x) {
        return None;
    }
    if !(0..L::KEYS_PANEL_H).contains(&y) {
        return None;
    }
    let index = (y / L::STRIP_PITCH) as usize;
    let within = y % L::STRIP_PITCH;
    (within < L::KBD_Y && index < L::STRIP_COUNT).then_some(index)
}

pub fn loop_field_hit(x: i32, y: i32) -> bool {
    let top = L::field_top(L::LOOP_FIELD_INDEX);
    (L::FIELD_X..=L::FIELD_VALUE_R).contains(&x) && (top..top + L::FIELD_H).contains(&y)
}

pub fn mode_hit(x: i32, y: i32) -> bool {
    (L::MODE_X - 50..L::MODE_X + L::MODE_W).contains(&x)
        && (L::MODE_Y - 2..L::MODE_Y + font::SMALL_H + 2).contains(&y)
}

pub fn list_row_hit(x: i32, y: i32) -> Option<usize> {
    if !(0..L::SCREEN_W).contains(&x) {
        return None;
    }
    let first = L::LIST_ROW_Y;
    if y < first {
        return None;
    }
    let row = ((y - first) / L::LIST_ROW_PITCH) as usize;
    (row < L::LIST_ROWS).then_some(row)
}

pub fn slider_hit(x: i32, y: i32) -> Option<f32> {
    let inside = (L::SLIDER_X..L::SLIDER_X + L::SLIDER_W).contains(&x)
        && (L::SLIDER_Y - 2..L::SLIDER_Y + 5).contains(&y);
    inside.then(|| ((x - L::SLIDER_X) as f32 / L::SLIDER_W as f32).clamp(0.0, 1.0))
}

pub fn hit_test(x: i32, y: i32) -> Option<Action> {
    buttons()
        .into_iter()
        .find_map(|(action, (bx, by, bw, bh))| {
            ((bx..bx + bw).contains(&x) && (by - 1..by - 1 + bh).contains(&y)).then_some(action)
        })
}

fn button(fb: &mut Framebuffer, x: i32, y: i32, label: &str, active: bool) -> i32 {
    let w = font::width(label) + 4;
    if active {
        fb.rect_fill(x, y - 1, w, font::SMALL_H + 2, P::TEXT);
        font::draw(fb, x + 2, y, label, P::BLACK);
    } else {
        fb.rect_fill(x, y + 1, 2, 3, P::TEXT);
        font::draw(fb, x + 3, y, label, P::TEXT);
    }
    x + w
}

fn stacked_field(fb: &mut Framebuffer, y: i32, line1: &str, line2: &str, value: &str) {
    fb.rect_fill(L::FIELD_X, y, L::FIELD_TICK_W, L::FIELD_H, P::TEXT);
    let text_x = L::FIELD_X + L::FIELD_TICK_W + 2;
    font::draw(fb, text_x, y, line1, P::TEXT);

    draw_right(fb, L::FIELD_LABEL_R, y + 7, line2, P::TEXT);
    font::draw_tall_right(fb, L::FIELD_VALUE_R, y + 2, value, P::TEXT);
}

fn fields(fb: &mut Framebuffer, view: &View) {
    let rows: [(&str, &str, String); L::FIELD_COUNT] = [
        ("PASSED", "TIME、", format_time(view.elapsed)),
        (
            "CLOCK",
            "COUNT、",
            format!("{:08}", view.clock % 100_000_000),
        ),
        ("TIMER", "CYCLE、", format!("{:03}", view.clock % 1000)),
        ("LOOP", "COUNT、", view.loop_field.clone()),
        ("VOLUME", "DOWN、", format!("{:03}", view.volume)),
        (
            "PGM",
            "NUMBER、",
            format!("{:04}", voice_number(view.snapshots[0])),
        ),
    ];
    for (index, (line1, line2, value)) in rows.iter().enumerate() {
        stacked_field(fb, L::field_top(index), line1, line2, value);
    }
}

fn cpu_field(fb: &mut Framebuffer, view: &View) {
    let y = L::CPU_Y;
    fb.rect_fill(L::RIGHT_X + 2, y, L::FIELD_TICK_W, L::FIELD_H, P::TEXT);
    font::draw(fb, L::RIGHT_X + 9, y, "CPU POWER", P::TEXT);
    draw_right(fb, L::RIGHT_X + 54, y + 7, "COUNT、", P::TEXT);
    font::draw_tall(fb, L::RIGHT_X + 60, y + 2, view.cpu, P::TEXT);
}

fn scale_gutter(fb: &mut Framebuffer, top: i32, height: i32, marks: &[(i32, &str)], sens: i32) {
    for index in 0..L::AXIS_SEG_COUNT {
        let y = top + L::AXIS_SEG_TOP + index * L::AXIS_SEG_PITCH;

        let colour = if index == sens { P::TEXT } else { P::DIM };
        fb.rect_fill(L::AXIS_SEG_X, y, L::AXIS_SEG_W, L::AXIS_SEG_H, colour);
    }
    for (offset, label) in marks {
        let y = top + offset;
        draw_right(fb, L::AXIS_LABEL_R, y, label, P::TEXT);

        fb.hline(L::AXIS_LABEL_R - 1, L::AXIS_RULE_X - 1, y + 2, P::TEXT);
    }
    fb.rect_fill(L::AXIS_RULE_X, top, L::AXIS_RULE_W, height, P::TEXT);
}

const SENS_SEGMENT: i32 = 4;

fn spectrum(fb: &mut Framebuffer, view: &View) {
    draw_right(fb, L::AXIS_LABEL_R, L::SPECTRUM_LABEL_Y, "dB", P::TEXT);
    draw_right(
        fb,
        L::SCREEN_W - 4,
        L::SPECTRUM_LABEL_Y,
        "SPECTRUM ANALYZER",
        P::TEXT,
    );
    font::draw(fb, L::AXIS_X, L::SPECTRUM_Y, "MAX.", P::TEXT);
    font::draw(
        fb,
        L::AXIS_X,
        L::SPECTRUM_Y + L::AXIS_BOTTOM_LABEL,
        "SENS",
        P::TEXT,
    );
    draw_right(fb, L::AXIS_LABEL_R, L::SPECTRUM_RULER_Y, "TUNE", P::TEXT);
    scale_gutter(
        fb,
        L::SPECTRUM_Y,
        L::SPECTRUM_H,
        &[(0, "63"), (L::AXIS_BOTTOM_LABEL, "0")],
        SENS_SEGMENT,
    );

    let rows = L::SPECTRUM_ROWS;
    for col in 0..L::SPECTRUM_COLUMNS {
        let x = L::SPECTRUM_X + col as i32 * L::SPECTRUM_COL_STEP;

        let source = if view.spectrum.len() == L::SPECTRUM_COLUMNS {
            col
        } else {
            col * view.spectrum.len() / L::SPECTRUM_COLUMNS
        };
        let magnitude = view
            .spectrum
            .get(source)
            .copied()
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        let lit_rows = (magnitude * rows as f32).round() as i32;
        for row in 0..rows {
            let y = L::SPECTRUM_Y + L::SPECTRUM_H - (row + 1) * L::SPECTRUM_ROW_STEP;
            let colour = if row < lit_rows { P::TEXT } else { P::DIM };

            fb.rect_fill(x, y, L::SPECTRUM_DOT_W, 1, colour);
        }

        let peak = view
            .peaks
            .get(source)
            .copied()
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        if peak > 0.0 {
            let peak_row = ((peak * rows as f32).round() as i32).clamp(1, rows) - 1;
            let py = L::SPECTRUM_Y + L::SPECTRUM_H - (peak_row + 1) * L::SPECTRUM_ROW_STEP;
            fb.rect_fill(x, py, L::SPECTRUM_DOT_W, 1, P::LIGHT);
        }
    }

    let ruler_y = L::SPECTRUM_RULER_Y + 2;
    for x in (L::SPECTRUM_X..L::SPECTRUM_X + L::SPECTRUM_W).step_by(2) {
        fb.set(x, ruler_y, P::DIM);
    }
    for (label, hertz) in [
        ("250", 250.0),
        ("500", 500.0),
        ("1K", 1_000.0),
        ("2K", 2_000.0),
        ("4K", 4_000.0),
    ] {
        let fraction = SpectrumAnalyzer::fraction_of_hz(hertz, view.analyser_rate);
        let centre = L::SPECTRUM_X + (L::SPECTRUM_W as f32 * fraction) as i32;
        let x = centre - font::width(label) / 2;
        fb.rect_fill(x - 1, ruler_y, font::width(label) + 2, 1, P::BLACK);
        font::draw(fb, x, L::SPECTRUM_RULER_Y, label, P::TEXT);
    }
}

const RHYTHM_NAMES: [&str; 6] = ["BD", "SD", "TOP", "HH", "TOM", "RIM"];

fn level_analyzer(fb: &mut Framebuffer, view: &View) {
    for (label, column) in [
        ("FM1", 0),
        ("FM4", 3),
        ("SSG", 6),
        ("RHY", 9),
        ("ADP", levels::ADPCM_COLUMN),
        ("Ex", levels::EXTENSION_COLUMN),
    ] {
        font::draw(
            fb,
            L::level_column_x(column),
            L::LEVEL_LABEL_Y,
            label,
            P::TEXT,
        );
    }
    draw_right(fb, L::AXIS_LABEL_R, L::LEVEL_LABEL_Y, "Lv", P::TEXT);
    font::draw(fb, L::AXIS_X, L::LEVEL_Y, "MAX.", P::TEXT);
    font::draw(
        fb,
        L::AXIS_X,
        L::LEVEL_Y + L::AXIS_BOTTOM_LABEL,
        "SENS",
        P::TEXT,
    );
    let marks: Vec<(i32, &str)> = L::LEVEL_MARK_Y
        .iter()
        .copied()
        .zip(["15", "12", "8", "4", "0"])
        .collect();
    scale_gutter(fb, L::LEVEL_Y, L::LEVEL_H, &marks, SENS_SEGMENT);

    let rows = L::LEVEL_ROWS;
    for column in 0..L::LEVEL_COUNT {
        let x = L::level_column_x(column);
        let level = view
            .levels
            .get(column)
            .copied()
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        let lit = (level * rows as f32).round() as i32;
        for row in 0..rows {
            let y = L::LEVEL_Y + L::LEVEL_H - (row + 1) * L::LEVEL_ROW_STEP;
            let colour = if row < lit { P::TEXT } else { P::DIM };
            fb.rect_fill(x, y, L::LEVEL_CELL_W, 1, colour);
        }
        let peak = view
            .level_peaks
            .get(column)
            .copied()
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        if peak > 0.0 {
            let peak_row = ((peak * rows as f32).round() as i32).clamp(1, rows) - 1;
            let y = L::LEVEL_Y + L::LEVEL_H - (peak_row + 1) * L::LEVEL_ROW_STEP;
            fb.rect_fill(x, y, L::LEVEL_CELL_W, 1, P::LIGHT);
        }

        pan_dial(fb, x, column, view);
        program_and_keycode(fb, x, column, view);
    }
    font::draw(fb, L::AXIS_X, L::ONOFF_Y, "ON/OFF", P::TEXT);
    font::draw(fb, L::AXIS_X, L::PANPOT_Y, "PANPOT", P::TEXT);
    font::draw(fb, L::AXIS_X, L::PROGRAM_Y, "PROGRAM", P::TEXT);
    font::draw(fb, L::AXIS_X, L::KEYCODE_Y, "KEYCODE", P::TEXT);
}

fn pan_dial(fb: &mut Framebuffer, x: i32, column: usize, view: &View) {
    let cx = x + L::LEVEL_CELL_W / 2;
    let cy = L::DIAL_ROW_CY;
    circle_outline(fb, cx, cy, L::DIAL_ROW_R, P::TEXT);
    let Some(channel) = levels::column_channel(column) else {
        return;
    };
    let pan = f32::from(view.snapshots[channel].pan) / 255.0 - 0.5;
    if pan.abs() < 0.05 {
        return;
    }
    let angle = std::f32::consts::PI * pan;
    let (s, c) = angle.sin_cos();
    fb.line(
        cx,
        cy,
        cx + (s * 6.0) as i32,
        cy - (c * 6.0) as i32,
        P::TEXT,
    );
}

fn program_and_keycode(fb: &mut Framebuffer, x: i32, column: usize, view: &View) {
    let (program, keycode) = match levels::column_channel(column) {
        _ if (levels::RHYTHM_FIRST..levels::ADPCM_COLUMN).contains(&column) => (
            String::from(RHYTHM_NAMES[column - levels::RHYTHM_FIRST]),
            String::from("---"),
        ),
        Some(channel) => {
            let snapshot = view.snapshots[channel];
            let keycode = if snapshot.active {
                format!("{:03}", snapshot.pitch % 1000)
            } else {
                String::from("---")
            };
            (format!("{:03}", voice_number(snapshot)), keycode)
        }
        None => (String::from("000"), String::from("---")),
    };
    font::draw(fb, x, L::PROGRAM_Y, &program, P::TEXT);
    font::draw(fb, x, L::KEYCODE_Y, &keycode, P::TEXT);
}

fn circle_outline(fb: &mut Framebuffer, cx: i32, cy: i32, r: i32, idx: u8) {
    let (mut x, mut y, mut err) = (r, 0, 1 - r);
    while x >= y {
        for (px, py) in [
            (cx + x, cy + y),
            (cx + y, cy + x),
            (cx - y, cy + x),
            (cx - x, cy + y),
            (cx - x, cy - y),
            (cx - y, cy - x),
            (cx + y, cy - x),
            (cx + x, cy - y),
        ] {
            fb.set(px, py, idx);
        }
        y += 1;
        if err < 0 {
            err += 2 * y + 1;
        } else {
            x -= 1;
            err += 2 * (y - x) + 1;
        }
    }
}

fn status_line(fb: &mut Framebuffer, view: &View) {
    let y = L::STATUS_Y;
    if view.playing {
        font::draw_playing(fb, 0, y, P::TEXT);
    } else {
        font::draw(fb, 0, y + 2, "STOPPED.....", P::TEXT);
    }

    fb.rect_fill(L::MUSIC_TICK_X, y, 3, 9, P::TEXT);
    font::draw(fb, L::MUSIC_TICK_X + 4, y + 2, "MUSIC FILE、", P::TEXT);
    fb.rect_fill(L::MUSIC_NAME_X - 5, y, 3, 9, P::TEXT);
    if let Some(name) = view.file_name {
        font::draw(fb, L::MUSIC_NAME_X, y + 2, name, P::TEXT);
    }

    fb.rect_fill(L::PCM1_X, y, 3, 9, P::TEXT);
    font::draw(fb, L::PCM1_X + 4, y + 2, "PCM1、", P::TEXT);
    fb.rect_fill(L::PCM2_X, y, 3, 9, P::TEXT);
    font::draw(fb, L::PCM2_X + 4, y + 2, "PCM2、", P::TEXT);

    fb.rect_fill(0, L::RULE_Y, L::SCREEN_W, L::RULE_H, P::TEXT);
}

fn playlist_panel(fb: &mut Framebuffer, list: &ListView, mode: &str, message: &str) {
    fb.rect_checker(0, L::BAND_Y, L::SCREEN_W, L::BAND_H, P::DIM);
    font::draw(fb, L::LIST_INDEX_X, L::BAND_Y + 2, "MUSIC LIST、", P::TEXT);
    play_mode_caption(fb, mode);
    message_line(fb, message);
    font::draw(
        fb,
        L::LIST_NAME_X + 60,
        L::BAND_Y + 2,
        &format!("{:03} FILES", list.entries.len().min(999)),
        P::TEXT,
    );

    let first = list
        .cursor
        .saturating_sub(L::LIST_ROWS - 1)
        .min(list.entries.len().saturating_sub(L::LIST_ROWS));
    for row in 0..L::LIST_ROWS {
        let Some(index) = first.checked_add(row) else {
            break;
        };
        let Some(entry) = list.entries.get(index) else {
            break;
        };
        let y = L::LIST_ROW_Y + row as i32 * L::LIST_ROW_PITCH;
        let selected = index == list.cursor;
        if selected {
            fb.rect_fill(0, y - 1, L::SCREEN_W, L::LIST_ROW_PITCH, P::TEXT);
        }
        let ink = if selected { P::BLACK } else { P::TEXT };
        font::draw(fb, L::LIST_INDEX_X, y, &format!("{:03}", index + 1), ink);

        if list.playing == Some(index) {
            font::draw(fb, L::LIST_MARK_X, y, ">", ink);
        }
        font::draw(fb, L::LIST_NAME_X, y, &entry.name, ink);
        draw_right(fb, L::SCREEN_W - 4, y, &entry.duration_label(), ink);
    }

    if list.entries.is_empty() {
        font::draw(
            fb,
            L::LIST_NAME_X,
            L::LIST_ROW_Y,
            "NO FILES LOADED. PRESS FILE OR DROP FILES HERE.",
            P::TEXT,
        );
    }
}

fn metadata_band(fb: &mut Framebuffer, view: &View) {
    fb.rect_checker(0, L::BAND_Y, L::SCREEN_W, L::BAND_H, P::DIM);

    let rows = [
        ("MUSIC TITLE、", view.title),
        ("COMPOSER、", view.composer),
        ("MEMO、", view.memo),
    ];
    for (index, (label, value)) in rows.iter().enumerate() {
        let y = L::BAND_ROW_Y + index as i32 * L::BAND_ROW_PITCH;
        draw_right(fb, L::BAND_LABEL_R, y + 4, label, P::TEXT);

        let right = if index == 0 {
            L::BAND_MESSAGE_R - font::width("STATUS、") - 4
        } else {
            L::SCREEN_W
        };
        font::draw_kanji_within(fb, L::BAND_VALUE_X, y, value, P::LIGHT, right);
    }
    let arranger_y = L::BAND_ROW_Y + L::BAND_ROW_PITCH;
    draw_right(
        fb,
        L::BAND_ARRANGER_LABEL_R,
        arranger_y + 4,
        "ARRANGER、",
        P::TEXT,
    );
    font::draw_kanji(
        fb,
        L::BAND_ARRANGER_VALUE_X,
        arranger_y,
        view.arranger,
        P::LIGHT,
    );
    play_mode_caption(fb, view.mode);
    message_line(fb, view.message);
}

fn message_line(fb: &mut Framebuffer, message: &str) {
    if message.is_empty() {
        return;
    }

    let fits = (L::BAND_MESSAGE_W / font::SMALL_W) as usize;
    let shown: String = message.chars().take(fits).collect();
    let left = L::BAND_MESSAGE_R - font::width(&shown);
    draw_right(fb, left - 4, L::MODE_Y + 8, "STATUS、", P::TEXT);
    font::draw(fb, left, L::MODE_Y + 8, &shown, P::LIGHT);
}

fn play_mode_caption(fb: &mut Framebuffer, mode: &str) {
    let y = L::MODE_Y;
    fb.rect_fill(L::MODE_X - 5, y - 1, 3, 7, P::TEXT);
    draw_right(fb, L::MODE_X - 8, y, "PLAY MODE、", P::TEXT);
    font::draw(fb, L::MODE_X, y, mode, P::LIGHT);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmd_core::ChannelSnapshot;

    fn blank_view<'a>(snapshots: &'a [ChannelSnapshot; CHANNEL_COUNT]) -> View<'a> {
        View {
            snapshots,
            spectrum: &[],
            peaks: &[],
            levels: &[],
            level_peaks: &[],
            file_name: None,
            title: "",
            composer: "",
            arranger: "",
            memo: "",
            muted: &[],
            list: None,
            mode: "SINGLE",
            message: "",
            playing: false,
            elapsed: 0.0,
            clock: 0,
            loop_field: format_loops(0, None),
            volume: 0,
            cpu: crate::cpu::MEASURING,
            analyser_rate: 22_050.0,
        }
    }

    #[test]
    fn a_full_render_touches_no_colour_outside_the_measured_palette() {
        let snapshots = [ChannelSnapshot::default(); CHANNEL_COUNT];
        let mut fb = Framebuffer::new();
        render(&mut fb, &blank_view(&snapshots));

        assert!(
            fb.indices().iter().all(|&i| i <= P::LIGHT),
            "render used an unassigned palette slot"
        );
    }

    #[test]
    fn rendering_is_deterministic_for_identical_input() {
        let snapshots = [ChannelSnapshot::default(); CHANNEL_COUNT];
        let mut a = Framebuffer::new();
        let mut b = Framebuffer::new();
        render(&mut a, &blank_view(&snapshots));
        render(&mut b, &blank_view(&snapshots));
        assert!(a == b, "the same view produced two different frames");
    }

    #[test]
    fn the_metadata_band_is_dithered_rather_than_filled() {
        let snapshots = [ChannelSnapshot::default(); CHANNEL_COUNT];
        let mut fb = Framebuffer::new();
        render(&mut fb, &blank_view(&snapshots));

        let y = L::BAND_Y + 2;
        let a = fb.get(10, y);
        let b = fb.get(11, y);
        assert_ne!(a, b, "the band is a flat fill, not a checker");
    }

    #[test]
    fn note_names_follow_the_reference_spelling() {
        assert_eq!(key_name(NO_NOTE), "--");
        assert_eq!(key_name(0x6f), "S");
        assert_eq!(key_name(0x60), "o6C");
        assert_eq!(key_name(0x6b), "o6B");
    }

    #[test]
    fn the_keyboard_covers_every_semitone_without_overlapping_keys() {
        let mut seen = std::collections::BTreeSet::new();
        for semitone in 0..12 {
            seen.insert(semitone_to_key(semitone));
        }
        assert_eq!(seen.len(), 12, "two semitones collapsed onto one key");
        assert_eq!(semitone_to_key(0), (0, false));
        assert_eq!(semitone_to_key(1), (0, true));
        assert_eq!(semitone_to_key(12), (7, false));
    }

    #[test]
    fn elapsed_time_uses_the_reference_format() {
        assert_eq!(format_time(0.0), "00:00.00");
        assert_eq!(format_time(65.5), "01:05.50");
    }

    #[test]
    fn no_point_on_screen_belongs_to_two_controls() {
        for y in (0..L::SCREEN_H).step_by(3) {
            for x in (0..L::SCREEN_W).step_by(3) {
                let claims = [
                    hit_test(x, y).is_some(),
                    slider_hit(x, y).is_some(),
                    loop_field_hit(x, y),
                    mode_hit(x, y),
                    strip_hit(x, y).is_some(),
                    list_row_hit(x, y).is_some(),
                ]
                .into_iter()
                .filter(|claimed| *claimed)
                .count();
                assert!(
                    claims <= 1,
                    "({x}, {y}) is claimed by {claims} controls at once"
                );
            }
        }
    }

    #[test]
    fn every_transport_button_is_reachable_at_its_own_centre() {
        for (action, (x, y, w, h)) in buttons() {
            let hit = hit_test(x + w / 2, y + h / 2 - 1);
            assert_eq!(hit, Some(action), "{action:?} is not clickable");
        }
    }

    #[test]
    fn the_strip_hit_covers_headers_and_stops_at_the_keyboard() {
        assert_eq!(strip_hit(10, 0), Some(0));
        assert_eq!(strip_hit(10, L::KBD_Y - 1), Some(0));
        assert_eq!(strip_hit(10, L::KBD_Y), None, "the keyboard is not a mute");
        assert_eq!(strip_hit(10, L::strip_top(9)), Some(9));
        assert_eq!(strip_hit(10, L::KEYS_PANEL_H), None);
        assert_eq!(
            strip_hit(L::RIGHT_X, 0),
            None,
            "the right panel is not a strip"
        );
    }

    #[test]
    fn the_loop_field_click_target_is_the_field_it_names() {
        let top = L::field_top(L::LOOP_FIELD_INDEX);
        assert!(loop_field_hit(L::FIELD_X + 2, top + 2));
        assert!(loop_field_hit(L::FIELD_VALUE_R - 1, top + L::FIELD_H - 1));
        assert!(!loop_field_hit(L::FIELD_X - 1, top + 2));
        assert!(!loop_field_hit(L::FIELD_X + 2, top - 1));
        assert!(!loop_field_hit(L::FIELD_X + 2, top + L::FIELD_H));
    }

    #[test]
    fn list_rows_map_to_consecutive_offsets_inside_the_panel() {
        assert_eq!(list_row_hit(100, L::LIST_ROW_Y), Some(0));
        assert_eq!(
            list_row_hit(100, L::LIST_ROW_Y + L::LIST_ROW_PITCH),
            Some(1)
        );
        assert_eq!(list_row_hit(100, L::LIST_ROW_Y - 1), None);
        let past = L::LIST_ROW_Y + L::LIST_ROWS as i32 * L::LIST_ROW_PITCH;
        assert_eq!(list_row_hit(100, past), None);
    }

    #[test]
    fn the_loop_field_prints_the_reference_format_until_a_cap_is_set() {
        assert_eq!(format_loops(0, None), "0000");
        assert_eq!(format_loops(1234, None), "1234");
        assert_eq!(format_loops(2, Some(3)), "02-03");
        assert_eq!(format_loops(500, Some(200)), "99-99", "fields saturate");
    }

    #[test]
    fn a_muted_strip_shows_no_note_and_recedes() {
        let mut snapshots = [ChannelSnapshot::default(); CHANNEL_COUNT];
        snapshots[0] = ChannelSnapshot {
            active: true,
            note: 0x50,
            level: 200,
            ..ChannelSnapshot::default()
        };
        let mut lit = Framebuffer::new();
        let mut muted = Framebuffer::new();
        strip(&mut lit, 0, snapshots[0], false);
        strip(&mut muted, 0, snapshots[0], true);
        assert!(lit != muted, "a mute has to be visible");

        let green = |fb: &Framebuffer| {
            (0..L::STRIP_PITCH)
                .flat_map(|y| (0..L::KEYS_PANEL_W).map(move |x| (x, y)))
                .filter(|(x, y)| fb.get(*x, *y) == P::NOTE)
                .count()
        };
        assert!(green(&lit) > 0, "an audible note lights a key");
        assert_eq!(green(&muted), 0, "a muted strip must show no notes");
    }

    #[test]
    fn a_field_draws_an_open_l_and_never_a_closed_box() {
        let mut fb = Framebuffer::new();
        field(&mut fb, 10, 10, 40, 10, "X", P::TEXT);

        assert_eq!(fb.get(10, 12), P::TEXT);
        assert_eq!(fb.get(30, 19), P::TEXT);

        assert_eq!(fb.get(30, 10), P::BLACK, "a top edge would close the frame");
        assert_eq!(
            fb.get(49, 14),
            P::BLACK,
            "a right edge would close the frame"
        );
    }
}
