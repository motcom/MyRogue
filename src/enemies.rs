//! Luaが選んだグール行動を、通常の移動・拾得・装備・戦闘の検査へ接続する。
use crate::{config::*, entity::*, game::Game};
use serde::Deserialize;
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GhoulBehavior {
    Runner,
    Butcher,
    Guard,
    Howler,
    Scavenger,
    Stalker,
    Brute,
    Rifleman,
}
impl Game {
    pub(crate) fn ghoul_turn(&mut self, actor: Id, behavior: GhoulBehavior) -> Result<()> {
        use GhoulBehavior::*;
        let a = self.actor(actor).ok_or("not an actor")?;
        ensure(
            self.entities[&actor].alive() && a.controller == Controller::Lua,
            "invalid AI actor",
        )?;
        if self.turn < a.stunned_until {
            return Ok(());
        }
        if behavior == Butcher && !self.turn.is_multiple_of(2) {
            return Ok(());
        }
        let range = self
            .definition(actor)
            .movable
            .as_ref()
            .unwrap()
            .senses
            .sight_range;
        let target = self.nearest_hostile(actor, range);
        if behavior == Scavenger && self.scavenge(actor)? {
            return Ok(());
        }
        let Some(target) = target else {
            self.respond_to_alert(actor)?;
            return Ok(());
        };
        let p = self.position(actor).unwrap();
        let q = self.position(target).unwrap();
        if behavior == Stalker && p.distance(q) > 3 {
            return Ok(());
        }
        if behavior == Howler && self.actor(actor).unwrap().alert_until < self.turn {
            let faction = self
                .definition(actor)
                .movable
                .as_ref()
                .unwrap()
                .faction
                .clone();
            let allies: Vec<_> = self
                .entities
                .iter()
                .filter_map(|(id, e)| {
                    (e.alive()
                        && self.actor(*id).is_some()
                        && e.map_position().is_some_and(|pos| pos.distance(p) <= 8)
                        && self
                            .definition(*id)
                            .movable
                            .as_ref()
                            .is_some_and(|a| a.faction == faction))
                    .then_some(*id)
                })
                .collect();
            let until = self.turn.saturating_add(6);
            for id in allies {
                let a = self.actor_mut(id).unwrap();
                a.alert_position = Some(q);
                a.alert_until = until;
            }
            if self.visible_positions().contains(&p) {
                self.log("グールの叫びが響いた");
            }
            return Ok(());
        }
        if self.can_attack(actor, target, None) {
            return self.attack(actor, target, None);
        }
        if self.view().reload_plan(actor).is_ok() {
            self.reload(actor)?;
            return Ok(());
        }
        let steps = if behavior == Runner { 2 } else { 1 };
        for _ in 0..steps {
            if let Some(next) = self.step_towards(actor, target) {
                self.move_to(actor, next)?;
            }
            // 高速型も一世界ターン一攻撃。切断による移動待ちはmove_toが守る。
            if self.can_attack(actor, target, None) {
                break;
            }
        }
        Ok(())
    }
    pub(crate) fn respond_to_alert(&mut self, actor: Id) -> Result<bool> {
        let Some(a) = self.actor(actor) else {
            return Ok(false);
        };
        let Some(p) = a.alert_position.filter(|_| self.turn <= a.alert_until) else {
            return Ok(false);
        };
        let range = self
            .definition(actor)
            .movable
            .as_ref()
            .unwrap()
            .senses
            .sight_range;
        if self.nearest_hostile(actor, range).is_some() {
            return Ok(false);
        };
        if self.definition(actor).id == "monster.ghoul_butcher" && !self.turn.is_multiple_of(2) {
            return Ok(true);
        };
        let occupant = self
            .at(p)
            .into_iter()
            .find(|id| self.actor(*id).is_some())
            .unwrap_or(0);
        if let Some(next) = self.view().step_towards_position(actor, p, occupant) {
            self.move_to(actor, next)?;
            return Ok(true);
        }
        Ok(false)
    }
    fn scavenge(&mut self, actor: Id) -> Result<bool> {
        let origin = self.position(actor).ok_or("actor missing")?;
        let range = self
            .definition(actor)
            .movable
            .as_ref()
            .unwrap()
            .senses
            .sight_range as i32;
        let current = self.view().active_weapon(actor, false);
        let wanted_ammo = current.and_then(|id| {
            self.definition(id)
                .item
                .as_ref()?
                .weapon
                .as_ref()?
                .ammo_definition_id
                .clone()
        });
        let loot = self
            .entities
            .iter()
            .filter_map(|(id, e)| {
                let Entity::Item(i) = e else { return None };
                let ItemLocation::Ground(p) = i.location else {
                    return None;
                };
                let d = self.definition(*id);
                let weapon = d.item.as_ref()?.weapon.as_ref();
                let useful = (current.is_none() && weapon.is_some())
                    || wanted_ammo.as_ref().is_some_and(|kind| *kind == d.id);
                (e.alive() && useful && origin.distance(p) <= range && self.sight(origin, p))
                    .then_some((origin.distance(p), *id, p))
            })
            .min_by_key(|(distance, id, _)| (*distance, *id));
        let Some((distance, item, p)) = loot else {
            return Ok(false);
        };
        if distance == 0 {
            // Capacity failures leave the item where it is and fall back to combat.
            if self.pickup(actor, item).is_err() {
                return Ok(false);
            };
            if let Some(hands) = crate::equipment::equipment_hands(self.definition(item)) {
                let grip = if hands == WeaponHands::One {
                    Grip::Right
                } else {
                    Grip::Both
                };
                if self.equipment_choices(actor, item).contains(&Some(grip)) {
                    self.equip_in_hand(actor, item, grip)?;
                }
            }
            return Ok(true);
        }
        if let Some(next) = self.view().step_towards_position(actor, p, item) {
            self.move_to(actor, next)?;
            return Ok(true);
        }
        Ok(false)
    }
}
