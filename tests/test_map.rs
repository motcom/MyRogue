use my_rogue::{
    config::Pos,
    entity::{Entity, ItemLocation},
    game::Game,
};
use std::{
    collections::{BTreeSet, VecDeque},
    path::Path,
};
fn load() -> Game {
    Game::load_test_map(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap()
}
fn pos(x: i32, y: i32) -> Pos {
    Pos { x, y, z: 0 }
}
fn reachable(g: &Game) -> BTreeSet<(i32, i32)> {
    let start = g.position(g.player_id).unwrap();
    let mut seen = BTreeSet::from([(start.x, start.y)]);
    let mut queue = VecDeque::from([start]);
    while let Some(p) = queue.pop_front() {
        for (dx, dy) in [
            (0, 1),
            (0, -1),
            (1, 0),
            (-1, 0),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
        ] {
            let q = p.offset(dx, dy);
            if g.can_step(p, q, g.player_id) && seen.insert((q.x, q.y)) {
                queue.push_back(q);
            }
        }
    }
    seen
}
#[test]
fn contains_every_definition_and_never_stacks_blocking_entities() {
    let g = load();
    assert_eq!(g.position(g.player_id), Some(pos(4, 4)));
    for (id, e) in &g.entities {
        if let Entity::Item(i) = e
            && let ItemLocation::Ground(p) = i.location
            && p.x < 29
        {
            assert!(
                p.x > 1 && p.x < 27 && p.y > 1 && p.y < 20,
                "outside upper-left warehouse: {}",
                g.definition(*id).id
            );
        }
    }
    let stored: BTreeSet<_> = g
        .entities
        .values()
        .filter_map(|e| match e {
            Entity::Item(i) if matches!(i.location,ItemLocation::Ground(p) if p.x<29) => {
                Some(i.script.definition_id.clone())
            }
            _ => None,
        })
        .collect();
    let required: BTreeSet<_> = g
        .entity_definitions
        .values()
        .filter(|d| d.category == "item")
        .map(|d| d.id.clone())
        .collect();
    assert_eq!(
        stored, required,
        "every item definition must have a sample in the upper-left warehouse"
    );
    let present: BTreeSet<_> = g
        .entities
        .values()
        .map(|e| e.script().definition_id.clone())
        .collect();
    assert_eq!(present, g.entity_definitions.keys().cloned().collect());
    assert_eq!(
        g.entities
            .values()
            .filter(|e| matches!(e, Entity::Movable(_)))
            .count(),
        18
    );
    for y in 0..g.map.height {
        for x in 0..g.map.width {
            assert!(
                g.at(pos(x, y))
                    .iter()
                    .filter(|id| g.entities[id].blocks_movement())
                    .count()
                    <= 1,
                "overlap at {x},{y}"
            );
        }
    }
    for label in &g.map_labels {
        assert!(label.position.x + label.text.chars().count() as i32 <= g.map.width);
    }
}
#[test]
fn every_supply_and_room_door_is_accessible_before_combat() {
    let mut g = load();
    let door = g
        .at(pos(27, 19))
        .into_iter()
        .find(|id| g.definition(*id).id == "obstacle.wooden_door")
        .unwrap();
    g.hook(
        door,
        "on_interact",
        serde_json::json!({"interaction":"open"}),
    )
    .unwrap();
    let seen = reachable(&g);
    for (id, e) in &g.entities {
        if let Entity::Item(i) = e
            && let ItemLocation::Ground(p) = i.location
            && p.x < 29
        {
            assert!(
                seen.contains(&(p.x, p.y)),
                "unreachable {}",
                g.definition(*id).id
            );
        }
        if g.definition(*id).id == "obstacle.wooden_door" {
            let p = e.map_position().unwrap();
            // Open the warehouse first in the accessibility test below.
            assert!(
                seen.contains(&(p.x, p.y + 1)) || seen.contains(&(p.x - 1, p.y)),
                "unreachable door {p:?}"
            );
        }
    }
    assert_eq!(g.movement_cost(pos(4, 28)), Some(2.0));
    assert_eq!(g.movement_cost(pos(12, 28)), Some(4.0));
    assert_eq!(g.movement_cost(pos(20, 28)), Some(1.0));
}
#[test]
fn closed_rooms_keep_player_safe_and_enemies_contained_for_preparation() {
    let mut g = load();
    let p = g.player_id;
    let initial: Vec<_> = g
        .entities
        .iter()
        .filter(|(_, e)| matches!(e, Entity::Movable(_)))
        .map(|(id, e)| (*id, e.map_position().unwrap()))
        .collect();
    let hp = g.actor(p).unwrap().hp.hp;
    for _ in 0..100 {
        g.advance_turn();
    }
    assert_eq!(g.actor(p).unwrap().hp.hp, hp);
    for (id, origin) in initial {
        let q = g.position(id).unwrap();
        if id == p {
            assert_eq!(q, origin);
            continue;
        }
        // Scavenger may walk to its supplies, but stays inside its own room.
        let base_x = if origin.x >= 60 { 60 } else { 30 };
        let x0 = base_x + ((origin.x - base_x) / 7) * 7;
        let y0 = 2 + ((origin.y - 2) / 10) * 10;
        assert!(q.x > x0 && q.x < x0 + 6 && q.y > y0 && q.y < y0 + 6);
        assert!(!g.sight(q, g.position(p).unwrap()));
    }
    assert!(
        !g.messages.iter().any(|m| m.contains("Lua ")),
        "{:?}",
        g.messages
    );
}
#[test]
fn opening_one_room_exposes_only_its_enemy() {
    let mut g = load();
    let p = g.player_id;
    // Leave the warehouse through its east door, then approach room 01.
    g.move_to(p, pos(3, 4)).unwrap();
    g.move_to(p, pos(2, 4)).unwrap();
    for y in 5..=19 {
        g.move_to(p, pos(2, y)).unwrap();
    }
    for x in 3..27 {
        g.move_to(p, pos(x, 19)).unwrap();
    }
    let exit = g
        .at(pos(27, 19))
        .into_iter()
        .find(|id| g.definition(*id).id == "obstacle.wooden_door")
        .unwrap();
    g.interact(exit, "open").unwrap();
    g.move_to(p, pos(27, 19)).unwrap();
    g.move_to(p, pos(28, 19)).unwrap();
    for y in (9..19).rev() {
        g.move_to(p, pos(28, y)).unwrap();
    }
    for x in 29..34 {
        g.move_to(p, pos(x, 9)).unwrap();
    }
    let door = g
        .at(pos(33, 8))
        .into_iter()
        .find(|id| g.definition(*id).id == "obstacle.wooden_door")
        .unwrap();
    let worker = g
        .entities
        .iter()
        .find(|(id, _)| g.definition(**id).id == "monster.ghoul_worker")
        .map(|(id, _)| *id)
        .unwrap();
    assert!(!g.can_attack(worker, p, None));
    assert!(!g.sight(g.position(worker).unwrap(), g.position(p).unwrap()));
    g.interact(door, "open").unwrap();
    assert!(g.sight(g.position(worker).unwrap(), g.position(p).unwrap()));
    for (id, e) in &g.entities {
        if matches!(e, Entity::Movable(_)) && *id != p && *id != worker {
            assert!(!g.sight(g.position(*id).unwrap(), g.position(p).unwrap()));
        }
    }
}

#[test]
fn warehouse_protects_preparation_even_with_open_enemy_rooms() {
    let mut g = load();
    let p = g.player_id;
    g.move_to(p, pos(3, 4)).unwrap();
    g.move_to(p, pos(2, 4)).unwrap();
    for y in 5..=19 {
        g.move_to(p, pos(2, y)).unwrap();
    }
    for x in 3..27 {
        g.move_to(p, pos(x, 19)).unwrap();
    }
    let doors: Vec<_> = g
        .entities
        .iter()
        .filter_map(|(id, e)| {
            (g.definition(*id).id == "obstacle.wooden_door"
                && e.map_position() != Some(pos(27, 19)))
            .then_some(*id)
        })
        .collect();
    for door in doors {
        g.hook(
            door,
            "on_interact",
            serde_json::json!({"interaction":"open"}),
        )
        .unwrap();
    }
    let rifle = g
        .spawn("monster.ghoul_rifleman", Some(pos(28, 19)), None, 1)
        .unwrap();
    g.reload(rifle).unwrap();
    assert!(!g.can_attack(rifle, p, None));
    let hp = g.actor(p).unwrap().hp.hp;
    for _ in 0..100 {
        g.advance_turn();
    }
    assert_eq!(g.actor(p).unwrap().hp.hp, hp);
    assert_eq!(g.position(p), Some(pos(26, 19)));
    // Every player supply is inside the stone warehouse; only scavenger bait is outside.
    for (id, e) in &g.entities {
        if let Entity::Item(i) = e
            && let ItemLocation::Ground(q) = i.location
            && q.x < 29
        {
            assert!(
                q.x > 1 && q.x < 27 && q.y > 1 && q.y < 20,
                "outside warehouse: {}",
                g.definition(*id).id
            );
        }
    }
}
