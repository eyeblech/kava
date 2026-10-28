use crate::audio::AudioFrame;
use crate::color::Palette;
use crate::physics::{PhysicsField, VisualMode};
use std::fmt::Write as FmtWrite;
use std::io::{self, Write};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderStyle {
    SubpixelHalfBlock,
    BrailleVector,
    SacredGlyphs,
}

impl RenderStyle {
    pub fn name(&self) -> &'static str {
        match self {
            RenderStyle::SubpixelHalfBlock => "TrueColor Subpixel (▀)",
            RenderStyle::BrailleVector => "2x4 Braille Vector (⣿)",
            RenderStyle::SacredGlyphs => "Sacred ASCII Glyphs (✦)",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            RenderStyle::SubpixelHalfBlock => RenderStyle::BrailleVector,
            RenderStyle::BrailleVector => RenderStyle::SacredGlyphs,
            RenderStyle::SacredGlyphs => RenderStyle::SubpixelHalfBlock,
        }
    }
}

pub struct TerminalRenderer {
    frame_str: String,
    pub style: RenderStyle,
    pub show_hud: bool,
    pub show_help: bool,
}

impl TerminalRenderer {
    pub fn new() -> Self {
        Self {
            frame_str: String::with_capacity(262_144),
            style: RenderStyle::SubpixelHalfBlock,
            show_hud: true,
            show_help: false,
        }
    }

    /// Returns the required simulation field resolution `(width, height)` for a given terminal `(cols, rows)`.
    pub fn field_dimensions(&self, cols: u16, rows: u16) -> (usize, usize) {
        let c = cols.max(10) as usize;
        let r = rows.max(6) as usize;
        // Subpixel half-block maps 1x2 vertical pixels per terminal cell
        (c, r * 2)
    }

    pub fn draw_frame<W: Write>(
        &mut self,
        out: &mut W,
        cols: u16,
        rows: u16,
        field: &PhysicsField,
        audio: &AudioFrame,
        mode: VisualMode,
        palette: Palette,
        fps: f32,
        sensitivity: f32,
    ) -> io::Result<()> {
        self.frame_str.clear();
        // Synchronized output start (\x1b[?2026h) + move cursor home (\x1b[H)
        self.frame_str.push_str("\x1b[?2026h\x1b[H");

        let w = field.width;
        let term_rows = rows as usize;
        let hud_rows = if self.show_hud && term_rows > 8 { 1 } else { 0 };
        let render_rows = term_rows.saturating_sub(hud_rows);

        let mut prev_fg: Option<[u8; 3]> = None;
        let mut prev_bg: Option<[u8; 3]> = None;

        for ty in 0..render_rows {
            let py_top = (ty * 2).min(field.height.saturating_sub(1));
            let py_bot = (ty * 2 + 1).min(field.height.saturating_sub(1));

            for tx in 0..(cols as usize).min(w) {
                let top_rgb = field.rgb_buf[py_top * w + tx];
                let bot_rgb = field.rgb_buf[py_bot * w + tx];

                match self.style {
                    RenderStyle::SubpixelHalfBlock => {
                        if prev_fg != Some(top_rgb) {
                            let _ = write!(
                                self.frame_str,
                                "\x1b[38;2;{};{};{}m",
                                top_rgb[0], top_rgb[1], top_rgb[2]
                            );
                            prev_fg = Some(top_rgb);
                        }
                        if prev_bg != Some(bot_rgb) {
                            let _ = write!(
                                self.frame_str,
                                "\x1b[48;2;{};{};{}m",
                                bot_rgb[0], bot_rgb[1], bot_rgb[2]
                            );
                            prev_bg = Some(bot_rgb);
                        }
                        self.frame_str.push('▀');
                    }
                    RenderStyle::BrailleVector => {
                        // Compute local luminance & gradient to synthesize crisp 8-dot Unicode Braille (U+2800..U+28FF)
                        let fg = [
                            top_rgb[0].max(bot_rgb[0]),
                            top_rgb[1].max(bot_rgb[1]),
                            top_rgb[2].max(bot_rgb[2]),
                        ];
                        let bg = [4, 4, 9];
                        if prev_fg != Some(fg) {
                            let _ = write!(
                                self.frame_str,
                                "\x1b[38;2;{};{};{}m",
                                fg[0], fg[1], fg[2]
                            );
                            prev_fg = Some(fg);
                        }
                        if prev_bg != Some(bg) {
                            self.frame_str.push_str("\x1b[48;2;4;4;9m");
                            prev_bg = Some(bg);
                        }

                        let l_top = luma(top_rgb);
                        let l_bot = luma(bot_rgb);
                        let right_x = (tx + 1).min(w.saturating_sub(1));
                        let l_top_r = luma(field.rgb_buf[py_top * w + right_x]);
                        let l_bot_r = luma(field.rgb_buf[py_bot * w + right_x]);

                        let mut mask: u8 = 0;
                        if l_top > 28 {
                            mask |= 0x01;
                        }
                        if l_top > 75 {
                            mask |= 0x02;
                        }
                        if l_bot > 28 {
                            mask |= 0x04;
                        }
                        if l_bot > 95 {
                            mask |= 0x40;
                        }
                        if l_top_r > 28 {
                            mask |= 0x08;
                        }
                        if l_top_r > 75 {
                            mask |= 0x10;
                        }
                        if l_bot_r > 28 {
                            mask |= 0x20;
                        }
                        if l_bot_r > 95 {
                            mask |= 0x80;
                        }

                        let ch = char::from_u32(0x2800 + mask as u32).unwrap_or(' ');
                        self.frame_str.push(ch);
                    }
                    RenderStyle::SacredGlyphs => {
                        let fg = [
                            ((top_rgb[0] as u16 + bot_rgb[0] as u16) / 2) as u8,
                            ((top_rgb[1] as u16 + bot_rgb[1] as u16) / 2) as u8,
                            ((top_rgb[2] as u16 + bot_rgb[2] as u16) / 2) as u8,
                        ];
                        let bg = [5, 5, 11];
                        if prev_fg != Some(fg) {
                            let _ = write!(
                                self.frame_str,
                                "\x1b[38;2;{};{};{}m",
                                fg[0], fg[1], fg[2]
                            );
                            prev_fg = Some(fg);
                        }
                        if prev_bg != Some(bg) {
                            self.frame_str.push_str("\x1b[48;2;5;5;11m");
                            prev_bg = Some(bg);
                        }
                        const GLYPHS: &[char] = &[' ', '·', '∙', '∘', '⋄', '✧', '✦', '❖', '❂', '✴', '✸', '✺'];
                        let lum = (luma(fg) as usize * (GLYPHS.len() - 1)) / 255;
                        self.frame_str.push(GLYPHS[lum.min(GLYPHS.len() - 1)]);
                    }
                }
            }
            if ty + 1 < term_rows {
                self.frame_str.push_str("\x1b[0m\r\n");
                prev_fg = None;
                prev_bg = None;
            }
        }

        if hud_rows > 0 {
            self.render_hud_bar(cols as usize, field, audio, mode, palette, fps, sensitivity);
        }

        if self.show_help && cols >= 54 && rows >= 18 {
            self.render_help_modal(cols, rows);
        }

        // End synchronized update
        self.frame_str.push_str("\x1b[0m\x1b[?2026l");
        out.write_all(self.frame_str.as_bytes())?;
        out.flush()
    }

    fn render_hud_bar(
        &mut self,
        cols: usize,
        field: &PhysicsField,
        audio: &AudioFrame,
        mode: VisualMode,
        palette: Palette,
        fps: f32,
        sensitivity: f32,
    ) {
        // Mini 8-bar CAVA spectrum sparkline right inside the status bar
        const SPARK: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        let mut sparkline = String::with_capacity(24);
        for i in 0..12 {
            let b = (audio.bands[i * 5].clamp(0.0, 1.0) * 7.0) as usize;
            sparkline.push(SPARK[b.min(7)]);
        }

        let beat_dot = if audio.beat_impulse > 0.35 || audio.spectral_flux > 0.45 {
            "✦"
        } else if audio.active_signal {
            "●"
        } else {
            "·"
        };
        let status = format!(
            " ❂ KAVA {} │ {} │ {} │ D{} │ {} │ {} {} │ Sens {:.1}x │ {:2.0} FPS │ [?] Help [Q] Quit ",
            beat_dot,
            mode.name(),
            palette.name(),
            field.symmetry_folds,
            self.style.name(),
            sparkline,
            audio.source_name,
            sensitivity,
            fps
        );

        self.frame_str.push_str("\x1b[48;2;12;14;26m\x1b[38;2;210;220;255m");
        let mut char_count = 0;
        for ch in status.chars() {
            if char_count >= cols {
                break;
            }
            self.frame_str.push(ch);
            char_count += 1;
        }
        while char_count < cols {
            self.frame_str.push(' ');
            char_count += 1;
        }
    }

    fn render_help_modal(&mut self, cols: u16, rows: u16) {
        let box_w: u16 = 54;
        let box_h: u16 = 15;
        let start_x = (cols.saturating_sub(box_w)) / 2 + 1;
        let start_y = (rows.saturating_sub(box_h)) / 2 + 1;

        let lines = [
            "╔════════════════════════════════════════════════════╗",
            "║       ❂ KAVA — COSMIC KALEIDOSCOPE CONTROLS        ║",
            "╠════════════════════════════════════════════════════╣",
            "║  [1-6] / [M]   Switch Physics Engine / Mode        ║",
            "║  [C] / [P]     Cycle Oklab Perceptual Palettes     ║",
            "║  [ [ ] / [ ] ] Decrease / Increase Symmetry (D_24) ║",
            "║  [+] / [-]     Audio Sensitivity / Gain Boost      ║",
            "║  [Up] / [Down] Zoom In / Out into Mandala Core     ║",
            "║  [R]           Cycle Subpixel / Braille / Glyph    ║",
            "║  [B]           Toggle Optical Neon Bloom Pass      ║",
            "║  [S]           Toggle Cosmic Harmonic Synth Demo   ║",
            "║  [H] / [Space] Toggle Bottom HUD / Pause Frame     ║",
            "║  [?] / [Q]     Close Help Overlay / Quit KAVA      ║",
            "╚════════════════════════════════════════════════════╝",
        ];

        for (i, line) in lines.iter().enumerate() {
            let _ = write!(
                self.frame_str,
                "\x1b[{};{}H\x1b[48;2;14;16;32m\x1b[38;2;235;240;255m{}",
                start_y + i as u16,
                start_x,
                line
            );
        }
    }
}

#[inline]
fn luma(rgb: [u8; 3]) -> u8 {
    ((rgb[0] as u16 * 54 + rgb[1] as u16 * 183 + rgb[2] as u16 * 19) >> 8) as u8
}
