//! Render the world and active panel.
use crate::{
    game::{Game, Panel},
    model::entity::*,
    ui::japanese::{JapanesePrint, message},
};
use bracket_lib::prelude as bl;
impl Game {
    /// Attach a separately registered console above the world for translucent popups.
    pub fn set_popup_console(&mut self, console: usize) {
        self.popup_console = Some(console);
    }
    pub fn render(&self, ctx: &mut bl::BTerm) {
        if self.game_over {
            self.render_game_over(ctx);
            return;
        }
        let world_console = ctx.active_console;
        if let Some(popup_console) = self.popup_console {
            ctx.set_active_console(popup_console);
            ctx.cls_bg(bl::RGBA::from_f32(0.0, 0.0, 0.0, 0.0));
            ctx.set_all_fg_alpha(0.0);
            ctx.set_active_console(world_console);
        }
        if self.map_console.is_some() || self.texture_renderer.is_some() {
            ctx.cls_bg(bl::RGBA::from_f32(0.0, 0.0, 0.0, 0.0));
            ctx.set_all_fg_alpha(0.0);
        } else {
            ctx.cls_bg(bl::RGB::named(crate::ui::theme::BACKGROUND));
        }
        ctx.print_ja(
            1,
            0,
            format!(
                "ターン:{}  {}",
                self.turn,
                self.actor(self.player_id)
                    .map_or_else(String::new, |a| format!(
                        "正気度:{}/{}",
                        a.sanity, a.max_sanity
                    ))
            ),
        );
        if let Some(a) = self.actor(self.player_id) {
            let hp = &a.hp;
            let [protein, carbohydrate, vitamin] = super::displayed_nutrition(&a.nutrition);
            let bleeding = a.bleeding_damage();
            let bleeding_status = if bleeding > 0 {
                format!("  出血-{}", bleeding)
            } else {
                String::new()
            };
            ctx.print_ja(
                1,
                1,
                format!(
                    "HP:{}/{}{bleeding_status}  水分:{}  タンパク質:{}/100  炭水化物:{}/100  ビタミン:{}/100{}",
                    hp.hp,
                    hp.max_hp,
                    a.hydration.current,
                    protein,
                    carbohydrate,
                    vitamin,
                    match a.respiratory_status() {
                        Some("pneumonia") => "  肺炎",
                        Some("cold") => "  風邪",
                        _ => "",
                    }
                ),
            );
            if let Some(id) = self.view().active_weapon(a.id, false) {
                let definition = self.definition(id);
                let ammo = match (
                    &self.entities[&id],
                    definition.item.as_ref().and_then(|i| i.weapon.as_ref()),
                ) {
                    (Entity::Item(item), Some(weapon)) => weapon
                        .magazine_capacity
                        .map(|capacity| format!("  装填:{}/{capacity}", item.loaded_ammo))
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                ctx.print_ja(
                    1,
                    2,
                    format!(
                        "{}  装備:{}  {}{ammo}",
                        self.combat_summary(self.player_id),
                        self.display_name(id),
                        self.held_weapon_summary(self.player_id, id)
                    ),
                );
            } else {
                ctx.print_ja(1, 2, self.combat_summary(self.player_id));
            }
        }
        if let Some(a) = self.actor(self.player_id) {
            ctx.print_ja(
                1,
                3,
                format!(
                    "v:速度切替  {}  スタミナ:{}/{}  {}",
                    if a.running_mode { "走行" } else { "歩行" },
                    a.stamina,
                    a.max_stamina,
                    self.level_progress_label()
                ),
            );
        }
        let view = self.visible_positions();
        if let Some(console) = self.motion_console {
            self.prepare_map_console(ctx, console);
        }
        if let Some(console) = self.map_overlay_console {
            self.prepare_map_console(ctx, console);
        }
        if let Some(console) = self.map_console {
            self.prepare_map_console(ctx, console);
        }
        if let Some(textures) = &self.texture_renderer {
            textures.render(self, ctx, &view);
        } else {
            let hidden = self.motion_actor_id(&view);
            let mut visible: Vec<_> = self
                .entities
                .values()
                .filter(|e| {
                    Some(e.id()) != hidden && e.map_position().is_some_and(|p| view.contains(&p))
                })
                .collect();
            visible.sort_by_key(|e| (self.definition(e.id()).common.appearance.layer, e.id()));
            for e in visible {
                if let Some(p) = e.map_position().and_then(|p| self.map_draw_position(p)) {
                    let a = &self.definition(e.id()).common.appearance;
                    let glyph = e
                        .script()
                        .glyph
                        .as_deref()
                        .unwrap_or(&a.glyph)
                        .chars()
                        .next()
                        .unwrap();
                    ctx.set(
                        p.x,
                        p.y,
                        bl::RGB::from_u8(a.fg[0], a.fg[1], a.fg[2]),
                        bl::RGB::from_u8(a.bg[0], a.bg[1], a.bg[2]),
                        bl::to_cp437(glyph),
                    );
                }
            }
        }
        ctx.set_active_console(world_console);
        if let Some(console) = self.map_overlay_console {
            ctx.set_active_console(console);
        }
        for label in &self.map_labels {
            for (offset, c) in label.text.chars().enumerate() {
                let p = label.position.offset(offset as i32, 0);
                if view.contains(&p)
                    && !self
                        .at(p)
                        .iter()
                        .any(|id| self.definition(*id).common.appearance.layer > 0)
                    && let Some(p) = self.map_draw_position(p)
                {
                    ctx.print_ja_color(
                        p.x,
                        p.y,
                        bl::RGB::named(crate::ui::theme::BORDER),
                        c.to_string(),
                    );
                }
            }
        }
        self.render_animations(ctx, &view);
        ctx.set_active_console(world_console);
        self.render_footing(ctx);
        ctx.print_ja(60, 0, format!("Zoom:{}x  +/-", self.map_zoom));
        for (row, commands) in [
            [
                "hjklyubn/矢印:移動",
                ".:待機",
                "g:拾う",
                "f:射撃",
                "r:装填",
                "s:固定物  o:開く",
            ],
            [
                "@:プロパティ",
                "i:所持品",
                "p:製作",
                "m:地図",
                "e:操作  ;:調査",
                "Ctrl+Q:終了",
            ],
        ]
        .iter()
        .enumerate()
        {
            for (column, command) in commands.iter().enumerate() {
                let x = [1, 23, 33, 43, 53, 65][column];
                ctx.print_ja(x, crate::ui::theme::COMMAND_TOP + row as i32, command);
            }
        }
        for (row, message_text) in self
            .messages
            .iter()
            .skip(
                self.messages
                    .len()
                    .saturating_sub(crate::ui::theme::LOG_ROWS),
            )
            .enumerate()
        {
            ctx.print_ja(
                1,
                crate::ui::theme::LOG_TOP + row as i32,
                message(message_text),
            );
        }
        for slot in 0..10 {
            let label = self.quick_slot_summary(slot);
            ctx.print_ja_color(
                (slot * 8) as i32,
                crate::ui::theme::SLOTS_ROW,
                bl::RGB::named(if label == "未登録" {
                    crate::ui::theme::MUTED
                } else {
                    crate::ui::theme::ACCENT
                }),
                format!(
                    "{}:{}",
                    crate::ui::quick_slots::slot_label(slot),
                    label.chars().take(5).collect::<String>()
                ),
            );
        }
        match self.panel {
            Panel::ItemTarget { item, position, .. } => {
                self.render_item_target(ctx, item, position)
            }
            Panel::Properties => self.render_properties(ctx),
            Panel::Equipment { item } => self.render_equipment(ctx, item),
            Panel::Bandage { item } => self.render_bandage_selection(ctx, item),
            Panel::ItemDetails { item } => self.render_item_details(ctx, item),
            Panel::Container { .. }
            | Panel::InteractionDirection
            | Panel::PickupMenu
            | Panel::SmashDirection
            | Panel::SmashMenu { .. }
            | Panel::OpenDirection
            | Panel::OpenMenu { .. }
            | Panel::InteractionMenu { .. } => {
                if let Some(popup_console) = self.popup_console {
                    ctx.set_active_console(popup_console);
                    if matches!(self.panel, Panel::Container { .. }) {
                        self.render_container(ctx, 0.6);
                    } else {
                        self.render_interaction_panel(ctx, 0.6);
                    }
                    ctx.set_active_console(world_console);
                } else {
                    if matches!(self.panel, Panel::Container { .. }) {
                        self.render_container(ctx, 1.0);
                    } else {
                        self.render_interaction_panel(ctx, 1.0);
                    }
                }
            }
            Panel::Inventory => {
                crate::ui::theme::draw_box(
                    ctx,
                    2,
                    3,
                    74,
                    31,
                    crate::ui::theme::TEXT,
                    crate::ui::theme::BACKGROUND,
                );
                ctx.print_ja_color(4, 4, bl::RGB::named(crate::ui::theme::ACCENT), "所持品");
                ctx.print_ja(
                    4,
                    5,
                    "j/k:選択  Enter:使用  w:登録・装備  e:詳細  d:置く  Esc:戻る",
                );
                for (x, label) in [
                    (6, "名前・数字キー"),
                    (31, "個数"),
                    (34, "装備"),
                    (41, "装填"),
                    (52, "性能"),
                ] {
                    ctx.print_ja(x, 7, label);
                }
                if let Some(actor) = self.actor(self.player_id) {
                    let start = self.selection.saturating_sub(24);
                    for (row, (index, id)) in actor
                        .inventory
                        .items
                        .iter()
                        .enumerate()
                        .skip(start)
                        .take(25)
                        .enumerate()
                    {
                        let count = match &self.entities[id] {
                            Entity::Item(i) => i.quantity,
                            _ => 0,
                        };
                        let selected = index == self.selection;
                        let fg = bl::RGB::named(if selected {
                            crate::ui::theme::ACCENT
                        } else {
                            crate::ui::theme::TEXT
                        });
                        let y = 8 + row as i32;
                        ctx.print_ja_color(4, y, fg, if selected { ">" } else { " " });
                        let ammo = match (
                            &self.entities[id],
                            self.definition(*id)
                                .item
                                .as_ref()
                                .and_then(|i| i.weapon.as_ref()),
                        ) {
                            (Entity::Item(item), Some(weapon)) => weapon
                                .magazine_capacity
                                .map(|capacity| format!("{}/{capacity}", item.loaded_ammo))
                                .unwrap_or_default(),
                            _ => String::new(),
                        };
                        for (x, width, value) in [
                            (
                                6,
                                20,
                                format!("{}{}", self.display_name(*id), self.item_quick_keys(*id)),
                            ),
                            (27, 6, format!("{count:>6}")),
                            (
                                34,
                                6,
                                if actor.inventory.armor.values().any(|armor| armor == id) {
                                    "装着中".into()
                                } else {
                                    actor
                                        .inventory
                                        .grip(*id)
                                        .map(|g| {
                                            format!(
                                                "{}{}",
                                                g.label(),
                                                if self.view().item_hand_usable(actor.id, *id) {
                                                    ""
                                                } else {
                                                    "不可"
                                                }
                                            )
                                        })
                                        .unwrap_or_default()
                                },
                            ),
                            (41, 10, ammo),
                            (52, 22, self.combat_summary(*id)),
                        ] {
                            ctx.print_ja_color(
                                x,
                                y,
                                fg,
                                value.chars().take(width).collect::<String>(),
                            );
                        }
                    }
                }
            }
            Panel::Map => {
                ctx.print_ja_color(
                    2,
                    3,
                    bl::RGB::named(crate::ui::theme::ACCENT),
                    "地図: 現在の視野を表示しています。何かキーを押すと戻ります",
                );
            }
            Panel::Crafting => self.render_crafting(ctx),
            Panel::Inspect { position, selected } => {
                self.render_inspection(ctx, position, selected)
            }
            Panel::Targeting { active_entity } => self.render_targeting(ctx, active_entity),
            Panel::World => {}
        }
    }
}
