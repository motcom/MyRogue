//! Experience, attribute growth and practice shared by all Lua-defined actors.
use crate::{
    game::Game,
    model::config::{Result, ensure},
    model::entity::{EntityId, Skill},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillKind {
    MeleeWeapon,
    Shield,
    Rifle,
    Bow,
    Touteki,
    Picking,
    Hacking,
    WoodCraft,
    MetalCraft,
    DenshiCraft,
    Saihou,
    Cooking,
}
impl SkillKind {
    pub const ALL: [Self; 12] = [
        Self::MeleeWeapon,
        Self::Shield,
        Self::Rifle,
        Self::Bow,
        Self::Touteki,
        Self::Picking,
        Self::Hacking,
        Self::WoodCraft,
        Self::MetalCraft,
        Self::DenshiCraft,
        Self::Saihou,
        Self::Cooking,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::MeleeWeapon => "近接武器",
            Self::Shield => "盾",
            Self::Rifle => "ライフル",
            Self::Bow => "弓",
            Self::Touteki => "投擲",
            Self::Picking => "解錠",
            Self::Hacking => "ハッキング",
            Self::WoodCraft => "木工",
            Self::MetalCraft => "金属加工",
            Self::DenshiCraft => "電子工作",
            Self::Saihou => "裁縫",
            Self::Cooking => "料理",
        }
    }
    pub fn value(self, skill: &Skill) -> u32 {
        match self {
            Self::MeleeWeapon => skill.melee_weapon,
            Self::Shield => skill.shield,
            Self::Rifle => skill.rifle,
            Self::Bow => skill.bow,
            Self::Touteki => skill.touteki,
            Self::Picking => skill.picking,
            Self::Hacking => skill.hacking,
            Self::WoodCraft => skill.wood_craft,
            Self::MetalCraft => skill.metal_craft,
            Self::DenshiCraft => skill.denshi_craft,
            Self::Saihou => skill.saihou,
            Self::Cooking => skill.cooking,
        }
    }
    fn value_mut(self, skill: &mut Skill) -> &mut u32 {
        match self {
            Self::MeleeWeapon => &mut skill.melee_weapon,
            Self::Shield => &mut skill.shield,
            Self::Rifle => &mut skill.rifle,
            Self::Bow => &mut skill.bow,
            Self::Touteki => &mut skill.touteki,
            Self::Picking => &mut skill.picking,
            Self::Hacking => &mut skill.hacking,
            Self::WoodCraft => &mut skill.wood_craft,
            Self::MetalCraft => &mut skill.metal_craft,
            Self::DenshiCraft => &mut skill.denshi_craft,
            Self::Saihou => &mut skill.saihou,
            Self::Cooking => &mut skill.cooking,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrowthRules {
    pub experience_per_level: u32,
    pub practice_base: u32,
    pub practice_per_level: u32,
    pub max_skill: u32,
    pub max_level: u32,
    pub attribute_chance_percent: u32,
    pub hp_min: u32,
    pub hp_max: u32,
    pub stamina_min: u32,
    pub stamina_max: u32,
    pub sanity_min: u32,
    pub sanity_max: u32,
    pub combat_skill_step: u32,
}
impl GrowthRules {
    pub fn validate(&self) -> Result<()> {
        ensure(
            (1..=10000).contains(&self.experience_per_level)
                && (1..=10000).contains(&self.practice_base)
                && self.practice_per_level <= 1000
                && (1..=100).contains(&self.max_skill)
                && (2..=100).contains(&self.max_level)
                && (1..=100).contains(&self.attribute_chance_percent)
                && self.hp_min > 0
                && self.hp_min <= self.hp_max
                && self.hp_max <= 10
                && self.stamina_min > 0
                && self.stamina_min <= self.stamina_max
                && self.stamina_max <= 5
                && self.sanity_min > 0
                && self.sanity_min <= self.sanity_max
                && self.sanity_max <= 10
                && (1..=100).contains(&self.combat_skill_step),
            "invalid growth rules",
        )
    }
    pub fn experience_needed(&self, level: u32) -> u64 {
        u64::from(self.experience_per_level) * u64::from(level.max(1))
    }
    pub fn practice_needed(&self, level: u32) -> u32 {
        self.practice_base + self.practice_per_level * level.min(self.max_skill)
    }
}
#[derive(Clone)]
pub struct Progression {
    pub level: u32,
    pub experience: u64,
    pub practice: BTreeMap<SkillKind, u32>,
    pub last_damage_source: Option<EntityId>,
}
impl Default for Progression {
    fn default() -> Self {
        Self {
            level: 1,
            experience: 0,
            practice: BTreeMap::new(),
            last_damage_source: None,
        }
    }
}
impl Game {
    pub(crate) fn level_progress_label(&self) -> String {
        let Some(a) = self.actor(self.player_id) else {
            return String::new();
        };
        if a.progression.level >= self.growth_rules.max_level {
            return format!("レベル:{}  上限", a.progression.level);
        }
        let needed = self.growth_rules.experience_needed(a.progression.level);
        format!(
            "レベル:{}  経験値:{}/{}  次まで:{}",
            a.progression.level,
            a.progression.experience,
            needed,
            needed.saturating_sub(a.progression.experience)
        )
    }

    pub(crate) fn practice_skill(&mut self, actor: EntityId, skill: SkillKind) {
        // Only the player grows; enemy definitions remain stable.
        if actor != self.player_id
            || !self
                .entities
                .get(&actor)
                .is_some_and(crate::model::entity::Entity::alive)
        {
            return;
        }
        let rules = self.growth_rules.clone();
        let Some(a) = self.actor_mut(actor).filter(|a| a.hp.hp > 0) else {
            return;
        };
        let value = skill.value(&a.skill);
        if value >= rules.max_skill {
            return;
        }
        let progress = a.progression.practice.entry(skill).or_default();
        *progress += 1;
        if *progress >= rules.practice_needed(value) {
            *progress = 0;
            *skill.value_mut(&mut a.skill) += 1;
            self.log(format!("{}が{}に上がりました", skill.label(), value + 1));
        }
    }
    pub(crate) fn gain_experience(&mut self, amount: u32) {
        self.check_sanity_game_over();
        if !self.running || self.game_over {
            return;
        }
        let rules = self.growth_rules.clone();
        let Some(a) = self.actor_mut(self.player_id) else {
            return;
        };
        if a.progression.level >= rules.max_level {
            return;
        }
        a.progression.experience = a.progression.experience.saturating_add(u64::from(amount));
        if amount > 0 {
            self.log(format!("経験値:{amount}を獲得"));
        }
        loop {
            let a = self.actor(self.player_id).unwrap();
            let needed = rules.experience_needed(a.progression.level);
            if a.progression.level >= rules.max_level || a.progression.experience < needed {
                break;
            }
            let mut increases = [false; 4];
            for increase in &mut increases {
                *increase = self.growth_rng.roll(100) <= rules.attribute_chance_percent;
            }
            if !increases.iter().any(|v| *v) {
                increases[(self.growth_rng.roll(4) - 1) as usize] = true;
            }
            let hp_gain = rules.hp_min + self.growth_rng.roll(rules.hp_max - rules.hp_min + 1) - 1;
            let stamina_gain = rules.stamina_min
                + self
                    .growth_rng
                    .roll(rules.stamina_max - rules.stamina_min + 1)
                - 1;
            let sanity_gain = rules.sanity_min
                + self
                    .growth_rng
                    .roll(rules.sanity_max - rules.sanity_min + 1)
                - 1;
            let a = self.actor_mut(self.player_id).unwrap();
            a.max_stamina = a.max_stamina.saturating_add(stamina_gain);
            a.stamina = a.stamina.saturating_add(stamina_gain).min(a.max_stamina);
            a.max_sanity = a.max_sanity.saturating_add(sanity_gain);
            a.sanity = a.sanity.saturating_add(sanity_gain).min(a.max_sanity);
            a.progression.experience -= needed;
            a.progression.level += 1;
            let mut changed = Vec::new();
            for ((name, value), increase) in [
                ("筋力", &mut a.kinryoku),
                ("敏捷", &mut a.subayasa),
                ("器用さ", &mut a.kiyousa),
                ("精神力", &mut a.seishinryoku),
            ]
            .into_iter()
            .zip(increases)
            {
                if increase {
                    *value = value.saturating_add(1);
                    changed.push(format!("{name}+1"));
                }
            }
            a.hp.max_hp = a.hp.max_hp.saturating_add(hp_gain);
            if a.hp.hp > 0 {
                a.hp.hp = a.hp.hp.saturating_add(hp_gain).min(a.hp.max_hp);
            }
            let level = a.progression.level;
            if level == rules.max_level {
                a.progression.experience = 0;
            }
            self.log(format!(
                "レベル:{level}  {}  最大HP+{hp_gain}  スタミナ+{stamina_gain}  正気度+{sanity_gain}",
                changed.join("  ")
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::entity::{Entity, InjuryStage},
        scripting::Action,
    };
    fn game() -> Game {
        let mut g = Game::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let enemies: Vec<_> = g
            .entities
            .iter()
            .filter(|(id, e)| **id != g.player_id && matches!(e, Entity::Movable(_)))
            .map(|(id, _)| *id)
            .collect();
        for id in enemies {
            g.remove(id);
        }
        g
    }
    fn attributes(g: &Game) -> [u32; 4] {
        let a = g.actor(g.player_id).unwrap();
        [a.kinryoku, a.subayasa, a.kiyousa, a.seishinryoku]
    }
    #[test]
    fn resource_growth_preserves_deficits_and_hp_respawn_keeps_new_maxima() {
        let mut g = game();
        let p = g.player_id;
        let a = g.actor_mut(p).unwrap();
        a.stamina = a.max_stamina - 23;
        a.sanity = a.max_sanity - 31;
        let old_stamina = a.max_stamina;
        let old_sanity = a.max_sanity;
        for _ in 0..20 {
            let level = g.actor(p).unwrap().progression.level;
            let before = g.actor(p).unwrap().clone();
            g.gain_experience(100 * level);
            let a = g.actor(p).unwrap();
            assert!((1..=5).contains(&(a.max_stamina - before.max_stamina)));
            assert!((1..=10).contains(&(a.max_sanity - before.max_sanity)));
            assert_eq!(a.max_stamina - a.stamina, 23);
            assert_eq!(a.max_sanity - a.sanity, 31);
        }
        let a = g.actor(p).unwrap();
        let (stamina, sanity, current) = (a.max_stamina, a.max_sanity, a.sanity);
        assert!(stamina > old_stamina && sanity > old_sanity);
        g.damage(p, p, u32::MAX).unwrap();
        g.update();
        let a = g.actor(p).unwrap();
        assert_eq!((a.stamina, a.max_stamina), (stamina, stamina));
        assert_eq!((a.sanity, a.max_sanity), (current, sanity));
        assert!(g.running && !g.game_over);
    }
    #[test]
    fn lethal_damage_awards_once_and_environment_and_self_do_not_award() {
        let mut g = game();
        let p = g.player_id;
        let origin = g.position(p).unwrap();
        let target = g
            .spawn("monster.ghoul_worker", Some(origin.offset(1, 0)), None, 1)
            .unwrap();
        g.damage(p, target, 1).unwrap();
        assert_eq!(g.actor(p).unwrap().progression.experience, 0);
        g.damage(p, target, u32::MAX).unwrap();
        assert_eq!(g.actor(p).unwrap().progression.experience, 25);
        g.damage(p, target, u32::MAX).unwrap();
        assert_eq!(g.actor(p).unwrap().progression.experience, 25);
        g.events();
        assert!(!g.entities.contains_key(&target));
        let target = g
            .spawn("monster.ghoul_worker", Some(origin.offset(1, 0)), None, 1)
            .unwrap();
        g.damage(target, target, u32::MAX).unwrap();
        g.damage(p, target, u32::MAX).unwrap();
        assert_eq!(g.actor(p).unwrap().progression.experience, 25);
        let wall = g
            .spawn("obstacle.chair", Some(origin.offset(0, 1)), None, 1)
            .unwrap();
        g.damage(p, wall, u32::MAX).unwrap();
        assert_eq!(g.actor(p).unwrap().progression.experience, 25);
    }
    #[test]
    fn fatal_critical_wound_awards_and_growth_survives_respawn() {
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
        g.actor_mut(target)
            .unwrap()
            .body
            .insert("head".into(), InjuryStage::Severed);
        g.damage(p, target, 1).unwrap();
        assert_eq!(g.actor(p).unwrap().progression.experience, 25);
        g.gain_experience(100);
        g.practice_skill(p, SkillKind::Bow);
        let stats = attributes(&g);
        let max_hp = g.actor(p).unwrap().hp.max_hp;
        g.damage(p, p, u32::MAX).unwrap();
        g.events();
        g.update();
        let a = g.actor(p).unwrap();
        assert_eq!((a.progression.level, a.progression.experience), (2, 25));
        assert_eq!(a.progression.practice[&SkillKind::Bow], 1);
        assert_eq!(attributes(&g), stats);
        assert_eq!((a.hp.hp, a.hp.max_hp), (max_hp, max_hp));
    }
    #[test]
    fn multiple_levels_keep_remainder_grow_within_bounds_and_use_separate_rng() {
        let mut g = game();
        let mut same = g.clone();
        let stats = attributes(&g);
        let hp = g.actor(g.player_id).unwrap().hp.max_hp;
        g.gain_experience(350);
        same.gain_experience(350);
        let a = g.actor(g.player_id).unwrap();
        assert_eq!((a.progression.level, a.progression.experience), (3, 50));
        let deltas: Vec<_> = attributes(&g)
            .into_iter()
            .zip(stats)
            .map(|(n, old)| n - old)
            .collect();
        assert!(deltas.iter().all(|d| *d <= 2));
        assert!(deltas.iter().sum::<u32>() >= 2);
        assert!((2..=20).contains(&(a.hp.max_hp - hp)));
        assert_eq!(attributes(&g), attributes(&same));
        assert_eq!(a.hp.max_hp, same.actor(same.player_id).unwrap().hp.max_hp);
        assert!(g.level_progress_label().contains("次まで:250"));
        let mut unchanged = game();
        assert_eq!(g.rng.roll(10000), unchanged.rng.roll(10000));
        g.growth_rules.max_level = 4;
        g.gain_experience(u32::MAX);
        assert_eq!(
            (
                g.actor(g.player_id).unwrap().progression.level,
                g.actor(g.player_id).unwrap().progression.experience
            ),
            (4, 0)
        );
        assert!(g.level_progress_label().contains("上限"));
    }
    #[test]
    fn practice_thresholds_increase_and_maximum_does_not_accumulate() {
        let mut g = game();
        let p = g.player_id;
        for _ in 0..9 {
            g.practice_skill(p, SkillKind::MeleeWeapon);
        }
        assert_eq!(g.actor(p).unwrap().skill.melee_weapon, 0);
        assert!(
            g.property_lines()
                .iter()
                .any(|l| l.contains("使用:9/10") && l.contains("次まで:1回"))
        );
        g.practice_skill(p, SkillKind::MeleeWeapon);
        assert_eq!(g.actor(p).unwrap().skill.melee_weapon, 1);
        for _ in 0..11 {
            g.practice_skill(p, SkillKind::MeleeWeapon);
        }
        assert_eq!(g.actor(p).unwrap().skill.melee_weapon, 1);
        g.practice_skill(p, SkillKind::MeleeWeapon);
        assert_eq!(g.actor(p).unwrap().skill.melee_weapon, 2);
        g.actor_mut(p).unwrap().skill.bow = 100;
        g.practice_skill(p, SkillKind::Bow);
        assert!(
            !g.actor(p)
                .unwrap()
                .progression
                .practice
                .contains_key(&SkillKind::Bow)
        );
        assert_eq!(g.turn, 0);
    }
    #[test]
    fn valid_attacks_practice_but_empty_guns_and_scenery_do_not() {
        let mut g = game();
        let p = g.player_id;
        let origin = g.position(p).unwrap();
        let enemy = g
            .spawn("monster.ghoul_worker", Some(origin.offset(1, 0)), None, 1)
            .unwrap();
        let gun = g.spawn("item.glock", None, Some(p), 1).unwrap();
        assert!(g.attack(p, enemy, Some(gun)).is_err());
        assert!(g.actor(p).unwrap().progression.practice.is_empty());
        g.equip_in_hand(p, gun, crate::model::entity::Grip::Right)
            .unwrap();
        g.actor_mut(enemy).unwrap().hp.hp = 10000;
        if let Entity::Item(i) = g.entities.get_mut(&gun).unwrap() {
            i.loaded_ammo = 15;
        }
        g.attack(p, enemy, Some(gun)).unwrap();
        assert_eq!(
            g.actor(p).unwrap().progression.practice[&SkillKind::Rifle],
            1
        );
        let knife = g
            .actor(p)
            .unwrap()
            .inventory
            .items
            .iter()
            .copied()
            .find(|id| g.definition(*id).id == "item.knife")
            .unwrap();
        g.equip_in_hand(p, knife, crate::model::entity::Grip::Right)
            .unwrap();
        g.attack(p, enemy, None).unwrap();
        assert_eq!(
            g.actor(p).unwrap().progression.practice[&SkillKind::MeleeWeapon],
            1
        );
        let wall = g
            .spawn("obstacle.chair", Some(origin.offset(0, 1)), None, 1)
            .unwrap();
        g.attack(p, wall, None).unwrap();
        assert_eq!(
            g.actor(p).unwrap().progression.practice[&SkillKind::MeleeWeapon],
            1
        );
    }
    #[test]
    fn craft_work_and_throw_train_only_after_validation_and_menus_are_free() {
        let mut g = game();
        let p = g.player_id;
        assert!(g.start_craft(p, "craft.bandage").is_err());
        assert!(g.actor(p).unwrap().progression.practice.is_empty());
        g.spawn("item.cloth", None, Some(p), 1).unwrap();
        g.start_craft(p, "craft.bandage").unwrap();
        assert!(g.actor(p).unwrap().progression.practice.is_empty());
        assert!(!g.continue_craft(p).unwrap());
        assert_eq!(
            g.actor(p).unwrap().progression.practice[&SkillKind::Saihou],
            1
        );
        assert!(g.continue_craft(p).unwrap());
        assert_eq!(
            g.actor(p).unwrap().progression.practice[&SkillKind::Saihou],
            2
        );
        assert!(g.continue_craft(p).is_err());
        let item = g.spawn("item.flash_grenade", None, Some(p), 1).unwrap();
        let origin = g.position(p).unwrap();
        assert!(g.throw_item(p, item, origin.offset(40, 0)).is_err());
        assert!(
            !g.actor(p)
                .unwrap()
                .progression
                .practice
                .contains_key(&SkillKind::Touteki)
        );
        g.throw_item(p, item, origin).unwrap();
        assert_eq!(
            g.actor(p).unwrap().progression.practice[&SkillKind::Touteki],
            1
        );
        let before = g.entity_snapshot(p).unwrap();
        for action in [
            Action::OpenProperties,
            Action::ScrollProperties { delta: 1 },
            Action::ClosePanel,
        ] {
            g.pending_action = Some(action);
            g.update();
        }
        assert_eq!(g.turn, 0);
        assert_eq!(g.entity_snapshot(p).unwrap(), before);
    }
    #[test]
    fn bleeding_death_credits_the_last_attacker_and_shields_train_only_when_usable() {
        let mut g = game();
        let p = g.player_id;
        let enemy = g
            .spawn(
                "monster.ghoul_worker",
                Some(g.position(p).unwrap().offset(1, 0)),
                None,
                1,
            )
            .unwrap();
        g.damage(p, enemy, 1).unwrap();
        g.damage(enemy, enemy, u32::MAX).unwrap();
        assert_eq!(g.actor(p).unwrap().progression.experience, 25);
        g.events();
        let enemy = g
            .spawn(
                "monster.ghoul_worker",
                Some(g.position(p).unwrap().offset(1, 0)),
                None,
                1,
            )
            .unwrap();
        let shield = g.spawn("item.shield", None, Some(p), 1).unwrap();
        g.equip_in_hand(p, shield, crate::model::entity::Grip::Left)
            .unwrap();
        let before_dv = g.defense_values(p).unwrap().1;
        g.actor_mut(p).unwrap().skill.shield = 5;
        assert_eq!(g.defense_values(p).unwrap().1, before_dv + 1);
        g.attack(enemy, p, None).unwrap();
        assert_eq!(
            g.actor(p).unwrap().progression.practice[&SkillKind::Shield],
            1
        );
        g.actor_mut(p)
            .unwrap()
            .body
            .insert("left_arm".into(), InjuryStage::Severed);
        g.attack(enemy, p, None).unwrap();
        assert_eq!(
            g.actor(p).unwrap().progression.practice[&SkillKind::Shield],
            1
        );
    }
}
