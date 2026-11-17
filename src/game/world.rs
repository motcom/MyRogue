use crate::{
    game::{Event, Game},
    model::combat::{self},
    model::config::*,
    model::entity::*,
};
use bracket_lib::prelude as bl;
use serde_json::json;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BinaryHeap},
};
impl Game {
    pub fn position(&self, id: Id) -> Option<Pos> {
        self.view().position(id)
    }
    pub fn contains(&self, p: Pos) -> bool {
        self.map.contains(p)
    }
    pub fn at(&self, p: Pos) -> Vec<Id> {
        self.map.entities_at(p)
    }
    pub fn entity_snapshot(&self, id: Id) -> Option<serde_json::Value> {
        self.view().entity_snapshot(id)
    }
    pub fn can_enter(&self, p: Pos, mover: Id) -> bool {
        self.view().can_enter(p, mover)
    }
    pub fn can_step(&self, p: Pos, q: Pos, mover: Id) -> bool {
        self.view().can_step(p, q, mover)
    }
    pub fn movement_cost(&self, p: Pos) -> Option<f32> {
        self.view().movement_cost(p)
    }
    pub fn sight(&self, p: Pos, q: Pos) -> bool {
        self.view().sight(p, q)
    }
    pub fn faction(&self, id: Id) -> Option<&str> {
        let actor = self.actor(id)?;
        if actor.ghoul {
            Some("ghoul")
        } else {
            Some(&self.definition(id).movable.as_ref()?.faction)
        }
    }
    pub fn nearest_hostile(&self, id: Id, range: u32) -> Option<Id> {
        self.view().nearest_hostile(id, range)
    }
    pub fn step_towards(&self, id: Id, target: Id) -> Option<Pos> {
        self.view().step_towards(id, target)
    }
    pub fn selected_weapon(&self, id: Id, target: Id, weapon: Option<Id>) -> Result<Option<Id>> {
        self.view().selected_weapon(id, target, weapon)
    }
    pub fn can_attack(&self, id: Id, target: Id, weapon: Option<Id>) -> bool {
        self.view().can_attack(id, target, weapon)
    }
    pub fn view(&self) -> EntityView<'_> {
        EntityView {
            entity_definitions: &self.entity_definitions,
            entities: &self.entities,
            map: &self.map,
        }
    }
    pub fn read_snapshot(&self) -> WorldSnapshot {
        WorldSnapshot {
            entity_definitions: self.entity_definitions.clone(),
            entities: self.entities.clone(),
            map: self.map.clone(),
        }
    }

    pub fn definition(&self, id: Id) -> &Definition {
        &self.entity_definitions[&self.entities[&id].script().definition_id]
    }
    pub fn actor(&self, id: Id) -> Option<&MoveEntity> {
        match self.entities.get(&id)? {
            Entity::Movable(e) => Some(e),
            _ => None,
        }
    }
    pub fn actor_mut(&mut self, id: Id) -> Option<&mut MoveEntity> {
        match self.entities.get_mut(&id)? {
            Entity::Movable(e) => Some(e),
            _ => None,
        }
    }

    pub fn log(&mut self, s: impl Into<String>) {
        self.messages.push_back(s.into());
        while self.messages.len() > 6 {
            self.messages.pop_front();
        }
    }
    pub fn spawn(
        &mut self,
        definition_id: &str,
        position: Option<Pos>,
        carrier: Option<Id>,
        quantity: u32,
    ) -> Result<Id> {
        let d = self
            .entity_definitions
            .get(definition_id)
            .ok_or_else(|| format!("unknown definition: {definition_id}"))?
            .clone();
        ensure(
            position.is_some() != carrier.is_some(),
            "spawn needs position xor carrier",
        )?;
        if let Some(p) = position {
            ensure(self.contains(p), "spawn outside map")?;
        }
        ensure(quantity > 0, "zero quantity")?;
        if let Some(c) = carrier {
            ensure(
                self.actor(c).is_some() && d.category == "item",
                "only actors can carry items",
            )?;
        }
        if let Some(i) = &d.item {
            ensure(quantity <= i.stack_limit, "quantity exceeds stack limit")?;
        } else {
            ensure(quantity == 1, "only items stack")?;
        }
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).ok_or("entity id overflow")?;
        let script = ScriptBinding {
            glyph: None,
            definition_id: definition_id.into(),
            state: d.initial_state.clone(),
        };
        let entity = match d.category.as_str() {
            "movable" => {
                let m = d.movable.as_ref().unwrap();
                let attr = |name: &str| m.attributes[name];
                let cap = &m.inventory;
                let water = &m.needs["hydration"];
                Entity::Movable(MoveEntity {
                    collision: d.common.collision.clone(),
                    script,
                    controller: m.controller,
                    nutrition: Nutrition::initial(),
                    carbohydrate_usage: 0,
                    stamina: 100,
                    max_stamina: 100,
                    running_mode: false,
                    run_step_credit: false,
                    stamina_recovery_usage: 0,
                    illness_turns: 0,
                    illness_recovery_turns: 0,
                    pneumonia_move_turns: 0,
                    id,
                    kind: m.role,
                    position: position.ok_or("actor must be placed")?,
                    treatment: Box::default(),
                    dominant_hand: m.dominant_hand,
                    next_move_turn: 0,
                    stunned_until: 0,
                    alert_position: None,
                    alert_until: 0,
                    hp: HP {
                        hp: m.hp,
                        max_hp: m.hp,
                    },
                    body: m
                        .body
                        .iter()
                        .map(|name| (name.clone(), InjuryStage::Healthy))
                        .collect(),
                    hydration: Meter {
                        max: water.max,
                        current: water.initial,
                    },
                    sanity: m.sanity,
                    max_sanity: m.sanity,
                    ghoul: m.faction == "ghoul",
                    status: m.status.clone(),
                    kinryoku: attr("kinryoku"),
                    subayasa: attr("subayasa"),
                    kiyousa: attr("kiyousa"),
                    seishinryoku: attr("seishinryoku"),
                    skill: Box::new(m.skills.clone()),
                    progression: Default::default(),
                    psykick: m.psykick.clone(),
                    inventory: Box::new(Inventory {
                        max_longsize: cap.max_length_mm,
                        max_youryou: cap.max_volume_ml,
                        max_jyuuryou: cap.max_mass_g,
                        items: vec![],
                        equipped: None,
                        right_hand: None,
                        left_hand: None,
                        armor: BTreeMap::new(),
                        quick_slots: std::array::from_fn(|_| None),
                    }),
                    crafting: None,
                })
            }
            "static" => {
                let st = d.r#static.as_ref().unwrap();
                Entity::Static(StaticEntity {
                    script,
                    id,
                    role: st.role,
                    name: d.common.name.clone(),
                    blocks_movement: d.common.collision.movement,
                    blocks_sight: d.common.collision.sight,
                    destructible: d.common.durability.max().is_some(),
                    facilities: st.facilities.clone(),
                    resources: st
                        .resources
                        .iter()
                        .map(|(kind, amount)| ResourceAmount {
                            kind: kind.clone(),
                            amount: *amount,
                        })
                        .collect(),
                    position: position.ok_or("static must be placed")?,
                    max_taikyudo: d.common.durability.max().unwrap_or(0),
                    taikyudo: d.common.durability.max().unwrap_or(0),
                    status: vec![],
                })
            }
            "item" => {
                let i = d.item.as_ref().unwrap();
                Entity::Item(ItemEntity {
                    quality: 0,
                    contents: i.contents.clone(),
                    loaded_ammo: 0,
                    collision: d.common.collision.clone(),
                    script,
                    id,
                    kind: definition_id.into(),
                    name: d.common.name.clone(),
                    location: if let Some(p) = position {
                        ItemLocation::Ground(p)
                    } else {
                        ItemLocation::CarriedBy(carrier.unwrap())
                    },
                    quantity,
                    resources: i
                        .resources
                        .iter()
                        .map(|(kind, amount)| ResourceAmount {
                            kind: kind.clone(),
                            amount: *amount,
                        })
                        .collect(),
                    longsize: i.length_mm,
                    youryou: i.volume_ml,
                    jyuuryou: i.mass_g,
                    max_taikyudo: d.common.durability.max().unwrap_or(0),
                    taikyudo: d.common.durability.max().unwrap_or(0),
                    zairyou: i.materials.clone(),
                    nouryoku: i.tool_functions.clone(),
                    status: vec![],
                })
            }
            _ => unreachable!(),
        };
        self.entities.insert(id, entity);
        if let Some(p) = position {
            self.map.insert(p, id);
        }
        if let Some(c) = carrier {
            self.actor_mut(c).unwrap().inventory.items.push(id);
        }
        self.events.push_back(Event {
            target: id,
            hook: "on_spawn".into(),
            data: json!({}),
        });
        if let Some(m) = d.movable {
            for entry in m.loadout {
                let limit = self
                    .entity_definitions
                    .get(&entry.definition_id)
                    .and_then(|d| d.item.as_ref())
                    .ok_or("loadout is not an item")?
                    .stack_limit;
                let mut remaining = entry.quantity;
                while remaining > 0 {
                    let amount = remaining.min(limit);
                    let item = self.spawn(&entry.definition_id, None, Some(id), amount)?;
                    if let Some(grip) = entry.grip {
                        self.equip_in_hand(id, item, grip)?;
                    } else if entry.equipped {
                        if crate::game::equipment::body_armor(self.definition(item)).is_some() {
                            self.toggle_armor(id, item)?;
                        } else {
                            self.equip_auto(id, item)?;
                        }
                    }
                    remaining -= amount;
                }
            }
        }
        Ok(id)
    }

    pub fn move_to(&mut self, id: Id, p: Pos) -> Result<()> {
        let delay = self
            .actor(id)
            .ok_or("only movable entities can move")?
            .movement_turns();
        self.move_with_duration(id, p, delay)
    }

    pub(crate) fn move_with_duration(&mut self, id: Id, p: Pos, delay: u64) -> Result<()> {
        ensure(self.actor(id).is_some(), "only movable entities can move")?;
        let actor = self.actor(id).unwrap();
        if self.turn < actor.next_move_turn {
            return Ok(());
        }
        let old = self.position(id).ok_or("not placed")?;
        ensure(self.can_step(old, p, id), "blocked movement")?;
        self.map.remove(old, id);
        match self.entities.get_mut(&id).unwrap() {
            Entity::Movable(a) => {
                a.position = p;
                a.next_move_turn = self.turn.saturating_add(if delay > 1 { delay } else { 0 });
            }
            _ => return Err("only movable entities can move".into()),
        }
        self.map.insert(p, id);
        if old != p {
            self.pneumonia_movement(id, delay)?;
        }
        if id == self.player_id {
            self.footing_scroll = 0;
        }
        Ok(())
    }

    pub fn attack(&mut self, actor: Id, target: Id, explicit: Option<Id>) -> Result<()> {
        let weapon = self.selected_weapon(actor, target, explicit)?;
        let ad = self.definition(actor).movable.as_ref().unwrap();
        let accuracy_attribute = combat::modifier(self.actor(actor).unwrap().subayasa)
            + combat::modifier(self.actor(actor).unwrap().kiyousa);
        let strength = combat::modifier(self.actor(actor).unwrap().kinryoku);
        let (dice, pv, accuracy, ammo) = if let Some(id) = weapon {
            let w = self
                .definition(id)
                .item
                .as_ref()
                .unwrap()
                .weapon
                .as_ref()
                .unwrap();
            (
                self.weapon_damage(id)?,
                self.view().effective_weapon_pv(actor, id),
                accuracy_attribute + ad.combat.accuracy_bonus + w.accuracy_bonus,
                w.ammo_definition_id
                    .as_ref()
                    .map(|k| (k.clone(), w.ammo_per_attack)),
            )
        } else {
            (
                ad.attack.damage.clone(),
                (ad.attack.base_pv as i32 + strength.min(ad.attack.strength_cap as i32)).max(0),
                accuracy_attribute + ad.combat.accuracy_bonus,
                None,
            )
        };
        let attack_skill = weapon
            .and_then(|id| self.definition(id).item.as_ref()?.weapon.as_ref()?.skill)
            .unwrap_or(crate::game::progression::SkillKind::MeleeWeapon);
        let practice_target = self.actor(target).is_some() && self.entities[&target].alive();
        let skill_bonus = attack_skill.value(&self.actor(actor).unwrap().skill)
            / self.growth_rules.combat_skill_step;
        let accuracy = accuracy + skill_bonus as i32;
        let (av, dv) = self
            .defense_values(target)
            .map(|(av, dv)| (av, self.actor(target).map(|_| dv)))
            .ok_or("target has no defense")?;
        let accuracy = accuracy
            - weapon.map_or(0, |id| {
                self.view().one_hand_accuracy_penalty(actor, id) as i32
            });
        let mut outcome = combat::resolve(&mut self.rng, accuracy, dv, pv, av, &dice);
        if weapon.is_some_and(|id| self.view().off_hand(actor, id)) {
            outcome.damage = (u64::from(outcome.damage) * 3 / 4) as u32;
        }
        // 発射時の位置・視野を保存し、死亡・移動後も元の弾道を再生する。
        let projectile = weapon.and_then(|id| self.projectile(actor, target, id, outcome.hit));
        let melee = weapon.is_none_or(|id| {
            self.definition(id)
                .item
                .as_ref()
                .unwrap()
                .weapon
                .as_ref()
                .unwrap()
                .mode
                == "melee"
        });
        let motion = if melee {
            self.melee_animation(actor, target)
        } else {
            None
        };
        if let Some(id) = weapon
            && self
                .definition(id)
                .item
                .as_ref()
                .and_then(|i| i.weapon.as_ref())
                .is_some_and(|w| w.magazine_capacity.is_some())
        {
            let count = self
                .definition(id)
                .item
                .as_ref()
                .unwrap()
                .weapon
                .as_ref()
                .unwrap()
                .ammo_per_attack;
            if let Some(Entity::Item(item)) = self.entities.get_mut(&id) {
                item.loaded_ammo -= count;
            }
        } else if let Some((kind, mut count)) = ammo {
            let ids = self.actor(actor).unwrap().inventory.items.clone();
            for id in ids {
                if count == 0 {
                    break;
                }
                if let Some(Entity::Item(i)) = self.entities.get(&id)
                    && i.script.definition_id == kind
                {
                    let n = i.quantity.min(count);
                    self.consume(id, n)?;
                    count -= n;
                }
            }
        }
        self.log(format!(
            "{} -> {}: {} x{} damage {}",
            self.definition(actor).common.name,
            self.definition(target).common.name,
            if outcome.hit { "hit" } else { "miss" },
            outcome.penetrations,
            outcome.damage
        ));
        if outcome.damage > 0 {
            if let Some(weapon) = weapon.filter(|_| self.actor(target).is_some()) {
                let threshold = self
                    .definition(weapon)
                    .item
                    .as_ref()
                    .unwrap()
                    .weapon
                    .as_ref()
                    .unwrap()
                    .critical_value;
                let roll = self.rng.roll(6) + self.rng.roll(6);
                if roll >= threshold {
                    let parts: Vec<_> = self
                        .actor(target)
                        .unwrap()
                        .body
                        .iter()
                        .filter(|(_, stage)| **stage != InjuryStage::Severed)
                        .map(|(name, _)| name.clone())
                        .collect();
                    if !parts.is_empty() {
                        let part = &parts[(self.rng.roll(parts.len() as u32) - 1) as usize];
                        let stage = self.actor_mut(target).unwrap().advance_wound(part).unwrap();
                        self.log(format!(
                            "クリティカル: {}の{}が{} ({roll}/{threshold})",
                            self.definition(target).common.name,
                            body_part_name(part),
                            stage.label()
                        ));
                    }
                }
            }
            self.damage(actor, target, outcome.damage)?;
        }
        // GUIなしの長いシミュレーションでも演出を無制限に蓄積しない。
        self.record_attack_exertion(actor);
        if practice_target {
            self.practice_skill(actor, attack_skill);
            if self.has_usable_shield(target) {
                self.practice_skill(target, crate::game::progression::SkillKind::Shield);
            }
        }
        if let Some(projectile) = motion.or(projectile)
            && self.animations.len() < 64
        {
            self.animations.push_back(projectile);
        }
        Ok(())
    }
    pub fn damage(&mut self, source: Id, target: Id, amount: u32) -> Result<()> {
        let source_actor = self
            .entities
            .get(&source)
            .and_then(Entity::carrier)
            .unwrap_or(source);
        let credit = if source_actor == target {
            self.actor(target)
                .and_then(|a| a.progression.last_damage_source)
        } else {
            Some(source_actor)
        };
        let reward = if credit == Some(self.player_id)
            && target != self.player_id
            && !self.destroying.contains(&target)
        {
            self.entities
                .get(&target)
                .and_then(|_| self.definition(target).movable.as_ref())
                .filter(|m| matches!(m.role, CreatureKind::Monster))
                .map_or(0, |m| m.experience_reward)
        } else {
            0
        };
        let entity = self
            .entities
            .get_mut(&target)
            .ok_or("damage target missing")?;
        let hook = "on_damage".to_string();
        match entity {
            Entity::Movable(m) => {
                if source_actor != target && amount > 0 {
                    m.progression.last_damage_source = Some(source_actor);
                }
                m.hp.hp = m.hp.hp.saturating_sub(amount);
            }
            Entity::Static(s) => {
                ensure(s.destructible, "indestructible target")?;
                s.taikyudo = s.taikyudo.saturating_sub(amount);
            }
            _ => return Err("cannot damage item".into()),
        }
        self.events.push_back(Event {
            target,
            hook,
            data: json!({"source":source,"amount":amount}),
        });
        if !self.entities[&target].alive() && self.destroying.insert(target) {
            self.gain_experience(reward);
            self.events.push_back(Event {
                target,
                hook: "on_destroy".into(),
                data: json!({"source":source,"reason":"damage"}),
            });
        }
        Ok(())
    }
    pub fn remove(&mut self, id: Id) {
        let Some(e) = self.entities.remove(&id) else {
            return;
        };
        if let Some(p) = e.map_position() {
            self.map.remove(p, id);
        }
        if let Some(c) = e.carrier()
            && let Some(a) = self.actor_mut(c)
        {
            a.inventory.items.retain(|e| *e != id);
            a.inventory.forget_item(id);
        }
        if let Entity::Movable(a) = e {
            for item in a.inventory.items {
                if let Some(Entity::Item(i)) = self.entities.get_mut(&item) {
                    i.location = ItemLocation::Ground(a.position);
                    self.map.insert(a.position, item);
                }
            }
        }
        self.destroying.remove(&id);
    }
    pub fn consume(&mut self, id: Id, n: u32) -> Result<()> {
        let Some(Entity::Item(i)) = self.entities.get_mut(&id) else {
            return Err("consume target not item".into());
        };
        ensure(n > 0 && i.quantity >= n, "not enough items")?;
        i.quantity -= n;
        if i.quantity == 0 {
            self.remove(id);
        }
        Ok(())
    }
    pub(crate) fn item_mass(&self, id: Id) -> u64 {
        let Entity::Item(item) = &self.entities[&id] else {
            return 0;
        };
        u64::from(item.jyuuryou) * u64::from(item.quantity)
            + item
                .contents
                .iter()
                .map(|entry| {
                    u64::from(
                        self.entity_definitions[&entry.definition_id]
                            .item
                            .as_ref()
                            .unwrap()
                            .mass_g,
                    ) * u64::from(entry.quantity)
                })
                .sum::<u64>()
    }
    pub fn pickup(&mut self, actor: Id, id: Id) -> Result<()> {
        self.pickup_with_reach(actor, id, 0)
    }
    pub(crate) fn pickup_with_reach(&mut self, actor: Id, id: Id, reach: i32) -> Result<()> {
        let p = self.position(actor).ok_or("not placed")?;
        let Some(Entity::Item(item)) = self.entities.get(&id) else {
            return Err("not an item".into());
        };
        let item_position = match item.location {
            ItemLocation::Ground(position) => position,
            _ => return Err("item is not here".into()),
        };
        ensure(
            p.distance(item_position) <= reach && self.sight(p, item_position),
            "item is not here",
        )?;
        ensure(
            item.quantity <= self.pickup_capacity(actor, id)?,
            "inventory capacity exceeded",
        )?;
        self.map.remove(item_position, id);
        if let Some(Entity::Item(i)) = self.entities.get_mut(&id) {
            i.location = ItemLocation::CarriedBy(actor);
        }
        self.actor_mut(actor).unwrap().inventory.items.push(id);
        Ok(())
    }
    /// Maximum quantity fitting the inventory, including container contents weight.
    pub(crate) fn pickup_capacity(&self, actor: Id, id: Id) -> Result<u32> {
        let Some(Entity::Item(item)) = self.entities.get(&id) else {
            return Err("not an item".into());
        };
        let data = self.definition(id).item.as_ref().unwrap();
        let inventory = &self.actor(actor).ok_or("not actor")?.inventory;
        let mass: u64 = inventory.items.iter().map(|id| self.item_mass(*id)).sum();
        let volume: u64 = inventory
            .items
            .iter()
            .filter_map(|id| match &self.entities[id] {
                Entity::Item(i) => Some(u64::from(i.youryou) * u64::from(i.quantity)),
                _ => None,
            })
            .sum();
        if item.quantity == 0
            || data.length_mm > inventory.max_longsize
            || mass > u64::from(inventory.max_jyuuryou)
            || volume > u64::from(inventory.max_youryou)
        {
            return Ok(0);
        }
        let mut count = u64::from(item.quantity);
        let unit_mass = self.item_mass(id) / u64::from(item.quantity);
        if let Some(capacity) = (u64::from(inventory.max_jyuuryou) - mass).checked_div(unit_mass) {
            count = count.min(capacity);
        }
        if let Some(capacity) =
            (u64::from(inventory.max_youryou) - volume).checked_div(u64::from(data.volume_ml))
        {
            count = count.min(capacity);
        }
        Ok(count as u32)
    }
    pub fn drop_item(&mut self, actor: Id, id: Id) -> Result<()> {
        ensure(
            self.entities
                .get(&id)
                .is_some_and(|e| e.carrier() == Some(actor)),
            "item not held",
        )?;
        let p = self.position(actor).ok_or("not placed")?;
        let a = self.actor_mut(actor).ok_or("not actor")?;
        a.inventory.items.retain(|i| *i != id);
        a.inventory.forget_item(id);
        if let Some(Entity::Item(i)) = self.entities.get_mut(&id) {
            i.location = ItemLocation::Ground(p);
        }
        self.map.insert(p, id);
        Ok(())
    }
}

// A Lua callback owns only read data, never Game or its LuaRuntime.
// This avoids an Rc cycle through Lua closures and the running VM.
pub struct WorldSnapshot {
    entity_definitions: std::sync::Arc<BTreeMap<String, Definition>>,
    entities: BTreeMap<Id, Entity>,
    map: crate::model::map::Map,
}
impl WorldSnapshot {
    pub fn view(&self) -> EntityView<'_> {
        EntityView {
            entity_definitions: &self.entity_definitions,
            entities: &self.entities,
            map: &self.map,
        }
    }
}
pub struct EntityView<'a> {
    pub(crate) entity_definitions: &'a BTreeMap<String, Definition>,
    pub(crate) entities: &'a BTreeMap<Id, Entity>,
    map: &'a crate::model::map::Map,
}
// Weighted Dijkstra; the pinned bracket-lib warns against non-uniform costs.
#[derive(Clone, Copy, PartialEq)]
struct PathNode {
    cost: f64,
    index: usize,
}
impl Eq for PathNode {}
impl Ord for PathNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .total_cmp(&self.cost)
            .then_with(|| other.index.cmp(&self.index))
    }
}
impl PartialOrd for PathNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
struct NavigationMap<'a, 'b> {
    view: &'a EntityView<'b>,
    actor: Id,
    target: Id,
    goal: Pos,
}
impl NavigationMap<'_, '_> {
    fn index(&self, p: Pos) -> usize {
        (p.y * self.view.map.width + p.x) as usize
    }
    fn position(&self, idx: usize) -> Pos {
        Pos {
            x: idx as i32 % self.view.map.width,
            y: idx as i32 / self.view.map.width,
            z: self.goal.z,
        }
    }
}
impl bl::BaseMap for NavigationMap<'_, '_> {
    fn get_available_exits(&self, idx: usize) -> bl::SmallVec<[(usize, f32); 10]> {
        let mut exits = bl::SmallVec::new();
        let p = self.position(idx);
        for (dx, dy) in [
            (0, -1),
            (-1, 0),
            (1, 0),
            (0, 1),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
        ] {
            let q = p.offset(dx, dy);
            if !self.view.contains(q) {
                continue;
            }
            // Search may end at the occupied target; actual movement rechecks occupancy.
            let target_ok = q == self.goal
                && self
                    .view
                    .at(q)
                    .iter()
                    .all(|id| *id == self.target || !self.view.entities[id].blocks_movement());
            let walk = target_ok || self.view.can_step(p, q, self.actor);
            if walk {
                exits.push((self.index(q), self.view.movement_cost(q).unwrap_or(1.0)));
            }
        }
        exits
    }
}

impl EntityView<'_> {
    pub fn definition(&self, id: Id) -> &Definition {
        &self.entity_definitions[&self.entities[&id].script().definition_id]
    }
    pub fn actor(&self, id: Id) -> Option<&MoveEntity> {
        match self.entities.get(&id)? {
            Entity::Movable(e) => Some(e),
            _ => None,
        }
    }
    pub fn position(&self, id: Id) -> Option<Pos> {
        self.entities.get(&id)?.map_position()
    }
    pub fn contains(&self, p: Pos) -> bool {
        self.map.contains(p)
    }
    pub fn at(&self, p: Pos) -> Vec<Id> {
        self.map.entities_at(p)
    }
    pub fn entity_snapshot(&self, id: Id) -> Option<serde_json::Value> {
        let e = self.entities.get(&id)?;
        let mut v = json!({"id":id,"category":self.definition(id).category,"config":self.definition(id),"state":e.script().state,"position":e.map_position(),"carrier_id":e.carrier()});
        match e {
            Entity::Movable(m) => {
                v["equipment"] = json!({"right_hand": m.inventory.right_hand, "left_hand": m.inventory.left_hand, "active": self.active_weapon(id, false), "dominant_hand": m.dominant_hand});
                v["quick_slots"] = json!(m.inventory.quick_slots);
                v["armor"] = json!(m.inventory.armor);
                v["bleeding"] = json!(
                    m.body
                        .keys()
                        .map(|part| (part.clone(), m.part_bleeding(part)))
                        .collect::<BTreeMap<_, _>>()
                );
                v["bandages"] = json!(m.treatment.bandages);
                v["stopped_bleeding"] = json!(m.treatment.stopped);
                v["stunned_until"] = json!(m.stunned_until);
                v["alert_position"] = json!(m.alert_position);
                v["alert_until"] = json!(m.alert_until);
                v["movement_turns"] = json!(m.movement_turns());
                v["bleeding_damage"] = json!(m.bleeding_damage());
                v["hp"] = json!(Meter {
                    current: m.hp.hp,
                    max: m.hp.max_hp
                });
                v["sanity"] = json!(Meter {
                    current: m.sanity,
                    max: m.max_sanity
                });
                v["stamina"] = json!(Meter {
                    current: m.stamina,
                    max: m.max_stamina
                });
                v["ghoul"] = json!(m.ghoul);
                v["faction"] = json!(self.faction(id));
                v["level"] = json!(m.progression.level);
                v["experience"] = json!(m.progression.experience);
                v["skills"] = json!(m.skill);
                v["skill_practice"] = json!(m.progression.practice);
                v["body"] = json!(m.body);
                v["needs"] = json!(m.needs());
                v["statuses"] = json!(
                    m.status
                        .iter()
                        .filter(|s| s.enable)
                        .map(|s| (s.name.clone(), s.value))
                        .collect::<BTreeMap<_, _>>()
                );
                v["nutrition"] = json!(m.nutrition);
            }
            Entity::Item(i) => {
                v["quantity"] = json!(i.quantity);
                v["loaded_ammo"] = json!(i.loaded_ammo);
            }
            _ => {}
        }
        if let Entity::Item(item) = e {
            v["quality"] = json!(item.quality);
            v["contents"] = json!(item.contents);
        }
        Some(v)
    }
    pub fn can_enter(&self, p: Pos, mover: Id) -> bool {
        self.map.can_enter(p, mover, self.entities)
    }
    pub fn movement_cost(&self, p: Pos) -> Option<f32> {
        if !self.contains(p) {
            return None;
        }
        Some(
            self.at(p)
                .into_iter()
                .filter_map(|id| {
                    let definition = self.definition(id);
                    definition
                        .r#static
                        .as_ref()
                        .map(|s| ((definition.common.appearance.layer, id), s.movement_cost))
                })
                .max_by_key(|(priority, _)| *priority)
                .map_or(1.0, |(_, cost)| cost),
        )
    }
    pub fn can_step(&self, from: Pos, to: Pos, mover: Id) -> bool {
        from.distance(to) == 1 && self.can_enter(to, mover)
    }

    pub fn sight(&self, a: Pos, b: Pos) -> bool {
        if !self.contains(a) || !self.contains(b) {
            return false;
        }
        let (mut x, mut y) = (a.x, a.y);
        let dx = (b.x - a.x).abs();
        let dy = -(b.y - a.y).abs();
        let sx = if a.x < b.x { 1 } else { -1 };
        let sy = if a.y < b.y { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            if (x, y) != (a.x, a.y)
                && (x, y) != (b.x, b.y)
                && self
                    .at(Pos { x, y, z: 0 })
                    .iter()
                    .any(|id| self.entities[id].blocks_sight())
            {
                return false;
            }
            if (x, y) == (b.x, b.y) {
                return true;
            }
            let e = 2 * err;
            if e >= dy {
                err += dy;
                x += sx;
            }
            if e <= dx {
                err += dx;
                y += sy;
            }
        }
    }
    pub fn faction(&self, id: Id) -> Option<&str> {
        let actor = self.actor(id)?;
        if actor.ghoul {
            Some("ghoul")
        } else {
            Some(&self.definition(id).movable.as_ref()?.faction)
        }
    }
    pub fn nearest_hostile(&self, id: Id, range: u32) -> Option<Id> {
        let a = self.actor(id)?;
        let p = a.position;
        let faction = self.faction(id)?;
        self.entities
            .iter()
            .filter_map(|(target, e)| {
                let Entity::Movable(m) = e else { return None };
                let q = m.position;
                let other = self.faction(*target)?;
                let hostile = (faction == "survivor" && other == "ghoul")
                    || (faction == "ghoul" && other == "survivor");
                let distance = p.distance(q);
                (hostile && e.alive() && distance <= range as i32 && self.sight(p, q))
                    .then_some((distance, *target))
            })
            .min()
            .map(|(_, id)| id)
    }
    pub fn step_towards(&self, actor: Id, target: Id) -> Option<Pos> {
        self.step_towards_position(actor, self.position(target)?, target)
    }
    pub fn step_towards_position(&self, actor: Id, goal: Pos, target: Id) -> Option<Pos> {
        let start = self.position(actor)?;
        if start == goal || !self.contains(start) || !self.contains(goal) || start.z != goal.z {
            return None;
        }
        let map = NavigationMap {
            view: self,
            actor,
            target,
            goal,
        };
        if self
            .at(goal)
            .iter()
            .any(|id| *id != target && self.entities[id].blocks_movement())
        {
            return None;
        }
        let goal_index = map.index(goal);
        let start_index = map.index(start);
        let mut distances = vec![f64::MAX; (self.map.width * self.map.height) as usize];
        distances[goal_index] = 0.0;
        let mut queue = BinaryHeap::from([PathNode {
            cost: 0.0,
            index: goal_index,
        }]);
        while let Some(PathNode { cost, index }) = queue.pop() {
            if cost > distances[index] {
                continue;
            }
            if index == start_index {
                break;
            }
            // Reverse search: entering this cell pays this cell's cost.
            let edge_cost = f64::from(self.movement_cost(map.position(index))?);
            for (previous, _) in bl::BaseMap::get_available_exits(&map, index) {
                let candidate = cost + edge_cost;
                if candidate < distances[previous] {
                    distances[previous] = candidate;
                    queue.push(PathNode {
                        cost: candidate,
                        index: previous,
                    });
                }
            }
        }
        if distances[start_index] == f64::MAX {
            return None;
        }
        let (next_index, _) = bl::BaseMap::get_available_exits(&map, start_index)
            .into_iter()
            .min_by(|(a, a_cost), (b, b_cost)| {
                (distances[*a] + f64::from(*a_cost))
                    .total_cmp(&(distances[*b] + f64::from(*b_cost)))
            })?;
        let next = map.position(next_index);
        (distances[next_index] < distances[start_index] && self.can_step(start, next, actor))
            .then_some(next)
    }

    pub(crate) fn reload_plan(&self, actor: Id) -> Result<(Id, String, u32)> {
        let actor = self.actor(actor).ok_or("attacker is not movable")?;
        ensure(self.entities[&actor.id].alive(), "dead attacker")?;
        let id = self
            .active_weapon(actor.id, true)
            .ok_or("射撃武器を装備してください")?;
        let item = match self.entities.get(&id) {
            Some(Entity::Item(i))
                if i.location == ItemLocation::CarriedBy(actor.id)
                    && i.quantity > 0
                    && (i.max_taikyudo == 0 || i.taikyudo > 0) =>
            {
                i
            }
            _ => return Err("射撃武器を装備してください".into()),
        };
        let weapon = self
            .definition(id)
            .item
            .as_ref()
            .and_then(|i| i.weapon.as_ref())
            .filter(|w| w.mode == "ranged")
            .ok_or("射撃武器を装備してください")?;
        let capacity = weapon
            .magazine_capacity
            .ok_or("この武器は装填操作に未対応です")?;
        ensure(item.loaded_ammo < capacity, "すでに満装填です")?;
        let kind = weapon.ammo_definition_id.as_ref().ok_or("missing ammo")?;
        let amount =
            u64::from(capacity - item.loaded_ammo).min(self.ammo_count(actor.id, kind)) as u32;
        ensure(amount > 0, "対応する弾薬を所持していません")?;
        Ok((id, kind.clone(), amount))
    }

    fn ammo_count(&self, actor: Id, kind: &str) -> u64 {
        self.actor(actor)
            .map(|a| {
                a.inventory
                    .items
                    .iter()
                    .filter_map(|id| match self.entities.get(id) {
                        Some(Entity::Item(i)) if i.script.definition_id == kind => {
                            Some(u64::from(i.quantity))
                        }
                        _ => None,
                    })
                    .sum()
            })
            .unwrap_or(0)
    }
    pub fn selected_weapon(
        &self,
        actor: Id,
        target: Id,
        explicit: Option<Id>,
    ) -> Result<Option<Id>> {
        let a = self.actor(actor).ok_or("attacker is not movable")?;
        ensure(self.entities[&actor].alive(), "dead attacker")?;
        let t = self.entities.get(&target).ok_or("target missing")?;
        ensure(actor != target && t.alive(), "invalid/dead target")?;
        ensure(
            matches!(t, Entity::Movable(_)) || matches!(t,Entity::Static(s) if s.destructible),
            "target cannot be attacked",
        )?;
        let (p, q) = (a.position, t.map_position().ok_or("target not placed")?);
        let distance = p.distance(q);
        let candidate = explicit.or_else(|| self.active_weapon(actor, false));
        if let Some(id) = candidate {
            let valid = self
                .entities
                .get(&id)
                .is_some_and(|e| e.alive() && e.carrier() == Some(actor));
            if valid
                && self.weapon_usable(actor, id)
                && let Some(w) = self
                    .entity_definitions
                    .get(&self.entities[&id].script().definition_id)
                    .and_then(|d| d.item.as_ref())
                    .and_then(|i| i.weapon.as_ref())
            {
                let ammo = if w.magazine_capacity.is_some() {
                    matches!(&self.entities[&id], Entity::Item(i) if i.loaded_ammo >= w.ammo_per_attack)
                } else {
                    w.ammo_definition_id
                        .as_deref()
                        .is_none_or(|k| self.ammo_count(actor, k) >= u64::from(w.ammo_per_attack))
                };
                if distance > 0
                    && distance <= self.effective_range(actor, id) as i32
                    && ammo
                    && self.sight(p, q)
                {
                    return Ok(Some(id));
                }
            }
            if explicit.is_some() {
                return Err("weapon unusable/out of range/ammo".into());
            }
        }
        ensure(distance == 1 && self.sight(p, q), "out of melee range")?;
        Ok(None)
    }
    pub fn can_attack(&self, a: Id, b: Id, weapon: Option<Id>) -> bool {
        self.selected_weapon(a, b, weapon).is_ok()
    }
}
