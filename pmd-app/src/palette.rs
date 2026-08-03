// SPDX-License-Identifier: AGPL-3.0-only

use crate::fb::Palette;

pub const BLACK: u8 = 0;

pub const SHADOW: u8 = 1;

pub const DIM: u8 = 2;

pub const BLUE: u8 = 3;

pub const TEXT: u8 = 4;

pub const NOTE: u8 = 5;

pub const KEY_WHITE: u8 = 6;

pub const KEY_SHADE: u8 = 7;

pub const LIGHT: u8 = 8;

pub const PALETTE: Palette = [
    [0x00, 0x00, 0x00],
    [0x00, 0x00, 0x11],
    [0x44, 0x44, 0x77],
    [0x33, 0x33, 0xEE],
    [0x66, 0x88, 0xFF],
    [0x88, 0xFF, 0x44],
    [0xCC, 0xCC, 0xBB],
    [0xBB, 0xBB, 0xAA],
    [0xCC, 0xCC, 0xCC],
    [0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_measured_entry_sits_on_the_pc98_four_bit_grid() {
        for (i, rgb) in PALETTE.iter().enumerate() {
            for (channel, &v) in rgb.iter().enumerate() {
                assert_eq!(
                    v % 17,
                    0,
                    "entry {i} channel {channel} = {v} is off the hardware grid"
                );
            }
        }
    }

    #[test]
    fn the_named_indices_address_the_entries_they_claim_to() {
        assert_eq!(PALETTE[BLACK as usize], [0x00, 0x00, 0x00]);
        assert_eq!(PALETTE[TEXT as usize], [0x66, 0x88, 0xFF]);
        assert_eq!(PALETTE[NOTE as usize], [0x88, 0xFF, 0x44]);
        assert_eq!(PALETTE[KEY_WHITE as usize], [0xCC, 0xCC, 0xBB]);
    }

    #[test]
    fn the_title_band_tone_is_a_checker_of_black_and_dim_not_a_palette_entry() {
        let black = PALETTE[BLACK as usize];
        let dim = PALETTE[DIM as usize];
        let avg = [
            (black[0] as u16 + dim[0] as u16) / 2,
            (black[1] as u16 + dim[1] as u16) / 2,
            (black[2] as u16 + dim[2] as u16) / 2,
        ];
        assert_eq!(avg, [0x22, 0x22, 0x3B]);
    }

    #[test]
    fn unassigned_slots_stay_black_rather_than_holding_invented_colours() {
        for (i, rgb) in PALETTE.iter().enumerate().skip(9) {
            assert_eq!(*rgb, [0, 0, 0], "slot {i} was filled without a measurement");
        }
    }
}
