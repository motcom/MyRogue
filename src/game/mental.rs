//! Shared mental damage, psychic strength and terminal ghoul transformation.
use crate::{
    game::Game,
    model::{
        combat,
        config::{Result, ensure},
        entity::{Controller, CreatureKind, Id},
    },
};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MentalRules {
    pub psychic_power_step: i32,
    pub sanity_resistance_step: i32,
    pub ghoul_glyph: String,
}
impl MentalRules {
    pub fn validate(&self) -> Result<()> {
        ensure(
            (0..=100).contains(&self.psychic_power_step)
                && (0..=100).contains(&self.sanity_resistance_step)
                && self.ghoul_glyph.chars().count() == 1,
            "invalid mental rules",
        )
    }
}
impl Game {
    pub fn psychic_power(&self, actor: Id, base: u32) -> Result<u32> {
        ensure(base <= 1_000_000, "psychic power too large")?;
        let a = self.actor(actor).ok_or("psychic source not actor")?;
        ensure(self.entities[&actor].alive(), "psychic source dead")?;
        if base == 0 {
            return Ok(0);
        }
        Ok((i64::from(base)
            + i64::from(combat::modifier(a.seishinryoku))
                * i64::from(self.mental_rules.psychic_power_step))
        .clamp(0, i64::from(u32::MAX)) as u32)
    }
    /// Psychic effects may originate from an actor or an item carried by that actor.
    pub fn psychic_attack(&mut self, source: Id, target: Id, power: u32) -> Result<u32> {
        ensure(self.running, "game ended")?;
        let caster = if self.actor(source).is_some() {
            source
        } else {
            self.entities
                .get(&source)
                .and_then(|e| e.carrier())
                .ok_or("psychic item must be carried")?
        };
        let amount = self.psychic_power(caster, power)?;
        self.damage_sanity(target, amount)
    }
    pub fn damage_sanity(&mut self, target: Id, amount: u32) -> Result<u32> {
        ensure(self.running, "game ended")?;
        ensure(amount <= 2_000_000, "sanity damage too large")?;
        let a = self.actor(target).ok_or("sanity target not actor")?;
        ensure(self.entities[&target].alive(), "sanity target dead")?;
        let resistance = i64::from(combat::modifier(a.seishinryoku))
            * i64::from(self.mental_rules.sanity_resistance_step);
        let effective = if amount == 0 {
            0
        } else {
            (i64::from(amount) - resistance).clamp(0, i64::from(u32::MAX)) as u32
        };
        let glyph = self.mental_rules.ghoul_glyph.clone();
        let a = self.actor_mut(target).unwrap();
        let lost = effective.min(a.sanity);
        a.sanity -= lost;
        if a.sanity == 0 {
            a.ghoul = true;
            a.kind = CreatureKind::Monster;
            a.controller = Controller::Lua;
            a.running_mode = false;
            a.script.glyph = Some(glyph);
        }
        if lost > 0 {
            self.log(format!("正気度に{lost}のダメージ"));
        }
        self.check_sanity_game_over();
        Ok(lost)
    }
    pub(crate) fn check_sanity_game_over(&mut self) {
        if self.game_over || !self.actor(self.player_id).is_some_and(|a| a.sanity == 0) {
            return;
        }
        let glyph = self.mental_rules.ghoul_glyph.clone();
        if let Some(a) = self.actor_mut(self.player_id) {
            a.ghoul = true;
            a.kind = CreatureKind::Monster;
            a.controller = Controller::Lua;
            a.running_mode = false;
            a.script.glyph = Some(glyph);
        }
        self.game_over = true;
        self.running = false;
        self.pending_action = None;
        self.animations.clear();
        self.sounds.clear();
        self.log("正気度が尽きてグールになりました。ゲームオーバー");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn game() -> Game {
        Game::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap()
    }
    #[test]
    fn psychic_strength_resistance_and_carried_sources_share_mental_rules() {
        let mut g = game();
        let p = g.player_id;
        let target = g
            .spawn(
                "monster.ghoul_worker",
                Some(g.position(p).unwrap().offset(1, 0)),
                None,
                1,
            )
            .unwrap();
        g.actor_mut(p).unwrap().seishinryoku = 14;
        g.actor_mut(target).unwrap().seishinryoku = 12;
        assert_eq!(g.psychic_power(p, 10).unwrap(), 13);
        assert_eq!(g.psychic_attack(p, target, 10).unwrap(), 11);
        let item = g.spawn("item.knife", None, Some(p), 1).unwrap();
        assert_eq!(g.psychic_attack(item, target, 10).unwrap(), 11);
        assert_eq!(g.damage_sanity(target, 1).unwrap(), 0);
        g.actor_mut(target).unwrap().seishinryoku = 6;
        assert_eq!(g.damage_sanity(target, 1).unwrap(), 2);
        assert_eq!(g.damage_sanity(target, 0).unwrap(), 0);
        assert_eq!(g.psychic_power(p, 0).unwrap(), 0);
        assert!(g.damage_sanity(target, 2_000_001).is_err());
    }
    #[test]
    fn zero_sanity_transforms_and_stops_the_world_without_respawn_or_growth() {
        let mut g = game();
        let p = g.player_id;
        let turn = g.turn;
        let respawns = g.respawn_count;
        g.actor_mut(p).unwrap().sanity = 1;
        g.damage_sanity(p, 100).unwrap();
        let a = g.actor(p).unwrap();
        assert!(a.ghoul);
        assert_eq!(g.view().faction(p), Some("ghoul"));
        assert!(matches!(a.kind, CreatureKind::Monster));
        assert_eq!(a.controller, Controller::Lua);
        assert!(g.game_over && !g.running);
        g.actor_mut(p).unwrap().hp.hp = 0;
        g.update();
        g.advance_turn();
        g.gain_experience(1000);
        assert_eq!((g.turn, g.respawn_count), (turn, respawns));
        assert_eq!(g.actor(p).unwrap().sanity, 0);
        assert_eq!(g.actor(p).unwrap().progression.level, 1);
        assert!(g.damage_sanity(p, 1).is_err());
    }
}
