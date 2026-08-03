// SPDX-License-Identifier: AGPL-3.0-only

pub const SCREEN_W: i32 = 640;

pub const SCREEN_H: i32 = 400;

pub const KEYS_PANEL_X: i32 = 0;

pub const KEYS_PANEL_W: i32 = 300;

pub const GUTTER_X: i32 = 300;

pub const GUTTER_W: i32 = 13;

pub const RIGHT_X: i32 = 313;

pub const RIGHT_W: i32 = SCREEN_W - RIGHT_X;

pub const STRIP_COUNT: usize = 10;

pub const STRIP_PITCH: i32 = 32;

pub const KEYS_PANEL_H: i32 = STRIP_PITCH * STRIP_COUNT as i32;

pub const RULE_Y: i32 = 331;

pub const RULE_H: i32 = 1;

pub const STATUS_Y: i32 = 323;

pub const MUSIC_TICK_X: i32 = 77;

pub const MUSIC_NAME_X: i32 = 141;

pub const PCM1_X: i32 = 464;

pub const PCM2_X: i32 = 530;

pub const BAND_Y: i32 = 334;

pub const BAND_H: i32 = SCREEN_H - BAND_Y;

pub const STRIP_KIND_X: i32 = 0;

pub const STRIP_LINE1_Y: i32 = 0;

pub const STRIP_LINE2_Y: i32 = 6;

pub const STRIP_NUMBER_X: i32 = 31;

pub const STRIP_BAR_X: i32 = 65;

pub const STRIP_BAR_W: i32 = 143;

pub const STRIP_BAR_H: i32 = 4;

pub const STRIP_FIELDS_X: i32 = 66;

pub const STRIP_FIELD_PITCH: i32 = 31;

pub const STRIP_MASK_X: i32 = 249;

pub const STRIP_METER_X: i32 = 263;

pub const STRIP_METER_CELLS: i32 = 8;

pub const STRIP_METER_PITCH: i32 = 4;

pub const STRIP_METER_CELL_W: i32 = 3;

pub const KBD_Y: i32 = 13;

pub const KBD_H: i32 = 17;

pub const KBD_BLACK_H: i32 = 10;

pub const KEY_BLACK_W: i32 = 3;

pub const KBD_SHADE_Y: i32 = 29;

pub const KBD_X: i32 = 0;

pub const KEY_W: i32 = 5;

pub const KEY_COUNT: i32 = 60;

pub const LOGO_Y: i32 = 0;

pub const LOGO_H: i32 = 14;

pub const TAGLINE_X: i32 = 422;

pub const TAGLINE_PITCH: i32 = 5;

pub const TAGLINE_W: i32 = 214;

pub const FIELD_X: i32 = 526;

pub const FIELD_Y: i32 = 19;

pub const FIELD_PITCH: i32 = 19;

pub const FIELD_COUNT: usize = 6;

pub const FIELD_W: i32 = 634 - FIELD_X + 1;

pub const FIELD_TICK_W: i32 = 4;

pub const FIELD_H: i32 = 15;

pub const FIELD_LABEL_R: i32 = 562;

pub const FIELD_VALUE_R: i32 = 637;

pub const DRIVER_Y: i32 = 24;

pub const DRIVER_VALUE_X: i32 = RIGHT_X + 42;

pub const DRIVER_LINE_PITCH: i32 = 7;

pub const DIAL_CX: i32 = 330;

pub const DIAL_CY: i32 = 84;

pub const DIAL_R: i32 = 15;

pub const SLIDER_X: i32 = 353;

pub const SLIDER_Y: i32 = 69;

pub const SLIDER_W: i32 = 165;

pub const TRANSPORT_Y: i32 = 76;

pub const TRANSPORT_ROW2_Y: i32 = 86;

pub const TRANSPORT_X: i32 = 355;

pub const CPU_Y: i32 = 113;

pub const SPECTRUM_RULE_Y: i32 = 131;

pub const AXIS_X: i32 = 319;

pub const AXIS_SEG_X: i32 = 324;

pub const AXIS_SEG_W: i32 = 13;

pub const AXIS_SEG_H: i32 = 3;

pub const AXIS_SEG_PITCH: i32 = 5;

pub const AXIS_SEG_COUNT: i32 = 10;

pub const AXIS_SEG_TOP: i32 = 7;

pub const AXIS_LABEL_R: i32 = 350;

pub const AXIS_RULE_X: i32 = 351;

pub const AXIS_RULE_W: i32 = 2;

pub const AXIS_BOTTOM_LABEL: i32 = 57;

pub const SPECTRUM_LABEL_Y: i32 = 136;

pub const SPECTRUM_Y: i32 = 144;

pub const SPECTRUM_X: i32 = 354;

pub const SPECTRUM_W: i32 = 634 - SPECTRUM_X;

pub const SPECTRUM_H: i32 = 64;

pub const SPECTRUM_ROW_STEP: i32 = 2;

pub const SPECTRUM_COL_STEP: i32 = 4;

pub const SPECTRUM_DOT_W: i32 = 2;

pub const SPECTRUM_COLUMNS: usize = (SPECTRUM_W / SPECTRUM_COL_STEP) as usize;

pub const SPECTRUM_ROWS: i32 = SPECTRUM_H / SPECTRUM_ROW_STEP;

pub const SPECTRUM_RULER_Y: i32 = 208;

pub const LEVEL_LABEL_Y: i32 = 218;

pub const LEVEL_Y: i32 = 226;

pub const LEVEL_X: i32 = 355;

pub const LEVEL_SPAN: i32 = 258;

pub const LEVEL_CELL_W: i32 = 13;

pub const LEVEL_ROW_STEP: i32 = 2;

pub const LEVEL_H: i32 = 64;

pub const LEVEL_ROWS: i32 = LEVEL_H / LEVEL_ROW_STEP;

pub const LEVEL_COUNT: usize = 17;

pub const LEVEL_MARK_Y: [i32; 5] = [0, 12, 28, 44, AXIS_BOTTOM_LABEL];

pub const ONOFF_Y: i32 = 289;

pub const PANPOT_Y: i32 = 297;

pub const PROGRAM_Y: i32 = 305;

pub const KEYCODE_Y: i32 = 312;

pub const DIAL_ROW_CY: i32 = 297;

pub const DIAL_ROW_R: i32 = 7;

pub const PCM_Y: i32 = 323;

pub const fn level_column_x(index: usize) -> i32 {
    LEVEL_X + (index as i32 * LEVEL_SPAN + 8) / (LEVEL_COUNT as i32 - 1)
}

pub const BAND_LABEL_R: i32 = 68;

pub const BAND_VALUE_X: i32 = 80;

pub const BAND_ARRANGER_LABEL_R: i32 = 357;

pub const BAND_ARRANGER_VALUE_X: i32 = 371;

pub const BAND_ROW_Y: i32 = 340;

pub const BAND_ROW_PITCH: i32 = 19;

pub const BAND_ROW_COUNT: usize = 3;

pub const MODE_X: i32 = 580;

pub const MODE_W: i32 = 40;

pub const MODE_Y: i32 = BAND_Y + 3;

pub const BAND_MESSAGE_R: i32 = SCREEN_W - 4;

pub const BAND_MESSAGE_W: i32 = 200;

pub const LOOP_FIELD_INDEX: usize = 3;

pub const LIST_INDEX_X: i32 = 4;

pub const LIST_MARK_X: i32 = 22;

pub const LIST_NAME_X: i32 = 30;

pub const LIST_ROW_Y: i32 = BAND_Y + 10;

pub const LIST_ROW_PITCH: i32 = 7;

pub const LIST_ROWS: usize = ((SCREEN_H - LIST_ROW_Y) / LIST_ROW_PITCH) as usize;

pub const fn strip_top(index: usize) -> i32 {
    STRIP_PITCH * index as i32
}

pub const fn field_top(index: usize) -> i32 {
    FIELD_Y + FIELD_PITCH * index as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ten_strips_fill_the_stack_exactly() {
        assert_eq!(KEYS_PANEL_H, 320);
        assert_eq!(strip_top(0), 0);
        assert_eq!(strip_top(STRIP_COUNT - 1), 288);
        const { assert!(STRIP_PITCH * (STRIP_COUNT as i32 - 1) + STRIP_PITCH <= STATUS_Y) };
    }

    #[test]
    fn the_keyboard_spans_the_measured_width() {
        assert_eq!(KEY_COUNT * KEY_W, KEYS_PANEL_W);
        assert_eq!(KBD_X + KEY_COUNT * KEY_W - 1, 299);
        const { assert!(KEY_BLACK_W < KEY_W) };
    }

    #[test]
    fn the_panels_tile_the_screen_without_overlapping() {
        assert_eq!(KEYS_PANEL_X + KEYS_PANEL_W, GUTTER_X);
        assert_eq!(GUTTER_X + GUTTER_W, RIGHT_X);
        assert_eq!(RIGHT_X + RIGHT_W, SCREEN_W);
    }

    #[test]
    fn six_fields_stack_without_running_into_the_spectrum_rule() {
        assert_eq!(field_top(0), 19);
        assert_eq!(field_top(FIELD_COUNT - 1), 114);
        const { assert!(FIELD_Y + FIELD_PITCH * (FIELD_COUNT as i32 - 1) < SPECTRUM_RULE_Y) };
        const { assert!(FIELD_X + FIELD_W <= SCREEN_W) };
    }

    #[test]
    fn the_vertical_regions_cover_the_screen_in_order() {
        const { assert!(KEYS_PANEL_H <= STATUS_Y) };
        const { assert!(STATUS_Y < RULE_Y) };
        const { assert!(RULE_Y + RULE_H <= BAND_Y) };
        assert_eq!(BAND_Y + BAND_H, SCREEN_H);
    }

    #[test]
    fn the_keyboard_fits_inside_its_strip() {
        const { assert!(KBD_Y + KBD_H <= STRIP_PITCH) };
        assert_eq!(KBD_SHADE_Y, KBD_Y + KBD_H - 1);
    }

    #[test]
    fn the_scale_gutter_is_reserved_so_no_graph_can_reach_into_it() {
        const { assert!(AXIS_X < AXIS_SEG_X) };
        const { assert!(AXIS_SEG_X + AXIS_SEG_W <= AXIS_LABEL_R) };
        const { assert!(AXIS_LABEL_R < AXIS_RULE_X) };
        const { assert!(AXIS_RULE_X + AXIS_RULE_W <= SPECTRUM_X) };
        const { assert!(AXIS_RULE_X + AXIS_RULE_W <= LEVEL_X) };
    }

    #[test]
    fn the_segment_stack_ends_above_the_bottom_caption() {
        let last = AXIS_SEG_TOP + (AXIS_SEG_COUNT - 1) * AXIS_SEG_PITCH + AXIS_SEG_H;
        assert_eq!(last, 55, "ten segments span y+7..=y+54");
        const {
            assert!(
                AXIS_SEG_TOP + (AXIS_SEG_COUNT - 1) * AXIS_SEG_PITCH + AXIS_SEG_H
                    <= AXIS_BOTTOM_LABEL
            )
        };
    }

    #[test]
    fn the_spectrum_matrix_tiles_its_measured_span() {
        assert_eq!(SPECTRUM_COLUMNS, 70);
        assert_eq!(SPECTRUM_ROWS, 32);
        assert_eq!(SPECTRUM_X, 354);
        let last = SPECTRUM_X + (SPECTRUM_COLUMNS as i32 - 1) * SPECTRUM_COL_STEP;
        assert!(
            (last + SPECTRUM_DOT_W - 1 - 633).abs() <= 2,
            "the matrix ends at {}, measured at 633",
            last + SPECTRUM_DOT_W - 1
        );
        assert!(last + SPECTRUM_DOT_W - 1 <= 634);
    }

    #[test]
    fn level_columns_follow_the_measured_fractional_pitch() {
        let measured = [
            355, 371, 387, 404, 420, 436, 452, 468, 484, 500, 516, 533, 549, 565, 581, 597, 613,
        ];
        for (index, expected) in measured.iter().enumerate() {
            let x = level_column_x(index);
            assert!(
                (x - expected).abs() <= 1,
                "column {index} at x={x}, measured at {expected}"
            );
        }
        let last = level_column_x(LEVEL_COUNT - 1) + LEVEL_CELL_W - 1;
        assert!(last <= 634, "the last bar runs off the panel at {last}");
    }

    #[test]
    fn the_metadata_band_holds_exactly_its_three_rows() {
        let last = BAND_ROW_Y + (BAND_ROW_COUNT as i32 - 1) * BAND_ROW_PITCH;
        assert_eq!(last, 378);

        const { assert!(BAND_ROW_Y + (BAND_ROW_COUNT as i32 - 1) * BAND_ROW_PITCH + 16 <= SCREEN_H) };
        const { assert!(BAND_LABEL_R < BAND_VALUE_X) };
        const { assert!(BAND_ARRANGER_LABEL_R < BAND_ARRANGER_VALUE_X) };
    }
}
