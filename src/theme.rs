//! UI colors based on gruvbox. Entity PNGs retain Ash & Rust 32.
pub const BACKGROUND: (u8, u8, u8) = (40, 40, 40);
pub const TEXT: (u8, u8, u8) = (213, 196, 161);
pub const ACCENT: (u8, u8, u8) = (215, 153, 33);
pub const BORDER: (u8, u8, u8) = (131, 165, 152);
pub const MUTED: (u8, u8, u8) = (124, 111, 100);
pub const DISABLED: (u8, u8, u8) = (80, 73, 69);
pub const DANGER: (u8, u8, u8) = (204, 36, 29);
pub const SUCCESS: (u8, u8, u8) = (152, 151, 26);

pub const MAP_TOP: i32 = 4;
pub const MAP_ROWS: i32 = 38;
pub const COMMAND_TOP: i32 = MAP_TOP + MAP_ROWS;
pub const LOG_TOP: i32 = COMMAND_TOP + 2;
pub const LOG_ROWS: usize = 4;
pub const SLOTS_ROW: i32 = 49;

/// Fill explicitly: bracket-lib's draw_box fills interiors with black.
pub fn draw_box(
    ctx: &mut bracket_lib::prelude::BTerm,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    fg: (u8, u8, u8),
    bg: (u8, u8, u8),
) {
    for row in y..=y + height {
        for column in x..=x + width {
            ctx.set(column, row, fg, bg, bracket_lib::prelude::to_cp437(' '));
        }
    }
    ctx.draw_hollow_box(x, y, width, height, fg, bg);
}
