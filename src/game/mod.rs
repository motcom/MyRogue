//! Game state, turn progression and shared world operations.

mod actions;
pub(crate) mod containers;
pub mod crafting;
pub mod enemies;
pub mod equipment;
pub mod interactions;
pub(crate) mod item_actions;
pub mod mental;
pub(crate) mod nutrition;
pub mod player;
pub mod progression;
pub mod statistics;
pub mod treatment;
pub mod visibility;
pub mod world;

use crate::{
    game::crafting::Recipe,
    model::combat::Rng,
    model::config::*,
    model::entity::*,
    model::map::Map,
    scripting::{Action, Effect, LuaRuntime, ScriptResult},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::Path,
    rc::Rc,
    sync::Arc,
};
#[derive(Clone, Copy)]
pub enum Panel {
    World,
    ItemTarget {
        item: Id,
        position: Pos,
        from_world: bool,
    },
    Inventory,
    Bandage {
        item: Id,
    },
    Equipment {
        item: Id,
    },
    ItemDetails {
        item: Id,
    },
    Container {
        item: Id,
    },
    InteractionDirection,
    OpenDirection,
    OpenMenu {
        position: Pos,
    },
    InteractionMenu {
        position: Pos,
        carried: Option<Id>,
    },
    Properties,
    Map,
    Crafting,
    Inspect {
        position: Pos,
        selected: usize,
    },
    Targeting {
        active_entity: Id,
    },
}
#[derive(Clone, Debug)]
pub struct Event {
    pub target: Id,
    pub hook: String,
    pub data: Value,
}

#[cfg(test)]
mod respawn_tests {
    use super::*;

    fn load() -> Game {
        Game::load(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn kill(game: &mut Game) {
        game.damage(game.player_id, game.player_id, u32::MAX)
            .unwrap();
        game.events();
    }

    #[test]
    fn death_keeps_inventory_and_world_then_respawns_repeatedly() {
        let mut game = load();
        let id = game.player_id;
        let origin = game.position(id).unwrap();
        let inventory = game.actor(id).unwrap().inventory.clone();
        let count = game.entities.len();
        game.move_to(id, origin.offset(0, 1)).unwrap();
        game.panel = Panel::Inventory;
        for _ in 0..2 {
            kill(&mut game);
            game.update();
            assert!(game.running);
            assert!(game.entities[&id].alive());
            assert_eq!(game.position(id), Some(origin));
            let actor = game.actor(id).unwrap();
            assert_eq!(actor.hp.hp, actor.hp.max_hp);
            assert!(actor.body.values().all(|s| *s == InjuryStage::Healthy));
            assert_eq!(actor.inventory.items, inventory.items);
            assert_eq!(actor.inventory.equipped, inventory.equipped);
            assert_eq!(game.entities.len(), count);
            assert!(!game.destroying.contains(&id));
            assert!(matches!(game.panel, Panel::World));
            assert_eq!(
                game.map
                    .cells
                    .iter()
                    .flat_map(|c| &c.entity_ids)
                    .filter(|i| **i == id)
                    .count(),
                1
            );
            for item in &inventory.items {
                assert_eq!(game.entities[item].carrier(), Some(id));
            }
        }
        game.pending_action = Some(Action::Quit);
        game.update();
        assert!(!game.running);
    }

    #[test]
    fn blocked_spawn_uses_nearby_free_cell_and_restores_needs() {
        let mut game = load();
        let id = game.player_id;
        let origin = game.position(id).unwrap();
        game.move_to(id, origin.offset(0, 1)).unwrap();
        let wall = game
            .spawn("obstacle.stone_wall", Some(origin), None, 1)
            .unwrap();
        let actor = game.actor_mut(id).unwrap();
        actor.hydration.current = 0;
        actor.nutrition = Nutrition::default();
        actor.body.insert("head".into(), InjuryStage::Severed);
        kill(&mut game);
        game.advance_turn();
        let actor = game.actor(id).unwrap();
        assert!(game.running && game.entities[&id].alive());
        assert_ne!(actor.position, origin);
        assert_eq!(actor.position.distance(origin), 1);
        assert!(game.can_enter(actor.position, id));
        assert_eq!(actor.hydration.current, actor.hydration.max);
        assert_eq!(actor.nutrition, Nutrition::initial());
        assert!(game.entities.contains_key(&wall));
    }

    #[test]
    fn lethal_enemy_turn_respawns_and_keeps_game_running() {
        let mut game = load();
        let id = game.player_id;
        let origin = game.position(id).unwrap();
        let enemies: Vec<_> = game
            .entities
            .iter()
            .filter(|(_, e)| matches!(e, Entity::Movable(a) if a.controller == Controller::Lua))
            .map(|(id, _)| *id)
            .collect();
        for enemy in enemies {
            game.remove(enemy);
        }
        game.spawn("monster.ghoul_worker", Some(origin.offset(1, 0)), None, 1)
            .unwrap();
        for _ in 0..100 {
            game.actor_mut(id).unwrap().hp.hp = 1;
            game.advance_turn();
            assert!(game.running && game.entities[&id].alive());
            if game.messages.iter().any(|m| m.contains("リスポーン")) {
                assert_eq!(
                    game.actor(id).unwrap().hp.hp,
                    game.actor(id).unwrap().hp.max_hp
                );
                return;
            }
        }
        panic!("enemy did not deal lethal damage");
    }
}
#[derive(Clone)]
pub struct Game {
    pub(crate) sounds: VecDeque<crate::media::sound::Sound>,
    pub(crate) animation_batch: u64,
    pub(crate) motion_console: Option<usize>,
    pub(crate) animations: VecDeque<crate::ui::animations::Animation>,
    pub player_id: Id,
    pub(crate) respawn_position: Pos,
    pub entities: BTreeMap<Id, Entity>,
    pub map: Map,
    pub map_labels: Vec<MapLabel>,
    pub recipes: Vec<Recipe>,
    pub crafting_rules: crate::game::crafting::CraftRules,
    pub mental_rules: mental::MentalRules,
    pub game_over: bool,
    pub growth_rules: crate::game::progression::GrowthRules,
    pub(crate) growth_rng: Rng,
    pub entity_definitions: Arc<BTreeMap<String, Definition>>,
    pub scripts: Rc<LuaRuntime>,
    pub pending_action: Option<Action>,
    pub(crate) direction_input: crate::ui::direction_input::DirectionInput,
    pub running: bool,
    pub turn: u64,
    pub(crate) respawn_count: u64,
    pub rng: Rng,
    pub(crate) disease_rng: Rng,
    pub(crate) next_id: Id,
    pub events: VecDeque<Event>,
    pub(crate) destroying: BTreeSet<Id>,
    pub messages: VecDeque<String>,
    pub(crate) map_zoom: u8,
    pub(crate) map_console: Option<usize>,
    pub(crate) map_overlay_console: Option<usize>,
    pub(crate) popup_console: Option<usize>,
    pub(crate) texture_renderer: Option<Arc<crate::media::textures::TextureRenderer>>,
    pub(crate) bandage_return_to_world: bool,
    pub(crate) container_selection: BTreeSet<usize>,
    pub(crate) panel: Panel,
    pub(crate) selection: usize,
    pub(crate) footing_scroll: usize,
}
impl Game {
    pub fn load(root: &Path) -> Result<Self> {
        Self::load_scenario(root, "data/scenario.lua")
    }
    pub fn load_test_map(root: &Path) -> Result<Self> {
        Self::load_scenario(root, "data/scenarios/entity_test.lua")
    }
    fn load_scenario(root: &Path, path: &str) -> Result<Self> {
        let runtime = Rc::new(LuaRuntime::load(root)?);
        let scenario = runtime.scenario_file(path)?;
        let crafting = runtime.crafting_config()?;
        ensure(scenario.labels.len() <= 100, "too many map labels")?;
        for label in &scenario.labels {
            let p = label.position;
            ensure(
                p.x >= 0
                    && p.x < scenario.width
                    && p.y >= 0
                    && p.z == 0
                    && p.y < scenario.height
                    && !label.text.is_empty()
                    && label.text.chars().count() <= 70
                    && p.x + label.text.chars().count() as i32 <= scenario.width
                    && !label.text.chars().any(char::is_control),
                "invalid map label",
            )?;
        }
        let mut world = Self {
            sounds: VecDeque::new(),
            animation_batch: 0,
            motion_console: None,
            animations: VecDeque::new(),
            player_id: 0,
            respawn_position: scenario.player.position,
            entities: BTreeMap::new(),
            map: Map::new(scenario.width, scenario.height, 1)?,
            map_labels: scenario.labels,
            recipes: crafting.recipes,
            crafting_rules: crafting.rules,
            growth_rules: runtime.growth_rules()?,
            mental_rules: runtime.mental_rules()?,
            game_over: false,
            growth_rng: Rng::new(scenario.seed ^ 0x4752_4F57),
            entity_definitions: runtime.definitions.clone(),
            scripts: runtime.clone(),
            pending_action: None,
            direction_input: Default::default(),
            running: true,
            turn: 0,
            respawn_count: 0,
            rng: Rng::new(scenario.seed),
            disease_rng: Rng::new(scenario.seed ^ 0x434F_4C44),
            next_id: 1,
            events: VecDeque::new(),
            destroying: BTreeSet::new(),
            messages: VecDeque::new(),
            map_zoom: 1,
            map_console: None,
            map_overlay_console: None,
            popup_console: None,
            texture_renderer: None,
            bandage_return_to_world: false,
            container_selection: BTreeSet::new(),
            panel: Panel::World,
            selection: 0,
            footing_scroll: 0,
        };
        let ground = world
            .entity_definitions
            .get(&scenario.ground)
            .ok_or("ground definition missing")?;
        ensure(
            ground.category == "static" && !ground.common.collision.movement,
            "ground must be walkable static",
        )?;
        ensure(
            world
                .entity_definitions
                .get(&scenario.boundary)
                .is_some_and(|d| d.category == "static" && d.common.collision.movement),
            "boundary must be blocking static",
        )?;
        for y in 0..world.map.height {
            for x in 0..world.map.width {
                let p = Pos { x, y, z: 0 };
                world.spawn(&scenario.ground, Some(p), None, 1)?;
                if x == 0 || y == 0 || x == world.map.width - 1 || y == world.map.height - 1 {
                    world.spawn(&scenario.boundary, Some(p), None, 1)?;
                }
            }
        }
        for spawn in scenario.spawns {
            world.spawn(
                &spawn.definition_id,
                Some(spawn.position),
                None,
                spawn.quantity,
            )?;
        }
        ensure(
            world.can_enter(scenario.player.position, 0),
            "player spawn blocked",
        )?;
        let player = world.spawn(
            &scenario.player.definition_id,
            Some(scenario.player.position),
            None,
            1,
        )?;
        ensure(
            world
                .definition(player)
                .movable
                .as_ref()
                .is_some_and(|m| m.controller == Controller::Input),
            "player must be input-controlled movable",
        )?;
        world.player_id = player;
        let mut game = world;
        let mut recipe_ids = BTreeSet::new();
        for recipe in &game.recipes {
            ensure(recipe_ids.insert(&recipe.id), "duplicate recipe id")?;
            game.validate_recipe(recipe)?;
        }
        game.events();
        Ok(game)
    }
    pub fn hook(&mut self, id: Id, hook: &str, event: serde_json::Value) -> Result<bool> {
        let result = self.scripts.call(self, id, hook, &event)?;
        let changed = result.changed();
        if result.action.is_some() {
            ensure(
                hook == "on_turn"
                    && self
                        .definition(id)
                        .movable
                        .as_ref()
                        .is_some_and(|m| m.controller == Controller::Lua),
                "action only allowed in Lua actor on_turn",
            )?;
        }
        let before = self.clone();
        if let Err(e) = self.apply(id, result) {
            *self = before;
            return Err(e);
        }
        Ok(changed)
    }
    fn apply(&mut self, source: Id, result: ScriptResult) -> Result<()> {
        for effect in result.effects {
            match effect {
                Effect::OpenContainer { target } => {
                    ensure(target == source, "container effect may only open self")?;
                    self.open_container(target)?;
                }
                Effect::SetCollision {
                    target,
                    movement,
                    sight,
                } => {
                    ensure(target == source, "collision effect may only change self")?;
                    let entity = self
                        .entities
                        .get(&target)
                        .ok_or("interaction target missing")?;
                    if movement && let Some(position) = entity.map_position() {
                        ensure(
                            !self.at(position).iter().any(|id| {
                                *id != target
                                    && (matches!(self.entities[id], Entity::Movable(_))
                                        || self.entities[id].blocks_movement())
                            }),
                            "その場所には誰か、または物があります",
                        )?;
                    }
                    match self.entities.get_mut(&target).unwrap() {
                        Entity::Static(s) => {
                            s.blocks_movement = movement;
                            s.blocks_sight = sight;
                        }
                        Entity::Item(i) => {
                            i.collision.movement = movement;
                            i.collision.sight = sight;
                        }
                        Entity::Movable(m) => {
                            m.collision.movement = movement;
                            m.collision.sight = sight;
                        }
                    }
                }
                Effect::SetGlyph { target, glyph } => {
                    ensure(
                        target == source
                            && glyph.len() == 1
                            && glyph.as_bytes()[0].is_ascii_graphic(),
                        "glyph effect requires self and a printable ASCII glyph",
                    )?;
                    self.entities
                        .get_mut(&target)
                        .ok_or("interaction target missing")?
                        .script_mut()
                        .glyph = Some(glyph);
                }
                Effect::Message { message } => self.log(message),

                Effect::Spawn {
                    definition_id,
                    position,
                    quantity,
                } => {
                    ensure(quantity <= 100, "spawn quantity exceeds 100")?;
                    self.spawn(&definition_id, Some(position), None, quantity)?;
                }
                Effect::WeaponAttack {
                    attacker,
                    target,
                    weapon,
                } => {
                    ensure(
                        weapon == source
                            && self
                                .entities
                                .get(&source)
                                .is_some_and(|e| e.carrier() == Some(attacker)),
                        "weapon caller must be held by attacker",
                    )?;
                    self.attack(attacker, target, Some(weapon))?;
                }
                Effect::Damage { target, amount } => self.damage(source, target, amount)?,
                Effect::PracticeSkill { target, skill } => {
                    ensure(
                        self.entities.get(&target).is_some_and(Entity::alive),
                        "practice target must be alive",
                    )?;
                    self.practice_skill(target, skill);
                }
                Effect::SanityDamage { target, amount } => {
                    self.damage_sanity(target, amount)?;
                }
                Effect::PsychicAttack { target, power } => {
                    self.psychic_attack(source, target, power)?;
                }
                Effect::Heal { target, amount } => {
                    ensure(
                        self.entities.get(&target).is_some_and(Entity::alive),
                        "cannot heal dead actor",
                    )?;
                    let a = self.actor_mut(target).ok_or("heal target not actor")?;
                    ensure(a.hp.hp > 0, "cannot heal dead actor")?;
                    a.hp.hp = a.hp.hp.saturating_add(amount).min(a.hp.max_hp);
                }
                Effect::ApplyBandage { target, body_part } => {
                    self.apply_bandage(source, target, &body_part)?
                }
                Effect::RemoveStatus { target, status_id } => {
                    let a = self.actor_mut(target).ok_or("status target not actor")?;
                    a.status.retain(|s| s.name != status_id);
                    if matches!(status_id.as_str(), "cold" | "pneumonia") {
                        a.illness_turns = 0;
                        a.illness_recovery_turns = 0;
                        a.pneumonia_move_turns = 0;
                    }
                }
                Effect::RestoreNeed {
                    target,
                    need,
                    amount,
                } => {
                    self.actor_mut(target)
                        .ok_or("need target not actor")?
                        .restore_need(&need, amount)?;
                }
                Effect::GainNutrition { target, nutrition } => self
                    .actor_mut(target)
                    .ok_or("nutrition target not actor")?
                    .nutrition
                    .add(&nutrition),
                Effect::ConsumeItem { target, quantity } => {
                    ensure(target == source, "hook may only consume its own item")?;
                    self.consume(target, quantity)?;
                }
            }
        }
        if let Some(action) = result.action {
            self.execute(source, action)?;
        }
        if let Some(state) = result.state {
            let e = self
                .entities
                .get_mut(&source)
                .ok_or("state/action target was removed by effects")?;
            e.script_mut().state = state;
        }
        Ok(())
    }
    pub fn execute(&mut self, actor: Id, action: Action) -> Result<()> {
        match action {
            Action::GhoulTurn { behavior } => self.ghoul_turn(actor, behavior),
            Action::WaitTurn => Ok(()),
            Action::MoveTo { position } => self.move_to(actor, position),
            Action::Attack { target } => self.attack(actor, target, None),
            Action::Reload => self.reload(actor).map(|_| ()),
            _ => Err("Lua on_turn may only wait, move, attack or reload".into()),
        }
    }
    pub fn advance_turn(&mut self) {
        self.check_sanity_game_over();
        self.advance_turn_inner();
        self.animation_batch = self.animation_batch.saturating_add(1);
    }
    fn advance_turn_inner(&mut self) {
        if !self.running {
            return;
        }
        if self.respawn_player() {
            return;
        }
        self.turn += 1;
        let targets: Vec<_> = self
            .entities
            .iter()
            .filter(|(_, e)| self.scripts.has_hook(&e.script().definition_id, "on_turn"))
            .map(|(id, _)| *id)
            .collect();
        for id in targets {
            if !self.running {
                return;
            }
            if !self.entities.get(&id).is_some_and(Entity::alive)
                || self.actor(id).is_some_and(|a| self.turn < a.stunned_until)
            {
                continue;
            }
            if self.respond_to_alert(id).unwrap_or(false) {
                continue;
            }
            if let Err(e) = self.hook(id, "on_turn", json!({"turn":self.turn})) {
                self.log(format!("Lua {}: {e}", self.definition(id).common.name));
            }
            self.events();
            if !self
                .entities
                .get(&self.player_id)
                .is_some_and(Entity::alive)
            {
                self.respawn_player();
                return;
            }
        }
        let status_events: Vec<_> = self
            .entities
            .iter()
            .flat_map(|(id, e)| {
                let status = match e {
                    Entity::Movable(e) => &e.status,
                    Entity::Static(e) => &e.status,
                    Entity::Item(e) => &e.status,
                };
                status
                    .iter()
                    .filter(|s| s.enable && s.value > 0)
                    .map(|s| Event {
                        target: *id,
                        hook: s.exe.hook_name.clone(),
                        data: json!({"status_id":s.name,"value":s.value}),
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        if self.running {
            self.events.extend(status_events);
            self.events();
        }
        // 一世界ターンごとに部位の負傷を合算する。UI閲覧では呼ばれない。
        let bleeding: Vec<_> = self
            .entities
            .iter()
            .filter_map(|(id, e)| {
                if !e.alive() {
                    return None;
                }
                self.actor(*id)
                    .map(|a| (*id, a.bleeding_damage()))
                    .filter(|(_, n)| *n > 0)
            })
            .collect();
        for (id, amount) in bleeding {
            if let Err(e) = self.damage(id, id, amount) {
                self.log(e.to_string());
            }
        }
        self.events();
        self.finish_bandages();
        self.update_nutrition_turn();
        if self.turn.is_multiple_of(10) {
            for e in self.entities.values_mut() {
                if let Entity::Movable(a) = e {
                    a.hydration.current = a.hydration.current.saturating_sub(1);
                }
            }
        }
        self.respawn_player();
    }

    /// 死亡したプレイヤーを復帰させる。敵・地形・所持品とターンは維持する。
    pub(crate) fn respawn_player(&mut self) -> bool {
        self.check_sanity_game_over();
        if !self.running
            || self
                .entities
                .get(&self.player_id)
                .is_some_and(Entity::alive)
        {
            return false;
        }
        let id = self.player_id;
        let Some(old_position) = self.position(id) else {
            self.running = false;
            return true;
        };
        let origin = self.respawn_position;
        // 初期位置が塞がっていたら、近い空きマスへ復帰する。
        let position = (0..self.map.height)
            .flat_map(|y| (0..self.map.width).map(move |x| Pos { x, y, z: origin.z }))
            .filter(|p| self.can_enter(*p, id))
            .min_by_key(|p| (p.distance(origin), p.y, p.x))
            .unwrap_or(old_position);
        self.respawn_count += 1;
        let turn = self.turn;
        self.map.remove(old_position, id);
        let actor = self.actor_mut(id).expect("player must be movable");
        actor.position = position;
        actor.next_move_turn = turn;
        actor.stunned_until = 0;
        actor.alert_position = None;
        actor.alert_until = 0;
        actor.hp.hp = actor.hp.max_hp;
        actor.progression.last_damage_source = None;
        *actor.treatment = WoundTreatment::default();
        actor
            .body
            .values_mut()
            .for_each(|stage| *stage = InjuryStage::Healthy);
        actor.nutrition = Nutrition::initial();
        actor.carbohydrate_usage = 0;
        actor.stamina = actor.max_stamina;
        actor.running_mode = false;
        actor.run_step_credit = false;
        actor.stamina_recovery_usage = 0;
        actor.illness_turns = 0;
        actor.illness_recovery_turns = 0;
        actor.pneumonia_move_turns = 0;
        actor.hydration.current = actor.hydration.max;
        for status in &mut actor.status {
            status.enable = false;
            status.value = 0;
        }
        actor.crafting = None;
        self.map.insert(position, id);
        self.events.retain(|event| event.target != id);
        self.destroying.remove(&id);
        self.pending_action = None;
        self.bandage_return_to_world = false;
        self.panel = Panel::World;
        self.selection = 0;
        self.footing_scroll = 0;
        self.log("死亡しました。体力を回復してリスポーンしました");
        true
    }

    pub fn events(&mut self) {
        let mut count = 0;
        while let Some(event) = self.events.pop_front() {
            count += 1;
            if count > 10_000 {
                self.log("Event limit reached; queue cleared");
                self.events.clear();
                break;
            }
            if !self.entities.contains_key(&event.target) {
                continue;
            }
            // プレイヤーは削除・ドロップせず、更新の区切りで復帰させる。
            if event.target == self.player_id && event.hook == "on_destroy" {
                continue;
            }
            if self.scripts.has_hook(
                &self.entities[&event.target].script().definition_id,
                &event.hook,
            ) && let Err(e) = self.hook(event.target, &event.hook, event.data)
            {
                self.log(format!(
                    "Lua {}: {e}",
                    self.definition(event.target).common.name
                ));
            }
            if event.hook == "on_destroy" {
                self.remove(event.target);
            }
        }
    }
    pub fn player_move(&mut self, dx: i32, dy: i32) -> Result<()> {
        let id = self.player_id;
        let p = self.position(id).ok_or("player missing")?;
        let dest = p.offset(dx, dy);
        let target = self
            .at(dest)
            .into_iter()
            .find(|target| matches!(self.entities[target], Entity::Movable(_)));
        let turns = if let Some(target) = target {
            self.attack(id, target, None)?;
            1
        } else {
            let actor = self.actor(id).unwrap();
            let running = actor.running_mode && actor.stamina >= 5 && actor.movement_turns() == 1;
            let turns = if running {
                u64::from(!actor.run_step_credit)
            } else {
                actor.movement_turns()
            };
            self.move_with_duration(id, dest, turns)?;
            if self.position(id) == Some(dest) && running {
                let actor = self.actor_mut(id).unwrap();
                actor.stamina -= 5;
                actor.run_step_credit = !actor.run_step_credit;
            }
            turns
        };
        self.events();
        let respawns = self.respawn_count;
        for _ in 0..turns {
            self.advance_turn();
            if self.respawn_count != respawns || !self.running {
                break;
            }
        }
        if self
            .actor(id)
            .is_some_and(|a| a.running_mode && (a.stamina < 5 || a.movement_turns() != 1))
        {
            self.actor_mut(id).unwrap().running_mode = false;
            self.log("走行を終了して歩行に戻りました");
        }
        Ok(())
    }
    pub fn attack_static(&mut self, target: Id) -> Result<()> {
        self.attack(self.player_id, target, None)?;
        self.events();
        self.advance_turn();
        Ok(())
    }
    pub fn use_item(&mut self, item: Id) -> Result<bool> {
        let player = self.player_id;
        ensure(
            self.entities.get(&player).is_some_and(Entity::alive),
            "player is not alive",
        )?;
        ensure(
            self.entities
                .get(&item)
                .is_some_and(|e| e.carrier() == Some(player) && e.alive()),
            "item not held/usable",
        )?;
        ensure(
            self.view().item_hand_usable(player, item),
            "切断された手のアイテムは使えません",
        )?;
        let definition = self.definition(item);
        if definition.item.as_ref().is_some_and(|i| {
            i.throwable.is_some()
                || i.weapon
                    .as_ref()
                    .is_some_and(|w| w.mode == "melee" && w.range > 1)
        }) {
            self.open_item_target(item)?;
            return Ok(false);
        }
        ensure(
            definition
                .item
                .as_ref()
                .and_then(|i| i.use_effect.as_ref())
                .is_some(),
            "this item has no consumable use",
        )?;
        if self
            .definition(item)
            .item
            .as_ref()
            .and_then(|i| i.use_effect.as_ref())
            .is_some_and(|u| u.kind == "bandage")
        {
            self.open_bandage_selection(item)?;
            return Ok(false);
        }
        let changed = self.hook(item, "on_use", json!({"user":player}))?;
        if changed {
            self.log("Used one item");
            self.events();
            self.advance_turn();
        } else {
            self.log("No effect; item was not consumed");
        }
        Ok(changed)
    }

    pub fn use_ability(&mut self, actor: Id, index: usize, target: Option<Id>) -> Result<bool> {
        let ability = self
            .actor(actor)
            .and_then(|a| a.psykick.get(index))
            .ok_or("ability not found")?
            .clone();
        ensure(ability.shiyou_kaisu > 0, "ability has no uses left")?;
        ensure(
            self.scripts.has_hook(
                &self.entities[&actor].script().definition_id,
                &ability.exe.hook_name,
            ),
            "ability hook not defined",
        )?;
        let changed = self.hook(
            actor,
            &ability.exe.hook_name,
            json!({"user":actor,"ability_id":ability.name,"target":target}),
        )?;
        if changed {
            self.actor_mut(actor)
                .ok_or("ability actor removed")?
                .psykick[index]
                .shiyou_kaisu -= 1;
            self.events();
            self.advance_turn();
        }
        Ok(changed)
    }
}
