//! player_entity.md に基づくプレイヤーの射撃・装填。
use crate::{config::*, entity::*, game::Game};

impl Game {
    pub(crate) fn toggle_running(&mut self) -> Result<()> {
        ensure(
            matches!(self.panel, crate::game::Panel::World),
            "通常画面で切り替えてください",
        )?;
        let actor = self.actor_mut(self.player_id).ok_or("player missing")?;
        if !actor.running_mode {
            ensure(actor.stamina >= 5, "スタミナが足りません")?;
            ensure(actor.movement_turns() == 1, "脚を失っているため走れません")?;
        }
        actor.running_mode = !actor.running_mode;
        let running = actor.running_mode;
        self.log(if running {
            "走行に切り替えました"
        } else {
            "歩行に切り替えました"
        });
        Ok(())
    }

    pub(crate) fn ranged_weapon(&self, actor: Id) -> Result<Id> {
        let id = self
            .view()
            .active_weapon(actor, true)
            .ok_or("射撃武器を装備してください")?;
        ensure(
            self.entities
                .get(&id)
                .is_some_and(|e| e.alive() && e.carrier() == Some(actor)),
            "射撃武器を装備してください",
        )?;
        ensure(
            self.definition(id)
                .item
                .as_ref()
                .and_then(|i| i.weapon.as_ref())
                .is_some_and(|w| w.mode == "ranged"),
            "射撃武器を装備してください",
        )?;
        Ok(id)
    }

    pub fn shoot(&mut self, actor: Id, target: Id) -> Result<()> {
        let weapon = self.ranged_weapon(actor)?;
        self.attack(actor, target, Some(weapon))
    }

    /// 弾薬を所持品から弾倉へ移す。ターン消費は呼び出し元が扱う。
    pub fn reload(&mut self, actor: Id) -> Result<u32> {
        let (weapon, kind, amount) = self.view().reload_plan(actor)?;
        let items = self
            .actor(actor)
            .ok_or("attacker is not movable")?
            .inventory
            .items
            .clone();
        let mut remaining = amount;
        for id in items {
            if remaining == 0 {
                break;
            }
            let count = match self.entities.get(&id) {
                Some(Entity::Item(i)) if i.script.definition_id == kind => {
                    i.quantity.min(remaining)
                }
                _ => continue,
            };
            self.consume(id, count)?;
            remaining -= count;
        }
        if let Some(Entity::Item(item)) = self.entities.get_mut(&weapon) {
            item.loaded_ammo += amount;
        }
        self.log(format!(
            "{}に{amount}発装填しました",
            self.definition(weapon).common.name
        ));
        if actor == self.player_id
            || self
                .position(actor)
                .is_some_and(|p| self.visible_positions().contains(&p))
        {
            self.queue_sound(crate::sound::Sound::Reload);
        }
        Ok(amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::Action;

    fn game() -> Game {
        let mut game = Game::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let enemies: Vec<_> = game
            .entities
            .iter()
            .filter(|(id, e)| **id != game.player_id && matches!(e, Entity::Movable(_)))
            .map(|(id, _)| *id)
            .collect();
        for id in enemies {
            game.remove(id);
        }
        game
    }

    fn loaded(game: &Game, id: Id) -> u32 {
        match game.entities[&id] {
            Entity::Item(ref i) => i.loaded_ammo,
            _ => panic!("item"),
        }
    }

    #[test]
    fn blunt_weapons_have_a_narrow_damage_range_and_mace_is_stronger() {
        let mut g = game();
        for (definition, bonus) in [("item.club", 4), ("item.mace", 5)] {
            let item = g.spawn(definition, None, Some(g.player_id), 1).unwrap();
            let damage = g.weapon_damage(item).unwrap();
            assert_eq!((damage.dice, damage.sides, damage.bonus), (1, 2, bonus));
        }
    }

    #[test]
    fn running_moves_twice_per_world_turn_and_exhaustion_returns_to_walking() {
        let mut g = game();
        let p = g.player_id;
        let origin = g.position(p).unwrap();
        g.pending_action = Some(Action::ToggleRunning);
        g.update();
        assert!(g.actor(p).unwrap().running_mode);
        assert_eq!(g.turn, 0);
        for step in 0..20 {
            g.player_move(0, if step % 2 == 0 { 1 } else { -1 })
                .unwrap();
            assert_eq!(g.turn, (step + 2) / 2);
            assert_eq!(g.actor(p).unwrap().stamina, 100 - (step as u32 + 1) * 5);
        }
        assert_eq!(g.position(p), Some(origin));
        assert!(!g.actor(p).unwrap().running_mode);
        assert!(g.toggle_running().is_err());
        g.player_move(0, 1).unwrap();
        assert_eq!(g.turn, 11);
        assert_eq!(g.actor(p).unwrap().stamina, 5);
    }

    #[test]
    fn blocked_running_and_toggling_do_not_spend_stamina_or_time() {
        let mut g = game();
        let p = g.player_id;
        let origin = g.position(p).unwrap();
        g.spawn("obstacle.stone_wall", Some(origin.offset(1, 0)), None, 1)
            .unwrap();
        g.toggle_running().unwrap();
        assert!(g.player_move(1, 0).is_err());
        assert_eq!(g.turn, 0);
        assert_eq!(g.actor(p).unwrap().stamina, 100);
        g.player_move(0, 1).unwrap();
        g.toggle_running().unwrap();
        g.toggle_running().unwrap();
        g.player_move(0, -1).unwrap();
        assert_eq!(g.turn, 1);
        assert_eq!(g.actor(p).unwrap().stamina, 90);
        assert!(g.actor(p).unwrap().running_mode);
    }

    #[test]
    fn stamina_recovery_spends_carbohydrate_only_for_actual_recovery() {
        let mut g = game();
        let p = g.player_id;
        g.actor_mut(p).unwrap().stamina = 80;
        for _ in 0..4 {
            g.advance_turn();
        }
        assert_eq!(g.actor(p).unwrap().stamina, 100);
        assert_eq!(g.actor(p).unwrap().nutrition.carbohydrate, 49);
        g.advance_turn();
        assert_eq!(g.actor(p).unwrap().nutrition.carbohydrate, 49);
        g.actor_mut(p).unwrap().stamina = 90;
        g.actor_mut(p).unwrap().nutrition.carbohydrate = 0;
        g.advance_turn();
        assert_eq!(g.actor(p).unwrap().stamina, 90);
        g.actor_mut(p).unwrap().nutrition.carbohydrate = 50;
        g.toggle_running().unwrap();
        g.advance_turn();
        assert_eq!(g.actor(p).unwrap().stamina, 90);
        g.actor_mut(p).unwrap().hp.hp = 0;
        g.update();
        let a = g.actor(p).unwrap();
        assert_eq!(a.stamina, 100);
        assert!(!a.running_mode);
        assert!(!a.run_step_credit);
        assert_eq!(a.stamina_recovery_usage, 0);
    }

    #[test]
    fn running_does_not_speed_up_attacks_or_allow_running_with_missing_legs() {
        let mut g = game();
        let p = g.player_id;
        let origin = g.position(p).unwrap();
        let enemy = g
            .spawn("monster.ghoul_worker", Some(origin.offset(1, 0)), None, 1)
            .unwrap();
        g.toggle_running().unwrap();
        g.player_move(1, 0).unwrap();
        assert_eq!(g.turn, 1);
        assert_eq!(g.position(p), Some(origin));
        assert_eq!(g.actor(p).unwrap().stamina, 100);
        g.remove(enemy);
        g.toggle_running().unwrap();
        g.actor_mut(p)
            .unwrap()
            .body
            .insert("left_leg".into(), InjuryStage::Severed);
        assert!(g.toggle_running().is_err());
    }

    #[test]
    fn reload_combines_stacks_and_failed_reload_does_not_spend_turn() {
        let mut game = game();
        let id = game.player_id;
        let gun = game.spawn("item.glock", None, Some(id), 1).unwrap();
        game.equip_auto(id, gun).unwrap();
        let first = game.spawn("ammo.pistol", None, Some(id), 2).unwrap();
        let second = game.spawn("ammo.pistol", None, Some(id), 3).unwrap();
        game.pending_action = Some(Action::Reload);
        game.update();
        assert_eq!(game.turn, 1);
        assert_eq!(loaded(&game, gun), 5);
        assert!(!game.entities.contains_key(&first) && !game.entities.contains_key(&second));
        game.pending_action = Some(Action::Reload);
        game.update();
        assert_eq!(game.turn, 1);
        assert_eq!(loaded(&game, gun), 5);
        game.drop_item(id, gun).unwrap();
        game.pickup(id, gun).unwrap();
        assert_eq!(loaded(&game, gun), 5);
    }

    #[test]
    fn full_magazine_and_wrong_ammo_leave_inventory_unchanged() {
        let mut game = game();
        let id = game.player_id;
        let gun = game.spawn("item.glock", None, Some(id), 1).unwrap();
        game.equip_auto(id, gun).unwrap();
        let rifle_ammo = game.spawn("ammo.rifle", None, Some(id), 4).unwrap();
        assert!(game.reload(id).is_err());
        assert_eq!(loaded(&game, gun), 0);
        assert!(game.entities.contains_key(&rifle_ammo));
        let ammo = game.spawn("ammo.pistol", None, Some(id), 20).unwrap();
        assert_eq!(game.reload(id).unwrap(), 15);
        assert!(game.reload(id).is_err());
        assert_eq!(loaded(&game, gun), 15);
        assert!(matches!(game.entities[&ammo], Entity::Item(ref i) if i.quantity == 5));
    }

    #[test]
    fn shoot_requires_ranged_weapon_and_loaded_rounds_without_melee_fallback() {
        let mut game = game();
        let id = game.player_id;
        let p = game.position(id).unwrap();
        let target = game
            .spawn("monster.ghoul_worker", Some(p.offset(1, 0)), None, 1)
            .unwrap();
        assert!(game.shoot(id, target).is_err());
        let gun = game.spawn("item.glock", None, Some(id), 1).unwrap();
        game.equip_auto(id, gun).unwrap();
        let hp = game.actor(target).unwrap().hp.hp;
        assert!(game.shoot(id, target).is_err());
        assert_eq!(game.actor(target).unwrap().hp.hp, hp);
        game.spawn("ammo.pistol", None, Some(id), 2).unwrap();
        game.reload(id).unwrap();
        assert!(game.shoot(id, 999999).is_err());
        assert_eq!(loaded(&game, gun), 2);
        game.shoot(id, target).unwrap();
        assert_eq!(loaded(&game, gun), 1);
    }

    #[test]
    fn lua_archer_reloads_then_fires() {
        let mut game = game();
        let id = game.player_id;
        let p = game.position(id).unwrap();
        let enemy = game
            .spawn("monster.ghoul_archer", Some(p.offset(3, 0)), None, 1)
            .unwrap();
        let bow = game.actor(enemy).unwrap().inventory.equipped.unwrap();
        game.events();
        game.advance_turn();
        assert_eq!(loaded(&game, bow), 1);
        game.advance_turn();
        assert_eq!(loaded(&game, bow), 0);
        assert!(game.messages.iter().all(|m| !m.starts_with("Lua ")));
    }
}
