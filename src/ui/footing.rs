//! プレイヤー自身を除く足元の一覧。表示・スクロールで時間を進めない。
use crate::{
    game::{Game, Panel},
    model::config::*,
    model::entity::*,
    ui::inspect::wrap_description,
    ui::japanese::JapanesePrint,
};
use bracket_lib::prelude as bl;
impl Game {
    pub(crate) fn footing_entries(&self) -> Vec<(Id, String)> {
        let Some(position) = self.position(self.player_id) else {
            return vec![];
        };
        self.inspected_entities(position)
            .into_iter()
            .filter(|id| *id != self.player_id)
            .map(|id| {
                let entity = &self.entities[&id];
                let kind = match entity {
                    Entity::Movable(_) => "生物",
                    Entity::Item(_) => "品物",
                    Entity::Static(s) => match s.role {
                        StaticRole::Terrain => "地形",
                        StaticRole::Obstacle => "固定物",
                        StaticRole::Facility => "設備",
                    },
                };
                let quantity = match entity {
                    Entity::Item(i) => format!(" x{}", i.quantity),
                    _ => String::new(),
                };
                let mut text = format!("[{kind}] {}{quantity}", self.display_name(id));
                let summary = self.footing_item_summary(id);
                if !summary.is_empty() {
                    text.push('\n');
                    text.push_str(&summary);
                }
                (id, text)
            })
            .collect()
    }
    fn footing_item_summary(&self, id: Id) -> String {
        let definition = self.definition(id);
        let Some(item) = &definition.item else {
            return String::new();
        };
        let mut parts = Vec::new();
        if let Some(weapon) = &item.weapon {
            parts.push(format!(
                "威力{} 貫通{} 射程{}",
                self.weapon_damage(id)
                    .unwrap_or_else(|_| weapon.damage.clone()),
                weapon.base_pv,
                weapon.range
            ));
        }
        if let Some(armor) = &item.armor {
            parts.push(format!("防御+{} 回避{:+}", armor.av, armor.dv));
        }
        if parts.is_empty() && !definition.common.description.trim().is_empty() {
            // Lua側だけで定義された副作用や使用条件も説明から伝える。
            parts.push(definition.common.description.clone());
        }
        if let Some(nutrition) = &item.nutrition {
            for (name, amount) in [
                ("タンパク質", nutrition.protein),
                ("炭水化物", nutrition.carbohydrate),
                ("ビタミン", nutrition.vitamin),
            ] {
                if amount > 0 {
                    parts.push(format!("{name}+{amount}"));
                }
            }
        }
        if let Some(effect) = &item.use_effect {
            match (effect.kind.as_str(), effect.amount) {
                ("heal", Some(amount)) => parts.push(format!("HP+{amount}")),
                ("restore_need", Some(amount)) if effect.need.as_deref() == Some("hydration") => {
                    parts.push(format!("水分+{amount}"));
                }
                _ => {}
            }
        }
        for function in &item.tool_functions {
            parts.push(format!(
                "{}Lv{}",
                crate::game::crafting::tool_name(function),
                crate::game::crafting::tool_level(item, function)
            ));
        }
        parts.join(" / ")
    }
    pub(crate) fn footing_lines(&self) -> Vec<String> {
        let width = (76 - self.map.width).max(1) as usize;
        self.footing_entries()
            .into_iter()
            .flat_map(|(_, text)| wrap_description(&text, width))
            .collect()
    }
    pub(crate) fn footing_page_size(&self) -> usize {
        (crate::ui::theme::MAP_ROWS - 5).max(1) as usize
    }
    pub(crate) fn scroll_footing(&mut self, delta: i32) -> Result<()> {
        ensure(
            matches!(self.panel, Panel::World),
            "footing scroll requires world panel",
        )?;
        let max = self
            .footing_lines()
            .len()
            .saturating_sub(self.footing_page_size());
        self.footing_scroll =
            (self.footing_scroll.min(max) as i64 + i64::from(delta)).clamp(0, max as i64) as usize;
        Ok(())
    }
    pub(crate) fn render_footing(&self, ctx: &mut bl::BTerm) {
        let x = self.map.width + 2;
        let count = self.footing_entries().len();
        let lines = self.footing_lines();
        let page = self.footing_page_size();
        let start = self.footing_scroll.min(lines.len().saturating_sub(page));
        let end = (start + page).min(lines.len());
        crate::ui::theme::draw_box(
            ctx,
            x,
            4,
            79 - x,
            crate::ui::theme::MAP_ROWS - 1,
            crate::ui::theme::BORDER,
            crate::ui::theme::BACKGROUND,
        );
        ctx.print_ja_color(
            x + 1,
            5,
            bl::RGB::named(crate::ui::theme::ACCENT),
            format!("足元 {count}件"),
        );
        ctx.print_ja(
            x + 1,
            6,
            format!(
                "{}-{end}/{}行",
                if lines.is_empty() { 0 } else { start + 1 },
                lines.len()
            ),
        );
        for (row, line) in lines.iter().skip(start).take(page).enumerate() {
            ctx.print_ja(x + 1, 7 + row as i32, line);
        }
        ctx.print_ja(
            x + 1,
            crate::ui::theme::MAP_TOP + crate::ui::theme::MAP_ROWS - 2,
            "PgUp/PgDn:一覧",
        );
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
            screen_burn_color: bl::RGB::named(crate::ui::theme::BACKGROUND),
            mouse_visible: true,
        }
    }
    #[test]
    fn includes_layers_and_quantity_but_excludes_self_ids_inventory_and_neighbors() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        let mut g = game();
        let p = g.position(g.player_id).unwrap();
        let terrain = g.spawn("terrain.field", Some(p), None, 1).unwrap();
        let lamp = g.spawn("obstacle.lamp", Some(p), None, 1).unwrap();
        let item = g.spawn("ammo.pistol", Some(p), None, 17).unwrap();
        let second = g.spawn("ammo.pistol", Some(p), None, 4).unwrap();
        let held = g.spawn("item.apple", None, Some(g.player_id), 1).unwrap();
        let neighbor = g
            .spawn("item.apple", Some(p.offset(1, 0)), None, 1)
            .unwrap();
        let entries = g.footing_entries();
        let ids: Vec<_> = entries.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids.len(), 5);
        for id in [terrain, lamp, item, second] {
            assert!(ids.contains(&id));
        }
        assert!(!ids.contains(&g.player_id));
        assert!(entries.iter().all(|(_, text)| !text.contains('#')));
        assert!(!ids.contains(&held));
        assert!(!ids.contains(&neighbor));
        assert!(
            entries
                .iter()
                .any(|(id, text)| *id == item && text.contains("x17"))
        );
        assert!(
            entries
                .iter()
                .any(|(id, text)| *id == second && text.contains("x4"))
        );
        g.pickup(g.player_id, item).unwrap();
        assert!(!g.footing_entries().iter().any(|(id, _)| *id == item));
        g.drop_item(g.player_id, item).unwrap();
        assert!(g.footing_entries().iter().any(|(id, _)| *id == item));
    }
    #[test]
    fn item_effects_show_combat_recovery_and_food_risks() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        let mut g = game();
        let pos = g.position(g.player_id).unwrap();
        for definition in [
            "item.knife",
            "item.leather_armor",
            "item.syringe",
            "item.dirty_water",
            "item.apple",
        ] {
            let id = g.spawn(definition, Some(pos), None, 1).unwrap();
            let summary = g.footing_item_summary(id);
            match definition {
                "item.knife" => assert!(
                    summary.contains("威力")
                        && summary.contains("貫通")
                        && summary.contains("射程")
                ),
                "item.leather_armor" => {
                    assert!(summary.contains("防御+1") && summary.contains("回避+0"))
                }
                "item.syringe" => assert!(summary.contains("HP+15")),
                "item.dirty_water" => {
                    assert!(summary.contains("水分+25") && summary.contains("HPに3の被害"))
                }
                "item.apple" => {
                    assert!(summary.contains("炭水化物+") && summary.contains("ビタミン+"))
                }
                _ => unreachable!(),
            }
            assert!(
                g.footing_entries()
                    .iter()
                    .any(|(entry, text)| *entry == id && text.contains(&summary))
            );
        }
    }
    #[test]
    fn paging_reaches_every_line_without_a_turn_and_movement_resets_it() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        let mut g = game();
        let p = g.player_id;
        let pos = g.position(p).unwrap();
        for _ in 0..40 {
            g.spawn("item.apple", Some(pos), None, 1).unwrap();
        }
        let lines = g.footing_lines();
        assert!(lines.len() > g.footing_page_size());
        let mut covered = std::collections::BTreeSet::new();
        loop {
            let start = g.footing_scroll;
            covered.extend(start..(start + g.footing_page_size()).min(lines.len()));
            g.pending_action = Some(Action::ScrollFooting {
                delta: g.footing_page_size() as i32,
            });
            g.update();
            assert_eq!(g.turn, 0);
            if start == g.footing_scroll {
                break;
            }
        }
        assert_eq!(covered.len(), lines.len());
        g.move_to(p, pos.offset(0, 1)).unwrap();
        assert_eq!(g.footing_scroll, 0);
        assert!(
            g.footing_entries()
                .iter()
                .all(|(_, text)| !text.contains("りんご"))
        );
    }
    #[test]
    fn page_keys_scroll_and_console_renders_a_quantity() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        let mut g = game();
        let pos = g.position(g.player_id).unwrap();
        for _ in 0..20 {
            g.spawn("item.apple", Some(pos), None, 1).unwrap();
        }
        let mut ctx = context(bl::VirtualKeyCode::PageDown);
        g.input(&ctx);
        assert!(matches!(g.pending_action,Some(Action::ScrollFooting{delta}) if delta>0));
        g.update();
        assert!(g.footing_scroll > 0);
        assert_eq!(g.turn, 0);
        ctx.key = Some(bl::VirtualKeyCode::PageUp);
        g.input(&ctx);
        g.update();
        assert_eq!(g.footing_scroll, 0);
        // Use only the ammo item and the existing player/ground for a small rendered list.
        let apples: Vec<_> = g
            .footing_entries()
            .into_iter()
            .filter(|(id, _)| g.definition(*id).id == "item.apple")
            .map(|(id, _)| id)
            .collect();
        for id in apples {
            g.remove(id);
        }
        g.spawn("ammo.pistol", Some(pos), None, 17).unwrap();
        ctx.active_console = ctx.register_console(bl::SimpleConsole::init(80, 50), 0);
        g.render(&mut ctx);
        let backend = bl::BACKEND_INTERNAL.lock();
        let layer = backend.consoles[ctx.active_console].console.to_xp_layer();
        let mut rendered = String::new();
        for y in 7..g.map.height + 2 {
            for x in g.map.width + 3..79 {
                let ch = layer.get(x as usize, y as usize).unwrap().ch;
                if let Some(c) = char::from_u32(ch) {
                    rendered.push(c);
                }
            }
        }
        assert!(rendered.contains("x17"));
        assert_eq!(g.turn, 0);
    }
}
