// SPDX-License-Identifier: AGPL-3.0-only

pub const WIDTH: usize = 640;

pub const HEIGHT: usize = 400;

pub const PIXELS: usize = WIDTH * HEIGHT;

pub type Palette = [[u8; 3]; 16];

#[derive(Clone, PartialEq, Eq)]
pub struct Framebuffer {
    px: Vec<u8>,
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Framebuffer {
    pub fn new() -> Self {
        Self {
            px: vec![0; PIXELS],
        }
    }

    pub fn indices(&self) -> &[u8] {
        &self.px
    }

    pub fn clear(&mut self, idx: u8) {
        debug_assert!(idx < 16, "palette index {idx} is outside the 16 colours");
        self.px.fill(idx);
    }

    pub fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= WIDTH as i32 || y >= HEIGHT as i32 {
            return 0;
        }
        self.px[y as usize * WIDTH + x as usize]
    }

    pub fn set(&mut self, x: i32, y: i32, idx: u8) {
        debug_assert!(idx < 16, "palette index {idx} is outside the 16 colours");
        if x < 0 || y < 0 || x >= WIDTH as i32 || y >= HEIGHT as i32 {
            return;
        }
        self.px[y as usize * WIDTH + x as usize] = idx;
    }

    pub fn hline(&mut self, x0: i32, x1: i32, y: i32, idx: u8) {
        let (lo, hi) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
        for x in lo..=hi {
            self.set(x, y, idx);
        }
    }

    pub fn vline(&mut self, x: i32, y0: i32, y1: i32, idx: u8) {
        let (lo, hi) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
        for y in lo..=hi {
            self.set(x, y, idx);
        }
    }

    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, idx: u8) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let (mut x, mut y) = (x0, y0);
        loop {
            self.set(x, y, idx);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    pub fn rect_fill(&mut self, x: i32, y: i32, w: i32, h: i32, idx: u8) {
        for row in 0..h {
            self.hline(x, x + w - 1, y + row, idx);
        }
    }

    pub fn rect_checker(&mut self, x: i32, y: i32, w: i32, h: i32, idx: u8) {
        for row in 0..h {
            let py = y + row;
            for col in 0..w {
                let px = x + col;
                if (px + py) & 1 == 0 {
                    self.set(px, py, idx);
                }
            }
        }
    }

    pub fn rect_hatch(&mut self, x: i32, y: i32, w: i32, h: i32, idx: u8) {
        for row in 0..h {
            let py = y + row;
            if py & 1 == 0 {
                self.hline(x, x + w - 1, py, idx);
            }
        }
    }

    pub fn blit_1bpp(&mut self, x: i32, y: i32, width: u32, rows: &[u8], idx: u8) {
        debug_assert!(width <= 8, "1bpp rows carry at most 8 columns");
        for (row, bits) in rows.iter().enumerate() {
            for col in 0..width {
                if bits & (0x80 >> col) != 0 {
                    self.set(x + col as i32, y + row as i32, idx);
                }
            }
        }
    }

    pub fn blit_1bpp_wide(&mut self, x: i32, y: i32, width: u32, rows: &[u16], idx: u8) {
        debug_assert!(width <= 16, "wide 1bpp rows carry at most 16 columns");
        for (row, bits) in rows.iter().enumerate() {
            for col in 0..width {
                if bits & (0x8000 >> col) != 0 {
                    self.set(x + col as i32, y + row as i32, idx);
                }
            }
        }
    }

    pub fn to_rgba(&self, palette: &Palette) -> Vec<u8> {
        let mut out = vec![0u8; PIXELS * 4];
        for (dst, &idx) in out.chunks_exact_mut(4).zip(self.px.iter()) {
            let rgb = palette[(idx & 0x0f) as usize];
            dst[0] = rgb[0];
            dst[1] = rgb[1];
            dst[2] = rgb[2];
            dst[3] = 0xff;
        }
        out
    }
}

pub fn present_size(scale: u32) -> (u32, u32) {
    (WIDTH as u32 * scale, HEIGHT as u32 * scale * 6 / 5)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_palette() -> Palette {
        let mut pal = [[0u8; 3]; 16];
        for (i, entry) in pal.iter_mut().enumerate() {
            let v = (i * 17) as u8;
            *entry = [v, v, v];
        }
        pal
    }

    #[test]
    fn a_new_framebuffer_is_exactly_the_pc98_screen_cleared_to_index_zero() {
        let fb = Framebuffer::new();
        assert_eq!(fb.indices().len(), 640 * 400);
        assert_eq!(PIXELS, 256_000);
        assert!(fb.indices().iter().all(|&i| i == 0));
    }

    #[test]
    fn drawing_outside_the_screen_is_clipped_rather_than_wrapped_or_panicking() {
        let mut fb = Framebuffer::new();

        fb.set(-1, 10, 3);
        fb.set(WIDTH as i32, 10, 3);
        fb.set(10, -1, 3);
        fb.set(10, HEIGHT as i32, 3);
        assert!(fb.indices().iter().all(|&i| i == 0));
        assert_eq!(fb.get(-1, 10), 0);
        assert_eq!(fb.get(WIDTH as i32, 10), 0);
    }

    #[test]
    fn a_rect_straddling_an_edge_keeps_only_its_on_screen_part() {
        let mut fb = Framebuffer::new();
        fb.rect_fill(-2, -2, 5, 5, 7);
        assert_eq!(fb.get(0, 0), 7);
        assert_eq!(fb.get(2, 2), 7);
        assert_eq!(fb.get(3, 3), 0);

        assert_eq!(fb.get(WIDTH as i32 - 1, 0), 0);
    }

    #[test]
    fn lines_are_inclusive_and_order_independent() {
        let mut fb = Framebuffer::new();
        fb.hline(10, 12, 5, 1);
        fb.vline(20, 8, 6, 2);
        assert_eq!(
            [fb.get(9, 5), fb.get(10, 5), fb.get(12, 5), fb.get(13, 5)],
            [0, 1, 1, 0]
        );
        assert_eq!(
            [fb.get(20, 5), fb.get(20, 6), fb.get(20, 8), fb.get(20, 9)],
            [0, 2, 2, 0]
        );
    }

    #[test]
    fn a_bresenham_line_reaches_both_endpoints_and_stays_one_pixel_per_step() {
        let mut fb = Framebuffer::new();
        fb.line(2, 2, 12, 7, 5);
        assert_eq!(fb.get(2, 2), 5);
        assert_eq!(fb.get(12, 7), 5);

        for x in 2..=12 {
            let lit = (0..400).filter(|&y| fb.get(x, y) == 5).count();
            assert_eq!(lit, 1, "column {x} should carry exactly one pixel");
        }
    }

    #[test]
    fn the_checker_pattern_is_keyed_on_absolute_coordinates_so_blocks_stay_in_phase() {
        let mut fb = Framebuffer::new();

        fb.rect_checker(4, 4, 4, 2, 9);
        fb.rect_checker(8, 4, 4, 2, 9);
        for x in 4..12 {
            let expected = if (x + 4) & 1 == 0 { 9 } else { 0 };
            assert_eq!(fb.get(x, 4), expected, "seam broke at x={x}");
        }
    }

    #[test]
    fn the_hatch_pattern_lights_even_scanlines_only() {
        let mut fb = Framebuffer::new();
        fb.rect_hatch(3, 5, 4, 4, 6);

        assert_eq!(fb.get(3, 5), 0);
        assert_eq!(fb.get(3, 6), 6);
        assert_eq!(fb.get(3, 7), 0);
        assert_eq!(fb.get(3, 8), 6);
    }

    #[test]
    fn glyph_blits_write_set_bits_only_and_leave_the_background_intact() {
        let mut fb = Framebuffer::new();
        fb.rect_fill(0, 0, 8, 2, 4);
        fb.blit_1bpp(0, 0, 8, &[0b1010_0000, 0b0000_0000], 11);
        assert_eq!(fb.get(0, 0), 11);
        assert_eq!(fb.get(1, 0), 4, "a clear bit must not erase the backdrop");
        assert_eq!(fb.get(2, 0), 11);
        assert_eq!(fb.get(0, 1), 4);
    }

    #[test]
    fn a_condensed_blit_ignores_columns_past_its_declared_width() {
        let mut fb = Framebuffer::new();
        fb.blit_1bpp(0, 0, 4, &[0b1111_1111], 3);
        assert_eq!(fb.get(3, 0), 3);
        assert_eq!(fb.get(4, 0), 0, "column 4 is outside the 4-wide cell");
    }

    #[test]
    fn wide_blits_cover_the_full_sixteen_column_kanji_cell() {
        let mut fb = Framebuffer::new();
        fb.blit_1bpp_wide(0, 0, 16, &[0b1000_0000_0000_0001], 8);
        assert_eq!(fb.get(0, 0), 8);
        assert_eq!(fb.get(15, 0), 8);
        assert_eq!(fb.get(1, 0), 0);
    }

    #[test]
    fn conversion_to_rgba_is_opaque_and_uses_the_palette_verbatim() {
        let mut fb = Framebuffer::new();
        fb.set(0, 0, 15);
        let rgba = fb.to_rgba(&test_palette());
        assert_eq!(rgba.len(), PIXELS * 4);
        assert_eq!(&rgba[0..4], &[255, 255, 255, 255]);
        assert_eq!(&rgba[4..8], &[0, 0, 0, 255]);
    }

    #[test]
    fn presentation_applies_the_pc98_aspect_so_the_result_is_four_by_three() {
        for scale in 1..=4 {
            let (w, h) = present_size(scale);
            assert_eq!((w, h), (640 * scale, 480 * scale));
            assert_eq!(w * 3, h * 4, "scale {scale} lost the 4:3 aspect");
        }
    }
}
