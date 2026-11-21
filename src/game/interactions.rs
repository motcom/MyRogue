//! Lua-registered interactions and item descriptions. Menus do not advance time.
use crate::{
    game::{Game, Panel},
    model::config::*,
    model::entity::*,
    scripting::Action,
    ui::inspect::wrap_description,
    ui::japanese::JapanesePrint,
};
use bracket_lib::prelude as bl;

pub(crate) struct InteractionChoice {
    pub target_name: String,
    pub name: String,
    pub action: Action,
}

impl Game {
    fn smash_targets(&self, position: Pos) -> Vec<Id> {
        self.at(position)
            .into_iter()
            .filter(|id| matches!(&self.entities[id], Entity::Static(entity) if entity.destructible && entity.taikyudo > 0))
            .collect()
    }

    pub(crate) fn choose_smash_direction(&mut self, dx: i32, dy: i32) -> Result<Option<Id>> {
        ensure(
            matches!(self.panel, Panel::SmashDirection),
            "先に方向を選択してください",
        )?;
        ensure(
            (-1..=1).contains(&dx) && (-1..=1).contains(&dy) && (dx, dy) != (0, 0),
            "invalid attack direction",
        )?;
        let position = self
            .position(self.player_id)
            .ok_or("player missing")?
            .offset(dx, dy);
        match self.smash_targets(position).as_slice() {
            [] => {
                self.log("その方向に攻撃できる固定物がありません");
                Ok(None)
            }
            [target] => Ok(Some(*target)),
            _ => {
                self.panel = Panel::SmashMenu { position };
                self.selection = 0;
                Ok(None)
            }
        }
    }

    pub(crate) fn items_at_feet(&self) -> Vec<Id> {
        self.position(self.player_id)
            .into_iter()
            .flat_map(|position| self.at(position))
            .filter(|id| matches!(self.entities[id], Entity::Item(_)))
            .collect()
    }

    /// Only ground entities exposing a currently available Lua `open` action.
    pub(crate) fn nearby_open_targets(&self) -> Vec<Id> {
        self.entities
            .iter()
            .filter_map(|(id, e)| {
                (e.map_position().is_some()
                    && self.can_reach_interaction(*id)
                    && self
                        .available_interactions(*id)
                        .iter()
                        .any(|i| i.id == "open"))
                .then_some(*id)
            })
            .collect()
    }

    pub(crate) fn begin_open(&mut self) -> Result<Option<Id>> {
        let targets = self.nearby_open_targets();
        self.selection = 0;
        match targets.as_slice() {
            [] => {
                self.log("近くに開ける物はありません");
                Ok(None)
            }
            [target] => Ok(Some(*target)),
            _ => {
                self.panel = Panel::OpenDirection;
                Ok(None)
            }
        }
    }

    pub(crate) fn choose_open_direction(&mut self, dx: i32, dy: i32) -> Result<Option<Id>> {
        ensure(
            matches!(self.panel, Panel::OpenDirection),
            "先に方向を選択してください",
        )?;
        ensure(
            (-1..=1).contains(&dx) && (-1..=1).contains(&dy),
            "invalid interaction direction",
        )?;
        let position = self
            .position(self.player_id)
            .ok_or("player missing")?
            .offset(dx, dy);
        let targets: Vec<_> = self
            .nearby_open_targets()
            .into_iter()
            .filter(|id| self.position(*id) == Some(position))
            .collect();
        match targets.as_slice() {
            [] => {
                self.log("その方向に開ける物はありません");
                Ok(None)
            }
            [target] => Ok(Some(*target)),
            _ => {
                self.panel = Panel::OpenMenu { position };
                self.selection = 0;
                Ok(None)
            }
        }
    }

    pub fn available_interactions(&self, target: Id) -> Vec<&InteractionDefinition> {
        let Some(entity) = self.entities.get(&target).filter(|e| e.alive()) else {
            return vec![];
        };
        self.definition(target)
            .interactions
            .iter()
            .filter(|interaction| {
                interaction
                    .when_state
                    .iter()
                    .all(|(key, value)| entity.script().state.get(key) == Some(value))
            })
            .collect()
    }

    pub(crate) fn can_reach_interaction(&self, target: Id) -> bool {
        let Some(entity) = self.entities.get(&target).filter(|e| e.alive()) else {
            return false;
        };
        entity.carrier() == Some(self.player_id)
            || entity.map_position().is_some_and(|p| {
                self.position(self.player_id)
                    .is_some_and(|q| p.distance(q) <= 1)
                    && self.visible_positions().contains(&p)
            })
    }

    pub fn interact(&mut self, target: Id, action: &str) -> Result<bool> {
        ensure(
            self.can_reach_interaction(target),
            "その対象には手が届きません",
        )?;
        let consumes_turn = self
            .available_interactions(target)
            .into_iter()
            .find(|i| i.id == action)
            .ok_or("その操作は現在できません")?
            .consumes_turn;
        let position = self
            .position(target)
            .or_else(|| self.position(self.player_id))
            .ok_or("target not placed")?;
        let changed = self.hook(
            target,
            "on_interact",
            serde_json::json!({
                "user": self.player_id, "interaction": action, "position": position,
            }),
        )?;
        if changed {
            self.selection = 0;
            self.panel = if matches!(self.panel, Panel::Container { item } if item == target)
                && self
                    .entities
                    .get(&target)
                    .is_some_and(|e| e.script().state["opened"] == true)
            {
                self.panel
            } else if self
                .entities
                .get(&target)
                .is_some_and(|e| e.carrier() == Some(self.player_id))
            {
                Panel::ItemDetails { item: target }
            } else {
                Panel::World
            };
            self.events();
            if consumes_turn {
                self.advance_turn();
            }
        }
        Ok(changed)
    }

    pub(crate) fn interaction_choices(&self) -> Vec<InteractionChoice> {
        let (position, carried, open_only) = match self.panel {
            Panel::SmashMenu { position } => {
                return self
                    .smash_targets(position)
                    .into_iter()
                    .map(|target| InteractionChoice {
                        target_name: format!("{} #{}", self.definition(target).common.name, target),
                        name: "攻撃".into(),
                        action: Action::Attack { target },
                    })
                    .collect();
            }
            Panel::PickupMenu => {
                return self
                    .items_at_feet()
                    .into_iter()
                    .map(|item| {
                        let Entity::Item(entity) = &self.entities[&item] else {
                            unreachable!()
                        };
                        InteractionChoice {
                            target_name: self.display_name(item),
                            name: format!("拾う x{}", entity.quantity),
                            action: Action::Pickup { item },
                        }
                    })
                    .collect();
            }
            Panel::InteractionMenu { position, carried } => (position, carried, false),
            Panel::OpenMenu { position } => (position, None, true),
            _ => return vec![],
        };
        let targets = carried.map_or_else(|| self.inspected_entities(position), |id| vec![id]);
        let mut choices = Vec::new();
        for target in targets {
            if !self.can_reach_interaction(target) {
                continue;
            }
            for interaction in self.available_interactions(target) {
                if open_only && interaction.id != "open" {
                    continue;
                }
                choices.push(InteractionChoice {
                    target_name: if open_only {
                        format!("{} #{}", self.definition(target).common.name, target)
                    } else {
                        self.definition(target).common.name.clone()
                    },
                    name: interaction.name.clone(),
                    action: Action::PerformInteraction {
                        target,
                        interaction: interaction.id.clone(),
                    },
                });
            }
            if !open_only
                && matches!(self.entities.get(&target), Some(Entity::Item(i)) if matches!(i.location, ItemLocation::Ground(_)))
            {
                choices.push(InteractionChoice {
                    target_name: self.definition(target).common.name.clone(),
                    name: "拾う".into(),
                    action: Action::PickupNearby { item: target },
                });
            }
        }
        choices
    }

    pub(crate) fn interaction_key_action(&self, key: bl::VirtualKeyCode) -> Option<Action> {
        use bl::VirtualKeyCode as K;
        match self.panel {
            Panel::InteractionDirection | Panel::OpenDirection | Panel::SmashDirection => {
                let (dx, dy) = match key {
                    K::H | K::Left => (-1, 0),
                    K::J | K::Down => (0, 1),
                    K::K | K::Up => (0, -1),
                    K::L | K::Right => (1, 0),
                    K::Y => (-1, -1),
                    K::U => (1, -1),
                    K::B => (-1, 1),
                    K::N => (1, 1),
                    K::Period | K::Space => (0, 0),
                    _ => return None,
                };
                Some(match self.panel {
                    Panel::OpenDirection => Action::OpenDirection { dx, dy },
                    Panel::SmashDirection if (dx, dy) == (0, 0) => return None,
                    Panel::SmashDirection => Action::SmashDirection { dx, dy },
                    _ => Action::InteractDirection { dx, dy },
                })
            }
            Panel::PickupMenu
            | Panel::InteractionMenu { .. }
            | Panel::OpenMenu { .. }
            | Panel::SmashMenu { .. } => match key {
                K::J | K::Down => Some(Action::SelectInteraction { delta: 1 }),
                K::K | K::Up => Some(Action::SelectInteraction { delta: -1 }),
                K::Return => self
                    .interaction_choices()
                    .get(self.selection)
                    .map(|c| c.action.clone()),
                _ => None,
            },
            Panel::ItemDetails { item } => match key {
                K::E => Some(Action::ClosePanel),
                K::J | K::Down => Some(Action::ScrollItemDetails { delta: 1 }),
                K::K | K::Up => Some(Action::ScrollItemDetails { delta: -1 }),
                K::PageDown => Some(Action::ScrollItemDetails { delta: 20 }),
                K::PageUp => Some(Action::ScrollItemDetails { delta: -20 }),
                K::Return if !self.available_interactions(item).is_empty() => {
                    Some(Action::OpenItemInteractions { item })
                }
                _ => None,
            },
            _ => None,
        }
    }

    pub(crate) fn open_direction_interactions(&mut self, dx: i32, dy: i32) -> Result<()> {
        ensure(
            matches!(self.panel, Panel::InteractionDirection),
            "choose interaction direction first",
        )?;
        ensure(
            (-1..=1).contains(&dx) && (-1..=1).contains(&dy),
            "invalid interaction direction",
        )?;
        let position = self
            .position(self.player_id)
            .ok_or("player missing")?
            .offset(dx, dy);
        self.panel = Panel::InteractionMenu {
            position,
            carried: None,
        };
        self.selection = 0;
        Ok(())
    }

    pub(crate) fn close_panel(&mut self) {
        self.container_selection.clear();
        self.panel = match self.panel {
            Panel::Container { item } => {
                self.selection = 0;
                if self
                    .entities
                    .get(&item)
                    .is_some_and(|e| e.carrier() == Some(self.player_id))
                {
                    Panel::ItemDetails { item }
                } else {
                    Panel::World
                }
            }
            Panel::Bandage { .. } if self.bandage_return_to_world => {
                self.bandage_return_to_world = false;
                self.selection = 0;
                Panel::World
            }

            Panel::ItemTarget {
                from_world: true, ..
            } => Panel::World,
            Panel::ItemTarget { item, .. }
            | Panel::ItemDetails { item }
            | Panel::Equipment { item }
            | Panel::Bandage { item } => {
                self.selection = self
                    .actor(self.player_id)
                    .and_then(|a| a.inventory.items.iter().position(|id| *id == item))
                    .unwrap_or(0);
                Panel::Inventory
            }
            Panel::InteractionMenu {
                carried: Some(item),
                ..
            } => {
                self.selection = 0;
                Panel::ItemDetails { item }
            }
            _ => Panel::World,
        };
    }

    pub(crate) fn render_interaction_panel(&self, ctx: &mut bl::BTerm, opacity: f32) {
        let mut bg: bl::RGBA = bl::RGB::named(crate::ui::theme::BACKGROUND).into();
        bg.a = opacity;
        let white = bl::RGB::named(crate::ui::theme::TEXT);
        let yellow = bl::RGB::named(crate::ui::theme::ACCENT);
        for y in 5..=36 {
            for x in 3..=75 {
                ctx.set(x, y, white, bg, bl::to_cp437(' '));
            }
        }
        ctx.draw_hollow_box(3, 5, 72, 31, white, bg);
        let mut print = |x, y, fg, text: &str| ctx.print_ja_color_bg(x, y, fg, bg, text);
        print(
            5,
            6,
            yellow,
            match self.panel {
                Panel::OpenDirection | Panel::OpenMenu { .. } => "開く",
                Panel::PickupMenu => "足元の品を拾う",
                Panel::SmashDirection | Panel::SmashMenu { .. } => "固定物を攻撃",
                _ => "インタラクト",
            },
        );
        if matches!(self.panel, Panel::OpenDirection) {
            print(5, 8, white, "hjklyubn/矢印:方向を選択");
            print(5, 10, white, "./Space:足元  Esc:戻る");
            let origin = self.position(self.player_id).unwrap();
            for (row, target) in self.nearby_open_targets().iter().take(20).enumerate() {
                let p = self.position(*target).unwrap();
                let key = match (p.x - origin.x, p.y - origin.y) {
                    (-1, -1) => 'y',
                    (0, -1) => 'k',
                    (1, -1) => 'u',
                    (-1, 0) => 'h',
                    (0, 0) => '.',
                    (1, 0) => 'l',
                    (-1, 1) => 'b',
                    (0, 1) => 'j',
                    (1, 1) => 'n',
                    _ => '?',
                };
                print(
                    5,
                    12 + row as i32,
                    white,
                    &format!(
                        "{key}: {} #{}",
                        self.definition(*target).common.name,
                        target
                    ),
                );
            }
            return;
        }
        if matches!(self.panel, Panel::InteractionDirection) {
            print(5, 8, white, "hjklyubn/矢印:方向を選択");
            print(5, 10, white, "./Space:足元  Esc:戻る");
            return;
        }
        if matches!(self.panel, Panel::SmashDirection) {
            print(5, 8, white, "hjklyubn/矢印:攻撃する方向  Esc:戻る");
            return;
        }
        print(5, 7, white, "j/k・矢印:選択  Enter:実行  Esc:戻る");
        print(7, 9, white, "対象");
        print(34, 9, white, "行動");
        let choices = self.interaction_choices();
        if choices.is_empty() {
            let message = if matches!(self.panel, Panel::PickupMenu) {
                "足元に拾える物はありません"
            } else {
                "その方向に操作できる物はありません"
            };
            print(5, 11, white, message);
        }
        let start = self.selection.saturating_sub(19);
        for (row, (index, choice)) in choices.iter().enumerate().skip(start).take(20).enumerate() {
            let fg = if index == self.selection {
                yellow
            } else {
                white
            };
            let y = 11 + row as i32;
            print(5, y, fg, if index == self.selection { ">" } else { " " });
            print(
                7,
                y,
                fg,
                &choice.target_name.chars().take(25).collect::<String>(),
            );
            print(34, y, fg, &choice.name.chars().take(39).collect::<String>());
        }
        print(5, 34, white, "選択中は時間が進みません");
    }

    pub(crate) fn render_item_details(&self, ctx: &mut bl::BTerm, item: Id) {
        crate::ui::theme::draw_box(
            ctx,
            1,
            2,
            77,
            46,
            crate::ui::theme::TEXT,
            crate::ui::theme::BACKGROUND,
        );
        let Some(Entity::Item(entity)) = self.entities.get(&item) else {
            ctx.print_ja(4, 4, "アイテムがありません。Escで戻ります");
            return;
        };
        let definition = self.definition(item);
        let data = definition.item.as_ref().unwrap();
        ctx.print_ja_color(
            4,
            4,
            bl::RGB::named(crate::ui::theme::ACCENT),
            self.display_name(item).chars().take(71).collect::<String>(),
        );
        ctx.print_ja(4, 6, "e/Esc:所持品へ戻る");
        if !self.available_interactions(item).is_empty() {
            ctx.print_ja(34, 6, "Enter:行動メニュー");
        }
        ctx.print_ja(4, 8, format!("{:<8} {}", "個数", entity.quantity));
        ctx.print_ja(24, 8, format!("Tier {}", data.tier));
        ctx.print_ja(
            4,
            9,
            format!(
                "{:<8} {}",
                "耐久",
                if entity.max_taikyudo == 0 {
                    "なし".into()
                } else {
                    format!("{}/{}", entity.taikyudo, entity.max_taikyudo)
                }
            ),
        );
        ctx.print_ja(
            4,
            10,
            format!(
                "長さ {} mm   容量 {} ml   重量 {} g",
                data.length_mm, data.volume_ml, data.mass_g
            ),
        );
        if let Some(t) = &data.throwable {
            ctx.print_ja(
                4,
                12,
                format!(
                    "投擲射程 {} / 効果範囲 5×5 / 中心の効果量 {}",
                    t.range, t.power
                ),
            );
            ctx.print_ja(4, 13, "中心から一マス離れるごとに効果が減少。壁で遮蔽。");
        }
        if let Some(w) = &data.weapon {
            let hands = match w.hands {
                crate::model::config::WeaponHands::One => "片手",
                crate::model::config::WeaponHands::Two => "両手",
                crate::model::config::WeaponHands::Either => "片手・両手",
            };
            ctx.print_ja(
                4,
                11,
                format!("持ち方: {hands}    両手PV加算: {}", w.two_hand_pv_bonus),
            );
        }
        if let Some(armor) = crate::game::equipment::body_armor(definition) {
            ctx.print_ja(
                4,
                11,
                format!(
                    "防具部位: {}    所持品のwで着脱",
                    crate::game::equipment::armor_slot_name(&armor.slot)
                ),
            );
        }
        let held = self.held_weapon_summary(self.player_id, item);
        ctx.print_ja(
            4,
            12,
            if held.is_empty() {
                self.combat_summary(item)
            } else {
                held
            },
        );
        if !data.tool_functions.is_empty() {
            ctx.print_ja(
                4,
                13,
                data.tool_functions
                    .iter()
                    .map(|function| {
                        format!(
                            "{}Lv{}",
                            crate::game::crafting::tool_name(function),
                            crate::game::crafting::tool_level(data, function)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" / "),
            );
        }
        ctx.print_ja_color(4, 14, bl::RGB::named(crate::ui::theme::ACCENT), "説明");
        ctx.print_ja(4, 46, "j/k・矢印・PageUp/Down:説明をスクロール");
        if let Some(weapon) = definition
            .item
            .as_ref()
            .and_then(|item| item.weapon.as_ref())
        {
            ctx.print_ja(
                4,
                13,
                format!("クリティカル: 3D6で{}以上", weapon.critical_value),
            );
        }
        for (row, line) in wrap_description(&definition.common.description, 71)
            .iter()
            .skip(self.selection)
            .take(28)
            .enumerate()
        {
            ctx.print_ja(4, 16 + row as i32, line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn game() -> Game {
        let mut game = Game::load(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let remove: Vec<_> = game
            .entities
            .iter()
            .filter(|(id, e)| **id != game.player_id && e.map_position().is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in remove {
            game.remove(id);
        }
        game
    }
    fn apply(game: &mut Game, action: Action) {
        game.pending_action = Some(action);
        game.update();
    }
    fn key(game: &mut Game, key: bl::VirtualKeyCode) {
        game.input(&context(key));
        game.update();
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
    fn inventory_w_equips_and_e_shows_details_without_using_item() {
        let mut game = game();
        let player = game.player_id;
        let item = game.spawn("item.club", None, Some(player), 1).unwrap();
        let selected = game
            .actor(player)
            .unwrap()
            .inventory
            .items
            .iter()
            .position(|id| *id == item)
            .unwrap();
        let equipped = game.actor(player).unwrap().inventory.equipped;
        std::sync::Arc::make_mut(&mut game.entity_definitions)
            .get_mut("item.club")
            .unwrap()
            .common
            .description = "line\n".repeat(50);
        key(&mut game, bl::VirtualKeyCode::I);
        game.selection = selected;
        key(&mut game, bl::VirtualKeyCode::E);
        assert!(matches!(game.panel, Panel::ItemDetails { item: id } if id == item));
        assert_eq!(game.turn, 0);
        assert_eq!(game.actor(player).unwrap().inventory.equipped, equipped);
        key(&mut game, bl::VirtualKeyCode::PageDown);
        assert!(game.selection > 0);
        assert_eq!(game.turn, 0);
        key(&mut game, bl::VirtualKeyCode::Escape);
        assert!(matches!(game.panel, Panel::Inventory));
        assert_eq!(game.selection, selected);
        key(&mut game, bl::VirtualKeyCode::W);
        assert!(matches!(game.panel, Panel::Equipment { .. }));
        assert_eq!(game.turn, 0);
        key(&mut game, bl::VirtualKeyCode::Return);
        assert_eq!(game.actor(player).unwrap().inventory.equipped, Some(item));
        assert_eq!(game.turn, 1);
    }

    #[test]
    fn equipment_menu_selects_each_grip_and_filters_severed_hands_without_time() {
        use crate::model::entity::{Grip, InjuryStage};
        let mut game = game();
        let player = game.player_id;
        let gun = game.spawn("item.glock", None, Some(player), 1).unwrap();
        let index = game
            .actor(player)
            .unwrap()
            .inventory
            .items
            .iter()
            .position(|id| *id == gun)
            .unwrap();
        for (row, grip) in [(0, Grip::Right), (1, Grip::Left), (2, Grip::Both)] {
            let turn = game.turn;
            game.panel = Panel::Inventory;
            game.selection = index;
            key(&mut game, bl::VirtualKeyCode::W);
            for _ in 0..4 {
                key(&mut game, bl::VirtualKeyCode::K);
            }
            for _ in 0..row {
                key(&mut game, bl::VirtualKeyCode::J);
            }
            assert_eq!(game.turn, turn);
            key(&mut game, bl::VirtualKeyCode::Return);
            assert_eq!(game.actor(player).unwrap().inventory.grip(gun), Some(grip));
            assert_eq!(game.turn, turn + 1);
            assert!(matches!(game.panel, Panel::Inventory));
            assert_eq!(game.selection, index);
        }
        game.actor_mut(player)
            .unwrap()
            .body
            .insert("right_arm".into(), InjuryStage::Severed);
        let turn = game.turn;
        key(&mut game, bl::VirtualKeyCode::W);
        assert_eq!(
            game.equipment_choices(player, gun),
            vec![Some(Grip::Left), None]
        );
        assert_eq!(game.turn, turn);
        assert!(matches!(game.panel, Panel::Equipment { .. }));
        key(&mut game, bl::VirtualKeyCode::Escape);
        assert!(matches!(game.panel, Panel::Inventory));
        assert_eq!(game.selection, index);
    }

    #[test]
    fn enter_uses_bandages_by_selecting_only_untreated_bleeding_parts() {
        use crate::model::entity::{Entity, HP, InjuryStage};
        let mut game = game();
        let p = game.player_id;
        let bandage = *game
            .actor(p)
            .unwrap()
            .inventory
            .items
            .iter()
            .find(|id| game.entities[id].script().definition_id == "item.bandage")
            .unwrap();
        let index = game
            .actor(p)
            .unwrap()
            .inventory
            .items
            .iter()
            .position(|id| *id == bandage)
            .unwrap();
        let a = game.actor_mut(p).unwrap();
        a.hp = HP {
            hp: 2000,
            max_hp: 2000,
        };
        a.nutrition.protein = 0.0;
        a.body.insert("left_arm".into(), InjuryStage::Bleeding);
        a.body.insert("left_leg".into(), InjuryStage::HeavyBleeding);
        key(&mut game, bl::VirtualKeyCode::I);
        game.selection = index;
        key(&mut game, bl::VirtualKeyCode::W);
        assert!(matches!(game.panel, Panel::Equipment { .. }));
        key(&mut game, bl::VirtualKeyCode::Escape);
        key(&mut game, bl::VirtualKeyCode::Return);
        assert!(matches!(game.panel, Panel::Bandage { .. }));
        assert_eq!(game.bandage_choices(p), vec!["left_arm", "left_leg"]);
        assert_eq!(game.turn, 0);
        key(&mut game, bl::VirtualKeyCode::Escape);
        assert!(matches!(game.panel, Panel::Inventory));
        assert_eq!(game.selection, index);
        key(&mut game, bl::VirtualKeyCode::Return);
        key(&mut game, bl::VirtualKeyCode::J);
        assert_eq!(game.turn, 0);
        key(&mut game, bl::VirtualKeyCode::Return);
        assert_eq!(game.turn, 1);
        assert_eq!(game.actor(p).unwrap().hp.hp, 1999);
        assert_eq!(game.actor(p).unwrap().treatment.bandages["left_leg"], 100);
        assert_eq!(game.bandage_choices(p), vec!["left_arm"]);
        assert!(matches!(game.entities[&bandage],Entity::Item(ref i) if i.quantity==1));
        assert!(matches!(game.panel, Panel::Inventory));
        key(&mut game, bl::VirtualKeyCode::Return);
        key(&mut game, bl::VirtualKeyCode::Return);
        assert!(!game.entities.contains_key(&bandage));
        assert_eq!(game.actor(p).unwrap().treatment.bandages.len(), 2);
        assert_eq!(game.actor(p).unwrap().hp.hp, 1999);
        assert_eq!(game.turn, 2);
    }

    #[test]
    fn only_control_q_quits_from_world_and_all_panels() {
        let base = game();
        let p = base.player_id;
        let item = base.actor(p).unwrap().inventory.items[0];
        let pos = base.position(p).unwrap();
        for panel in [
            Panel::World,
            Panel::Inventory,
            Panel::Equipment { item },
            Panel::Bandage { item },
            Panel::ItemDetails { item },
            Panel::Properties,
            Panel::Map,
            Panel::Crafting,
            Panel::InteractionDirection,
            Panel::OpenDirection,
            Panel::OpenMenu { position: pos },
            Panel::InteractionMenu {
                position: pos,
                carried: None,
            },
            Panel::Inspect {
                position: pos,
                selected: 0,
            },
            Panel::Targeting { active_entity: p },
        ] {
            let mut game = base.clone();
            game.panel = panel;
            key(&mut game, bl::VirtualKeyCode::Q);
            assert!(game.running);
            assert_eq!(game.turn, 0);
            game.panel = panel;
            key(&mut game, bl::VirtualKeyCode::Escape);
            assert!(game.running);
            assert_eq!(game.turn, 0);
            game.panel = panel;
            let mut ctx = context(bl::VirtualKeyCode::Q);
            ctx.control = true;
            game.input(&ctx);
            game.update();
            assert!(!game.running);
            assert_eq!(game.turn, 0);
        }
    }

    #[test]
    fn inventory_w_binds_numbers_and_world_numbers_equip_or_use_bandages() {
        use crate::model::entity::{Grip, HP, InjuryStage, QuickSlot};
        let mut game = game();
        let p = game.player_id;
        let knife = game.spawn("item.knife", None, Some(p), 1).unwrap();
        let rifle = game.spawn("item.sks", None, Some(p), 1).unwrap();
        let index = |g: &Game, id| {
            g.actor(p)
                .unwrap()
                .inventory
                .items
                .iter()
                .position(|i| *i == id)
                .unwrap()
        };
        key(&mut game, bl::VirtualKeyCode::I);
        game.selection = index(&game, knife);
        key(&mut game, bl::VirtualKeyCode::W);
        assert_eq!(game.turn, 0);
        key(&mut game, bl::VirtualKeyCode::Key1);
        assert_eq!(
            game.actor(p).unwrap().inventory.grip(knife),
            Some(Grip::Right)
        );
        assert_eq!(game.turn, 1);
        assert!(matches!(game.panel, Panel::Inventory));
        game.selection = index(&game, rifle);
        key(&mut game, bl::VirtualKeyCode::W);
        key(&mut game, bl::VirtualKeyCode::Key0);
        assert_eq!(game.turn, 2);
        key(&mut game, bl::VirtualKeyCode::I);
        assert!(matches!(game.panel, Panel::World));
        key(&mut game, bl::VirtualKeyCode::Key1);
        assert_eq!(game.turn, 3);
        assert_eq!(
            game.actor(p).unwrap().inventory.grip(knife),
            Some(Grip::Right)
        );
        key(&mut game, bl::VirtualKeyCode::Numpad0);
        assert_eq!(game.turn, 4);
        assert_eq!(
            game.actor(p).unwrap().inventory.grip(rifle),
            Some(Grip::Both)
        );
        key(&mut game, bl::VirtualKeyCode::Key0);
        assert_eq!(game.turn, 4);
        let mut shifted = context(bl::VirtualKeyCode::Key1);
        shifted.shift = true;
        game.input(&shifted);
        game.update();
        assert_eq!(game.turn, 4);
        let b = *game
            .actor(p)
            .unwrap()
            .inventory
            .items
            .iter()
            .find(|id| game.entities[id].script().definition_id == "item.bandage")
            .unwrap();
        key(&mut game, bl::VirtualKeyCode::I);
        game.selection = index(&game, b);
        key(&mut game, bl::VirtualKeyCode::W);
        key(&mut game, bl::VirtualKeyCode::Key2);
        assert_eq!(game.turn, 4);
        assert!(matches!(
            game.actor(p).unwrap().inventory.quick_slots[1],
            Some(QuickSlot::Consumable { .. })
        ));
        key(&mut game, bl::VirtualKeyCode::I);
        game.actor_mut(p).unwrap().hp = HP {
            hp: 1000,
            max_hp: 1000,
        };
        game.actor_mut(p)
            .unwrap()
            .body
            .insert("left_arm".into(), InjuryStage::Bleeding);
        key(&mut game, bl::VirtualKeyCode::Key2);
        assert!(matches!(game.panel, Panel::Bandage { .. }));
        assert_eq!(game.turn, 4);
        key(&mut game, bl::VirtualKeyCode::Escape);
        assert!(matches!(game.panel, Panel::World));
        key(&mut game, bl::VirtualKeyCode::Key2);
        key(&mut game, bl::VirtualKeyCode::Return);
        assert_eq!(game.turn, 5);
        assert!(matches!(game.panel, Panel::World));
        assert!(
            game.actor(p)
                .unwrap()
                .treatment
                .bandages
                .contains_key("left_arm")
        );
    }

    #[test]
    fn inventory_w_toggles_body_armor_directly_without_number_registration() {
        let mut game = game();
        let p = game.player_id;
        let armor = game.spawn("item.leather_armor", None, Some(p), 1).unwrap();
        let index = game
            .actor(p)
            .unwrap()
            .inventory
            .items
            .iter()
            .position(|i| *i == armor)
            .unwrap();
        key(&mut game, bl::VirtualKeyCode::I);
        game.selection = index;
        let before = game.defense_values(p).unwrap();
        key(&mut game, bl::VirtualKeyCode::W);
        assert!(matches!(game.panel, Panel::Inventory));
        assert_eq!(game.turn, 1);
        assert_eq!(game.actor(p).unwrap().inventory.armor["body"], armor);
        assert!(
            game.actor(p)
                .unwrap()
                .inventory
                .quick_slots
                .iter()
                .all(Option::is_none)
        );
        assert_eq!(game.defense_values(p), Some((before.0 + 1, before.1)));
        key(&mut game, bl::VirtualKeyCode::W);
        assert_eq!(game.turn, 2);
        assert!(game.actor(p).unwrap().inventory.armor.is_empty());
        assert_eq!(game.defense_values(p), Some(before));
    }

    #[test]
    fn o_opens_the_only_adjacent_target_in_every_direction_and_chest_at_feet() {
        for (dx, dy) in [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
            (0, 0),
        ] {
            let mut g = game();
            let p = g.position(g.player_id).unwrap();
            let definition = if (dx, dy) == (0, 0) {
                "item.treasure_chest"
            } else {
                "obstacle.wooden_door"
            };
            let target = g
                .spawn(definition, Some(p.offset(dx, dy)), None, 1)
                .unwrap();
            key(&mut g, bl::VirtualKeyCode::O);
            assert_eq!(g.entities[&target].script().state["opened"], true);
            assert_eq!(g.turn, 1);
            if (dx, dy) == (0, 0) {
                assert!(matches!(g.panel, Panel::Container { item } if item == target));
            } else {
                assert!(matches!(g.panel, Panel::World));
                assert!(!g.entities[&target].blocks_movement());
                assert!(!g.entities[&target].blocks_sight());
            }
        }
    }

    #[test]
    fn o_multiple_targets_choose_direction_without_moving_or_spending_selection_turns() {
        use bl::VirtualKeyCode as K;
        let mut g = game();
        let p = g.position(g.player_id).unwrap();
        let west = g
            .spawn("obstacle.wooden_door", Some(p.offset(-1, 0)), None, 1)
            .unwrap();
        let east = g
            .spawn("obstacle.wooden_door", Some(p.offset(1, 0)), None, 1)
            .unwrap();
        key(&mut g, K::O);
        assert!(matches!(g.panel, Panel::OpenDirection));
        assert_eq!(g.turn, 0);
        key(&mut g, K::K); // No object north: remain in selection.
        assert!(matches!(g.panel, Panel::OpenDirection));
        assert_eq!(g.turn, 0);
        key(&mut g, K::Escape);
        assert!(matches!(g.panel, Panel::World));
        assert_eq!(g.turn, 0);
        key(&mut g, K::O);
        key(&mut g, K::Left);
        assert_eq!(g.entities[&west].script().state["opened"], true);
        assert_eq!(g.entities[&east].script().state["opened"], false);
        assert_eq!(g.position(g.player_id), Some(p));
        assert_eq!(g.turn, 1);
        key(&mut g, K::O); // Already-open door no longer counts.
        assert_eq!(g.entities[&east].script().state["opened"], true);
        assert_eq!(g.turn, 2);
        key(&mut g, K::O); // Must never close a door.
        assert_eq!(g.turn, 2);
    }

    #[test]
    fn o_stacked_chests_offer_only_open_and_can_be_cancelled_or_selected() {
        use bl::VirtualKeyCode as K;
        let mut g = game();
        let p = g.position(g.player_id).unwrap();
        let a = g.spawn("item.treasure_chest", Some(p), None, 1).unwrap();
        let b = g.spawn("item.treasure_chest", Some(p), None, 1).unwrap();
        key(&mut g, K::O);
        key(&mut g, K::Space);
        assert!(matches!(g.panel, Panel::OpenMenu { .. }));
        assert_eq!(g.interaction_choices().len(), 2);
        assert!(g.interaction_choices().iter().all(|c| matches!(&c.action, Action::PerformInteraction { interaction, .. } if interaction == "open")));
        key(&mut g, K::Escape);
        assert_eq!(g.turn, 0);
        key(&mut g, K::O);
        key(&mut g, K::Period);
        key(&mut g, K::J);
        assert_eq!(g.turn, 0);
        key(&mut g, K::Return);
        assert_eq!(g.turn, 1);
        // Same layer: the newest entity is first, so j selects the older chest.
        assert_eq!(g.entities[&a].script().state["opened"], true);
        assert_eq!(g.entities[&b].script().state["opened"], false);
        assert!(matches!(g.panel, Panel::Container { item } if item == a));
    }

    #[test]
    fn o_ignores_inventory_far_and_hidden_objects_and_rechecks_deleted_targets() {
        use bl::VirtualKeyCode as K;
        let mut g = game();
        let player = g.player_id;
        let p = g.position(player).unwrap();
        g.spawn("item.treasure_chest", None, Some(player), 1)
            .unwrap();
        g.spawn("item.treasure_chest", Some(p.offset(2, 0)), None, 1)
            .unwrap();
        g.spawn(
            "obstacle.wooden_door",
            Some(Pos { x: 40, y: 25, z: 0 }),
            None,
            1,
        )
        .unwrap();
        key(&mut g, K::O);
        assert!(matches!(g.panel, Panel::World));
        assert_eq!(g.turn, 0);
        let a = g
            .spawn("obstacle.wooden_door", Some(p.offset(-1, 0)), None, 1)
            .unwrap();
        let b = g
            .spawn("obstacle.wooden_door", Some(p.offset(1, 0)), None, 1)
            .unwrap();
        key(&mut g, K::O);
        g.remove(b);
        key(&mut g, K::L);
        assert!(matches!(g.panel, Panel::OpenDirection));
        assert_eq!(g.turn, 0);
        key(&mut g, K::H);
        assert_eq!(g.entities[&a].script().state["opened"], true);
        assert_eq!(g.turn, 1);
    }

    #[test]
    fn o_cannot_bypass_flash_stun_when_opening_automatically() {
        let mut g = game();
        let p = g.position(g.player_id).unwrap();
        let door = g
            .spawn("obstacle.wooden_door", Some(p.offset(1, 0)), None, 1)
            .unwrap();
        g.actor_mut(g.player_id).unwrap().stunned_until = 100;
        key(&mut g, bl::VirtualKeyCode::O);
        assert_eq!(g.turn, 1);
        assert_eq!(g.entities[&door].script().state["opened"], false);
    }

    #[test]
    fn e_then_direction_opens_actions_and_enter_operates_door() {
        let mut game = game();
        let p = game.position(game.player_id).unwrap();
        let door = game
            .spawn("obstacle.wooden_door", Some(p.offset(1, 0)), None, 1)
            .unwrap();
        key(&mut game, bl::VirtualKeyCode::E);
        assert!(matches!(game.panel, Panel::InteractionDirection));
        key(&mut game, bl::VirtualKeyCode::Right);
        assert!(matches!(game.panel, Panel::InteractionMenu { .. }));
        assert_eq!(game.interaction_choices().len(), 1);
        assert_eq!(game.interaction_choices()[0].name, "開く");
        assert_eq!(game.turn, 0);
        key(&mut game, bl::VirtualKeyCode::Return);
        assert!(matches!(game.panel, Panel::World));
        assert_eq!(game.turn, 1);
        assert!(!game.entities[&door].blocks_movement());
        key(&mut game, bl::VirtualKeyCode::E);
        key(&mut game, bl::VirtualKeyCode::L);
        assert_eq!(game.interaction_choices()[0].name, "閉じる");
        key(&mut game, bl::VirtualKeyCode::Escape);
        assert_eq!(game.turn, 1);
        key(&mut game, bl::VirtualKeyCode::E);
        key(&mut game, bl::VirtualKeyCode::Space);
        assert!(game.interaction_choices().is_empty());
        assert_eq!(game.turn, 1);
    }

    #[test]
    fn doors_change_visibility_pathing_and_refuse_to_close_on_occupants() {
        let mut game = game();
        let player = game.player_id;
        let p = game.position(player).unwrap();
        let q = p.offset(1, 0);
        let door = game
            .spawn("obstacle.wooden_door", Some(q), None, 1)
            .unwrap();
        let behind = p.offset(2, 0);
        let target = game.spawn("item.wood", Some(behind), None, 1).unwrap();
        assert!(!game.can_step(p, q, player));
        assert!(!game.visible_positions().contains(&behind));
        assert!(game.interact(door, "close").is_err());
        assert_eq!(game.turn, 0);
        game.interact(door, "open").unwrap();
        assert!(game.can_step(p, q, player));
        assert!(game.visible_positions().contains(&behind));
        assert_eq!(game.step_towards(player, target), Some(q));
        assert_eq!(game.entities[&door].script().glyph.as_deref(), Some("/"));
        let cloned = game.clone();
        game.move_to(player, q).unwrap();
        assert!(game.interact(door, "close").is_err());
        assert_eq!(game.turn, 1);
        assert!(!game.entities[&door].blocks_sight());
        assert_eq!(game.entities[&door].script().state["opened"], true);
        game.move_to(player, p).unwrap();
        game.interact(door, "close").unwrap();
        assert_eq!(game.turn, 2);
        assert!(game.entities[&door].blocks_sight());
        assert!(!game.visible_positions().contains(&behind));
        assert!(!cloned.entities[&door].blocks_sight());
        assert_eq!(game.entities[&door].script().glyph.as_deref(), Some("+"));
    }

    #[test]
    fn chest_can_be_picked_up_opened_while_carried_and_does_not_duplicate_loot() {
        let mut game = game();
        let player = game.player_id;
        let p = game.position(player).unwrap();
        let chest = game
            .spawn("item.treasure_chest", Some(p.offset(1, 0)), None, 1)
            .unwrap();
        crate::game::containers::set_test_chest_contents(&mut game, chest);
        key(&mut game, bl::VirtualKeyCode::E);
        key(&mut game, bl::VirtualKeyCode::L);
        assert_eq!(game.interaction_choices().len(), 2);
        key(&mut game, bl::VirtualKeyCode::J);
        key(&mut game, bl::VirtualKeyCode::Return);
        assert_eq!(game.entities[&chest].carrier(), Some(player));
        assert!(!game.at(p.offset(1, 0)).contains(&chest));
        assert_eq!(game.turn, 1);
        apply(&mut game, Action::ItemDetails { item: chest });
        key(&mut game, bl::VirtualKeyCode::Return);
        assert_eq!(game.interaction_choices().len(), 1);
        key(&mut game, bl::VirtualKeyCode::Return);
        assert!(matches!(game.panel, Panel::Container { item } if item == chest));
        assert_eq!(game.turn, 2);
        let count = game.entities.len();
        assert!(
            !game
                .at(p)
                .iter()
                .any(|id| matches!(&game.entities[id], Entity::Item(_)))
        );
        key(&mut game, bl::VirtualKeyCode::Space); // bread
        key(&mut game, bl::VirtualKeyCode::J);
        key(&mut game, bl::VirtualKeyCode::Space); // bandages
        assert_eq!(game.turn, 2);
        key(&mut game, bl::VirtualKeyCode::Return);
        assert_eq!(game.turn, 3);
        assert_eq!(game.entities.len(), count + 2);
        assert_eq!(game.container_entries(chest).len(), 1);
        assert_eq!(
            game.container_entries(chest)[0].1.definition_id,
            "ammo.pistol"
        );
        for (kind, quantity) in [("item.bread", 2), ("item.bandage", 2)] {
            assert!(game.actor(player).unwrap().inventory.items.iter().any(|id| matches!(&game.entities[id], Entity::Item(i) if i.script.definition_id == kind && i.quantity == quantity)));
        }
        key(&mut game, bl::VirtualKeyCode::Escape);
        assert!(matches!(game.panel, Panel::ItemDetails { item } if item == chest));
        game.interact(chest, "close").unwrap();
        game.interact(chest, "open").unwrap();
        assert_eq!(game.entities.len(), count + 2);
        game.drop_item(player, chest).unwrap();
        game.pickup(player, chest).unwrap();
        assert_eq!(game.container_entries(chest)[0].1.quantity, 10);
        key(&mut game, bl::VirtualKeyCode::Space);
        key(&mut game, bl::VirtualKeyCode::Return);
        assert!(game.container_entries(chest).is_empty());
        let after = game.entities.len();
        game.interact(chest, "close").unwrap();
        game.interact(chest, "open").unwrap();
        assert_eq!(game.entities.len(), after);
        assert!(game.container_entries(chest).is_empty());
    }

    #[test]
    fn smash_uses_the_chosen_direction_and_filters_to_destructible_statics() {
        let mut game = game();
        let origin = game.position(game.player_id).unwrap();
        let east = origin.offset(1, 0);
        let chair = game.spawn("obstacle.chair", Some(east), None, 1).unwrap();
        let table = game.spawn("obstacle.table", Some(east), None, 1).unwrap();
        let west = origin.offset(-1, 0);
        let tree = game.spawn("obstacle.tree", Some(west), None, 1).unwrap();
        let north = origin.offset(0, -1);
        game.spawn("obstacle.lamp", Some(north), None, 1).unwrap();
        game.spawn("item.bread", Some(north), None, 1).unwrap();

        key(&mut game, bl::VirtualKeyCode::T);
        assert!(matches!(game.panel, Panel::World));
        key(&mut game, bl::VirtualKeyCode::S);
        assert!(matches!(game.panel, Panel::SmashDirection));
        key(&mut game, bl::VirtualKeyCode::Up);
        assert!(matches!(game.panel, Panel::SmashDirection));
        key(&mut game, bl::VirtualKeyCode::Escape);
        assert_eq!(game.turn, 0);

        key(&mut game, bl::VirtualKeyCode::S);
        key(&mut game, bl::VirtualKeyCode::Right);
        assert!(matches!(game.panel, Panel::SmashMenu { position } if position == east));
        let targets: Vec<_> = game
            .interaction_choices()
            .into_iter()
            .map(|choice| {
                let Action::Attack { target } = choice.action else {
                    unreachable!()
                };
                target
            })
            .collect();
        assert_eq!(targets, [chair, table]);
        key(&mut game, bl::VirtualKeyCode::Down);
        key(&mut game, bl::VirtualKeyCode::Return);
        assert_eq!(game.turn, 1);
        assert!(matches!(game.panel, Panel::World));
        assert!(
            game.messages
                .iter()
                .any(|message| message.contains(" -> テーブル:"))
        );

        // Directional input is available again after the previous attack animation.
        game.animations.clear();
        key(&mut game, bl::VirtualKeyCode::S);
        key(&mut game, bl::VirtualKeyCode::H);
        assert_eq!(game.turn, 2);
        assert!(game.messages.iter().any(|message| {
            message.contains(&format!(" -> {}:", game.definition(tree).common.name))
        }));
    }

    #[test]
    fn pickup_menu_selects_a_stack_at_feet_without_spending_a_turn_until_confirmed() {
        let mut game = game();
        let player = game.player_id;
        let position = game.position(player).unwrap();
        let bread = game.spawn("item.bread", Some(position), None, 1).unwrap();
        let bolts = game.spawn("ammo.bolt", Some(position), None, 4).unwrap();
        let nearby = game
            .spawn("item.bandage", Some(position.offset(1, 0)), None, 1)
            .unwrap();

        key(&mut game, bl::VirtualKeyCode::G);
        assert!(matches!(game.panel, Panel::PickupMenu));
        assert_eq!(game.interaction_choices().len(), 2);
        key(&mut game, bl::VirtualKeyCode::Down);
        key(&mut game, bl::VirtualKeyCode::Escape);
        assert_eq!(game.turn, 0);
        assert_eq!(game.position(bolts), Some(position));

        key(&mut game, bl::VirtualKeyCode::G);
        key(&mut game, bl::VirtualKeyCode::Down);
        key(&mut game, bl::VirtualKeyCode::Return);
        assert!(matches!(game.panel, Panel::World));
        assert_eq!(game.turn, 1);
        assert!(matches!(&game.entities[&bolts], Entity::Item(item)
            if item.location == ItemLocation::CarriedBy(player) && item.quantity == 4));
        assert_eq!(game.position(bread), Some(position));
        assert_eq!(game.position(nearby), Some(position.offset(1, 0)));
    }

    #[test]
    fn item_capacity_and_stale_or_distant_interactions_cannot_change_world() {
        let mut game = game();
        let player = game.player_id;
        let p = game.position(player).unwrap();
        let chest = game
            .spawn("item.treasure_chest", Some(p.offset(1, 0)), None, 1)
            .unwrap();
        game.actor_mut(player).unwrap().inventory.max_youryou = 0;
        apply(&mut game, Action::PickupNearby { item: chest });
        assert_eq!(game.entities[&chest].map_position(), Some(p.offset(1, 0)));
        assert_eq!(game.turn, 0);
        let distant = game
            .spawn("obstacle.wooden_door", Some(p.offset(4, 0)), None, 1)
            .unwrap();
        assert!(game.interact(distant, "open").is_err());
        game.remove(chest);
        assert!(game.interact(chest, "open").is_err());
        assert_eq!(game.turn, 0);
    }

    #[test]
    fn interaction_selection_uses_menu_length_even_with_many_objects() {
        let mut game = game();
        let p = game.position(game.player_id).unwrap();
        for _ in 0..4 {
            game.spawn("item.treasure_chest", Some(p.offset(1, 0)), None, 1)
                .unwrap();
        }
        key(&mut game, bl::VirtualKeyCode::E);
        key(&mut game, bl::VirtualKeyCode::L);
        assert_eq!(game.interaction_choices().len(), 8);
        apply(&mut game, Action::SelectInteraction { delta: i32::MAX });
        assert_eq!(game.selection, 7);
        assert_eq!(game.turn, 0);
    }
}
