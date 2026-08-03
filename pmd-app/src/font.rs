// SPDX-License-Identifier: AGPL-3.0-only

use crate::fb::Framebuffer;
use crate::kanji;

pub const SMALL_W: i32 = 5;

pub const SMALL_H: i32 = 5;

pub const BEARING_X: i32 = 1;

pub const BEARING_Y: i32 = 1;

pub const TALL_W: i32 = 8;

pub const TALL_H: i32 = 11;

pub const TALL_BEARING_X: i32 = -2;

const fn row(pattern: &[u8]) -> u8 {
    let mut bits = 0u8;
    let mut i = 0;
    while i < pattern.len() {
        if pattern[i] == b'#' {
            bits |= 0x80 >> i;
        }
        i += 1;
    }
    bits
}

type Small = [u8; SMALL_H as usize];

type Tall = [u8; TALL_H as usize];

const fn g5(p: [&[u8]; 5]) -> Small {
    [row(p[0]), row(p[1]), row(p[2]), row(p[3]), row(p[4])]
}

const fn g8(p: [&[u8]; 11]) -> Tall {
    [
        row(p[0]),
        row(p[1]),
        row(p[2]),
        row(p[3]),
        row(p[4]),
        row(p[5]),
        row(p[6]),
        row(p[7]),
        row(p[8]),
        row(p[9]),
        row(p[10]),
    ]
}

const BLANK: Small = g5([b"....", b"....", b"....", b"....", b"...."]);

#[rustfmt::skip]
const LETTERS: [Small; 26] = [
    g5([b".##.", b"#..#", b"####", b"#..#", b"#..#"]),
    g5([b"###.", b"#..#", b"###.", b"#..#", b"###."]),
    g5([b".###", b"#...", b"#...", b"#...", b".###"]),
    g5([b"###.", b"#..#", b"#..#", b"#..#", b"###."]),
    g5([b"####", b"#...", b"###.", b"#...", b"####"]),
    g5([b"####", b"#...", b"###.", b"#...", b"#..."]),
    g5([b".###", b"#...", b"#.##", b"#..#", b".###"]),
    g5([b"#..#", b"#..#", b"####", b"#..#", b"#..#"]),
    g5([b"###.", b".#..", b".#..", b".#..", b"###."]),
    g5([b"..##", b"...#", b"...#", b"#..#", b".##."]),
    g5([b"#..#", b"#.#.", b"##..", b"#.#.", b"#..#"]),
    g5([b"#...", b"#...", b"#...", b"#...", b"####"]),
    g5([b"#..#", b"####", b"####", b"#..#", b"#..#"]),
    g5([b"#..#", b"##.#", b"#.##", b"#..#", b"#..#"]),
    g5([b".##.", b"#..#", b"#..#", b"#..#", b".##."]),
    g5([b"###.", b"#..#", b"###.", b"#...", b"#..."]),
    g5([b".##.", b"#..#", b"#..#", b"#.#.", b".#.#"]),
    g5([b"###.", b"#..#", b"###.", b"#.#.", b"#..#"]),
    g5([b".###", b"#...", b".##.", b"...#", b"###."]),
    g5([b"####", b".#..", b".#..", b".#..", b".#.."]),
    g5([b"#..#", b"#..#", b"#..#", b"#..#", b".##."]),
    g5([b"#..#", b"#..#", b"#..#", b".##.", b".##."]),
    g5([b"#..#", b"#..#", b"####", b"####", b"#..#"]),
    g5([b"#..#", b".##.", b".##.", b".##.", b"#..#"]),
    g5([b"#..#", b"#..#", b".##.", b".#..", b".#.."]),
    g5([b"####", b"...#", b".##.", b"#...", b"####"]),
];

#[rustfmt::skip]
const DIGITS: [Small; 10] = [
    g5([b".##.", b"#..#", b"#..#", b"#..#", b".##."]),
    g5([b".#..", b"##..", b".#..", b".#..", b"###."]),
    g5([b"###.", b"...#", b".##.", b"#...", b"####"]),
    g5([b"###.", b"...#", b".##.", b"...#", b"###."]),
    g5([b"#..#", b"#..#", b"####", b"...#", b"...#"]),
    g5([b"####", b"#...", b"###.", b"...#", b"###."]),
    g5([b".###", b"#...", b"###.", b"#..#", b".##."]),
    g5([b"####", b"...#", b"..#.", b".#..", b".#.."]),
    g5([b".##.", b"#..#", b".##.", b"#..#", b".##."]),
    g5([b".##.", b"#..#", b".###", b"...#", b"###."]),
];

#[rustfmt::skip]
const PUNCT: [(u8, Small); 15] = [
    (b'.', g5([b"....", b"....", b"....", b"....", b".#.."])),
    (b',', g5([b"....", b"....", b"....", b".#..", b"#..."])),
    (b'\'', g5([b"....", b"....", b"....", b".#..", b"#..."])),
    (b':', g5([b"....", b".#..", b"....", b".#..", b"...."])),
    (b'-', g5([b"....", b"....", b"###.", b"....", b"...."])),
    (b'+', g5([b"....", b".#..", b"###.", b".#..", b"...."])),
    (b'/', g5([b"...#", b"..#.", b".#..", b"#...", b"...."])),
    (b'~', g5([b"....", b"....", b".#.#", b"#.#.", b"...."])),
    (b'(', g5([b"..#.", b".#..", b".#..", b".#..", b"..#."])),
    (b')', g5([b".#..", b"..#.", b"..#.", b"..#.", b".#.."])),
    (b'*', g5([b"....", b"#.#.", b".#..", b"#.#.", b"...."])),
    (b'%', g5([b"#..#", b"...#", b".##.", b"#...", b"#..#"])),
    (b'!', g5([b".#..", b".#..", b".#..", b"....", b".#.."])),
    (b'=', g5([b"....", b"###.", b"....", b"###.", b"...."])),
    (b'&', g5([b".##.", b"##..", b"###.", b"#.#.", b".###"])),
];

#[rustfmt::skip]
const LOWER: [Small; 26] = [
    g5([b"....", b".##.", b"#.#.", b"#.#.", b".###"]),
    g5([b"#...", b"###.", b"#..#", b"#..#", b"###."]),
    g5([b"....", b".##.", b"#...", b"#...", b".##."]),
    g5([b"...#", b".###", b"#..#", b"#..#", b".###"]),
    g5([b"....", b".##.", b"####", b"#...", b".##."]),
    g5([b"..##", b".#..", b"###.", b".#..", b".#.."]),
    g5([b"....", b".###", b"#..#", b".###", b"...#"]),
    g5([b"#...", b"###.", b"#..#", b"#..#", b"#..#"]),
    g5([b".#..", b"....", b".#..", b".#..", b".#.."]),
    g5([b"..#.", b"....", b"..#.", b"..#.", b"##.."]),
    g5([b"#...", b"#.#.", b"##..", b"#.#.", b"#..#"]),
    g5([b"##..", b".#..", b".#..", b".#..", b".##."]),
    g5([b"....", b"##..", b"####", b"#.##", b"#..#"]),
    g5([b"....", b"###.", b"#..#", b"#..#", b"#..#"]),
    g5([b"....", b".##.", b"#..#", b"#..#", b".##."]),
    g5([b"....", b"###.", b"#..#", b"###.", b"#..."]),
    g5([b"....", b".###", b"#..#", b".###", b"...#"]),
    g5([b"....", b"#.##", b"##..", b"#...", b"#..."]),
    g5([b"....", b".###", b".##.", b"...#", b"###."]),
    g5([b".#..", b"###.", b".#..", b".#..", b"..##"]),
    g5([b"....", b"#..#", b"#..#", b"#..#", b".###"]),
    g5([b"....", b"#..#", b"#..#", b".##.", b".##."]),
    g5([b"....", b"#..#", b"#.##", b"####", b"##.."]),
    g5([b"....", b"#..#", b".##.", b".##.", b"#..#"]),
    g5([b"....", b"#..#", b".###", b"...#", b"###."]),
    g5([b"....", b"####", b"..#.", b".#..", b"####"]),
];

pub fn glyph(ch: char) -> Small {
    match ch {
        'A'..='Z' => LETTERS[ch as usize - 'A' as usize],
        'a'..='z' => LOWER[ch as usize - 'a' as usize],
        '0'..='9' => DIGITS[ch as usize - '0' as usize],
        '、' => PUNCT[2].1,
        ' ' => BLANK,
        _ => {
            let byte = ch as u32;
            let mut found = BLANK;
            if byte < 128 {
                let byte = byte as u8;
                let mut i = 0;
                while i < PUNCT.len() {
                    if PUNCT[i].0 == byte {
                        found = PUNCT[i].1;
                        break;
                    }
                    i += 1;
                }
            }
            found
        }
    }
}

#[rustfmt::skip]
const TALL_DIGITS: [Tall; 10] = [

    g8([b"...####.", b"..##..##", b"..##..##", b"..##..##", b"..##..##", b"..##.##.",
        b".##..##.", b".##..##.", b".##..##.", b".######.", b".#####.."]),
    g8([b".....##.", b"....###.", b"...####.", b".....##.", b".....##.", b"....##..",
        b"....##..", b"....##..", b"....##..", b"..######", b"..######"]),
    g8([b"...####.", b"..######", b"..##..##", b"......##", b".....##.", b"...###..",
        b"..##....", b".##.....", b".##.....", b".#######", b".#######"]),
    g8([b"...####.", b"..######", b"..##..##", b"......##", b"....###.", b"...###..",
        b".....##.", b".##..##.", b".##..##.", b".######.", b".#####.."]),
    g8([b".....##.", b"....###.", b"...####.", b"..##.##.", b".##..##.", b".#######",
        b".#######", b".....##.", b".....##.", b".....##.", b".....##."]),
    g8([b"..######", b"..######", b"..##....", b"..##....", b"..#####.", b"...####.",
        b".....##.", b".##..##.", b".##..##.", b".######.", b".#####.."]),
    g8([b"....###.", b"...###..", b"..##....", b"..##....", b"..#####.", b"..######",
        b".##..##.", b".##..##.", b".##..##.", b".######.", b".#####.."]),
    g8([b"..######", b"..######", b"......##", b".....##.", b".....##.", b"....##..",
        b"....##..", b"...##...", b"...##...", b"...##...", b"..##...."]),
    g8([b"...####.", b"..######", b"..##..##", b"..##..##", b"..######", b"...###..",
        b".##..##.", b".##..##.", b".##..##.", b".######.", b".#####.."]),
    g8([b"...####.", b"..######", b"..##..##", b"..##..##", b"..######", b"...####.",
        b".....##.", b".....##.", b"....##..", b"..####..", b"..###..."]),
];

#[rustfmt::skip]
const TALL_PUNCT: [(char, Tall); 3] = [
    (':', g8([b"........", b"........", b"...##...", b"...##...", b"........", b"........",
              b"........", b"..##....", b"..##....", b"........", b"........"])),
    ('.', g8([b"........", b"........", b"........", b"........", b"........", b"........",
              b"........", b"........", b"........", b"..##....", b"..##...."])),
    ('-', g8([b"........", b"........", b"........", b"........", b"........", b"...####.",
              b"...####.", b"........", b"........", b"........", b"........"])),
];

pub fn tall_digit(ch: char) -> Option<Tall> {
    match ch {
        '0'..='9' => Some(TALL_DIGITS[ch as usize - '0' as usize]),
        _ => TALL_PUNCT
            .iter()
            .find(|(c, _)| *c == ch)
            .map(|(_, glyph)| *glyph),
    }
}

pub const LOGO_W: i32 = 104;

pub const LOGO_H: i32 = 13;

const fn logo_row(pattern: &[u8]) -> [u8; 14] {
    let mut out = [0u8; 14];
    let mut i = 0;
    while i < pattern.len() {
        if pattern[i] == b'#' {
            out[i / 8] |= 0x80 >> (i % 8);
        }
        i += 1;
    }
    out
}

#[rustfmt::skip]
const LOGO: [[u8; 14]; LOGO_H as usize] = [
    logo_row(b"###########...####...####...#########..............####...####......####.......####...####......####...."),
    logo_row(b"###########...#####.#####...###########............#####.#####......####.......#####..####......####...."),
    logo_row(b"####...####...#####.#####...####...####............#####.#####......####.......#####..####......####...."),
    logo_row(b"####...####...###########...####...####............###########......####.......######.####......####...."),
    logo_row(b"####...####...###########...####...####............###########......####.......######.####......####...."),
    logo_row(b"###########...###########...####...####............###########......####.......###########......####...."),
    logo_row(b"###########...###########...####...####............###########......####.......###########......####...."),
    logo_row(b"####..........####...####...####...####............####...####......####.......###########......####...."),
    logo_row(b"####..........####...####...####...####............####...####......####.......####.######......####...."),
    logo_row(b"####..........####...####...####...####............####...####......####.......####.######......####...."),
    logo_row(b"####..........####...####...####...####............####...####......####.......####..#####......####...."),
    logo_row(b"####..........####...####...###########............####...####......####.......####..#####......####...."),
    logo_row(b"####..........####...####...#########..............####...####......####.......####...####......####...."),
];

pub fn draw_logo(fb: &mut Framebuffer, x: i32, y: i32, idx: u8) {
    for (row, bytes) in LOGO.iter().enumerate() {
        for (chunk, bits) in bytes.iter().enumerate() {
            fb.blit_1bpp(x + chunk as i32 * 8, y + row as i32, 8, &[*bits], idx);
        }
    }
}

pub const PLAYING_W: i32 = 74;

pub const PLAYING_H: i32 = 7;

const fn playing_row(pattern: &[u8]) -> [u8; 10] {
    let mut out = [0u8; 10];
    let mut i = 0;
    while i < pattern.len() {
        if pattern[i] == b'#' {
            out[i / 8] |= 0x80 >> (i % 8);
        }
        i += 1;
    }
    out
}

#[rustfmt::skip]
const PLAYING: [[u8; 10]; PLAYING_H as usize] = [
    playing_row(b"######..##.......######..##....##.##.##....##..######....................."),
    playing_row(b"#....##.##......##....##..##..##..##.###...##.##.....#...................."),
    playing_row(b"#....##.##......##....##...####...##.####..##.##.........................."),
    playing_row(b"#....##.##......##....##....##....##.########.##...###...................."),
    playing_row(b"######..##......########....##....##.##..####.##....##...................."),
    playing_row(b"#.......##......##....##....##....##.##...###.##....##..##..##..##..##..##"),
    playing_row(b"#........######.##....##....##.....#.##....##...#####...##..##..##..##..##"),
];

pub fn draw_playing(fb: &mut Framebuffer, x: i32, y: i32, idx: u8) {
    for (row, bytes) in PLAYING.iter().enumerate() {
        for (chunk, bits) in bytes.iter().enumerate() {
            fb.blit_1bpp(x + chunk as i32 * 8, y + row as i32, 8, &[*bits], idx);
        }
    }
}

pub fn draw(fb: &mut Framebuffer, x: i32, y: i32, text: &str, idx: u8) -> i32 {
    draw_at_pitch(fb, x, y, text, idx, SMALL_W)
}

pub fn draw_fitted(fb: &mut Framebuffer, x: i32, y: i32, text: &str, idx: u8, width: i32) {
    let n = text.chars().count().max(1) as i32;
    for (i, ch) in text.chars().enumerate() {
        let pen = x + (i as i32 * width) / n;
        fb.blit_1bpp(
            pen + BEARING_X,
            y + BEARING_Y,
            SMALL_W as u32,
            &glyph(ch),
            idx,
        );
    }
}

pub fn draw_at_pitch(fb: &mut Framebuffer, x: i32, y: i32, text: &str, idx: u8, pitch: i32) -> i32 {
    let mut pen = x + BEARING_X;
    for ch in text.chars() {
        fb.blit_1bpp(pen, y + BEARING_Y, SMALL_W as u32, &glyph(ch), idx);
        pen += pitch;
    }
    x + text.chars().count() as i32 * pitch
}

pub fn width(text: &str) -> i32 {
    text.chars().count() as i32 * SMALL_W
}

pub fn draw_kanji(fb: &mut Framebuffer, x: i32, y: i32, text: &str, idx: u8) -> i32 {
    draw_kanji_within(fb, x, y, text, idx, i32::MAX)
}

pub fn draw_kanji_within(
    fb: &mut Framebuffer,
    x: i32,
    y: i32,
    text: &str,
    idx: u8,
    right: i32,
) -> i32 {
    let mut pen = x;
    for ch in text.chars() {
        if pen + kanji::advance(ch) > right {
            break;
        }
        if let Some(rows) = kanji::cell(ch) {
            fb.blit_1bpp_wide(pen, y, kanji::CELL as u32, &rows, idx);
        }
        pen += kanji::advance(ch);
    }
    pen
}

pub fn kanji_width(text: &str) -> i32 {
    kanji::width(text)
}

pub fn draw_tall(fb: &mut Framebuffer, x: i32, y: i32, text: &str, idx: u8) -> i32 {
    let mut pen = x;
    for ch in text.chars() {
        if let Some(rows) = tall_digit(ch) {
            fb.blit_1bpp(pen + TALL_BEARING_X, y, TALL_W as u32, &rows, idx);
        }
        pen += TALL_W;
    }
    pen
}

pub fn tall_width(text: &str) -> i32 {
    text.chars().count() as i32 * TALL_W
}

pub fn draw_tall_right(fb: &mut Framebuffer, right: i32, y: i32, text: &str, idx: u8) -> i32 {
    draw_tall(fb, right - tall_width(text), y, text, idx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette;

    #[test]
    fn rows_are_packed_from_the_left() {
        assert_eq!(row(b"#..."), 0b1000_0000);
        assert_eq!(row(b"...#"), 0b0001_0000);
        assert_eq!(row(b"####"), 0b1111_0000);
        assert_eq!(row(b"#######"), 0b1111_1110);
    }

    #[test]
    fn the_label_face_matches_the_measured_five_pixel_cell() {
        assert_eq!(SMALL_W, 5);
        assert_eq!(SMALL_H, 5);
        assert_eq!(width("TRACK、"), 30);
    }

    #[test]
    fn the_numeric_face_is_oblique_with_its_upper_half_shifted_right() {
        assert_eq!((TALL_W, TALL_H), (8, 11));
        let zero = tall_digit('0').expect("digit");
        assert_eq!(zero.len(), 11);

        let leftmost = |bits: u8| (0..8).find(|c| bits & (0x80 >> c) != 0);
        let upper = leftmost(zero[2]).expect("upper row has ink");
        let lower = leftmost(zero[7]).expect("lower row has ink");
        assert!(
            upper > lower,
            "upper half at column {upper} should sit right of lower half at {lower}"
        );
    }

    #[test]
    fn unmapped_characters_render_blank_rather_than_panicking() {
        assert_eq!(glyph('\u{263A}'), BLANK);
        assert_eq!(glyph(' '), BLANK);
    }

    #[test]
    fn lowercase_is_its_own_set_because_the_credit_lines_are_mixed_case() {
        assert_ne!(glyph('a'), glyph('A'));
        assert_ne!(glyph('z'), glyph('Z'));
        assert_ne!(glyph('a'), BLANK, "lowercase must render");
        assert_ne!(glyph('&'), BLANK, "& appears in both credit lines");
    }

    #[test]
    fn the_ideographic_comma_is_a_low_left_stroke() {
        let comma = glyph('、');
        assert_ne!(comma, BLANK, "、 must render; it ends every FMDSP label");

        assert_eq!(
            (comma[0], comma[1], comma[2]),
            (0, 0, 0),
            "the comma must not reach the cap line"
        );
        assert_ne!(comma[4], 0, "the comma must sit on the last rows");
    }

    #[test]
    fn drawing_advances_one_cell_per_character() {
        let mut fb = Framebuffer::new();
        let end = draw(&mut fb, 10, 0, "FM", palette::TEXT);
        assert_eq!(end, 20);
    }

    #[test]
    fn zero_is_an_open_bowl_and_cannot_be_confused_with_eight() {
        let zero = tall_digit('0').expect("digit");
        let eight = tall_digit('8').expect("digit");
        assert_ne!(zero, eight, "`0` and `8` must not be the same glyph");

        let open_interior = |glyph: &Tall| {
            (2..7).any(|col| {
                glyph[1..TALL_H as usize - 2]
                    .iter()
                    .all(|row| row & (0x80 >> col) == 0)
            })
        };
        assert!(open_interior(&zero), "`0` has no counter, so it is a block");
        assert!(!open_interior(&eight), "`8` should have a closed waist");
    }

    #[test]
    fn a_numeric_field_carries_no_second_layer_behind_its_digits() {
        let mut fb = Framebuffer::new();
        draw_tall_right(&mut fb, 4 * TALL_W, 0, "0000", palette::TEXT);

        let zero = tall_digit('0').expect("digit");
        let eight = tall_digit('8').expect("digit");
        for (row, (zero_bits, eight_bits)) in zero.iter().zip(&eight).enumerate() {
            let ghost_only = eight_bits & !zero_bits;
            for col in 0..8 {
                if ghost_only & (0x80 >> col) != 0 {
                    let x = TALL_BEARING_X + col;
                    assert_eq!(
                        fb.get(x, row as i32),
                        palette::BLACK,
                        "a segment belonging only to `8` was lit at ({x}, {row})"
                    );
                }
            }
        }
    }

    #[test]
    fn numeric_runs_anchor_on_their_right_edge() {
        let mut fb = Framebuffer::new();
        let end = draw_tall_right(&mut fb, 100, 0, "123", palette::TEXT);
        assert_eq!(end, 100);
        assert_eq!(tall_width("123"), 24);
    }
}
