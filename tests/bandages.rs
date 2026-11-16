use my_rogue::{combat::Rng, entity::*, game::Game};
use std::path::Path;
fn game() -> Game {
    let mut g = Game::load(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let ids: Vec<_> = g
        .entities
        .iter()
        .filter(|(id, e)| **id != g.player_id && matches!(e, Entity::Movable(_)))
        .map(|(id, _)| *id)
        .collect();
    for id in ids {
        g.remove(id);
    }
    g.actor_mut(g.player_id).unwrap().hp = HP {
        hp: 5000,
        max_hp: 5000,
    };
    g
}
fn bandage(g: &mut Game) -> Id {
    g.spawn("item.bandage", None, Some(g.player_id), 3).unwrap()
}
fn dress(g: &mut Game, part: &str, stage: InjuryStage) -> Id {
    let p = g.player_id;
    g.actor_mut(p).unwrap().body.insert(part.into(), stage);
    let b = bandage(g);
    assert!(g.use_bandage_on(b, part).unwrap());
    b
}
fn quantity(g: &Game, id: Id) -> u32 {
    match &g.entities[&id] {
        Entity::Item(i) => i.quantity,
        _ => 0,
    }
}
#[test]
fn bandage_consumes_one_then_stops_bleeding_and_disappears_at_one_hundred_turns() {
    let mut g = game();
    let p = g.player_id;
    let b = dress(&mut g, "left_arm", InjuryStage::Bleeding);
    assert_eq!(quantity(&g, b), 2);
    assert_eq!(g.actor(p).unwrap().treatment.bandages["left_arm"], 100);
    assert_eq!(g.turn, 1);
    assert_eq!(g.actor(p).unwrap().hp.hp, 5000);
    assert!(g.bandage_choices(p).is_empty());
    assert!(g.use_bandage_on(b, "left_arm").is_err());
    for _ in 1..99 {
        g.advance_turn();
    }
    assert_eq!(g.turn, 99);
    assert_eq!(g.actor(p).unwrap().part_bleeding("left_arm"), 1);
    g.advance_turn();
    assert_eq!(g.turn, 100);
    assert_eq!(g.actor(p).unwrap().hp.hp, 4950);
    assert_eq!(g.actor(p).unwrap().nutrition.protein, 0);
    assert_eq!(g.actor(p).unwrap().part_bleeding("left_arm"), 0);
    assert!(g.actor(p).unwrap().treatment.bandages.is_empty());
    assert_eq!(g.actor(p).unwrap().body["left_arm"], InjuryStage::Bleeding);
    let snapshot = g.entity_snapshot(p).unwrap();
    assert_eq!(snapshot["bleeding"]["left_arm"], 0);
    assert!(snapshot["bandages"].as_object().unwrap().is_empty());
    g.advance_turn();
    assert_eq!(g.actor(p).unwrap().hp.hp, 4950);
    assert_eq!(quantity(&g, b), 2);
}
#[test]
fn stopping_amputation_bleeding_preserves_missing_leg_and_movement_penalty() {
    let mut g = game();
    let p = g.player_id;
    dress(&mut g, "left_leg", InjuryStage::Severed);
    for _ in 1..100 {
        g.advance_turn();
    }
    assert_eq!(g.actor(p).unwrap().bleeding_damage(), 0);
    assert_eq!(g.actor(p).unwrap().body["left_leg"], InjuryStage::Severed);
    assert_eq!(g.actor(p).unwrap().movement_turns(), 4);
    assert!(g.bandage_choices(p).is_empty());
    assert!(
        g.actor(p)
            .unwrap()
            .wound_label("left_leg")
            .contains("止血済み")
    );
}
fn attack_arm(g: &mut Game) -> (Id, Id, u64) {
    let p = g.player_id;
    let enemy = g
        .spawn(
            "monster.ghoul_worker",
            Some(g.position(p).unwrap().offset(1, 0)),
            None,
            1,
        )
        .unwrap();
    let weapon = g.spawn("item.knife", None, Some(enemy), 1).unwrap();
    std::sync::Arc::make_mut(&mut g.entity_definitions)
        .get_mut("item.knife")
        .unwrap()
        .item
        .as_mut()
        .unwrap()
        .weapon
        .as_mut()
        .unwrap()
        .critical_value = 2;
    g.equip_auto(enemy, weapon).unwrap();
    let before = g.actor(p).unwrap().body["left_arm"];
    let seed = (0..500)
        .find(|seed| {
            let mut probe = g.clone();
            probe.rng = Rng::new(*seed);
            probe.attack(enemy, p, Some(weapon)).unwrap();
            probe.actor(p).unwrap().body["left_arm"] == before.next()
        })
        .unwrap();
    (enemy, weapon, seed)
}
#[test]
fn criticals_advance_bandaged_wounds_and_keep_the_existing_completion_time() {
    let mut g = game();
    let p = g.player_id;
    dress(&mut g, "left_arm", InjuryStage::Bleeding);
    let (enemy, weapon, seed) = attack_arm(&mut g);
    for expected in [InjuryStage::HeavyBleeding, InjuryStage::Severed] {
        g.rng = Rng::new(seed);
        g.attack(enemy, p, Some(weapon)).unwrap();
        assert_eq!(g.actor(p).unwrap().body["left_arm"], expected);
        assert_eq!(g.actor(p).unwrap().treatment.bandages["left_arm"], 100);
    }
    assert_eq!(g.actor(p).unwrap().part_bleeding("left_arm"), 4);
    assert!(!g.actor(p).unwrap().hand_usable(Hand::Left));
    g.remove(enemy);
    for _ in 1..100 {
        g.advance_turn();
    }
    assert_eq!(g.actor(p).unwrap().bleeding_damage(), 0);
    assert!(!g.actor(p).unwrap().hand_usable(Hand::Left));
}
#[test]
fn a_critical_after_hemostasis_advances_the_retained_wound_and_restarts_bleeding() {
    let mut g = game();
    let p = g.player_id;
    dress(&mut g, "left_arm", InjuryStage::Bleeding);
    for _ in 1..100 {
        g.advance_turn();
    }
    let (enemy, weapon, seed) = attack_arm(&mut g);
    g.rng = Rng::new(seed);
    g.attack(enemy, p, Some(weapon)).unwrap();
    assert_eq!(
        g.actor(p).unwrap().body["left_arm"],
        InjuryStage::HeavyBleeding
    );
    assert_eq!(g.actor(p).unwrap().part_bleeding("left_arm"), 2);
    assert!(!g.actor(p).unwrap().treatment.stopped.contains("left_arm"));
    assert!(g.bandage_choices(p).contains(&"left_arm".to_string()));
}
#[test]
fn healthy_and_nonexistent_parts_cannot_spend_bandages_or_time_and_respawn_clears_treatment() {
    let mut g = game();
    let p = g.player_id;
    let b = bandage(&mut g);
    let before = g.entity_snapshot(p).unwrap();
    for part in ["head", "unknown"] {
        assert!(g.use_bandage_on(b, part).is_err());
    }
    assert_eq!(g.turn, 0);
    assert_eq!(quantity(&g, b), 3);
    assert_eq!(g.entity_snapshot(p).unwrap(), before);
    g.actor_mut(p)
        .unwrap()
        .body
        .insert("body".into(), InjuryStage::HeavyBleeding);
    g.use_bandage_on(b, "body").unwrap();
    g.damage(p, p, u32::MAX).unwrap();
    g.update();
    assert!(g.actor(p).unwrap().treatment.bandages.is_empty());
    assert!(g.actor(p).unwrap().treatment.stopped.is_empty());
    assert_eq!(quantity(&g, b), 2);
}
