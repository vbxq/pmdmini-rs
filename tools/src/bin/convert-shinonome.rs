// SPDX-License-Identifier: AGPL-3.0-only

use encoding_rs::EUC_JP;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;
use std::process;

const MAGIC: &[u8; 8] = b"PMDK16\0\0";
const VERSION: u32 = 1;
const HEADER_BYTES: usize = 20;
const INDEX_ENTRY_BYTES: usize = 8;
const GLYPH_BYTES: usize = 32;

type Rows = [u16; 16];

fn main() {
    if let Err(message) = run() {
        eprintln!("convert-shinonome: {message}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    if arguments.len() != 3 {
        return Err("usage: convert-shinonome JIS0208_BDF JIS0201_BDF OUTPUT_BIN".to_owned());
    }

    let jis0208 = read_bdf(Path::new(&arguments[0]))?;
    let jis0201 = read_bdf(Path::new(&arguments[1]))?;
    let mut glyphs = BTreeMap::<u32, Rows>::new();

    for (encoding, rows) in jis0208 {
        let Some(codepoint) = jis0208_codepoint(encoding)? else {
            continue;
        };
        glyphs.entry(codepoint).or_insert(rows);
    }
    for (encoding, rows) in jis0201 {
        let Some(codepoint) = jis0201_codepoint(encoding) else {
            continue;
        };
        glyphs.entry(codepoint).or_insert(rows);
    }

    if glyphs.is_empty() {
        return Err("the input BDF files contained no Unicode glyphs".to_owned());
    }
    let index_bytes = glyphs
        .len()
        .checked_mul(INDEX_ENTRY_BYTES)
        .ok_or_else(|| "glyph index is too large".to_owned())?;
    let glyph_offset = HEADER_BYTES
        .checked_add(index_bytes)
        .ok_or_else(|| "font table is too large".to_owned())?;
    let total_bytes = glyph_offset
        .checked_add(
            glyphs
                .len()
                .checked_mul(GLYPH_BYTES)
                .ok_or_else(|| "glyph table is too large".to_owned())?,
        )
        .ok_or_else(|| "font table is too large".to_owned())?;

    let mut output = Vec::with_capacity(total_bytes);
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&VERSION.to_le_bytes());
    output.extend_from_slice(&(glyphs.len() as u32).to_le_bytes());
    output.extend_from_slice(&(glyph_offset as u32).to_le_bytes());

    for (index, codepoint) in glyphs.keys().enumerate() {
        output.extend_from_slice(&codepoint.to_le_bytes());
        output.extend_from_slice(&(index as u32).to_le_bytes());
    }
    for rows in glyphs.values() {
        for row in rows {
            output.extend_from_slice(&row.to_be_bytes());
        }
    }

    if output.len() != total_bytes {
        return Err("internal font table size mismatch".to_owned());
    }
    fs::write(&arguments[2], &output)
        .map_err(|error| format!("cannot write {}: {error}", arguments[2]))?;
    println!(
        "wrote {} glyphs ({} bytes) to {}",
        glyphs.len(),
        output.len(),
        arguments[2]
    );
    Ok(())
}

fn read_bdf(path: &Path) -> Result<Vec<(u32, Rows)>, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{} is not UTF-8-compatible ASCII: {error}", path.display()))?;
    let mut glyphs = Vec::new();
    let mut in_glyph = false;
    let mut encoding = None;
    let mut width = None;
    let mut height = None;
    let mut bitmap = false;
    let mut rows = Vec::new();

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line == "STARTCHAR" || line.starts_with("STARTCHAR ") {
            if in_glyph {
                return Err(format!("{} contains a nested STARTCHAR", path.display()));
            }
            in_glyph = true;
            encoding = None;
            width = None;
            height = None;
            bitmap = false;
            rows.clear();
            continue;
        }
        if !in_glyph {
            continue;
        }
        if let Some(value) = line.strip_prefix("ENCODING ") {
            encoding = Some(parse_u32(value, path, "ENCODING")?);
        } else if let Some(value) = line.strip_prefix("BBX ") {
            let mut fields = value.split_whitespace();
            width = Some(parse_u32(
                fields
                    .next()
                    .ok_or_else(|| format!("{} has an incomplete BBX", path.display()))?,
                path,
                "BBX width",
            )?);
            height = Some(parse_u32(
                fields
                    .next()
                    .ok_or_else(|| format!("{} has an incomplete BBX", path.display()))?,
                path,
                "BBX height",
            )?);
        } else if line == "BITMAP" {
            bitmap = true;
        } else if line == "ENDCHAR" {
            let Some(encoding) = encoding else {
                return Err(format!("{} has a glyph without ENCODING", path.display()));
            };
            if width != Some(8) && width != Some(16) {
                return Err(format!(
                    "{} has glyph {encoding} with unsupported width {:?}",
                    path.display(),
                    width
                ));
            }
            if height != Some(16) || rows.len() != 16 {
                return Err(format!(
                    "{} has glyph {encoding} with a non-16-row bitmap",
                    path.display()
                ));
            }
            let mut normalized = [0u16; 16];
            normalized.copy_from_slice(&rows);
            glyphs.push((encoding, normalized));
            in_glyph = false;
            bitmap = false;
        } else if bitmap {
            let value = u32::from_str_radix(line, 16).map_err(|error| {
                format!(
                    "{} has an invalid bitmap row {line:?}: {error}",
                    path.display()
                )
            })?;
            let row = match width {
                Some(8) => (value as u16) << 8,
                Some(16) => value as u16,
                _ => {
                    return Err(format!(
                        "{} contains bitmap data before a valid BBX",
                        path.display()
                    ));
                }
            };
            rows.push(row);
        }
    }

    if in_glyph {
        return Err(format!("{} ends inside a glyph", path.display()));
    }
    Ok(glyphs)
}

fn parse_u32(value: &str, path: &Path, field: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .map_err(|error| format!("{} has invalid {field} {value:?}: {error}", path.display()))
}

fn jis0201_codepoint(encoding: u32) -> Option<u32> {
    match encoding {
        0x20..=0x7e => Some(encoding),
        0xa1..=0xdf => Some(0xff61 + encoding - 0xa1),
        _ => None,
    }
}

fn jis0208_codepoint(encoding: u32) -> Result<Option<u32>, String> {
    let row = encoding >> 8;
    let cell = encoding & 0xff;
    if !(0x21..=0x7e).contains(&row) || !(0x21..=0x7e).contains(&cell) {
        return Ok(None);
    }

    let bytes = [(row + 0x80) as u8, (cell + 0x80) as u8];
    let (decoded, _, had_errors) = EUC_JP.decode(&bytes);
    if had_errors {
        return Err(format!(
            "JIS X 0208 code {encoding:04x} cannot be decoded as EUC-JP"
        ));
    }
    let mut chars = decoded.chars();
    let Some(ch) = chars.next() else {
        return Ok(None);
    };
    if chars.next().is_some() {
        return Err(format!(
            "JIS X 0208 code {encoding:04x} decoded to more than one code point"
        ));
    }
    Ok(Some(ch as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_jis0201_ascii_and_halfwidth_katakana() {
        assert_eq!(jis0201_codepoint(0x41), Some('A' as u32));
        assert_eq!(jis0201_codepoint(0xa1), Some('｡' as u32));
        assert_eq!(jis0201_codepoint(0xdf), Some('ﾟ' as u32));
        assert_eq!(jis0201_codepoint(0x1f), None);
    }

    #[test]
    fn maps_known_jis0208_codepoints_to_unicode() {
        assert_eq!(jis0208_codepoint(0x456c).unwrap(), Some('東' as u32));
        assert_eq!(jis0208_codepoint(0x3278).unwrap(), Some('怪' as u32));
        assert_eq!(jis0208_codepoint(0x2121).unwrap(), Some('\u{3000}' as u32));
    }
}
