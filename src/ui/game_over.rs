//! Keep a terminal game-over screen visible until it is dismissed.
use crate::{
    game::Game,
    ui::{japanese::JapanesePrint, theme},
};
use bracket_lib::prelude as bl;
impl Game {
    pub(crate) fn render_game_over(&self, ctx: &mut bl::BTerm) {
        let active = ctx.active_console;
        if let Some(popup) = self.popup_console {
            ctx.set_active_console(popup);
            ctx.cls_bg(bl::RGBA::from_f32(0.0, 0.0, 0.0, 0.0));
            ctx.set_all_fg_alpha(0.0);
            ctx.set_active_console(active);
        }
        ctx.cls_bg(bl::RGB::named(theme::BACKGROUND));
        ctx.print_ja_color(30, 17, bl::RGB::named(theme::ACCENT), "GAME OVER");
        ctx.print_ja(16, 21, "正気度が尽き、あなたはグールになりました。");
        ctx.print_ja(
            16,
            24,
            format!(
                "レベル:{}  正気度:0/{}",
                self.actor(self.player_id)
                    .map_or(1, |a| a.progression.level),
                self.actor(self.player_id).map_or(0, |a| a.max_sanity)
            ),
        );
        ctx.print_ja(16, 28, "Enter / Esc / Ctrl+Q: 終了");
    }
    pub(crate) fn game_over_tick(&self, ctx: &mut bl::BTerm) {
        let mut close = false;
        bl::INPUT.lock().for_each_message(|e| {
            if matches!(e, bl::BEvent::CloseRequested) {
                close = true;
            }
        });
        self.render_game_over(ctx);
        if close
            || matches!(
                ctx.key,
                Some(bl::VirtualKeyCode::Return | bl::VirtualKeyCode::Escape)
            )
            || (ctx.control && ctx.key == Some(bl::VirtualKeyCode::Q))
        {
            ctx.quitting = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bl::GameState;
    fn context(key: bl::VirtualKeyCode) -> bl::BTerm {
        bl::BTerm {
            width_pixels: 1280,
            height_pixels: 800,
            original_height_pixels: 800,
            original_width_pixels: 1280,
            fps: 60.0,
            frame_time_ms: 16.0,
            active_console: 0,
            key: Some(key),
            mouse_pos: (0, 0),
            left_click: false,
            shift: false,
            control: false,
            alt: false,
            web_button: None,
            quitting: false,
            post_scanlines: false,
            post_screenburn: false,
            screen_burn_color: bl::RGB::named(bl::BLACK),
            mouse_visible: true,
        }
    }

    #[test]
    fn terminal_screen_waits_for_exit_and_does_not_run_turns() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        let mut g = Game::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        g.damage_sanity(g.player_id, 1000).unwrap();
        let mut ctx = context(bl::VirtualKeyCode::Space);
        ctx.active_console = ctx.register_console(bl::SimpleConsole::init(80, 50), 0);
        g.set_popup_console(ctx.register_console(bl::SimpleConsole::init(80, 50), 0));
        let turn = g.turn;
        g.tick(&mut ctx);
        assert!(!ctx.quitting);
        assert!(g.game_over && !g.running);
        assert_eq!(g.turn, turn);
        ctx.key = Some(bl::VirtualKeyCode::Return);
        g.tick(&mut ctx);
        assert!(ctx.quitting);
        assert_eq!(g.turn, turn);
    }
}
