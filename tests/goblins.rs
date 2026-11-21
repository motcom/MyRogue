use my_rogue::{config::*, entity::*, game::Game, scripting::Action};
use std::path::Path;
fn pos(x: i32, y: i32) -> Pos {
    Pos { x, y, z: 0 }
}
fn game() -> Game {
    let mut g = Game::load(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let remove: Vec<_> = g
        .entities
        .iter()
        .filter_map(|(id, e)| {
            ((matches!(e, Entity::Movable(_)) && *id != g.player_id)
                || matches!(e, Entity::Item(i) if matches!(i.location,ItemLocation::Ground(_))))
            .then_some(*id)
        })
        .collect();
    for id in remove {
        g.remove(id);
    }
    g
}
fn spawn(g: &mut Game, kind: &str, x: i32, y: i32) -> Id {
    g.spawn(&format!("monster.goblin_{kind}"), Some(pos(x, y)), None, 1)
        .unwrap()
}
fn tick(g: &mut Game, id: Id, turn: u64) {
    g.turn = turn;
    g.hook(id, "on_turn", serde_json::json!({"turn":turn}))
        .unwrap();
}
#[test]
fn goblin_factions_are_mutually_hostile_to_survivors_and_ghouls_but_not_allies() {
    let mut g = game();
    let goblin = spawn(&mut g, "spearman", 10, 10);
    let ally = spawn(&mut g, "caller", 10, 11);
    assert_eq!(g.nearest_hostile(goblin, 10), Some(g.player_id));
    assert_eq!(g.nearest_hostile(g.player_id, 10), Some(goblin));
    assert_eq!(g.faction(ally), Some("goblin"));
    assert!(!g.actor(goblin).unwrap().ghoul);
    let ghoul = g
        .spawn("monster.ghoul_worker", Some(pos(11, 10)), None, 1)
        .unwrap();
    assert_eq!(g.nearest_hostile(goblin, 10), Some(ghoul));
    assert_eq!(g.nearest_hostile(ghoul, 10), Some(goblin));
}
#[test]
fn scavenger_picks_up_equips_and_flees_when_hurt() {
    let mut g = game();
    let id = spawn(&mut g, "scavenger", 12, 10);
    let item = g
        .spawn("item.short_sword", Some(pos(12, 10)), None, 1)
        .unwrap();
    tick(&mut g, id, 1);
    assert_eq!(g.entities[&item].carrier(), Some(id));
    tick(&mut g, id, 2);
    assert_eq!(g.actor(id).unwrap().inventory.right_hand, Some(item));
    g.actor_mut(id).unwrap().hp.hp = 1;
    let before = g
        .position(id)
        .unwrap()
        .distance(g.position(g.player_id).unwrap());
    tick(&mut g, id, 3);
    assert!(
        g.position(id)
            .unwrap()
            .distance(g.position(g.player_id).unwrap())
            > before
    );
}
#[test]
fn scrap_guard_is_slow_spear_has_reach_and_chief_wears_armor() {
    let mut g = game();
    let guard = spawn(&mut g, "scrap_guard", 12, 10);
    assert!(g.actor(guard).unwrap().inventory.left_hand.is_some());
    tick(&mut g, guard, 1);
    assert_eq!(g.position(guard), Some(pos(12, 10)));
    tick(&mut g, guard, 2);
    assert_ne!(g.position(guard), Some(pos(12, 10)));
    g.remove(guard);
    let spear = spawn(&mut g, "spearman", 10, 10);
    assert!(g.can_attack(spear, g.player_id, None));
    tick(&mut g, spear, 3);
    assert_eq!(g.position(spear), Some(pos(10, 10)));
    let chief = spawn(&mut g, "chief", 13, 10);
    assert!(!g.actor(chief).unwrap().inventory.armor.is_empty());
    assert!(g.actor(chief).unwrap().inventory.left_hand.is_some());
}
#[test]
fn crossbowman_reloads_fires_and_retreats_at_close_range() {
    let mut g = game();
    let id = spawn(&mut g, "crossbowman", 12, 10);
    let weapon = g.actor(id).unwrap().inventory.right_hand.unwrap();
    tick(&mut g, id, 1);
    let Entity::Item(item) = &g.entities[&weapon] else {
        panic!()
    };
    assert_eq!(item.loaded_ammo, 1);
    tick(&mut g, id, 2);
    let Entity::Item(item) = &g.entities[&weapon] else {
        panic!()
    };
    assert_eq!(item.loaded_ammo, 0);
    g.remove(id);
    let id = spawn(&mut g, "crossbowman", 10, 10);
    tick(&mut g, id, 3);
    assert!(
        g.position(id)
            .unwrap()
            .distance(g.position(g.player_id).unwrap())
            > 2
    );
}

#[test]
fn crossbowman_fires_three_bolts_then_switches_to_knife_and_closes_in() {
    let mut g = game();
    let player = g.player_id;
    g.actor_mut(player).unwrap().hp = HP {
        hp: 1000,
        max_hp: 1000,
    };
    let id = spawn(&mut g, "crossbowman", 12, 10);
    let crossbow = g.actor(id).unwrap().inventory.right_hand.unwrap();
    let knife = *g
        .actor(id)
        .unwrap()
        .inventory
        .items
        .iter()
        .find(|item| g.definition(**item).id == "item.knife")
        .unwrap();
    let loaded = |g: &Game| match &g.entities[&crossbow] {
        Entity::Item(item) => item.loaded_ammo,
        _ => unreachable!(),
    };
    for turn in [1, 3, 5] {
        tick(&mut g, id, turn);
        assert_eq!(loaded(&g), 1);
        assert_eq!(g.actor(id).unwrap().inventory.right_hand, Some(crossbow));
        tick(&mut g, id, turn + 1);
        assert_eq!(loaded(&g), 0);
    }
    assert!(
        g.actor(id)
            .unwrap()
            .inventory
            .items
            .iter()
            .all(|item| g.definition(*item).id != "ammo.bolt")
    );
    tick(&mut g, id, 7);
    assert_eq!(g.actor(id).unwrap().inventory.right_hand, Some(knife));
    assert_eq!(g.actor(id).unwrap().inventory.left_hand, None);

    let player_pos = g.position(player).unwrap();
    for turn in 8..=10 {
        tick(&mut g, id, turn);
    }
    assert_eq!(g.position(id).unwrap().distance(player_pos), 1);
    assert!(g.can_attack(id, player, Some(knife)));
    g.messages.clear();
    tick(&mut g, id, 11);
    assert!(
        g.messages
            .iter()
            .any(|message| message.contains(" -> ") && message.contains("damage"))
    );
    assert_eq!(loaded(&g), 0);
    g.actor_mut(id)
        .unwrap()
        .body
        .insert("right_arm".into(), InjuryStage::Severed);
    tick(&mut g, id, 12);
    assert_eq!(g.actor(id).unwrap().inventory.left_hand, Some(knife));
}
#[test]
fn caller_alerts_only_nearby_allies_with_cooldown_and_validated_parameters() {
    let mut g = game();
    let id = spawn(&mut g, "caller", 12, 10);
    let ally = spawn(&mut g, "spearman", 20, 10);
    let far = spawn(&mut g, "chief", 30, 10);
    tick(&mut g, id, 1);
    assert_eq!(g.actor(ally).unwrap().alert_position, Some(pos(8, 10)));
    assert_eq!(g.actor(ally).unwrap().alert_until, 7);
    assert_eq!(g.actor(far).unwrap().alert_position, None);
    tick(&mut g, id, 2);
    assert_eq!(g.actor(ally).unwrap().alert_until, 7);
    assert!(
        g.execute(
            id,
            Action::AlertAllies {
                target: g.player_id,
                radius: 100,
                duration: 6
            }
        )
        .is_err()
    );
    assert_eq!(g.actor(ally).unwrap().alert_until, 7);
    let guard = spawn(&mut g, "scrap_guard", 21, 11);
    g.execute(
        id,
        Action::AlertAllies {
            target: g.player_id,
            radius: 10,
            duration: 6,
        },
    )
    .unwrap();
    let origin = g.position(guard);
    tick(&mut g, guard, 3);
    assert_eq!(g.position(guard), origin);
    tick(&mut g, guard, 4);
    assert_ne!(g.position(guard), origin);
}

#[test]
fn monster_death_drops_one_tier_matched_reward_and_preserves_carried_gear() {
    let mut g = game();
    for (kind, tier) in [
        ("scavenger", 1),
        ("spearman", 2),
        ("crossbowman", 3),
        ("chief", 4),
    ] {
        let position = pos(12, 10);
        let id = spawn(&mut g, kind, position.x, position.y);
        let carried = g.actor(id).unwrap().inventory.items.clone();
        let mut same = g.clone();
        let player = g.player_id;
        let mut combat_rng = g.rng.clone();
        g.damage(player, id, u32::MAX).unwrap();
        g.damage(player, id, u32::MAX).unwrap();
        g.events();
        same.damage(player, id, u32::MAX).unwrap();
        same.events();
        assert!(!g.entities.contains_key(&id));
        let ground: Vec<_> = g
            .at(position)
            .into_iter()
            .filter(|id| matches!(g.entities[id], Entity::Item(_)))
            .collect();
        assert_eq!(ground.len(), carried.len() + 1);
        assert!(carried.iter().all(|id| ground.contains(id)));
        let drop = *ground.iter().find(|id| !carried.contains(id)).unwrap();
        assert_eq!(g.definition(drop).item.as_ref().unwrap().tier, tier);
        assert_eq!(g.entity_snapshot(drop), same.entity_snapshot(drop));
        assert_eq!(g.rng.roll(1000), combat_rng.roll(1000));
        for id in ground {
            g.remove(id);
        }
    }
}

#[test]
fn camp_loads_six_roles_and_the_chiefs_guard() {
    let mut g = Game::load_scenario(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        "data/scenarios/goblin_camp.lua",
    )
    .unwrap();
    for kind in [
        "scavenger",
        "scrap_guard",
        "spearman",
        "crossbowman",
        "caller",
        "chief",
    ] {
        let id = format!("monster.goblin_{kind}");
        assert!(g.entities.values().any(|e| e.script().definition_id == id));
    }
    assert_eq!(
        g.entities
            .values()
            .filter(|e| e.script().definition_id == "monster.goblin_scrap_guard")
            .count(),
        2
    );
    assert_eq!(
        g.entities
            .values()
            .filter(|e| e.script().definition_id == "item.treasure_chest")
            .count(),
        1
    );
    for _ in 0..20 {
        g.advance_turn();
    }
    assert!(
        !g.messages.iter().any(|m| m.starts_with("Lua ")),
        "{:?}",
        g.messages
    );
}
