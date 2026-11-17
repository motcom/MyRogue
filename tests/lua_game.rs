use my_rogue::{
    combat::Rng,
    config::{Nutrition, Pos},
    entity::*,
    game::Game,
    scripting::Action,
};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};
fn load() -> Game {
    Game::load(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap()
}
fn without_enemies(game: &mut Game) {
    let ids: Vec<_> = game
        .entities
        .iter()
        .filter(|(id, e)| **id != game.player_id && matches!(e, Entity::Movable(_)))
        .map(|(id, _)| *id)
        .collect();
    for id in ids {
        game.remove(id);
    }
}
fn inventory_item(game: &Game, kind: &str) -> EntityId {
    *game
        .actor(game.player_id)
        .unwrap()
        .inventory
        .items
        .iter()
        .find(|id| game.entities[id].script().definition_id == kind)
        .unwrap()
}
fn quantity(game: &Game, id: EntityId) -> u32 {
    match game.entities.get(&id) {
        Some(Entity::Item(i)) => i.quantity,
        _ => 0,
    }
}
fn assert_index(game: &Game) {
    for (id, e) in &game.entities {
        let occurrences = game
            .map
            .cells
            .iter()
            .flat_map(|c| &c.entity_ids)
            .filter(|i| *i == id)
            .count();
        if let Some(p) = e.map_position() {
            assert_eq!(occurrences, 1);
            assert!(game.map.entities_at(p).contains(id));
        } else {
            assert_eq!(occurrences, 0);
            let carrier = e.carrier().unwrap();
            assert!(game.actor(carrier).unwrap().inventory.items.contains(id));
        }
    }
    for cell in &game.map.cells {
        for id in &cell.entity_ids {
            assert!(game.entities.contains_key(id));
        }
    }
}
#[test]
fn load_matches_class_model_and_all_terrain_is_entity() {
    let g = load();
    assert_eq!(g.entity_definitions.len(), 77);
    assert_eq!(g.map.depth, 1);
    let a = g.actor(g.player_id).unwrap();
    assert_eq!(a.sanity, 100);
    assert_eq!(a.kinryoku, 8);
    assert_eq!(a.body.len(), 6);
    assert!(a.psykick.is_empty());
    assert!(a.crafting.is_none());
    assert_eq!(a.nutrition, Nutrition::initial());
    assert!(g.map.cells.iter().all(|c| {
        c.entity_ids
            .iter()
            .any(|id| g.entities[id].script().definition_id == "terrain.field")
    }));
    assert_index(&g);
}
#[test]
fn pending_action_advances_once_and_ui_does_not_advance_turn() {
    let mut g = load();
    without_enemies(&mut g);
    let p = g.position(g.player_id).unwrap();
    g.pending_action = Some(Action::MoveTo {
        position: p.offset(1, 0),
    });
    g.update();
    assert_eq!(g.position(g.player_id), Some(p.offset(1, 0)));
    assert_eq!(g.turn, 1);
    assert!(g.pending_action.is_none());
    g.update();
    assert_eq!(g.turn, 1);
    g.pending_action = Some(Action::OpenInventory);
    g.update();
    assert_eq!(g.turn, 1);
    g.pending_action = Some(Action::Quit);
    g.update();
    assert!(!g.running);
    assert_eq!(g.turn, 1);
}
#[test]
fn diagonal_corner_is_allowed_and_pickup_drop_update_index() {
    let mut g = load();
    without_enemies(&mut g);
    let id = g.player_id;
    let p = g.position(id).unwrap();
    let wall = g
        .spawn("obstacle.stone_wall", Some(p.offset(1, 0)), None, 1)
        .unwrap();
    assert!(g.can_step(p, p.offset(1, 1), id));
    g.remove(wall);
    assert!(g.can_step(p, p.offset(1, 1), id));
    let item = g.spawn("item.wood", Some(p), None, 1).unwrap();
    g.pickup(id, item).unwrap();
    assert_eq!(g.entities[&item].carrier(), Some(id));
    assert!(!g.at(p).contains(&item));
    g.drop_item(id, item).unwrap();
    assert!(g.at(p).contains(&item));
    assert_index(&g);
}
#[test]
fn lua_enemy_moves_then_attacks_and_can_attack_does_not_roll_rng() {
    let mut g = load();
    without_enemies(&mut g);
    let p = g.position(g.player_id).unwrap();
    let enemy = g
        .spawn("monster.ghoul_worker", Some(p.offset(2, 0)), None, 1)
        .unwrap();
    g.events();
    g.advance_turn();
    assert_eq!(g.position(enemy), Some(p.offset(1, 0)));
    assert!(g.messages.iter().all(|m| !m.starts_with("Lua ")));
    let mut expected = g.rng.clone();
    let next = expected.roll(20);
    assert!(g.can_attack(enemy, g.player_id, None));
    assert_eq!(g.rng.clone().roll(20), next);
    g.advance_turn();
    assert!(g.messages.iter().any(|m| m.contains("damage")));
    assert!(g.messages.iter().all(|m| !m.starts_with("Lua ")));
}
#[test]
fn food_consumption_gains_one_units_nutrition_and_saturates() {
    let mut g = load();
    without_enemies(&mut g);
    let id = g.player_id;
    let food = inventory_item(&g, "item.bread");
    let before = quantity(&g, food);
    g.actor_mut(id).unwrap().nutrition = Nutrition {
        protein: 98,
        carbohydrate: 0,
        vitamin: 0,
    };
    assert!(g.use_item(food).unwrap());
    assert_eq!(quantity(&g, food), before - 1);
    let a = g.actor(id).unwrap();
    assert_eq!(a.nutrition.protein, 100);
    assert_eq!(a.nutrition.carbohydrate, 45);
    assert_eq!(a.nutrition.vitamin, 2);
    g.actor_mut(id).unwrap().nutrition = Nutrition {
        protein: 100,
        carbohydrate: 100,
        vitamin: 100,
    };
    let turn = g.turn;
    assert!(!g.use_item(food).unwrap());
    assert_eq!(g.turn, turn);
    assert_eq!(quantity(&g, food), before - 1);
}
#[test]
fn shooting_miss_spends_ammo_but_invalid_attack_does_not() {
    let mut g = load();
    without_enemies(&mut g);
    let actor = g.player_id;
    let p = g.position(actor).unwrap();
    let gun = g.spawn("item.glock", None, Some(actor), 1).unwrap();
    let ammo = g.spawn("ammo.pistol", None, Some(actor), 2).unwrap();
    let target = g
        .spawn("monster.ghoul_worker", Some(p.offset(3, 0)), None, 1)
        .unwrap();
    g.actor_mut(target).unwrap().subayasa = 100;
    let seed = (1..1000).find(|s| Rng::new(*s).roll(20) == 1).unwrap();
    g.rng = Rng::new(seed);
    let hp = g.actor(target).unwrap().hp.hp;
    g.equip_auto(actor, gun).unwrap();
    g.reload(actor).unwrap();
    assert!(!g.entities.contains_key(&ammo));
    g.equip_auto(actor, gun).unwrap();
    g.attack(actor, target, Some(gun)).unwrap();
    assert!(matches!(&g.entities[&gun], Entity::Item(i) if i.loaded_ammo == 1));
    assert_eq!(g.actor(target).unwrap().hp.hp, hp);
    assert!(g.messages.back().unwrap().contains("miss"));
    assert!(g.attack(actor, 999999, Some(gun)).is_err());
    assert!(matches!(&g.entities[&gun], Entity::Item(i) if i.loaded_ammo == 1));
}
#[test]
fn destruction_runs_lua_drop_then_removes_entity_and_player_respawns() {
    let mut g = load();
    without_enemies(&mut g);
    let p = g.position(g.player_id).unwrap();
    let tree = g
        .spawn("obstacle.tree", Some(p.offset(1, 0)), None, 1)
        .unwrap();
    g.damage(g.player_id, tree, 999).unwrap();
    g.events();
    assert!(!g.entities.contains_key(&tree));
    let wood: Vec<_> = g
        .at(p.offset(1, 0))
        .into_iter()
        .filter(|id| g.entities[id].script().definition_id == "item.wood")
        .collect();
    assert_eq!(wood.len(), 1);
    assert_eq!(quantity(&g, wood[0]), 2);
    assert_index(&g);
    g.damage(g.player_id, g.player_id, 999).unwrap();
    g.events();
    g.update();
    assert!(g.running);
    assert!(g.entities[&g.player_id].alive());
    assert_eq!(g.position(g.player_id), Some(p));
    assert_index(&g);
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(1);
        let root = std::env::temp_dir().join(format!(
            "my-rogue-lua-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fn copy(src: &Path, dst: &Path) {
            std::fs::create_dir_all(dst).unwrap();
            for e in std::fs::read_dir(src).unwrap() {
                let e = e.unwrap();
                let dest = dst.join(e.file_name());
                if e.path().is_dir() {
                    copy(&e.path(), &dest);
                } else {
                    std::fs::copy(e.path(), dest).unwrap();
                }
            }
        }
        copy(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("data"),
            &root.join("data"),
        );
        Self(root)
    }
    fn change(&self, path: &str, extra: &str) {
        let path = self.0.join(path);
        let original = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            path,
            format!(
                "local definition = (function()\n{original}\nend)()\n{extra}\nreturn definition\n"
            ),
        )
        .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn failed_lua_effect_batch_rolls_back_nutrition_and_item_count() {
    let fixture = Fixture::new();
    fixture.change("data/entity_templates/item/bread.lua",r#"definition.hooks.on_use = function(self,ctx,event)
        return {effects={
            {type="gain_nutrition",target=event.user,nutrition={protein=100,carbohydrate=100,vitamin=100}},
            {type="consume_item",target=self.id,quantity=999}
        }} end"#);
    let mut g = Game::load(&fixture.0).unwrap();
    without_enemies(&mut g);
    let item = inventory_item(&g, "item.bread");
    let amount = quantity(&g, item);
    assert!(g.use_item(item).is_err());
    assert_eq!(quantity(&g, item), amount);
    assert_eq!(
        g.actor(g.player_id).unwrap().nutrition,
        Nutrition::initial()
    );
    assert_eq!(g.turn, 0);
}
#[test]
fn lua_state_is_per_instance_and_instruction_budget_is_enforced() {
    let fixture = Fixture::new();
    fixture.change("data/entity_templates/movable/ghoul_worker.lua",r#"definition.hooks.on_turn=function(self,ctx,event) return {state={ticks=(self.state.ticks or 0)+1},action={type="wait"}} end"#);
    let mut g = Game::load(&fixture.0).unwrap();
    let worker = g
        .entities
        .iter()
        .find(|(_, e)| e.script().definition_id == "monster.ghoul_worker")
        .map(|(id, _)| *id)
        .unwrap();
    let another = g
        .spawn(
            "monster.ghoul_worker",
            Some(Pos { x: 40, y: 20, z: 0 }),
            None,
            1,
        )
        .unwrap();
    g.hook(worker, "on_turn", json!({"turn":1})).unwrap();
    g.hook(worker, "on_turn", json!({"turn":2})).unwrap();
    g.hook(another, "on_turn", json!({"turn":2})).unwrap();
    assert_eq!(g.entities[&worker].script().state["ticks"], 2);
    assert_eq!(g.entities[&another].script().state["ticks"], 1);
    let slow = Fixture::new();
    slow.change(
        "data/entity_templates/movable/ghoul_worker.lua",
        "definition.hooks.on_turn=function() while true do end end",
    );
    let mut g = Game::load(&slow.0).unwrap();
    let worker = g
        .entities
        .iter()
        .find(|(_, e)| e.script().definition_id == "monster.ghoul_worker")
        .map(|(id, _)| *id)
        .unwrap();
    let error = g
        .hook(worker, "on_turn", json!({}))
        .unwrap_err()
        .to_string();
    assert!(error.contains("budget"));
}
#[test]
fn nutrition_rejects_fractional_negative_overflow_and_missing_values() {
    for replacement in ["-1", "0.5", "4294967296", "nil"] {
        let fixture = Fixture::new();
        fixture.change(
            "data/entity_templates/item/bread.lua",
            &format!("definition.item.nutrition.protein={replacement}"),
        );
        assert!(Game::load(&fixture.0).is_err(), "accepted {replacement}");
    }
}
#[test]
fn hp_status_and_ability_lua_functions_are_dispatched() {
    let fixture = Fixture::new();
    fixture.change("data/entity_templates/player.lua",r#"
        definition.movable.status={{name="test_status",enable=true,value=1,exe={hook_name="on_status"}}}
        definition.movable.psykick={{name="test_power",disc="test",shiyou_kaisu=2,exe={hook_name="on_ability"}}}
        definition.hooks.on_damage=function(self,ctx,event) return {state={damaged=true}} end
        definition.hooks.on_status=function(self,ctx,event) return {state={status_seen=event.status_id}} end
        definition.hooks.on_ability=function(self,ctx,event) return {state={ability_seen=event.ability_id}} end
    "#);
    let mut g = Game::load(&fixture.0).unwrap();
    without_enemies(&mut g);
    let id = g.player_id;
    g.damage(id, id, 1).unwrap();
    g.events();
    assert_eq!(g.entities[&id].script().state["damaged"], true);
    g.advance_turn();
    assert_eq!(g.entities[&id].script().state["status_seen"], "test_status");
    // Ability processing is followed by the status hook; count verifies the successful ability use.
    assert!(g.use_ability(id, 0, None).unwrap());
    assert_eq!(g.actor(id).unwrap().psykick[0].shiyou_kaisu, 1);
}

#[test]
fn invalid_effect_after_removing_source_is_rejected_without_losing_item() {
    let fixture = Fixture::new();
    fixture.change(
        "data/entity_templates/item/bread.lua",
        r#"
        definition.hooks.on_use=function(self,ctx,event)
            return {effects={
                {type="consume_item",target=self.id,quantity=self.quantity},
                {type="weapon_attack",attacker=event.user,target=event.user,weapon=self.id}
            }}
        end
    "#,
    );
    let mut game = Game::load(&fixture.0).unwrap();
    let item = inventory_item(&game, "item.bread");
    let count = quantity(&game, item);
    assert!(game.use_item(item).is_err());
    assert_eq!(quantity(&game, item), count);
    assert!(
        game.actor(game.player_id)
            .unwrap()
            .inventory
            .items
            .contains(&item)
    );
    assert_index(&game);
}

#[test]
fn failed_lua_attack_batch_discards_sound_events() {
    let fixture = Fixture::new();
    fixture.change(
        "data/entity_templates/item.lua",
        r#"
        definition.item.weapon.critical_value=2
        definition.hooks.on_use=function(self,ctx,event)
            return {effects={
                {type="weapon_attack",attacker=event.user,target=event.target,weapon=self.id},
                {type="consume_item",target=999999999,quantity=1}
            }}
        end
    "#,
    );
    let mut game = Game::load(&fixture.0).unwrap();
    without_enemies(&mut game);
    let player = game.player_id;
    let knife = game.spawn("item.knife", None, Some(player), 1).unwrap();
    let target = game
        .spawn(
            "monster.ghoul_worker",
            Some(game.position(player).unwrap().offset(1, 0)),
            None,
            1,
        )
        .unwrap();
    let seed = (0..100)
        .find(|seed| {
            let mut probe = game.clone();
            probe.rng = Rng::new(*seed);
            probe.equip_auto(player, knife).unwrap();
            probe.attack(player, target, Some(knife)).unwrap();
            probe
                .actor(target)
                .unwrap()
                .body
                .values()
                .any(|s| *s == InjuryStage::Bleeding)
        })
        .unwrap();
    game.rng = Rng::new(seed);
    game.equip_auto(player, knife).unwrap();
    let mut expected_rng = game.rng.clone();
    let before = game.entity_snapshot(target).unwrap();
    assert!(
        game.hook(knife, "on_use", json!({"user":player,"target":target}))
            .is_err()
    );
    assert_eq!(game.entity_snapshot(target).unwrap(), before);
    assert_eq!(game.take_sounds().count(), 0);
    assert_eq!(game.rng.clone().roll(100000), expected_rng.roll(100000));
    game.equip_auto(player, knife).unwrap();
    game.attack(player, target, Some(knife)).unwrap();
    game.update_frame(0.0);
    assert_eq!(
        game.take_sounds().collect::<Vec<_>>(),
        vec![my_rogue::sound::Sound::Melee]
    );
}

#[test]
fn dice_notation_and_weapon_damage_match_actual_damage() {
    use my_rogue::{combat, config::Dice};
    let mut defaults = load();
    for (weapon, expected) in [
        ("item.glock", "1d6+1 PV6"),
        ("item.sks", "1d8+2 PV7"),
        ("item.crossbow", "1d8 PV5"),
    ] {
        let id = defaults
            .spawn(weapon, None, Some(defaults.player_id), 1)
            .unwrap();
        assert_eq!(defaults.combat_summary(id), expected);
    }
    let ammo = defaults
        .spawn("ammo.pistol", None, Some(defaults.player_id), 1)
        .unwrap();
    assert!(defaults.combat_summary(ammo).is_empty());
    let invalid = Fixture::new();
    invalid.change(
        "data/entity_templates/item/glock.lua",
        "definition.item.weapon.damage=nil",
    );
    assert!(Game::load(&invalid.0).is_err());
    assert_eq!(
        Dice {
            dice: 5,
            sides: 5,
            bonus: 1
        }
        .to_string(),
        "5d5+1"
    );
    assert_eq!(
        Dice {
            dice: 2,
            sides: 6,
            bonus: -2
        }
        .to_string(),
        "2d6-2"
    );
    assert_eq!(
        Dice {
            dice: 1,
            sides: 6,
            bonus: 0
        }
        .to_string(),
        "1d6"
    );
    let fixture = Fixture::new();
    fixture.change(
        "data/entity_templates/item/glock.lua",
        "definition.item.weapon.damage={dice=5,sides=1,bonus=3}",
    );
    let mut g = Game::load(&fixture.0).unwrap();
    without_enemies(&mut g);
    let actor = g.player_id;
    let p = g.position(actor).unwrap();
    let gun = g.spawn("item.glock", None, Some(actor), 1).unwrap();
    if let Entity::Item(i) = g.entities.get_mut(&gun).unwrap() {
        i.loaded_ammo = 1;
    }
    let damage = g.weapon_damage(gun).unwrap();
    assert_eq!(damage.to_string(), "5d1+3");
    assert!(g.combat_summary(gun).contains("5d1+3"));
    let tree = g
        .spawn("obstacle.tree", Some(p.offset(1, 0)), None, 1)
        .unwrap();
    let av = g.definition(tree).r#static.as_ref().unwrap().combat.base_av;
    g.rng = Rng::new(42);
    let expected = combat::resolve(&mut g.rng.clone(), 0, None, 6, av, &damage);
    assert!(expected.penetrations > 0);
    assert_eq!(expected.damage, 8 * expected.penetrations);
    let before = match &g.entities[&tree] {
        Entity::Static(s) => s.taikyudo,
        _ => unreachable!(),
    };
    g.equip_auto(actor, gun).unwrap();
    g.attack(actor, tree, Some(gun)).unwrap();
    assert!(
        matches!(&g.entities[&tree], Entity::Static(s) if s.taikyudo==before.saturating_sub(expected.damage))
    );
    assert!(matches!(&g.entities[&gun], Entity::Item(i) if i.loaded_ammo==0));
}
#[test]
fn armor_stat_display_and_ammunition_tag_validation() {
    let fixture = Fixture::new();
    fixture.change(
        "data/entity_templates/item/hammer.lua",
        "definition.item.armor={av=3,dv=-1}",
    );
    let mut g = Game::load(&fixture.0).unwrap();
    let item = g.spawn("item.hammer", None, Some(g.player_id), 1).unwrap();
    assert_eq!(g.combat_summary(item), "AV3 DV-1");
    assert_eq!(g.combat_summary(g.player_id), "AV1 DV6");
    let invalid = Fixture::new();
    invalid.change(
        "data/entity_templates/item/ammo_pistol.lua",
        "definition.common.tags={}",
    );
    assert!(Game::load(&invalid.0).is_err());
}

#[test]
fn dijkstra_routes_around_walls_without_rng_consumption() {
    let fixture = Fixture::new();
    fixture.change("data/scenario.lua", r#"
        definition.width=9
        definition.height=9
        definition.player.position={x=2,y=4,z=0}
        definition.spawns={{definition_id="monster.ghoul_worker",position={x=6,y=4,z=0}}}
        for y=1,6 do
            table.insert(definition.spawns,{definition_id="obstacle.stone_wall",position={x=4,y=y,z=0}})
        end
    "#);
    let mut g = Game::load(&fixture.0).unwrap();
    let actor = g.player_id;
    let target = *g
        .entities
        .iter()
        .find(|(_, e)| e.script().definition_id == "monster.ghoul_worker")
        .unwrap()
        .0;
    let mut expected_rng = g.rng.clone();
    let mut used_gap = false;
    for _ in 0..30 {
        if g.can_attack(actor, target, None) {
            break;
        }
        let from = g.position(actor).unwrap();
        let next = g
            .step_towards(actor, target)
            .expect("route around the wall");
        assert_eq!(
            Some(next),
            g.read_snapshot().view().step_towards(actor, target)
        );
        assert!(g.can_step(from, next, actor));
        used_gap |= next.x == 4 && !(1..=6).contains(&next.y);
        g.move_to(actor, next).unwrap();
    }
    assert!(used_gap);
    assert!(g.can_attack(actor, target, None));
    assert_eq!(g.rng.roll(1000), expected_rng.roll(1000));
    assert_index(&g);
}

#[test]
fn dijkstra_handles_blocked_and_invalid_goals_and_replans_after_removal() {
    let mut g = load();
    without_enemies(&mut g);
    let actor = g.player_id;
    let p = g.position(actor).unwrap();
    let target = g
        .spawn("monster.ghoul_worker", Some(p.offset(3, 0)), None, 1)
        .unwrap();
    let mut walls = Vec::new();
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx != 0 || dy != 0 {
                walls.push(
                    g.spawn("obstacle.stone_wall", Some(p.offset(dx, dy)), None, 1)
                        .unwrap(),
                );
            }
        }
    }
    assert_eq!(g.step_towards(actor, target), None);
    for wall in walls {
        g.remove(wall);
    }
    let next = g.step_towards(actor, target).unwrap();
    assert!(g.can_step(p, next, actor));
    let blocker = g
        .spawn("obstacle.stone_wall", g.position(target), None, 1)
        .unwrap();
    assert_eq!(g.step_towards(actor, target), None);
    g.remove(blocker);
    assert!(g.step_towards(actor, target).is_some());
    assert_eq!(g.step_towards(actor, actor), None);
    assert_eq!(g.step_towards(actor, 999999), None);
    g.remove(target);
    assert_eq!(g.step_towards(actor, target), None);
}

#[test]
fn diagonal_moves_and_attacks_cross_wall_corners_in_both_directions() {
    let fixture = Fixture::new();
    fixture.change(
        "data/scenario.lua",
        r#"
        definition.width=9
        definition.height=9
        definition.player.position={x=4,y=4,z=0}
        definition.spawns={}
    "#,
    );
    let initial = Game::load(&fixture.0).unwrap();
    for (dx, dy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
        for both_walls in [false, true] {
            let mut g = initial.clone();
            let player = g.player_id;
            let p = g.position(player).unwrap();
            let q = p.offset(dx, dy);
            g.spawn("obstacle.stone_wall", Some(p.offset(dx, 0)), None, 1)
                .unwrap();
            if both_walls {
                g.spawn("obstacle.stone_wall", Some(p.offset(0, dy)), None, 1)
                    .unwrap();
            }
            assert!(g.can_step(p, q, player));
            g.move_to(player, q).unwrap();
            assert!(g.can_step(q, p, player));
            g.move_to(player, p).unwrap();
            let wall = g.spawn("obstacle.stone_wall", Some(q), None, 1).unwrap();
            assert!(!g.can_step(p, q, player));
            assert!(g.move_to(player, q).is_err());
            g.remove(wall);
            let enemy = g.spawn("monster.ghoul_worker", Some(q), None, 1).unwrap();
            assert!(!g.can_step(p, q, player));
            assert!(!g.can_step(q, p, enemy));
            let snapshot = g.read_snapshot();
            for (attacker, target) in [(player, enemy), (enemy, player)] {
                assert!(g.can_attack(attacker, target, None));
                assert!(snapshot.view().can_attack(attacker, target, None));
                g.attack(attacker, target, None).unwrap();
            }
            // The enemy uses the same movement rule after the player vacates p.
            g.move_to(player, p.offset(-dx, -dy)).unwrap();
            assert!(g.can_step(q, p, enemy));
            g.move_to(enemy, p).unwrap();
            assert_index(&g);
        }
    }
}

#[test]
fn lua_enemy_tracks_and_attacks_diagonally_between_two_walls() {
    let fixture = Fixture::new();
    fixture.change(
        "data/scenario.lua",
        r#"
        definition.width=9
        definition.height=9
        definition.player.position={x=4,y=4,z=0}
        definition.spawns={
            {definition_id="monster.ghoul_worker",position={x=2,y=2,z=0}},
            {definition_id="obstacle.stone_wall",position={x=3,y=2,z=0}},
            {definition_id="obstacle.stone_wall",position={x=2,y=3,z=0}},
            {definition_id="obstacle.stone_wall",position={x=4,y=3,z=0}},
            {definition_id="obstacle.stone_wall",position={x=3,y=4,z=0}},
        }
    "#,
    );
    let mut g = Game::load(&fixture.0).unwrap();
    let enemy = *g
        .entities
        .iter()
        .find(|(_, e)| e.script().definition_id == "monster.ghoul_worker")
        .unwrap()
        .0;
    let next = Pos { x: 3, y: 3, z: 0 };
    assert_eq!(g.step_towards(enemy, g.player_id), Some(next));
    assert_eq!(
        g.read_snapshot().view().step_towards(enemy, g.player_id),
        Some(next)
    );
    g.advance_turn();
    assert_eq!(g.position(enemy), Some(next));
    let count = g.messages.len();
    g.advance_turn();
    assert_eq!(g.position(enemy), Some(next));
    assert!(
        g.messages
            .iter()
            .skip(count)
            .any(|s| s.starts_with("グール ワーカー -> 生存者:"))
    );
    assert_index(&g);
}

#[test]
fn static_movement_cost_selects_cheaper_routes_and_lua_can_query_it() {
    for (cost, direct) in [(20.0, false), (0.25, true)] {
        let fixture = Fixture::new();
        fixture.change(
            "data/entity_templates/static/road.lua",
            &format!("definition.static.movement_cost={cost}"),
        );
        fixture.change("data/scenario.lua", r#"
            definition.width=9
            definition.height=7
            definition.player.position={x=2,y=3,z=0}
            definition.spawns={{definition_id="monster.ghoul_worker",position={x=6,y=3,z=0}}}
            for x=3,5 do
                table.insert(definition.spawns,{definition_id="terrain.road",position={x=x,y=3,z=0}})
            end
        "#);
        fixture.change(
            "data/entity_templates/movable/ghoul_worker.lua",
            r#"
            definition.hooks.on_turn=function(self,ctx,event)
                return {state={cost=ctx:movement_cost({x=3,y=3,z=0}),
                    outside=ctx:movement_cost({x=-1,y=3,z=0}) == nil}}
            end
        "#,
        );
        let mut g = Game::load(&fixture.0).unwrap();
        let player = g.player_id;
        let target = *g
            .entities
            .iter()
            .find(|(_, e)| e.script().definition_id == "monster.ghoul_worker")
            .unwrap()
            .0;
        let p = Pos { x: 3, y: 3, z: 0 };
        assert_eq!(g.movement_cost(p), Some(cost));
        assert_eq!(g.movement_cost(Pos { x: -1, ..p }), None);
        let mut expected_rng = g.rng.clone();
        let next = g.step_towards(player, target).unwrap();
        assert_eq!(next == p, direct);
        assert_eq!(
            g.read_snapshot().view().step_towards(player, target),
            Some(next)
        );
        assert_eq!(g.rng.roll(1000), expected_rng.roll(1000));
        g.advance_turn();
        assert_eq!(g.entities[&target].script().state["cost"], json!(cost));
        assert_eq!(g.entities[&target].script().state["outside"], json!(true));
        // Querying or applying a path does not create additional game turns.
        let turn = g.turn;
        g.move_to(player, next).unwrap();
        assert_eq!(g.turn, turn);
        let road = *g
            .at(p)
            .iter()
            .find(|id| g.definition(**id).id == "terrain.road")
            .unwrap();
        g.remove(road);
        assert_eq!(g.movement_cost(p), Some(2.0));
        assert_index(&g);
    }
}

#[test]
fn static_movement_cost_defaults_and_rejects_invalid_lua_values() {
    let fixture = Fixture::new();
    fixture.change(
        "data/entity_templates/static.lua",
        "definition.static.movement_cost=nil",
    );
    let g = Game::load(&fixture.0).unwrap();
    assert_eq!(g.movement_cost(g.position(g.player_id).unwrap()), Some(1.0));
    for value in [
        "0",
        "-1",
        "0.001",
        "1001",
        "0/0",
        "math.huge",
        "'slow'",
        "true",
    ] {
        let fixture = Fixture::new();
        fixture.change(
            "data/entity_templates/static.lua",
            &format!("definition.static.movement_cost={value}"),
        );
        assert!(Game::load(&fixture.0).is_err(), "accepted {value}");
    }
}

#[test]
fn weighted_routes_match_independent_relaxation_in_both_directions() {
    let fixture = Fixture::new();
    fixture.change(
        "data/entity_templates/static/road.lua",
        "definition.static.movement_cost=8",
    );
    fixture.change("data/scenario.lua", r#"
        definition.width=7
        definition.height=7
        definition.player.position={x=1,y=1,z=0}
        definition.spawns={
            {definition_id="monster.ghoul_worker",position={x=5,y=5,z=0}},
            {definition_id="obstacle.stone_wall",position={x=3,y=3,z=0}},
        }
        for y=1,5 do for x=1,5 do
            if (x*3+y)%4==0 then
                table.insert(definition.spawns,{definition_id="terrain.road",position={x=x,y=y,z=0}})
            end
        end end
    "#);
    let initial = Game::load(&fixture.0).unwrap();
    let enemy = *initial
        .entities
        .iter()
        .find(|(_, e)| e.script().definition_id == "monster.ghoul_worker")
        .unwrap()
        .0;
    for (actor, target) in [(initial.player_id, enemy), (enemy, initial.player_id)] {
        let mut g = initial.clone();
        let start = g.position(actor).unwrap();
        let goal = g.position(target).unwrap();
        let index = |p: Pos| (p.y * 7 + p.x) as usize;
        let mut distances = [f32::MAX; 49];
        distances[index(goal)] = 0.0;
        // Independent Bellman-Ford relaxation, including each destination's cost.
        for _ in 0..49 {
            for y in 1..6 {
                for x in 1..6 {
                    let p = Pos { x, y, z: 0 };
                    if p == goal || !g.can_enter(p, actor) {
                        continue;
                    }
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            let q = p.offset(dx, dy);
                            if p.distance(q) != 1 || !(q == goal || g.can_step(p, q, actor)) {
                                continue;
                            }
                            distances[index(p)] = distances[index(p)]
                                .min(distances[index(q)] + g.movement_cost(q).unwrap());
                        }
                    }
                }
            }
        }
        let expected = distances[index(start)];
        let mut total = 0.0;
        let mut steps = 0;
        while g.position(actor).unwrap().distance(goal) > 1 {
            let next = g.step_towards(actor, target).unwrap();
            assert_eq!(
                g.read_snapshot().view().step_towards(actor, target),
                Some(next)
            );
            total += g.movement_cost(next).unwrap();
            g.move_to(actor, next).unwrap();
            steps += 1;
            assert!(steps < 49, "path must not loop");
        }
        total += g.movement_cost(goal).unwrap();
        assert_eq!(total, expected);
    }
}

#[test]
fn lua_interaction_registration_supports_custom_actions_conditions_and_rollback() {
    let fixture = Fixture::new();
    fixture.change("data/entity_templates/item/wood.lua", r#"
        definition.interactions={{id="custom",name="Custom action",when_state={ready=true},consumes_turn=false}}
        definition.initial_state={ready=true}
        definition.hooks.on_interact=function(self,ctx,event)
            return {state={ready=false,action=event.interaction}, effects={{type="message",message="custom ok"}}}
        end
    "#);
    let mut g = Game::load(&fixture.0).unwrap();
    without_enemies(&mut g);
    let item = g.spawn("item.wood", None, Some(g.player_id), 1).unwrap();
    assert_eq!(g.available_interactions(item).len(), 1);
    g.interact(item, "custom").unwrap();
    assert_eq!(g.turn, 0);
    assert!(g.available_interactions(item).is_empty());
    assert_eq!(g.entities[&item].script().state["action"], "custom");
    assert!(g.interact(item, "custom").is_err());
    let broken = Fixture::new();
    broken.change(
        "data/entity_templates/static/wooden_door.lua",
        r#"
        definition.hooks.on_interact=function(self,ctx,event)
            return {state={opened=true},effects={
                {type="set_collision",target=self.id,movement=false,sight=false},
                {type="set_glyph",target=self.id,glyph="invalid"}}}
        end
    "#,
    );
    let mut g = Game::load(&broken.0).unwrap();
    without_enemies(&mut g);
    let p = g.position(g.player_id).unwrap();
    let door = g
        .spawn("obstacle.wooden_door", Some(p.offset(1, 0)), None, 1)
        .unwrap();
    assert!(g.interact(door, "open").is_err());
    assert!(g.entities[&door].blocks_movement());
    assert!(g.entities[&door].blocks_sight());
    assert_eq!(g.entities[&door].script().state["opened"], false);
    assert!(g.entities[&door].script().glyph.is_none());
    assert_eq!(g.turn, 0);
}

#[test]
fn lua_interaction_definitions_reject_missing_hooks_and_duplicate_ids() {
    for change in [
        "definition.hooks.on_interact=nil",
        "definition.interactions[2].id=definition.interactions[1].id",
        "definition.interactions[1].when_state={opened={}}",
    ] {
        let fixture = Fixture::new();
        fixture.change("data/entity_templates/static/wooden_door.lua", change);
        assert!(Game::load(&fixture.0).is_err());
    }
}

#[test]
fn weapon_criticals_follow_damage_rolls_for_both_sides_and_all_weapons() {
    use my_rogue::combat;
    let mut base = load();
    without_enemies(&mut base);
    let player = base.player_id;
    let enemy = base
        .spawn(
            "monster.ghoul_worker",
            Some(base.position(player).unwrap().offset(1, 0)),
            None,
            1,
        )
        .unwrap();
    let mut saw_critical = false;
    let mut saw_damage_without_critical = false;
    let mut saw_miss = false;
    for (kind, threshold) in [
        ("item.knife", 10),
        ("item.club", 12),
        ("item.glock", 11),
        ("item.sks", 11),
        ("item.crossbow", 10),
    ] {
        for (attacker, target) in [(player, enemy), (enemy, player)] {
            let mut initial = base.clone();
            let weapon = initial.spawn(kind, None, Some(attacker), 1).unwrap();
            if let Entity::Item(i) = initial.entities.get_mut(&weapon).unwrap() {
                i.loaded_ammo = 1;
            }
            initial.equip_auto(attacker, weapon).unwrap();
            initial.actor_mut(target).unwrap().hp = HP {
                hp: 10000,
                max_hp: 10000,
            };
            let w = initial
                .definition(weapon)
                .item
                .as_ref()
                .unwrap()
                .weapon
                .as_ref()
                .unwrap();
            assert_eq!(w.critical_value, threshold);
            let accuracy = combat::modifier(initial.actor(attacker).unwrap().subayasa)
                + combat::modifier(initial.actor(attacker).unwrap().kiyousa)
                + initial
                    .definition(attacker)
                    .movable
                    .as_ref()
                    .unwrap()
                    .combat
                    .accuracy_bonus
                + w.accuracy_bonus;
            let pv = initial.view().effective_weapon_pv(attacker, weapon);
            let (av, dv) = initial.defense_values(target).unwrap();
            let dice = w.damage.clone();
            for seed in 0..128 {
                let mut game = initial.clone();
                game.rng = Rng::new(seed);
                let mut expected_rng = game.rng.clone();
                let outcome = combat::resolve(&mut expected_rng, accuracy, Some(dv), pv, av, &dice);
                let mut expected_body = game.actor(target).unwrap().body.clone();
                if outcome.damage > 0 {
                    let roll = expected_rng.roll(6) + expected_rng.roll(6);
                    if roll >= threshold {
                        let part = expected_body
                            .keys()
                            .nth((expected_rng.roll(6) - 1) as usize)
                            .unwrap()
                            .clone();
                        expected_body.insert(part, InjuryStage::Bleeding);
                        saw_critical = true;
                    } else {
                        saw_damage_without_critical = true;
                    }
                } else if !outcome.hit {
                    saw_miss = true;
                }
                game.equip_auto(attacker, weapon).unwrap();
                game.attack(attacker, target, Some(weapon)).unwrap();
                assert_eq!(game.actor(target).unwrap().hp.hp, 10000 - outcome.damage);
                assert_eq!(
                    game.actor(target).unwrap().body,
                    expected_body,
                    "{kind}, seed {seed}"
                );
                assert_eq!(game.rng.roll(100000), expected_rng.roll(100000));
            }
        }
    }
    assert!(saw_critical && saw_damage_without_critical && saw_miss);
}

#[test]
fn repeated_criticals_advance_one_stage_and_head_or_torso_severing_is_fatal() {
    let fixture = Fixture::new();
    fixture.change(
        "data/entity_templates/item.lua",
        "definition.item.weapon.critical_value=2",
    );
    let mut base = Game::load(&fixture.0).unwrap();
    without_enemies(&mut base);
    let player = base.player_id;
    let weapon = inventory_item(&base, "item.knife");
    let target = base
        .spawn(
            "monster.ghoul_worker",
            Some(base.position(player).unwrap().offset(1, 0)),
            None,
            1,
        )
        .unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for seed in 0..200 {
        let mut game = base.clone();
        game.actor_mut(target).unwrap().hp = HP {
            hp: 10000,
            max_hp: 10000,
        };
        game.rng = Rng::new(seed);
        game.equip_auto(player, weapon).unwrap();
        game.attack(player, target, Some(weapon)).unwrap();
        let Some(part) = game
            .actor(target)
            .unwrap()
            .body
            .iter()
            .find(|(_, s)| **s == InjuryStage::Bleeding)
            .map(|(p, _)| p.clone())
        else {
            continue;
        };
        seen.insert(part.clone());
        for stage in [InjuryStage::HeavyBleeding, InjuryStage::Severed] {
            game.rng = Rng::new(seed);
            game.equip_auto(player, weapon).unwrap();
            game.attack(player, target, Some(weapon)).unwrap();
            assert_eq!(game.actor(target).unwrap().body[&part], stage);
        }
        assert!(game.actor(target).unwrap().hp.hp > 0);
        let fatal = matches!(part.as_str(), "head" | "body");
        assert_eq!(!game.entities[&target].alive(), fatal);
        assert_eq!(
            game.events
                .iter()
                .filter(|e| e.target == target && e.hook == "on_destroy")
                .count(),
            usize::from(fatal)
        );
        if !fatal {
            let mut expected_rng = game.rng.clone();
            game.damage(player, target, 1).unwrap();
            assert_eq!(
                game.actor(target).unwrap().body[&part],
                InjuryStage::Severed
            );
            assert_eq!(game.rng.roll(100), expected_rng.roll(100));
        }
        if seen.len() == 6 {
            break;
        }
    }
    assert_eq!(seen.len(), 6);
}

#[test]
fn critical_values_are_required_and_restricted_to_two_d6() {
    for value in ["nil", "1", "13", "10.5", "'10'"] {
        let fixture = Fixture::new();
        fixture.change(
            "data/entity_templates/item/glock.lua",
            &format!("definition.item.weapon.critical_value={value}"),
        );
        assert!(Game::load(&fixture.0).is_err(), "{value}");
    }
    for change in [
        "definition.movable.hp=0",
        "definition.movable.body={'head','body'}",
        "definition.movable.body={'head','body','left_arm','right_arm','left_leg','left_leg'}",
    ] {
        let fixture = Fixture::new();
        fixture.change("data/entity_templates/player.lua", change);
        assert!(Game::load(&fixture.0).is_err());
    }
}

#[test]
fn lua_healing_and_snapshots_use_whole_hp_and_preserve_injury_stages() {
    let mut game = load();
    without_enemies(&mut game);
    let player = game.player_id;
    let syringe = game.spawn("item.syringe", None, Some(player), 2).unwrap();
    let actor = game.actor_mut(player).unwrap();
    actor.hp.hp = 10;
    actor
        .body
        .insert("left_arm".into(), InjuryStage::HeavyBleeding);
    let body = actor.body.clone();
    let snapshot = game.entity_snapshot(player).unwrap();
    assert_eq!(snapshot["hp"], json!({"current":10,"max":60}));
    assert_eq!(snapshot["body"]["left_arm"], "heavy_bleeding");
    assert!(snapshot["body"]["body"].is_string());
    assert!(game.use_item(syringe).unwrap());
    assert_eq!(game.actor(player).unwrap().hp.hp, 24);
    assert_eq!(game.actor(player).unwrap().body, body);
    assert_eq!(quantity(&game, syringe), 1);
    game.actor_mut(player).unwrap().hp.hp = 59;
    assert!(game.use_item(syringe).unwrap());
    assert_eq!(game.actor(player).unwrap().hp.hp, 59);
    assert_eq!(game.actor(player).unwrap().body, body);
    for kind in ["damage", "heal"] {
        assert!(
            serde_json::from_value::<my_rogue::scripting::Effect>(
                json!({"type":kind,"target":player,"amount":1,"body_part":"left_arm"})
            )
            .is_err()
        );
    }
}

#[test]
fn lua_hands_and_grip_fields_validate_and_initial_equipment_uses_both_slots() {
    for change in [
        "definition.item.weapon.hands=nil",
        "definition.item.weapon.hands='three'",
        "definition.item.weapon.one_hand_range_penalty=6",
        "definition.item.weapon.one_hand_accuracy_penalty=-1",
        "definition.item.weapon.two_hand_pv_bonus=1",
        "definition.item.weapon.one_hand_range_penalty=1.5",
    ] {
        let fixture = Fixture::new();
        fixture.change("data/entity_templates/item/glock.lua", change);
        assert!(Game::load(&fixture.0).is_err(), "{change}");
    }
    for change in [
        "definition.movable.dominant_hand='both'",
        "definition.movable.loadout[1].grip='both'",
        "definition.movable.loadout[2].grip='right'",
    ] {
        let fixture = Fixture::new();
        fixture.change("data/entity_templates/player.lua", change);
        assert!(Game::load(&fixture.0).is_err(), "{change}");
    }
    let fixture = Fixture::new();
    fixture.change("data/entity_templates/player.lua", "definition.movable.dominant_hand='left'; definition.movable.loadout[1].grip='right'; definition.movable.loadout[2].grip='left'; definition.movable.loadout[2].quantity=1; definition.movable.loadout[2].definition_id='item.glock'");
    let game = Game::load(&fixture.0).unwrap();
    let a = game.actor(game.player_id).unwrap();
    assert_eq!(a.dominant_hand, Hand::Left);
    assert!(a.inventory.right_hand.is_some() && a.inventory.left_hand.is_some());
    assert_ne!(a.inventory.right_hand, a.inventory.left_hand);
    assert_eq!(
        game.entity_snapshot(a.id).unwrap()["equipment"]["dominant_hand"],
        "left"
    );
}

#[test]
fn failed_bandage_lua_batch_rolls_back_treatment_count_and_time() {
    let fixture = Fixture::new();
    fixture.change(
        "data/entity_templates/item/bandage.lua",
        r#"
        definition.hooks.on_use=function(self,ctx,event)
            return {effects={
                {type="apply_bandage",target=event.user,body_part=event.body_part},
                {type="consume_item",target=999999999,quantity=1}}}
        end
    "#,
    );
    let mut g = Game::load(&fixture.0).unwrap();
    without_enemies(&mut g);
    let p = g.player_id;
    g.actor_mut(p)
        .unwrap()
        .body
        .insert("left_arm".into(), InjuryStage::Bleeding);
    let b = inventory_item(&g, "item.bandage");
    let count = quantity(&g, b);
    let before = g.entity_snapshot(p).unwrap();
    let mut rng = g.rng.clone();
    assert!(g.use_bandage_on(b, "left_arm").is_err());
    assert_eq!(g.entity_snapshot(p).unwrap(), before);
    assert_eq!(quantity(&g, b), count);
    assert_eq!(g.turn, 0);
    assert_eq!(g.rng.roll(1000), rng.roll(1000));
}

#[test]
fn armor_slots_validate_and_initial_loadouts_can_wear_multiple_slots() {
    for change in [
        "definition.item.armor.slot='tail'",
        "definition.item.armor.slot=1",
    ] {
        let fixture = Fixture::new();
        fixture.change("data/entity_templates/item/leather_armor.lua", change);
        assert!(Game::load(&fixture.0).is_err());
    }
    for change in [
        "definition.movable.loadout={{definition_id='item.leather_armor',quantity=1,grip='right'}}",
        "definition.movable.loadout={{definition_id='item.leather_armor',quantity=1,equipped=true},{definition_id='item.leather_armor',quantity=1,equipped=true}}",
        "definition.movable.loadout={{definition_id='item.leather_armor',quantity=0}}",
    ] {
        let fixture = Fixture::new();
        fixture.change("data/entity_templates/player.lua", change);
        assert!(Game::load(&fixture.0).is_err());
    }
    let fixture = Fixture::new();
    fixture.change(
        "data/entity_templates/item/hammer.lua",
        "definition.item.armor={slot='head',av=2,dv=0}",
    );
    fixture.change(
        "data/entity_templates/item/wood.lua",
        "definition.item.armor={slot='feet',av=1,dv=-1}",
    );
    fixture.change(
        "data/entity_templates/player.lua",
        r#"
        definition.movable.loadout={
            {definition_id='item.knife',quantity=1,equipped=true},
            {definition_id='item.leather_armor',quantity=1,equipped=true},
            {definition_id='item.hammer',quantity=1,equipped=true},
            {definition_id='item.wood',quantity=1,equipped=true}}
    "#,
    );
    let g = Game::load(&fixture.0).unwrap();
    let p = g.player_id;
    assert_eq!(g.actor(p).unwrap().inventory.armor.len(), 3);
    assert_eq!(g.defense_values(p), Some((5, 5)));
    assert!(
        g.actor(p)
            .unwrap()
            .inventory
            .quick_slots
            .iter()
            .all(Option::is_none)
    );
}

#[test]
fn lua_container_contents_validate_references_quantities_and_stacking() {
    for change in [
        "definition.item.contents[1].definition_id='missing.item'",
        "definition.item.contents[1].definition_id='terrain.field'",
        "definition.item.contents[1].definition_id='item.treasure_chest'",
        "definition.item.contents[1].quantity=0",
        "definition.item.contents[1].quantity=1001",
        "definition.item.contents[1].quantity=1.5",
        "definition.item.stack_limit=2",
    ] {
        let fixture = Fixture::new();
        fixture.change("data/entity_templates/item/treasure_chest.lua", change);
        assert!(Game::load(&fixture.0).is_err(), "{change}");
    }
}

#[test]
fn lua_controls_production_recipes_rules_and_function_levels() {
    let fixture = Fixture::new();
    fixture.change(
        "data/production/recipes.lua",
        r#"
        definition.rules.success_base=75
        definition.rules.skill_step=4
        definition.rules.failure_material_percent=25
        definition.recipes[1].work_turns=2
        definition.recipes[1].ingredients[1].quantity=2
        definition.recipes[1].required_tools[1].level=2
        definition.recipes[1].required_skills[1].recommended_level=1
    "#,
    );
    fixture.change(
        "data/entity_templates/item.lua",
        "definition.item.tool_levels={cutting=2}",
    );
    let mut g = Game::load(&fixture.0).unwrap();
    g.move_to(g.player_id, Pos { x: 7, y: 10, z: 0 }).unwrap();
    g.spawn("item.wood", None, Some(g.player_id), 2).unwrap();
    let recipe = &g.recipes[0];
    assert_eq!(recipe.work_turns, 2);
    assert_eq!(recipe.ingredients[0].quantity, 2);
    assert_eq!(g.crafting_rules.failure_material_percent, 25);
    let plan = g.craft_plan(g.player_id, recipe).unwrap();
    assert_eq!(plan.success_rate, 71);
}
#[test]
fn invalid_lua_production_data_is_rejected_at_load() {
    for change in [
        "definition.recipes[1].ingredients={}",
        "definition.recipes[1].required_tools={}",
        "definition.recipes[1].required_tools[1].level=0",
        "definition.recipes[1].outputs[1].kind='item.missing'",
        "definition.recipes[1].required_skills[1].skill='unknown'",
        "definition.rules.quality_thresholds={30,10,75,90,100}",
        "definition.rules.failure_material_percent=101",
        "definition.recipes[2].id=definition.recipes[1].id",
    ] {
        let fixture = Fixture::new();
        fixture.change("data/production/recipes.lua", change);
        assert!(Game::load(&fixture.0).is_err(), "{change}");
    }
    for change in [
        "definition.item.tool_levels={cutting=0}",
        "definition.item.tool_levels={cutting=21}",
        "definition.item.tool_levels={drilling=2}",
    ] {
        let fixture = Fixture::new();
        fixture.change("data/entity_templates/item.lua", change);
        assert!(Game::load(&fixture.0).is_err(), "{change}");
    }
}

#[test]
fn growth_rules_reject_invalid_bounds_and_unknown_fields() {
    for change in [
        "definition.experience_per_level=0",
        "definition.practice_base=0",
        "definition.max_level=1",
        "definition.max_skill=101",
        "definition.hp_max=11",
        "definition.hp_min=0",
        "definition.attribute_chance_percent=101",
        "definition.combat_skill_step=0",
        "definition.unknown=1",
    ] {
        let fixture = Fixture::new();
        fixture.change("data/progression.lua", change);
        assert!(Game::load(&fixture.0).is_err(), "{change}");
    }
}

#[test]
fn lua_can_practice_a_skill_and_failed_effect_batches_roll_it_back() {
    use my_rogue::progression::SkillKind;
    for failing in [false, true] {
        let fixture = Fixture::new();
        fixture.change("data/entity_templates/item.lua", &format!(r#"
            definition.hooks.on_use=function(self,ctx,event)
                return {{effects={{{{type="practice_skill",target=event.user,skill="picking"}} {}}}}}
            end
        "#, if failing { ",{type=\"damage\",target=999999,amount=1}" } else { "" }));
        let mut g = Game::load(&fixture.0).unwrap();
        let p = g.player_id;
        let knife = inventory_item(&g, "item.knife");
        let result = g.hook(knife, "on_use", json!({"user":p}));
        assert_eq!(result.is_err(), failing);
        let practice = &g.actor(p).unwrap().progression.practice;
        assert_eq!(
            practice.get(&SkillKind::Picking).copied().unwrap_or(0),
            u32::from(!failing)
        );
    }
}

#[test]
fn psychic_lua_effects_apply_and_failed_terminal_batch_restores_the_world() {
    let fixture = Fixture::new();
    fixture.change(
        "data/entity_templates/item/bread.lua",
        r#"
        definition.hooks.on_use = function(self,ctx,event)
            return {effects={
                {type="psychic_attack",target=event.user,power=1},
                {type="consume_item",target=self.id,quantity=1}
            }}
        end
        definition.hooks.on_damage = function(self,ctx,event)
            return {effects={
                {type="sanity_damage",target=event.user,amount=10000},
                {type="consume_item",target=self.id,quantity=999999}
            }}
        end
    "#,
    );
    let mut g = Game::load(&fixture.0).unwrap();
    without_enemies(&mut g);
    let p = g.player_id;
    let item = inventory_item(&g, "item.bread");
    g.actor_mut(p).unwrap().seishinryoku = 8;
    let sanity = g.actor(p).unwrap().sanity;
    assert!(g.hook(item, "on_damage", json!({"user":p})).is_err());
    assert!(g.running && !g.game_over);
    assert!(!g.actor(p).unwrap().ghoul);
    assert_eq!(g.actor(p).unwrap().sanity, sanity);
    g.use_item(item).unwrap();
    assert_eq!(g.actor(p).unwrap().sanity, sanity - 1);
}

#[test]
fn dexterity_improves_real_unarmed_hit_chance_for_player_and_enemies() {
    let mut base = load();
    without_enemies(&mut base);
    let p = base.player_id;
    let enemy = base
        .spawn(
            "monster.ghoul_worker",
            Some(base.position(p).unwrap().offset(1, 0)),
            None,
            1,
        )
        .unwrap();
    for (attacker, target) in [(p, enemy), (enemy, p)] {
        // Remove held weapons so this exercises the common unarmed accuracy path.
        let a = base.actor_mut(attacker).unwrap();
        a.inventory.right_hand = None;
        a.inventory.left_hand = None;
        base.actor_mut(target).unwrap().hp = HP {
            hp: 10000,
            max_hp: 10000,
        };
        let mut improved = 0;
        for seed in 0..64 {
            let mut clumsy = base.clone();
            let mut dexterous = base.clone();
            clumsy.rng = Rng::new(seed);
            dexterous.rng = Rng::new(seed);
            clumsy.actor_mut(attacker).unwrap().kiyousa = 0;
            dexterous.actor_mut(attacker).unwrap().kiyousa = 30;
            clumsy.attack(attacker, target, None).unwrap();
            dexterous.attack(attacker, target, None).unwrap();
            if dexterous.actor(target).unwrap().hp.hp < clumsy.actor(target).unwrap().hp.hp {
                improved += 1;
            }
        }
        assert!(improved > 0);
    }
}

#[test]
fn removed_attributes_social_skill_and_invalid_initial_sanity_are_rejected() {
    for change in [
        "definition.movable.attributes.tairyoku=8",
        "definition.movable.attributes.tiryoku=8",
        "definition.movable.skills.syakou=0",
        "definition.movable.sanity=0",
        "definition.movable.sanity=-1",
    ] {
        let fixture = Fixture::new();
        fixture.change("data/entity_templates/player.lua", change);
        assert!(Game::load(&fixture.0).is_err(), "{change}");
    }
}
