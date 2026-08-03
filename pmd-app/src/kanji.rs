// SPDX-License-Identifier: AGPL-3.0-only

pub const CELL: i32 = 16;

pub const HALF: i32 = 8;

pub type Cell = [u16; CELL as usize];

const MAGIC: &[u8; 8] = b"PMDK16\0\0";
const VERSION: u32 = 1;
const HEADER_BYTES: usize = 20;
const INDEX_ENTRY_BYTES: usize = 8;
const GLYPH_BYTES: usize = 32;

static FONT_BYTES: &[u8] = include_bytes!("../assets/shinonome-k16.bin");

pub fn is_full_width(ch: char) -> bool {
    !matches!(ch, '\u{20}'..='\u{7e}' | '\u{ff61}'..='\u{ff9f}')
}

pub fn advance(ch: char) -> i32 {
    if is_full_width(ch) {
        CELL
    } else {
        HALF
    }
}

pub fn width(text: &str) -> i32 {
    text.chars().map(advance).sum()
}

pub fn cell(ch: char) -> Option<Cell> {
    let count = table_count()?;
    let glyph_offset = table_glyph_offset()?;
    let index_end = HEADER_BYTES.checked_add(count.checked_mul(INDEX_ENTRY_BYTES)?)?;
    if index_end > FONT_BYTES.len() || glyph_offset < index_end {
        return None;
    }

    let codepoint = ch as u32;
    let mut low = 0usize;
    let mut high = count;
    while low < high {
        let middle = low + (high - low) / 2;
        let entry = HEADER_BYTES.checked_add(middle.checked_mul(INDEX_ENTRY_BYTES)?)?;
        let found = read_u32(entry)?;
        match found.cmp(&codepoint) {
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Greater => high = middle,
            std::cmp::Ordering::Equal => {
                let glyph_index = read_u32(entry + 4)? as usize;
                if glyph_index >= count {
                    return None;
                }
                let offset = glyph_offset.checked_add(glyph_index.checked_mul(GLYPH_BYTES)?)?;
                let end = offset.checked_add(GLYPH_BYTES)?;
                let bytes = FONT_BYTES.get(offset..end)?;
                let mut rows = [0u16; CELL as usize];
                for (row, output) in rows.iter_mut().enumerate() {
                    let start = row * 2;
                    *output = u16::from_be_bytes([bytes[start], bytes[start + 1]]);
                }
                return Some(rows);
            }
        }
    }
    None
}

fn table_count() -> Option<usize> {
    if FONT_BYTES.get(..MAGIC.len())? != MAGIC || read_u32(8)? != VERSION {
        return None;
    }
    Some(read_u32(12)? as usize)
}

fn table_glyph_offset() -> Option<usize> {
    Some(read_u32(16)? as usize)
}

fn read_u32(offset: usize) -> Option<u32> {
    let bytes = FONT_BYTES.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_width_follows_the_jis_rule_rather_than_the_glyph_shape() {
        assert!(!is_full_width('A'));
        assert!(!is_full_width('1'));
        assert!(!is_full_width(' '));
        assert!(is_full_width('東'));
        assert!(is_full_width('ヴ'));
        assert!(is_full_width('Ｚ'), "fullwidth latin is a full cell");
        assert!(!is_full_width('ｱ'), "halfwidth katakana is a half cell");
    }

    #[test]
    fn a_mixed_run_measures_as_the_sum_of_its_cells() {
        assert_eq!(width("ZUN"), 24);
        assert_eq!(width("東方"), 32);
        assert_eq!(width("東方1969"), 32 + 32);
    }

    #[test]
    fn an_empty_run_has_no_width() {
        assert_eq!(width(""), 0);
    }

    #[test]
    fn the_embedded_table_covers_metadata_script_examples() {
        assert!(cell('A').is_some());
        assert!(cell('東').is_some());
        assert!(cell('怪').is_some());
        assert!(cell('ｱ').is_some());
        assert!(cell('～').is_some());
        assert!(cell('😀').is_none());
    }
}
