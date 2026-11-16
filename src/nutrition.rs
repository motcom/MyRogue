//! 栄養を消費する自然回復と、ビタミンに応じた風邪・肺炎の進行と回復。
use crate::{
    entity::{Entity, LuaHookRef, Status},
    game::Game,
};
impl Game {
    pub(crate) fn record_attack_exertion(&mut self, id: crate::entity::Id) {
        if let Some(a) = self.actor_mut(id) {
            a.carbohydrate_usage = a.carbohydrate_usage.saturating_add(1);
        }
    }

    pub(crate) fn update_nutrition_turn(&mut self) {
        let ids: Vec<_> = self
            .entities
            .iter()
            .filter_map(|(id, e)| (matches!(e, Entity::Movable(_)) && e.alive()).then_some(*id))
            .collect();
        for id in ids {
            let consume_nutrition = self.turn > 0 && self.turn.is_multiple_of(20);
            let is_player = id == self.player_id;
            let a = self.actor_mut(id).unwrap();
            a.carbohydrate_usage = a.carbohydrate_usage.saturating_add(1);
            a.nutrition.carbohydrate = a
                .nutrition
                .carbohydrate
                .saturating_sub(a.carbohydrate_usage / 20);
            a.carbohydrate_usage %= 20;
            if consume_nutrition {
                a.nutrition.vitamin = a.nutrition.vitamin.saturating_sub(1);
            }
            if is_player && !a.running_mode && a.stamina < 100 && a.nutrition.carbohydrate > 0 {
                let restored = (100 - a.stamina).min(5);
                a.stamina += restored;
                a.stamina_recovery_usage += restored;
                a.nutrition.carbohydrate = a
                    .nutrition
                    .carbohydrate
                    .saturating_sub(a.stamina_recovery_usage / 20);
                a.stamina_recovery_usage %= 20;
            }
            let illness = a.respiratory_status();
            let recovering = a.nutrition.vitamin > 0;
            let eligible = if recovering {
                a.illness_turns = 0;
                illness.is_some()
            } else {
                a.illness_recovery_turns = 0;
                illness != Some("pneumonia")
            };
            let counter = if recovering {
                &mut a.illness_recovery_turns
            } else {
                &mut a.illness_turns
            };
            if !eligible {
                *counter = 0;
            } else {
                *counter += 1;
                if *counter == 100 {
                    *counter = 0;
                    if self.disease_rng.roll(if recovering { 2 } else { 6 }) == 1 {
                        let next = match (illness, recovering) {
                            (Some("pneumonia"), true) => Some("cold"),
                            (Some("cold"), true) => None,
                            (Some("cold"), false) => Some("pneumonia"),
                            _ => Some("cold"),
                        };
                        let a = self.actor_mut(id).unwrap();
                        a.status
                            .retain(|s| s.name != "cold" && s.name != "pneumonia");
                        if let Some(name) = next {
                            a.status.push(Status {
                                name: name.into(),
                                enable: true,
                                value: 1,
                                exe: LuaHookRef::new("on_status"),
                            });
                        }
                        a.illness_turns = 0;
                        a.illness_recovery_turns = 0;
                        a.pneumonia_move_turns = 0;
                        if id == self.player_id {
                            self.log(match (next, recovering) {
                                (None, _) => "風邪が治りました",
                                (Some("cold"), true) => "肺炎が改善して風邪になりました",
                                (Some("pneumonia"), _) => "風邪が悪化して肺炎になりました",
                                _ => "風邪をひきました",
                            });
                        }
                    }
                }
            }
            let a = self.actor_mut(id).unwrap();
            if a.can_regenerate() && a.hp.hp < a.hp.max_hp {
                a.hp.hp += 1;
                a.nutrition.protein -= 1;
            }
        }
    }

    /// 成功した移動の所要ターンだけを数える。待機・失敗・攻撃では進めない。
    pub(crate) fn pneumonia_movement(
        &mut self,
        id: crate::entity::Id,
        turns: u64,
    ) -> crate::config::Result<()> {
        let a = self.actor_mut(id).ok_or("movement target not actor")?;
        if a.respiratory_status() != Some("pneumonia") || a.hp.hp == 0 {
            a.pneumonia_move_turns = 0;
            return Ok(());
        }
        let elapsed = a.pneumonia_move_turns.saturating_add(turns);
        a.pneumonia_move_turns = elapsed % 10;
        let amount = (elapsed / 10).min(u64::from(a.hp.hp.saturating_sub(1))) as u32;
        if amount > 0 {
            self.damage(id, id, amount)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{combat::Rng, config::Nutrition, entity::InjuryStage, scripting::Action};
    fn game() -> Game {
        let mut g = Game::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let ids: Vec<_> = g
            .entities
            .iter()
            .filter_map(|(id, e)| {
                (matches!(e, Entity::Movable(_)) && *id != g.player_id).then_some(*id)
            })
            .collect();
        for id in ids {
            g.remove(id);
        }
        g
    }
    #[test]
    fn nutrition_starts_at_fifty_and_addition_caps_at_one_hundred() {
        let g = game();
        let actor = g.actor(g.player_id).unwrap();
        assert_eq!(actor.nutrition, Nutrition::initial());
        assert!(
            g.entity_snapshot(g.player_id).unwrap()["needs"]
                .get("satiety")
                .is_none()
        );
        let mut n = actor.nutrition.clone();
        n.add(&Nutrition {
            protein: u32::MAX,
            carbohydrate: 60,
            vitamin: 49,
        });
        assert_eq!(
            n,
            Nutrition {
                protein: 100,
                carbohydrate: 100,
                vitamin: 99
            }
        );
    }
    #[test]
    fn carbohydrate_and_vitamins_decrease_every_twenty_world_turns_even_when_waiting() {
        let mut g = game();
        let p = g.player_id;
        turns(&mut g, 19);
        assert_eq!(g.actor(p).unwrap().nutrition.carbohydrate, 50);
        assert_eq!(g.actor(p).unwrap().nutrition.vitamin, 50);
        g.pending_action = Some(Action::OpenInventory);
        g.update();
        g.pending_action = Some(Action::ClosePanel);
        g.update();
        assert_eq!(g.turn, 19);
        g.advance_turn();
        assert_eq!(g.actor(p).unwrap().nutrition.carbohydrate, 49);
        assert_eq!(g.actor(p).unwrap().nutrition.vitamin, 49);
        turns(&mut g, 20);
        assert_eq!(g.actor(p).unwrap().nutrition.carbohydrate, 48);
        assert_eq!(g.actor(p).unwrap().nutrition.vitamin, 48);
        let a = g.actor_mut(p).unwrap();
        a.nutrition.carbohydrate = 0;
        a.nutrition.vitamin = 0;
        turns(&mut g, 10);
        assert_eq!(g.actor(p).unwrap().nutrition.carbohydrate, 0);
        assert_eq!(g.actor(p).unwrap().nutrition.vitamin, 0);
    }
    #[test]
    fn movement_has_no_extra_nutrition_cost_and_slow_steps_use_world_time() {
        let mut g = game();
        let p = g.player_id;
        let origin = g.position(p).unwrap();
        g.player_move(0, 1).unwrap();
        assert_eq!(g.actor(p).unwrap().nutrition.carbohydrate, 50);
        let blocked = origin.offset(1, 1);
        g.spawn("obstacle.stone_wall", Some(blocked), None, 1)
            .unwrap();
        assert!(g.player_move(1, 0).is_err());
        assert_eq!(g.turn, 1);
        let a = g.actor_mut(p).unwrap();
        a.body.insert("left_leg".into(), InjuryStage::Severed);
        a.treatment.stopped.insert("left_leg".into());
        g.player_move(0, 1).unwrap();
        g.player_move(0, -1).unwrap();
        assert_eq!(g.turn, 9);
        assert_eq!(g.actor(p).unwrap().nutrition.carbohydrate, 50);
        g.player_move(0, 1).unwrap();
        assert_eq!(g.turn, 13);
        assert_eq!(g.actor(p).unwrap().nutrition.carbohydrate, 50);
        assert_eq!(g.actor(p).unwrap().nutrition.vitamin, 50);
        turns(&mut g, 7);
        assert_eq!(g.actor(p).unwrap().nutrition.carbohydrate, 49);
        assert_eq!(g.actor(p).unwrap().nutrition.vitamin, 49);
    }
    #[test]
    fn attacks_double_carbohydrate_use_and_mix_with_normal_turns() {
        let mut g = game();
        let p = g.player_id;
        let origin = g.position(p).unwrap();
        let wall = g
            .spawn("obstacle.stone_wall", Some(origin.offset(1, 0)), None, 1)
            .unwrap();
        if let Entity::Static(s) = g.entities.get_mut(&wall).unwrap() {
            s.taikyudo = 100000;
        }
        turns(&mut g, 4);
        for _ in 0..2 {
            g.attack_static(wall).unwrap();
        }
        assert_eq!(g.turn, 6);
        assert_eq!(g.actor(p).unwrap().nutrition.carbohydrate, 50);
        assert_eq!(g.actor(p).unwrap().carbohydrate_usage, 8);
        let far = g
            .spawn("obstacle.stone_wall", Some(origin.offset(5, 0)), None, 1)
            .unwrap();
        assert!(g.attack_static(far).is_err());
        assert_eq!(g.actor(p).unwrap().carbohydrate_usage, 8);
        assert_eq!(g.turn, 6);
        for _ in 0..6 {
            g.attack_static(wall).unwrap();
        }
        assert_eq!(g.actor(p).unwrap().nutrition.carbohydrate, 49);
        assert_eq!(g.actor(p).unwrap().nutrition.vitamin, 50);
        for _ in 0..10 {
            g.attack_static(wall).unwrap();
        }
        assert_eq!(g.actor(p).unwrap().nutrition.carbohydrate, 48);
        assert_eq!(g.actor(p).unwrap().nutrition.vitamin, 49);
    }
    #[test]
    fn vitamin_depletion_switches_disease_checks_to_worsening_on_the_same_turn() {
        let mut g = game();
        let p = g.player_id;
        set_illness(&mut g, "cold");
        g.actor_mut(p).unwrap().nutrition.vitamin = 1;
        turns(&mut g, 19);
        assert_eq!(g.actor(p).unwrap().illness_recovery_turns, 19);
        g.advance_turn();
        assert_eq!(g.actor(p).unwrap().nutrition.vitamin, 0);
        assert_eq!(g.actor(p).unwrap().illness_recovery_turns, 0);
        assert_eq!(g.actor(p).unwrap().illness_turns, 1);
    }
    #[test]
    fn recovery_spends_protein_only_when_hp_increases() {
        let mut g = game();
        let p = g.player_id;
        g.advance_turn();
        assert_eq!(g.actor(p).unwrap().nutrition.protein, 50);
        g.actor_mut(p).unwrap().hp.hp = 40;
        g.pending_action = Some(Action::OpenProperties);
        g.update();
        assert_eq!(g.actor(p).unwrap().hp.hp, 40);
        g.advance_turn();
        assert_eq!(g.actor(p).unwrap().hp.hp, 41);
        assert_eq!(g.actor(p).unwrap().nutrition.protein, 49);
        g.actor_mut(p).unwrap().nutrition.protein = 0;
        g.advance_turn();
        assert_eq!(g.actor(p).unwrap().hp.hp, 41);
        g.actor_mut(p).unwrap().nutrition.protein = 1;
        g.actor_mut(p).unwrap().hp.hp = 59;
        g.advance_turn();
        g.advance_turn();
        assert_eq!(g.actor(p).unwrap().hp.hp, 60);
        assert_eq!(g.actor(p).unwrap().nutrition.protein, 0);
        g.actor_mut(p).unwrap().hp.hp = 0;
        g.update_nutrition_turn();
        assert_eq!(g.actor(p).unwrap().hp.hp, 0);
    }
    fn set_illness(g: &mut Game, name: &str) {
        let p = g.player_id;
        g.actor_mut(p).unwrap().status.push(Status {
            name: name.into(),
            enable: true,
            value: 1,
            exe: LuaHookRef::new("on_status"),
        });
    }
    fn turns(g: &mut Game, count: usize) {
        for _ in 0..count {
            g.advance_turn();
        }
    }
    fn roll_seed(sides: u32, success: bool) -> u64 {
        (0..10000)
            .find(|seed| (Rng::new(*seed).roll(sides) == 1) == success)
            .unwrap()
    }
    #[test]
    fn deficiency_and_cold_worsen_only_after_a_full_hundred_turns() {
        let mut g = game();
        let p = g.player_id;
        let mut expected = g.rng.clone();
        g.actor_mut(p).unwrap().nutrition.vitamin = 0;
        g.disease_rng = Rng::new(roll_seed(6, true));
        turns(&mut g, 99);
        assert_eq!(g.actor(p).unwrap().respiratory_status(), None);
        g.advance_turn();
        assert_eq!(g.actor(p).unwrap().respiratory_status(), Some("cold"));
        assert_eq!(g.entity_snapshot(p).unwrap()["statuses"]["cold"], 1);
        g.disease_rng = Rng::new(roll_seed(6, true));
        turns(&mut g, 99);
        assert_eq!(g.actor(p).unwrap().respiratory_status(), Some("cold"));
        g.advance_turn();
        assert_eq!(g.actor(p).unwrap().respiratory_status(), Some("pneumonia"));
        assert!(
            !g.entity_snapshot(p).unwrap()["statuses"]
                .as_object()
                .unwrap()
                .contains_key("cold")
        );
        turns(&mut g, 200);
        assert_eq!(g.actor(p).unwrap().respiratory_status(), Some("pneumonia"));
        assert_eq!(g.rng.roll(1000), expected.roll(1000));
    }
    #[test]
    fn failed_disease_roll_waits_another_hundred_and_vitamins_prevent_onset() {
        let mut g = game();
        let p = g.player_id;
        let seed = roll_seed(6, true);
        g.disease_rng = Rng::new(seed);
        turns(&mut g, 200);
        assert_eq!(g.actor(p).unwrap().respiratory_status(), None);
        assert_eq!(g.disease_rng.roll(6), Rng::new(seed).roll(6));
        g.actor_mut(p).unwrap().nutrition.vitamin = 0;
        g.disease_rng = Rng::new(roll_seed(6, false));
        turns(&mut g, 100);
        assert_eq!(g.actor(p).unwrap().respiratory_status(), None);
        g.disease_rng = Rng::new(seed);
        turns(&mut g, 99);
        assert_eq!(g.actor(p).unwrap().respiratory_status(), None);
        g.advance_turn();
        assert_eq!(g.actor(p).unwrap().respiratory_status(), Some("cold"));
    }
    #[test]
    fn vitamins_recover_one_stage_per_hundred_with_half_chance() {
        let mut g = game();
        let p = g.player_id;
        set_illness(&mut g, "pneumonia");
        g.actor_mut(p).unwrap().hp.hp = 30;
        g.disease_rng = Rng::new(roll_seed(2, false));
        turns(&mut g, 100);
        assert_eq!(g.actor(p).unwrap().respiratory_status(), Some("pneumonia"));
        for expected in [Some("cold"), None] {
            g.disease_rng = Rng::new(roll_seed(2, true));
            turns(&mut g, 99);
            assert!(g.actor(p).unwrap().respiratory_status().is_some());
            assert_eq!(g.actor(p).unwrap().hp.hp, 30);
            assert_eq!(g.actor(p).unwrap().nutrition.protein, 50);
            g.advance_turn();
            assert_eq!(g.actor(p).unwrap().respiratory_status(), expected);
        }
        assert_eq!(g.actor(p).unwrap().hp.hp, 31);
        assert_eq!(g.actor(p).unwrap().nutrition.protein, 49);
    }
    #[test]
    fn changing_vitamin_availability_restarts_the_appropriate_interval_and_ui_is_free() {
        let mut g = game();
        let p = g.player_id;
        g.actor_mut(p).unwrap().nutrition.vitamin = 0;
        turns(&mut g, 99);
        g.actor_mut(p).unwrap().nutrition.vitamin = 50;
        g.advance_turn();
        assert_eq!(g.actor(p).unwrap().illness_turns, 0);
        set_illness(&mut g, "cold");
        turns(&mut g, 98);
        g.pending_action = Some(Action::OpenProperties);
        g.update();
        assert_eq!(g.actor(p).unwrap().illness_recovery_turns, 98);
        g.pending_action = Some(Action::ClosePanel);
        g.update();
        g.actor_mut(p).unwrap().nutrition.vitamin = 0;
        g.advance_turn();
        assert_eq!(g.actor(p).unwrap().illness_recovery_turns, 0);
        assert_eq!(g.actor(p).unwrap().illness_turns, 1);
        g.actor_mut(p).unwrap().nutrition.vitamin = 50;
        g.advance_turn();
        assert_eq!(g.actor(p).unwrap().illness_recovery_turns, 1);
        assert_eq!(g.actor(p).unwrap().illness_turns, 0);
    }
    #[test]
    fn pneumonia_damages_only_successful_movement_every_ten_turns_and_cannot_kill() {
        let mut g = game();
        let p = g.player_id;
        set_illness(&mut g, "pneumonia");
        g.actor_mut(p).unwrap().hp.hp = 10;
        g.actor_mut(p).unwrap().nutrition.vitamin = 0;
        let origin = g.position(p).unwrap();
        let dest = origin.offset(0, 1);
        turns(&mut g, 20);
        assert_eq!(g.actor(p).unwrap().hp.hp, 10);
        for i in 0..9 {
            g.player_move(0, if i % 2 == 0 { 1 } else { -1 }).unwrap();
        }
        assert_eq!(g.actor(p).unwrap().hp.hp, 10);
        assert_eq!(g.actor(p).unwrap().pneumonia_move_turns, 9);
        let blocked = dest.offset(1, 0);
        g.spawn("obstacle.stone_wall", Some(blocked), None, 1)
            .unwrap();
        assert!(g.player_move(1, 0).is_err());
        assert_eq!(g.actor(p).unwrap().pneumonia_move_turns, 9);
        g.player_move(0, -1).unwrap();
        assert_eq!(g.actor(p).unwrap().hp.hp, 9);
        assert_eq!(g.actor(p).unwrap().pneumonia_move_turns, 0);
        g.actor_mut(p).unwrap().hp.hp = 1;
        for i in 0..20 {
            g.player_move(0, if i % 2 == 0 { 1 } else { -1 }).unwrap();
        }
        assert_eq!(g.actor(p).unwrap().hp.hp, 1);
        // 他のダメージは通常どおり死亡させられる。
        g.damage(p, p, 1).unwrap();
        assert_eq!(g.actor(p).unwrap().hp.hp, 0);
    }
    #[test]
    fn pneumonia_counts_slow_movement_turns_but_not_movement_cooldown() {
        let mut g = game();
        let p = g.player_id;
        set_illness(&mut g, "pneumonia");
        let a = g.actor_mut(p).unwrap();
        a.nutrition.vitamin = 0;
        a.hp.hp = 10;
        a.body.insert("left_leg".into(), InjuryStage::Severed);
        a.treatment.stopped.insert("left_leg".into());
        let origin = g.position(p).unwrap();
        g.move_to(p, origin.offset(0, 1)).unwrap();
        g.move_to(p, origin).unwrap(); // 移動待ち中は動かず、病気の累計も変えない。
        assert_eq!(g.actor(p).unwrap().pneumonia_move_turns, 4);
        turns(&mut g, 4);
        g.player_move(0, -1).unwrap();
        g.player_move(0, 1).unwrap();
        assert_eq!(g.actor(p).unwrap().pneumonia_move_turns, 2);
        assert_eq!(g.actor(p).unwrap().hp.hp, 9);
    }
    #[test]
    fn illness_stops_passive_recovery_but_syringes_still_heal() {
        let mut g = game();
        let p = g.player_id;
        for name in ["cold", "pneumonia"] {
            g.actor_mut(p).unwrap().status.clear();
            set_illness(&mut g, name);
            g.actor_mut(p).unwrap().hp.hp = 30;
            g.advance_turn();
            assert_eq!(g.actor(p).unwrap().hp.hp, 30);
            let item = g.spawn("item.syringe", None, Some(p), 1).unwrap();
            assert!(g.use_item(item).unwrap());
            assert_eq!(g.actor(p).unwrap().hp.hp, 45);
            assert_eq!(g.actor(p).unwrap().nutrition.protein, 50);
        }
    }
    #[test]
    fn respawn_resets_nutrition_and_illness() {
        let mut g = game();
        let p = g.player_id;
        g.actor_mut(p).unwrap().nutrition = Nutrition::default();
        g.actor_mut(p).unwrap().status.push(Status {
            name: "cold".into(),
            enable: true,
            value: 20,
            exe: LuaHookRef::new("on_status"),
        });
        g.actor_mut(p).unwrap().carbohydrate_usage = 9;
        g.actor_mut(p).unwrap().illness_turns = 99;
        g.actor_mut(p).unwrap().illness_recovery_turns = 99;
        g.actor_mut(p).unwrap().pneumonia_move_turns = 9;
        g.damage(p, p, u32::MAX).unwrap();
        g.update();
        assert_eq!(g.actor(p).unwrap().nutrition, Nutrition::initial());
        assert!(!g.actor(p).unwrap().status.iter().any(|s| s.enable));
        assert_eq!(g.actor(p).unwrap().carbohydrate_usage, 0);
        assert_eq!(g.actor(p).unwrap().illness_turns, 0);
        assert_eq!(g.actor(p).unwrap().illness_recovery_turns, 0);
        assert_eq!(g.actor(p).unwrap().pneumonia_move_turns, 0);
    }
}
