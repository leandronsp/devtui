use base64::Engine;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::fmt::Write as FmtWrite;
use std::io::Write;

use super::tmux;

const CHUNK_SIZE: usize = 4096;
const IMAGE_ID: u32 = 31;

static DIACRITICS: [char; 285] = [
    '\u{305}', '\u{30D}', '\u{30E}', '\u{310}', '\u{312}', '\u{33D}', '\u{33E}', '\u{33F}',
    '\u{346}', '\u{34A}', '\u{34B}', '\u{34C}', '\u{350}', '\u{351}', '\u{352}', '\u{357}',
    '\u{35B}', '\u{363}', '\u{364}', '\u{365}', '\u{366}', '\u{367}', '\u{368}', '\u{369}',
    '\u{36A}', '\u{36B}', '\u{36C}', '\u{36D}', '\u{36E}', '\u{36F}', '\u{483}', '\u{484}',
    '\u{485}', '\u{486}', '\u{487}', '\u{592}', '\u{593}', '\u{594}', '\u{595}', '\u{597}',
    '\u{598}', '\u{599}', '\u{59C}', '\u{59D}', '\u{59E}', '\u{59F}', '\u{5A0}', '\u{5A1}',
    '\u{5A8}', '\u{5A9}', '\u{5AB}', '\u{5AC}', '\u{5AF}', '\u{5C4}', '\u{610}', '\u{611}',
    '\u{612}', '\u{613}', '\u{614}', '\u{615}', '\u{616}', '\u{617}', '\u{657}', '\u{658}',
    '\u{659}', '\u{65A}', '\u{65B}', '\u{65D}', '\u{65E}', '\u{6D6}', '\u{6D7}', '\u{6D8}',
    '\u{6D9}', '\u{6DA}', '\u{6DB}', '\u{6DC}', '\u{6DF}', '\u{6E0}', '\u{6E1}', '\u{6E2}',
    '\u{6E4}', '\u{6E7}', '\u{6E8}', '\u{6EB}', '\u{6EC}', '\u{730}', '\u{732}', '\u{733}',
    '\u{735}', '\u{736}', '\u{73A}', '\u{73D}', '\u{73F}', '\u{740}', '\u{741}', '\u{743}',
    '\u{745}', '\u{747}', '\u{749}', '\u{74A}', '\u{7EB}', '\u{7EC}', '\u{7ED}', '\u{7EE}',
    '\u{7EF}', '\u{7F0}', '\u{7F1}', '\u{7F3}', '\u{816}', '\u{817}', '\u{818}', '\u{819}',
    '\u{81B}', '\u{81C}', '\u{81D}', '\u{81E}', '\u{81F}', '\u{820}', '\u{821}', '\u{822}',
    '\u{823}', '\u{825}', '\u{826}', '\u{827}', '\u{829}', '\u{82A}', '\u{82B}', '\u{82C}',
    '\u{82D}', '\u{951}', '\u{953}', '\u{954}', '\u{F82}', '\u{F83}', '\u{F86}', '\u{F87}',
    '\u{135D}', '\u{135E}', '\u{135F}', '\u{17DD}', '\u{193A}', '\u{1A17}', '\u{1A75}',
    '\u{1A76}', '\u{1A77}', '\u{1A78}', '\u{1A79}', '\u{1A7A}', '\u{1A7B}', '\u{1A7C}',
    '\u{1B6B}', '\u{1B6D}', '\u{1B6E}', '\u{1B6F}', '\u{1B70}', '\u{1B71}', '\u{1B72}',
    '\u{1B73}', '\u{1CD0}', '\u{1CD1}', '\u{1CD2}', '\u{1CDA}', '\u{1CDB}', '\u{1CE0}',
    '\u{1DC0}', '\u{1DC1}', '\u{1DC3}', '\u{1DC4}', '\u{1DC5}', '\u{1DC6}', '\u{1DC7}',
    '\u{1DC8}', '\u{1DC9}', '\u{1DCB}', '\u{1DCC}', '\u{1DD1}', '\u{1DD2}', '\u{1DD3}',
    '\u{1DD4}', '\u{1DD5}', '\u{1DD6}', '\u{1DD7}', '\u{1DD8}', '\u{1DD9}', '\u{1DDA}',
    '\u{1DDB}', '\u{1DDC}', '\u{1DDD}', '\u{1DDE}', '\u{1DDF}', '\u{1DE0}', '\u{1DE1}',
    '\u{1DE2}', '\u{1DE3}', '\u{1DE4}', '\u{1DE5}', '\u{1DE6}', '\u{1DFE}', '\u{20D0}',
    '\u{20D1}', '\u{20D4}', '\u{20D5}', '\u{20D6}', '\u{20D7}', '\u{20DB}', '\u{20DC}',
    '\u{20E1}', '\u{20E7}', '\u{20E9}', '\u{20F0}', '\u{2CEF}', '\u{2CF0}', '\u{2CF1}',
    '\u{2DE0}', '\u{2DE1}', '\u{2DE2}', '\u{2DE3}', '\u{2DE4}', '\u{2DE5}', '\u{2DE6}',
    '\u{2DE7}', '\u{2DE8}', '\u{2DE9}', '\u{2DEA}', '\u{2DEB}', '\u{2DEC}', '\u{2DED}',
    '\u{2DEE}', '\u{2DEF}', '\u{2DF0}', '\u{2DF1}', '\u{2DF2}', '\u{2DF3}', '\u{2DF4}',
    '\u{2DF5}', '\u{2DF6}', '\u{2DF7}', '\u{2DF8}', '\u{2DF9}', '\u{2DFA}', '\u{2DFB}',
    '\u{2DFC}', '\u{2DFD}', '\u{2DFE}', '\u{2DFF}', '\u{A66F}', '\u{A674}', '\u{A675}',
    '\u{A676}', '\u{A677}', '\u{A678}', '\u{A679}', '\u{A67A}', '\u{A67B}', '\u{A67C}',
    '\u{A67D}', '\u{A69E}', '\u{A69F}', '\u{A6F0}', '\u{A6F1}', '\u{A8E0}', '\u{A8E1}',
    '\u{A8E2}', '\u{A8E3}', '\u{A8E4}', '\u{A8E5}', '\u{A8E6}', '\u{A8E7}', '\u{A8E8}',
    '\u{A8E9}', '\u{A8EA}', '\u{A8EB}', '\u{A8EC}', '\u{A8ED}', '\u{A8EE}', '\u{A8EF}',
    '\u{A8F0}', '\u{A8F1}', '\u{FE20}', '\u{FE21}', '\u{FE22}', '\u{FE23}', '\u{FE24}',
    '\u{FE25}', '\u{FE26}',
];

fn diacritic(val: u16) -> char {
    DIACRITICS.get(val as usize).copied().unwrap_or(DIACRITICS[0])
}

pub fn max_rows() -> u16 {
    DIACRITICS.len() as u16
}

/// A Kitty graphics protocol image stored in terminal memory.
pub struct KittyImage {
    pub width: u32,
    pub height: u32,
    pub scroll_row: u16,
    max_rows: u16,
    id_color: String,
    id_extra: u16,
}

impl KittyImage {
    /// Transmit a PNG image to the terminal via Kitty graphics protocol.
    /// In tmux: transmit without virtual placement (U=1), use direct placement later.
    /// Outside tmux: transmit with virtual placement, use Unicode placeholders.
    pub fn transmit(png_bytes: &[u8], img_width: u32, img_height: u32) -> std::io::Result<Self> {
        let is_tmux = tmux::in_tmux();
        log::info!("KittyImage::transmit: {}x{} px, {} bytes, tmux={}", img_width, img_height, png_bytes.len(), is_tmux);
        let mut stdout = std::io::stdout().lock();

        // Delete any previous image with this ID
        tmux::write_kitty_cmd(&mut stdout, &format!("a=d,d=I,i={IMAGE_ID},q=2"))?;

        // Base64 encode the PNG data
        let b64 = base64::engine::general_purpose::STANDARD.encode(png_bytes);
        let chunks: Vec<&[u8]> = b64.as_bytes().chunks(CHUNK_SIZE).collect();

        for (idx, chunk) in chunks.iter().enumerate() {
            let more = if idx == chunks.len() - 1 { 0 } else { 1 };
            let params = if idx == 0 {
                // U=1 enables virtual placement (Unicode placeholders).
                // Works in both tmux (via DCS passthrough) and direct.
                format!("i={IMAGE_ID},a=T,U=1,f=100,t=d,m={more},q=2")
            } else {
                format!("m={more}")
            };
            tmux::write_kitty_cmd_with_data(&mut stdout, &params, chunk)?;
        }

        stdout.flush()?;

        // Encode image ID as RGB color for placeholder foreground
        let [id_extra_byte, id_r, id_g, id_b] = IMAGE_ID.to_be_bytes();
        let id_color = format!("\x1b[38;2;{id_r};{id_g};{id_b}m");

        Ok(Self {
            width: img_width,
            height: img_height,
            scroll_row: 0,
            max_rows: 0,
            id_color,
            id_extra: u16::from(id_extra_byte),
        })
    }

    /// Render unicode placeholders into the ratatui buffer.
    /// Outside tmux: write U+10EEEE directly into cells.
    /// Inside tmux: wrap U+10EEEE in DCS passthrough to bypass tmux's utf8 validation.
    pub fn render_placeholders(&self, area: Rect, buf: &mut Buffer) {
        let width = area.width;
        let row_placeholders: String = std::iter::repeat_n('\u{10EEEE}', (width as usize).saturating_sub(1)).collect();
        let max_rows = area.height.min(DIACRITICS.len() as u16);
        let in_tmux = tmux::in_tmux();

        // Non-tmux needs cursor save/restore suffix
        let restore_cursor = if in_tmux {
            String::new()
        } else {
            let right = area.width - 1;
            let down = area.height - 1;
            format!("\x1b[u\x1b[{right}C\x1b[{down}B")
        };
        // Tmux needs ESC-doubled id_color
        let tmux_id_color = if in_tmux {
            self.id_color.replace('\x1b', "\x1b\x1b")
        } else {
            String::new()
        };

        for y in 0..max_rows {
            let img_row = y + self.scroll_row;
            if img_row >= DIACRITICS.len() as u16 {
                break;
            }

            let mut symbol = String::with_capacity(512);
            if in_tmux {
                let term_row = area.top() + y + 1;
                let term_col = area.left() + 1;
                let _ = write!(
                    symbol,
                    "\x1bPtmux;\x1b\x1b[{term_row};{term_col}H{tmux_id_color}\u{10EEEE}{}{}{}",
                    diacritic(img_row), diacritic(0), diacritic(self.id_extra),
                );
            } else {
                let _ = write!(
                    symbol,
                    "\x1b[s{}\u{10EEEE}{}{}{}",
                    self.id_color, diacritic(img_row), diacritic(0), diacritic(self.id_extra),
                );
            }

            symbol.push_str(&row_placeholders);
            if in_tmux {
                symbol.push_str("\x1b\\");
            } else {
                symbol.push_str(&restore_cursor);
            }

            if let Some(cell) = buf.cell_mut((area.left(), area.top() + y)) {
                cell.set_symbol(&symbol);
            }
            for x in 1..width {
                if let Some(cell) = buf.cell_mut((area.left() + x, area.top() + y)) {
                    cell.set_skip(true);
                }
            }
        }
    }

    /// Place image directly via Kitty APC escape, bypassing ratatui buffer.
    /// Used in tmux where Unicode placeholders don't survive.
    /// Must be called AFTER `terminal.draw()` so it overlays the rendered frame.
    pub fn place_direct(&self, area: Rect, font_width: u32, font_height: u32) {
        let mut stdout = std::io::stdout().lock();

        // Delete previous placement before placing again
        let _ = tmux::write_kitty_cmd(&mut stdout, &format!("a=d,d=i,i={IMAGE_ID},q=2"));

        // Crop source image to the visible portion
        let src_y = self.scroll_row as u32 * font_height;
        let src_w = (area.width as u32 * font_width).min(self.width);
        let src_h = (area.height as u32 * font_height).min(self.height.saturating_sub(src_y));

        log::debug!(
            "place_direct: area={}x{}+{}+{}, src_crop={}x{}@y={}, font={}x{}",
            area.width, area.height, area.x, area.y,
            src_w, src_h, src_y, font_width, font_height
        );

        // Move cursor + place image inside the same DCS passthrough,
        // so the outer terminal (Ghostty) sees both the cursor move and the placement.
        let row = area.top() + 1;
        let col = area.left() + 1;

        let params = format!(
            "a=p,i={IMAGE_ID},x=0,y={src_y},w={src_w},h={src_h},c={},r={},C=1,q=2",
            area.width, area.height
        );

        if tmux::in_tmux() {
            // Cursor move + Kitty APC in one DCS passthrough sequence.
            // All ESC bytes inside DCS must be doubled.
            let _ = write!(
                stdout,
                "\x1bPtmux;\x1b\x1b[{row};{col}H\x1b\x1b_G{params}\x1b\x1b\\\x1b\\"
            );
        } else {
            let _ = write!(stdout, "\x1b[{row};{col}H");
            let _ = write!(stdout, "\x1b_G{params}\x1b\\");
        }
        let _ = stdout.flush();
    }

    pub fn set_max_rows(&mut self, max_rows: u16) {
        if max_rows > DIACRITICS.len() as u16 {
            log::warn!("Image rows ({}) exceed diacritic limit ({}), display will be clipped", max_rows, DIACRITICS.len());
        }
        self.max_rows = max_rows;
    }

    pub fn max_rows(&self) -> u16 {
        self.max_rows
    }

    pub fn scroll_down(&mut self, rows: u16, visible_rows: u16) {
        let max_scroll = self.max_rows.saturating_sub(visible_rows);
        self.scroll_row = (self.scroll_row + rows).min(max_scroll);
    }

    pub fn scroll_up(&mut self, rows: u16) {
        self.scroll_row = self.scroll_row.saturating_sub(rows);
    }

    pub fn delete(&self) {
        let mut stdout = std::io::stdout().lock();
        let _ = tmux::write_kitty_cmd(&mut stdout, &format!("a=d,d=I,i={IMAGE_ID},q=2"));
        let _ = stdout.flush();
    }
}

impl Drop for KittyImage {
    fn drop(&mut self) {
        self.delete();
    }
}
