//! 射撃と射程のある近接武器で攻撃する可動Entityを方向入力で選ぶ。確定までターンを進めない。
use crate::{
    config::*,
    entity::*,
    game::{Game, Panel},
    japanese::JapanesePrint,
    scripting::Action,
};
use bracket_lib::prelude as bl;

impl Game {
    pub(crate) fn targeting_key_action(
        &self,
        key: bl::VirtualKeyCode,
        shift: bool,
    ) -> Option<Action> {
        if matches!(key, bl::VirtualKeyCode::F | bl::VirtualKeyCode::Return) {
            return Some(Action::FireSelected);
        }
        match self.inspect_key_action(key, shift)? {
            Action::MoveCursor { dx, dy } => Some(Action::AimDirection { dx, dy }),
            Action::SelectInspected { delta } => Some(Action::CycleTarget { delta }),
            Action::ClosePanel => Some(Action::ClosePanel),
            _ => None,
        }
    }

    pub(crate) fn targeting_weapon(&self) -> Result<Id> {
        self.reach_weapon()
            .map(Ok)
            .unwrap_or_else(|| self.ranged_weapon(self.player_id))
    }
    pub(crate) fn shooting_targets(&self) -> Result<Vec<Id>> {
        let actor = self.player_id;
        let weapon = self.targeting_weapon()?;
        let position = self.position(actor).ok_or("player missing")?;
        let sight = self
            .definition(actor)
            .movable
            .as_ref()
            .unwrap()
            .senses
            .sight_range;
        let visible = self.visible_positions();
        let mut targets: Vec<_> = self
            .entities
            .iter()
            .filter_map(|(id, e)| {
                (matches!(e, Entity::Movable(_))
                    && *id != actor
                    && e.alive()
                    && e.map_position().is_some_and(|p| {
                        position.distance(p) <= sight as i32 && visible.contains(&p)
                    })
                    && self.can_attack(actor, *id, Some(weapon)))
                .then_some(*id)
            })
            .collect();
        targets.sort_by_key(|id| (position.distance(self.position(*id).unwrap()), *id));
        Ok(targets)
    }

    pub(crate) fn open_targeting(&mut self) -> Result<()> {
        let targets = self.shooting_targets()?;
        let nearest = *targets.first().ok_or("射程内に攻撃できる対象がいません")?;
        let preferred = self
            .nearest_hostile(
                self.player_id,
                self.definition(self.player_id)
                    .movable
                    .as_ref()
                    .unwrap()
                    .senses
                    .sight_range,
            )
            .filter(|id| targets.contains(id));
        self.panel = Panel::Targeting {
            active_entity: preferred.unwrap_or(nearest),
        };
        Ok(())
    }

    pub(crate) fn aim_direction(&mut self, dx: i32, dy: i32) -> Result<()> {
        ensure(
            (-1..=1).contains(&dx) && (-1..=1).contains(&dy) && (dx != 0 || dy != 0),
            "invalid aim direction",
        )?;
        let Panel::Targeting { active_entity } = self.panel else {
            return Ok(());
        };
        let origin = self
            .position(active_entity)
            .or_else(|| self.position(self.player_id))
            .ok_or("player missing")?;
        let next = self
            .shooting_targets()?
            .into_iter()
            .filter(|id| *id != active_entity)
            .filter_map(|id| {
                let p = self.position(id)?;
                let (x, y) = (p.x - origin.x, p.y - origin.y);
                let forward = x * dx + y * dy;
                let sideways = (x * dy - y * dx).abs();
                // 指示方向を中心とする90度の範囲で、近い対象を選択する。
                (forward > 0 && sideways <= forward)
                    .then_some(((origin.distance(p), sideways, id), id))
            })
            .min_by_key(|(key, _)| *key)
            .map(|(_, id)| id);
        if let Some(active_entity) = next {
            self.panel = Panel::Targeting { active_entity };
        }
        Ok(())
    }

    pub(crate) fn cycle_target(&mut self, delta: i32) -> Result<()> {
        ensure((-1..=1).contains(&delta), "invalid target selection")?;
        let Panel::Targeting { active_entity } = self.panel else {
            return Ok(());
        };
        let targets = self.shooting_targets()?;
        ensure(!targets.is_empty(), "射程内に攻撃できる対象がいません")?;
        let current = targets
            .iter()
            .position(|id| *id == active_entity)
            .unwrap_or(0);
        let index = (current as i64 + i64::from(delta)).rem_euclid(targets.len() as i64) as usize;
        self.panel = Panel::Targeting {
            active_entity: targets[index],
        };
        Ok(())
    }

    pub(crate) fn fire_selected(&mut self) -> Result<()> {
        let Panel::Targeting { active_entity } = self.panel else {
            return Ok(());
        };
        ensure(
            self.shooting_targets()?.contains(&active_entity),
            "選択した対象には攻撃できません",
        )?;
        let weapon = self.targeting_weapon()?;
        self.attack(self.player_id, active_entity, Some(weapon))?;
        self.panel = Panel::World;
        self.events();
        self.advance_turn();
        Ok(())
    }

    pub(crate) fn render_targeting(&self, ctx: &mut bl::BTerm, active_entity: Id) {
        if let Some(p) = self.position(active_entity) {
            self.map_highlight(ctx, p, bl::RGB::named(crate::theme::DANGER));
        }
        let top = self.map.height + 4;
        crate::theme::draw_box(
            ctx,
            0,
            top,
            79,
            49 - top,
            crate::theme::ACCENT,
            crate::theme::BACKGROUND,
        );
        ctx.print_ja_color(
            2,
            top + 1,
            bl::RGB::named(crate::theme::ACCENT),
            "攻撃対象を選択  確定するまで時間は進みません",
        );
        ctx.print_ja(
            2,
            top + 2,
            "hjklyubn/矢印:方向選択  Tab:切替  Enter/f:攻撃  Esc:中止",
        );
        if let Some(Entity::Movable(target)) = self.entities.get(&active_entity) {
            let definition = self.definition(active_entity);
            ctx.print_ja(
                2,
                top + 4,
                format!(
                    "対象: {} ({}, {})",
                    definition.common.name, target.position.x, target.position.y
                ),
            );
            let mut description_top = top + 5;
            if let Some(lines) = self.observed_condition_lines(active_entity) {
                for (row, line) in lines
                    .iter()
                    .take((49 - description_top).max(0) as usize)
                    .enumerate()
                {
                    ctx.print_ja_color(
                        2,
                        description_top + row as i32,
                        bl::RGB::named(if row == 0 {
                            crate::theme::ACCENT
                        } else {
                            crate::theme::TEXT
                        }),
                        line,
                    );
                }
                description_top += lines.len() as i32;
            }
            for (row, line) in crate::inspect::wrap_description(&definition.common.description, 76)
                .iter()
                .take((49 - description_top).max(0) as usize)
                .enumerate()
            {
                ctx.print_ja(2, description_top + row as i32, line);
            }
        } else {
            ctx.print_ja(
                2,
                top + 4,
                "対象がいません。方向キーかTabで選び直してください",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::japanese::message;

    fn setup() -> (Game, Id, Pos) {
        let mut game = Game::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let player = game.player_id;
        let enemies: Vec<_> = game
            .entities
            .iter()
            .filter(|(id, e)| **id != player && matches!(e, Entity::Movable(_)))
            .map(|(id, _)| *id)
            .collect();
        for enemy in enemies {
            game.remove(enemy);
        }
        let gun = game.spawn("item.glock", None, Some(player), 1).unwrap();
        game.equip_auto(player, gun).unwrap();
        game.spawn("ammo.pistol", None, Some(player), 20).unwrap();
        game.reload(player).unwrap();
        let p = game.position(player).unwrap();
        (game, gun, p)
    }

    fn update(game: &mut Game, action: Action) {
        game.pending_action = Some(action);
        game.update();
    }

    #[test]
    fn reach_melee_uses_directional_enemy_selection_and_bump_attack() {
        for (kind, range) in [("item.spear", 2), ("item.club", 3)] {
            let (mut game, _, p) = setup();
            let actor = game.player_id;
            let weapon = game.spawn(kind, None, Some(actor), 1).unwrap();
            if range == 3 {
                let defs = std::sync::Arc::make_mut(&mut game.entity_definitions);
                defs.get_mut(kind)
                    .unwrap()
                    .item
                    .as_mut()
                    .unwrap()
                    .weapon
                    .as_mut()
                    .unwrap()
                    .range = 3;
                defs[kind].validate().unwrap();
            }
            game.equip_in_hand(actor, weapon, Grip::Both).unwrap();
            game.take_sounds().for_each(drop);
            let near = game
                .spawn("monster.ghoul_brute", Some(p.offset(2, 0)), None, 1)
                .unwrap();
            let side = game
                .spawn("monster.ghoul_brute", Some(p.offset(0, 2)), None, 1)
                .unwrap();
            assert!(matches!(game.key_action('f'), Action::OpenTargeting));
            update(&mut game, Action::OpenTargeting);
            assert!(matches!(game.panel,Panel::Targeting{active_entity} if active_entity==near));
            update(&mut game, Action::AimDirection { dx: -1, dy: 1 });
            assert!(matches!(game.panel,Panel::Targeting{active_entity} if active_entity==side));
            assert_eq!(game.turn, 0);
            update(&mut game, Action::FireSelected);
            assert_eq!(game.turn, 1);
            assert!(game.projectiles.is_empty());
            assert!(game.take_sounds().any(|s| s == crate::sound::Sound::Melee));
            let adjacent = game
                .spawn("monster.ghoul_brute", Some(p.offset(-1, 0)), None, 1)
                .unwrap();
            game.player_move(-1, 0).unwrap();
            assert_eq!(game.position(actor), Some(p));
            assert_eq!(game.turn, 2);
            assert!(game.entities.contains_key(&adjacent));
            assert!(game.projectiles.is_empty());
        }
    }
    #[test]
    fn reach_target_is_rechecked_when_removed_or_hidden() {
        let (mut game, _, p) = setup();
        let actor = game.player_id;
        let spear = game.spawn("item.spear", None, Some(actor), 1).unwrap();
        game.equip_in_hand(actor, spear, Grip::Both).unwrap();
        let target = game
            .spawn("monster.ghoul_brute", Some(p.offset(2, 0)), None, 1)
            .unwrap();
        game.open_targeting().unwrap();
        let wall = game
            .spawn("obstacle.stone_wall", Some(p.offset(1, 0)), None, 1)
            .unwrap();
        assert!(game.fire_selected().is_err());
        assert_eq!(game.turn, 0);
        game.remove(wall);
        game.remove(target);
        assert!(game.fire_selected().is_err());
        assert_eq!(game.turn, 0);
    }

    #[test]
    fn direction_selection_and_cancel_do_not_fire_or_advance_turn() {
        let (mut game, gun, p) = setup();
        let near = game
            .spawn("monster.ghoul_warrior", Some(p.offset(2, 0)), None, 1)
            .unwrap();
        let east = game
            .spawn("monster.ghoul_archer", Some(p.offset(4, 0)), None, 1)
            .unwrap();
        let north = game
            .spawn("monster.ghoul_worker", Some(p.offset(2, -3)), None, 1)
            .unwrap();
        update(&mut game, Action::OpenTargeting);
        assert!(matches!(game.panel, Panel::Targeting { active_entity } if active_entity == near));
        let action = game
            .targeting_key_action(bl::VirtualKeyCode::Right, false)
            .unwrap();
        update(&mut game, action);
        assert!(matches!(game.panel, Panel::Targeting { active_entity } if active_entity == east));
        update(&mut game, Action::AimDirection { dx: 0, dy: -1 });
        assert!(matches!(game.panel, Panel::Targeting { active_entity } if active_entity == north));
        update(&mut game, Action::ClosePanel);
        assert_eq!(game.turn, 0);
        assert_eq!(game.position(game.player_id), Some(p));
        assert!(matches!(game.entities[&gun], Entity::Item(ref i) if i.loaded_ammo == 15));
        assert_eq!(game.position(east), Some(p.offset(4, 0)));
        assert!(matches!(game.panel, Panel::World));
    }

    #[test]
    fn selected_entity_is_fired_at_once_and_log_uses_names() {
        let (mut game, gun, p) = setup();
        game.spawn("monster.ghoul_worker", Some(p.offset(2, 0)), None, 1)
            .unwrap();
        let east = game
            .spawn("monster.ghoul_archer", Some(p.offset(4, 0)), None, 1)
            .unwrap();
        update(&mut game, Action::OpenTargeting);
        update(&mut game, Action::AimDirection { dx: 1, dy: 0 });
        assert!(matches!(game.panel, Panel::Targeting { active_entity } if active_entity == east));
        let action = game
            .targeting_key_action(bl::VirtualKeyCode::Return, false)
            .unwrap();
        update(&mut game, action);
        assert_eq!(game.turn, 1);
        assert!(matches!(game.panel, Panel::World));
        assert!(matches!(game.entities[&gun], Entity::Item(ref i) if i.loaded_ammo == 14));
        let log = game
            .messages
            .iter()
            .find(|m| m.starts_with("生存者 -> "))
            .unwrap();
        assert!(message(log).contains("生存者 → グール アーチャー"));
        assert!(!log.contains('#'));
    }

    #[test]
    fn equipment_log_uses_item_name() {
        let (mut game, gun, _) = setup();
        update(&mut game, Action::Equip { item: gun });
        assert_eq!(game.messages.back().unwrap(), "グロックを装備しました");
    }

    #[test]
    fn dead_static_occluded_and_out_of_range_entities_are_not_targets() {
        let (mut game, _, p) = setup();
        let valid = game
            .spawn("monster.ghoul_worker", Some(p.offset(2, 0)), None, 1)
            .unwrap();
        game.spawn("monster.ghoul_worker", Some(p.offset(7, 0)), None, 1)
            .unwrap();
        game.spawn("obstacle.stone_wall", Some(p.offset(1, 1)), None, 1)
            .unwrap();
        game.spawn("monster.ghoul_worker", Some(p.offset(2, 2)), None, 1)
            .unwrap();
        let dead = game
            .spawn("monster.ghoul_worker", Some(p.offset(0, -2)), None, 1)
            .unwrap();
        game.damage(game.player_id, dead, u32::MAX).unwrap();
        assert_eq!(game.shooting_targets().unwrap(), vec![valid]);
    }

    #[test]
    fn removed_target_cannot_spend_ammo_or_turn_and_names_survive_death() {
        let (mut game, gun, p) = setup();
        let target = game
            .spawn("monster.ghoul_worker", Some(p.offset(2, 0)), None, 1)
            .unwrap();
        update(&mut game, Action::OpenTargeting);
        game.remove(target);
        update(&mut game, Action::FireSelected);
        assert_eq!(game.turn, 0);
        assert!(matches!(game.entities[&gun], Entity::Item(ref i) if i.loaded_ammo == 15));
        let target = game
            .spawn("monster.ghoul_worker", Some(p.offset(2, 0)), None, 1)
            .unwrap();
        game.actor_mut(target).unwrap().hp.hp = 1;
        for _ in 0..15 {
            game.shoot(game.player_id, target).unwrap();
            game.events();
            if !game.entities.contains_key(&target) {
                break;
            }
        }
        assert!(!game.entities.contains_key(&target));
        let log = game
            .messages
            .iter()
            .rev()
            .find(|log| message(log).contains("生存者 → グール ワーカー"))
            .unwrap();
        assert!(message(log).contains("生存者 → グール ワーカー"));
        assert!(!log.contains('#'));
    }
}
