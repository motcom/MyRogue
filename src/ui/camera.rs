//! Map-only zoom and camera. Screen UI stays on its unscaled console.
use crate::{
    game::{Game, Panel},
    model::config::Pos,
};
use bracket_lib::prelude as bl;

#[derive(Clone, Copy, Debug)]
pub(crate) struct MapCamera {
    pub origin: Pos,
    pub columns: i32,
    pub rows: i32,
}

pub(crate) fn zoom_delta(
    events: &[bl::BEvent],
    key: Option<bl::VirtualKeyCode>,
    shift: bool,
) -> Option<i32> {
    // Character events handle both JIS '+' (Shift+semicolon) and US '+' (Shift+=).
    let character = events.iter().rev().find_map(|e| match e {
        bl::BEvent::Character { c: '+' } => Some(1),
        bl::BEvent::Character { c: '-' } => Some(-1),
        _ => None,
    });
    let key_delta = |key| match key {
        Some(bl::VirtualKeyCode::Add) => Some(1),
        Some(bl::VirtualKeyCode::Subtract | bl::VirtualKeyCode::Minus) => Some(-1),
        Some(bl::VirtualKeyCode::Equals) if shift => Some(1),
        _ => None,
    };
    // BTerm::key is cleared by a release, even if the press was in this frame.
    // Prefer text for keyboard layouts, then preserve short physical key taps.
    character
        .or_else(|| {
            events.iter().rev().find_map(|event| match event {
                bl::BEvent::KeyboardInput {
                    key, pressed: true, ..
                } => key_delta(Some(*key)),
                _ => None,
            })
        })
        .or_else(|| key_delta(key))
}

impl Game {
    pub fn set_map_consoles(&mut self, world: usize, overlay: usize) {
        self.map_console = Some(world);
        self.map_overlay_console = Some(overlay);
    }

    pub(crate) fn map_camera(&self) -> MapCamera {
        let zoom = i32::from(self.map_zoom);
        let columns = (self.map.width.min(70) / zoom).max(1);
        let rows = (crate::ui::theme::MAP_ROWS / zoom)
            .max(1)
            .min(self.map.height);
        let player = self.position(self.player_id).unwrap_or_default();
        let focus = match self.panel {
            Panel::Inspect { position, .. } | Panel::ItemTarget { position, .. } => position,
            Panel::Targeting { active_entity } => self.position(active_entity).unwrap_or(player),
            _ => player,
        };
        MapCamera {
            origin: Pos {
                z: focus.z,
                x: (focus.x - columns / 2).clamp(0, self.map.width - columns),
                y: (focus.y - rows / 2).clamp(0, self.map.height - rows),
            },
            columns,
            rows,
        }
    }

    /// Coordinates on the map console, or legacy UI coordinates in headless tests.
    pub(crate) fn map_draw_position(&self, p: Pos) -> Option<Pos> {
        let camera = self.map_camera();
        let local = Pos {
            z: p.z,
            x: p.x - camera.origin.x,
            y: p.y - camera.origin.y,
        };
        if local.x < 0 || local.y < 0 || local.x >= camera.columns || local.y >= camera.rows {
            return None;
        }
        Some(if self.map_console.is_some() {
            local
        } else {
            local.offset(1, 4)
        })
    }

    pub(crate) fn prepare_map_console(&self, ctx: &mut bl::BTerm, console: usize) {
        ctx.set_active_console(console);
        ctx.cls();
        let scale = f32::from(self.map_zoom);
        if ctx.get_scale() != (scale, 0, 0) {
            ctx.set_scale(scale, 0, 0);
        }
        ctx.set_offset(1.0, -4.0);
        // The OpenGL sparse cache compares tiles only (even set_char_size does
        // not invalidate it). A transparent tile outside the map makes scale
        // part of that comparison, including when the visible map is empty.
        let invisible = bl::RGBA::from_f32(scale / 4.0, 0.0, 0.0, 0.0);
        ctx.set(79, 49, invisible, invisible, 0);
    }

    pub(crate) fn map_highlight(&self, ctx: &mut bl::BTerm, p: Pos, color: bl::RGB) {
        let Some(p) = self.map_draw_position(p) else {
            return;
        };
        let ui = ctx.active_console;
        if let Some(console) = self.map_overlay_console {
            ctx.set_active_console(console);
        }
        ctx.set_bg(p.x, p.y, color);
        ctx.set_active_console(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::Action;

    fn game() -> Game {
        Game::load_test_map(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

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
    fn plus_and_minus_input_update_only_the_camera_and_respect_modifiers() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        let mut g = game();
        let mut ctx = context(bl::VirtualKeyCode::Equals);
        ctx.shift = true;
        g.input(&ctx);
        assert!(matches!(
            g.pending_action,
            Some(Action::ZoomMap { delta: 1 })
        ));
        assert_eq!(g.map_zoom, 1, "input must not update zoom directly");
        g.update();
        assert_eq!(g.map_zoom, 2);
        ctx.key = Some(bl::VirtualKeyCode::Minus);
        ctx.shift = false;
        g.input(&ctx);
        g.update();
        assert_eq!(g.map_zoom, 1);
        assert_eq!(g.turn, 0);
        ctx.key = Some(bl::VirtualKeyCode::Equals);
        ctx.shift = true;
        ctx.control = true;
        g.input(&ctx);
        assert!(!matches!(g.pending_action, Some(Action::ZoomMap { .. })));
    }

    #[test]
    fn jis_us_and_numpad_zoom_keys_are_one_action_per_frame() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        use bl::{BEvent as E, VirtualKeyCode as K};
        assert_eq!(
            zoom_delta(&[E::Character { c: '+' }], Some(K::Semicolon), true),
            Some(1)
        );
        assert_eq!(
            zoom_delta(&[E::Character { c: '+' }], Some(K::Equals), true),
            Some(1)
        );
        assert_eq!(zoom_delta(&[], Some(K::Equals), true), Some(1));
        assert_eq!(zoom_delta(&[], Some(K::Equals), false), None);
        assert_eq!(zoom_delta(&[], Some(K::Add), false), Some(1));
        assert_eq!(zoom_delta(&[], Some(K::Subtract), false), Some(-1));
        assert_eq!(
            zoom_delta(&[E::Character { c: '-' }], Some(K::Minus), false),
            Some(-1)
        );
    }

    #[test]
    fn short_zoom_taps_survive_release_and_do_not_double_count_text() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        use bl::{BEvent as E, VirtualKeyCode as K};
        for (key, character, shift, expected) in [
            (K::Minus, '-', false, -1),
            (K::Subtract, '-', false, -1),
            (K::Add, '+', false, 1),
            (K::Equals, '+', true, 1),
        ] {
            let press = E::KeyboardInput {
                key,
                scan_code: 0,
                pressed: true,
            };
            let release = E::KeyboardInput {
                key,
                scan_code: 0,
                pressed: false,
            };
            assert_eq!(
                zoom_delta(&[press.clone(), release.clone()], None, shift),
                Some(expected)
            );
            assert_eq!(
                zoom_delta(
                    &[press, E::Character { c: character }, release.clone()],
                    None,
                    shift
                ),
                Some(expected)
            );
            assert_eq!(zoom_delta(&[release], None, shift), None);
        }
        // JIS '+' is supplied by text even if Shift has already been released.
        assert_eq!(zoom_delta(&[E::Character { c: '+' }], None, false), Some(1));
    }

    #[test]
    fn empty_map_scale_changes_invalidate_sparse_tile_cache() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        let mut g = game();
        let mut ctx = context(bl::VirtualKeyCode::Minus);
        let console = ctx.register_console(bl::SparseConsole::init(80, 50), 0);
        let mut previous = None;
        for zoom in [1, 2, 3, 4, 3, 2, 1] {
            g.map_zoom = zoom;
            g.prepare_map_console(&mut ctx, console);
            let backend = bl::BACKEND_INTERNAL.lock();
            let layer = backend.consoles[console]
                .console
                .as_any()
                .downcast_ref::<bl::SparseConsole>()
                .unwrap();
            assert_eq!(layer.scale, f32::from(zoom));
            assert!(
                layer
                    .tiles
                    .iter()
                    .all(|tile| tile.fg.a == 0.0 && tile.bg.a == 0.0)
            );
            if let Some(previous) = &previous {
                // This is the comparison used by the OpenGL vertex cache.
                assert!(
                    layer.tiles != *previous,
                    "unchanged tiles skip the scale update"
                );
            }
            previous = Some(layer.tiles.clone());
        }
    }

    #[test]
    fn full_render_preserves_zoom_cache_marker_in_both_map_layers() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        let mut g = game();
        let mut ctx = context(bl::VirtualKeyCode::Add);
        let world = ctx.register_console(bl::SparseConsole::init(80, 50), 0);
        let overlay = ctx.register_console(bl::SparseConsole::init(80, 50), 0);
        let ui = ctx.register_console(bl::SimpleConsole::init(80, 50), 0);
        g.set_map_consoles(world, overlay);
        crate::media::textures::TextureCatalog::load(std::path::Path::new(env!(
            "CARGO_MANIFEST_DIR"
        )))
        .unwrap()
        .install(&mut g, world);
        let position = g.position(g.player_id);
        for textured in [true, false] {
            if !textured {
                g.texture_renderer = None;
            }
            for zoom in [1, 2, 3, 4, 3, 2, 1] {
                g.map_zoom = zoom;
                ctx.set_active_console(ui);
                g.render(&mut ctx);
                assert_eq!(ctx.active_console, ui);
                assert_eq!(ctx.get_scale().0, 1.0, "UI must stay unscaled");
                let backend = bl::BACKEND_INTERNAL.lock();
                for console in [world, overlay] {
                    let layer = backend.consoles[console]
                        .console
                        .as_any()
                        .downcast_ref::<bl::SparseConsole>()
                        .unwrap();
                    assert_eq!(layer.scale, f32::from(zoom));
                    let marker = layer
                        .tiles
                        .first()
                        .expect("cache marker must survive rendering");
                    assert_eq!(
                        marker.fg,
                        bl::RGBA::from_f32(f32::from(zoom) / 4.0, 0.0, 0.0, 0.0)
                    );
                    assert_eq!(marker.bg.a, 0.0);
                }
            }
        }
        assert_eq!(g.position(g.player_id), position);
        assert_eq!(g.turn, 0);
    }

    #[test]
    fn zoom_is_bounded_without_spending_turns_or_moving_the_player() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        let mut g = game();
        let position = g.position(g.player_id);
        let turn = g.turn;
        let food = g.actor(g.player_id).unwrap().nutrition.clone();
        for _ in 0..10 {
            g.pending_action = Some(Action::ZoomMap { delta: 1 });
            g.update();
        }
        assert_eq!(g.map_zoom, 4);
        for _ in 0..10 {
            g.pending_action = Some(Action::ZoomMap { delta: -1 });
            g.update();
        }
        assert_eq!(g.map_zoom, 1);
        assert_eq!(g.turn, turn);
        assert_eq!(g.position(g.player_id), position);
        assert_eq!(g.actor(g.player_id).unwrap().nutrition, food);
    }

    #[test]
    fn camera_follows_cursor_and_clips_whole_tiles_inside_the_map_area() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        let mut g = game();
        g.set_map_consoles(0, 1);
        g.map_zoom = 3;
        for focus in [
            Pos { x: 0, y: 0, z: 0 },
            Pos {
                x: g.map.width - 1,
                y: g.map.height - 1,
                z: 0,
            },
        ] {
            g.panel = Panel::Inspect {
                position: focus,
                selected: 0,
            };
            let camera = g.map_camera();
            assert!(g.map_draw_position(focus).is_some());
            assert!(camera.columns * 3 <= g.map.width);
            assert!(camera.rows * 3 <= g.map.height);
            assert!(g.map_draw_position(camera.origin.offset(-1, 0)).is_none());
            assert!(
                g.map_draw_position(camera.origin.offset(camera.columns, 0))
                    .is_none()
            );
            assert!(
                g.map_draw_position(camera.origin.offset(0, camera.rows))
                    .is_none()
            );
        }
        g.map_zoom = 1;
        assert_eq!(g.map_camera().origin.x, 0);
        assert_eq!(g.map_camera().origin.y, 0);
    }
    #[test]
    fn expanded_layout_keeps_commands_logs_and_slots_below_the_map() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        let mut g = game();
        let mut ctx = context(bl::VirtualKeyCode::Escape);
        let ui = ctx.register_console(bl::SimpleConsole::init(80, 50), 0);
        ctx.set_active_console(ui);
        g.messages.clear();
        for i in 0..6 {
            g.log(format!("log{i}"));
        }
        g.render(&mut ctx);
        {
            let backend = bl::BACKEND_INTERNAL.lock();
            let layer = backend.consoles[ui]
                .console
                .as_any()
                .downcast_ref::<bl::SimpleConsole>()
                .unwrap();
            assert_eq!(g.map_camera().rows, 38);
            assert_eq!(
                layer.tiles[bl::Console::at(layer, 1, crate::ui::theme::COMMAND_TOP)].glyph,
                bl::to_cp437('h')
            );
            assert_eq!(
                layer.tiles[bl::Console::at(layer, 4, crate::ui::theme::LOG_TOP)].glyph,
                bl::to_cp437('2')
            );
            assert_eq!(
                layer.tiles[bl::Console::at(layer, 0, crate::ui::theme::SLOTS_ROW)].glyph,
                bl::to_cp437('1')
            );
            assert_eq!(
                layer.tiles[bl::Console::at(layer, g.map.width + 2, 4)].fg,
                bl::RGB::named(crate::ui::theme::BORDER).into()
            );
        }
        // Optional CPU-rendered previews for reviewing layout without OpenGL.
        if let Some(directory) = std::env::var_os("ROGUE_UI_CAPTURE_DIR") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            let font = image::load_from_memory(include_bytes!("../assets/japanese16x16.png"))
                .unwrap()
                .to_rgba8();
            for (name, panel, selection) in [
                ("world", Panel::World, 0),
                ("crafting", Panel::Crafting, 0),
                (
                    "skills",
                    Panel::Properties,
                    g.property_lines()
                        .iter()
                        .position(|l| l == "【技能】")
                        .unwrap(),
                ),
                ("properties", Panel::Properties, 0),
                ("inventory", Panel::Inventory, 0),
                ("interaction", Panel::InteractionDirection, 0),
            ] {
                g.panel = panel;
                g.selection = selection;
                g.render(&mut ctx);
                let backend = bl::BACKEND_INTERNAL.lock();
                let layer = backend.consoles[ui]
                    .console
                    .as_any()
                    .downcast_ref::<bl::SimpleConsole>()
                    .unwrap();
                let mut canvas = image::RgbaImage::new(1280, 800);
                for (index, tile) in layer.tiles.iter().enumerate() {
                    let gx = u32::from(tile.glyph) % 32 * 16;
                    let gy = u32::from(tile.glyph) / 32 * 16;
                    for y in 0..16 {
                        for x in 0..16 {
                            let mask = font.get_pixel(gx + x, gy + y).0;
                            let alpha = f32::from(mask[3]) / 255.0 * f32::from(mask[0]) / 255.0;
                            let bg = tile.bg;
                            let fg = tile.fg;
                            let color = [
                                fg.r * alpha + bg.r * (1.0 - alpha),
                                fg.g * alpha + bg.g * (1.0 - alpha),
                                fg.b * alpha + bg.b * (1.0 - alpha),
                            ];
                            canvas.put_pixel(
                                (index as u32 % 80) * 16 + x,
                                (49 - index as u32 / 80) * 16 + y,
                                image::Rgba([
                                    (color[0] * 255.0) as u8,
                                    (color[1] * 255.0) as u8,
                                    (color[2] * 255.0) as u8,
                                    255,
                                ]),
                            );
                        }
                    }
                }
                canvas.save(directory.join(format!("{name}.png"))).unwrap();
            }
        }
    }
}
