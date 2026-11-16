use my_rogue::{combat::Rng, config::Pos, entity::*, game::Game, scripting::Action};
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
    g
}
fn item(g: &mut Game, actor: Id, kind: &str) -> Id {
    let id = g.spawn(kind, None, Some(actor), 1).unwrap();
    if let Entity::Item(i) = g.entities.get_mut(&id).unwrap() {
        i.loaded_ammo = 1;
    }
    id
}
fn wound(g: &mut Game, actor: Id, part: &str, stage: InjuryStage) {
    g.actor_mut(actor).unwrap().body.insert(part.into(), stage);
}
fn apply(g: &mut Game, action: Action) {
    g.pending_action = Some(action);
    g.update();
}
#[test]
fn bleeding_sums_per_part_every_world_turn_and_ui_is_free() {
    let mut g = game();
    let p = g.player_id;
    wound(&mut g, p, "left_arm", InjuryStage::Bleeding);
    wound(&mut g, p, "right_arm", InjuryStage::HeavyBleeding);
    wound(&mut g, p, "left_leg", InjuryStage::Severed);
    assert_eq!(g.actor(p).unwrap().bleeding_damage(), 7);
    let hp = g.actor(p).unwrap().hp.hp;
    apply(&mut g, Action::OpenInventory);
    apply(&mut g, Action::OpenProperties);
    assert_eq!(g.actor(p).unwrap().hp.hp, hp);
    assert_eq!(g.turn, 0);
    for turn in 1..=3 {
        g.advance_turn();
        assert_eq!(g.actor(p).unwrap().hp.hp, hp - 6 * turn);
    }
    let enemy = g
        .spawn(
            "monster.ghoul_worker",
            Some(Pos { x: 30, y: 20, z: 0 }),
            None,
            1,
        )
        .unwrap();
    wound(&mut g, enemy, "right_arm", InjuryStage::HeavyBleeding);
    g.actor_mut(enemy).unwrap().hp.hp = 1;
    g.advance_turn();
    assert!(!g.entities.contains_key(&enemy));
    g.actor_mut(p).unwrap().hp.hp = 1;
    g.advance_turn();
    assert_eq!(g.actor(p).unwrap().hp.hp, g.actor(p).unwrap().hp.max_hp);
    assert_eq!(g.actor(p).unwrap().bleeding_damage(), 0);
}
#[test]
fn leg_severing_costs_four_or_eight_turns_for_player_and_enemy_movement() {
    for legs in 0..=2 {
        let mut g = game();
        let p = g.player_id;
        g.actor_mut(p).unwrap().hp = HP {
            hp: 1000,
            max_hp: 1000,
        };
        if legs > 0 {
            wound(&mut g, p, "left_leg", InjuryStage::Severed);
        }
        if legs > 1 {
            wound(&mut g, p, "right_leg", InjuryStage::Severed);
        }
        let cost = [1, 4, 8][legs];
        let start = g.position(p).unwrap();
        g.player_move(1, 0).unwrap();
        assert_eq!(g.position(p), Some(start.offset(1, 0)));
        assert_eq!(g.turn, cost);
        assert_eq!(
            g.actor(p).unwrap().hp.hp,
            1000 - (legs as u32 * 4).saturating_sub(1) * cost as u32
        );
        g.player_move(1, 0).unwrap();
        assert_eq!(g.position(p), Some(start.offset(2, 0)));
        assert_eq!(g.turn, 2 * cost);
        let e = g
            .spawn(
                "monster.ghoul_worker",
                Some(Pos { x: 25, y: 20, z: 0 }),
                None,
                1,
            )
            .unwrap();
        g.actor_mut(e).unwrap().hp = HP {
            hp: 1000,
            max_hp: 1000,
        };
        if legs > 0 {
            wound(&mut g, e, "left_leg", InjuryStage::Severed);
        }
        if legs > 1 {
            wound(&mut g, e, "right_leg", InjuryStage::Severed);
        }
        let at = g.position(e).unwrap();
        g.execute(
            e,
            Action::MoveTo {
                position: at.offset(1, 0),
            },
        )
        .unwrap();
        for elapsed in 1..=cost {
            g.advance_turn();
            g.execute(
                e,
                Action::MoveTo {
                    position: at.offset(2, 0),
                },
            )
            .unwrap();
            assert_eq!(
                g.position(e),
                Some(at.offset(if elapsed < cost { 1 } else { 2 }, 0))
            );
        }
    }
}
#[test]
fn slow_movement_stops_remaining_ticks_when_player_bleeds_to_death() {
    let mut g = game();
    let p = g.player_id;
    wound(&mut g, p, "left_leg", InjuryStage::Severed);
    g.actor_mut(p).unwrap().hp.hp = 1;
    g.player_move(1, 0).unwrap();
    assert_eq!(g.turn, 1);
    assert_eq!(g.actor(p).unwrap().bleeding_damage(), 0);
    assert!(g.entities[&p].alive());
    assert_eq!(g.actor(p).unwrap().next_move_turn, g.turn);
}
#[test]
fn hands_are_independent_two_handed_items_occupy_both_and_cleanup_is_atomic() {
    let mut g = game();
    let p = g.player_id;
    let a = item(&mut g, p, "item.knife");
    let b = item(&mut g, p, "item.glock");
    let rifle = item(&mut g, p, "item.sks");
    g.equip_in_hand(p, a, Grip::Right).unwrap();
    g.equip_in_hand(p, b, Grip::Left).unwrap();
    assert_eq!(g.actor(p).unwrap().inventory.right_hand, Some(a));
    assert_eq!(g.actor(p).unwrap().inventory.left_hand, Some(b));
    assert!(g.equip_in_hand(p, rifle, Grip::Right).is_err());
    assert_eq!(g.actor(p).unwrap().inventory.right_hand, Some(a));
    g.equip_in_hand(p, rifle, Grip::Both).unwrap();
    assert_eq!(g.actor(p).unwrap().inventory.grip(rifle), Some(Grip::Both));
    assert_eq!(g.actor(p).unwrap().inventory.grip(a), None);
    assert_eq!(g.actor(p).unwrap().inventory.grip(b), None);
    g.equip_in_hand(p, a, Grip::Left).unwrap();
    assert_eq!(g.actor(p).unwrap().inventory.right_hand, None);
    assert_eq!(g.actor(p).unwrap().inventory.grip(rifle), None);
    g.drop_item(p, a).unwrap();
    assert_eq!(g.actor(p).unwrap().inventory.left_hand, None);
    g.equip_in_hand(p, b, Grip::Both).unwrap();
    g.remove(b);
    assert_eq!(g.actor(p).unwrap().inventory.right_hand, None);
    assert_eq!(g.actor(p).unwrap().inventory.left_hand, None);
}
#[test]
fn severed_hands_disable_weapons_and_reload_and_consumables_are_not_equipment() {
    let mut g = game();
    let p = g.player_id;
    let at = g.position(p).unwrap();
    let target = g
        .spawn("monster.ghoul_worker", Some(at.offset(1, 0)), None, 1)
        .unwrap();
    let gun = item(&mut g, p, "item.glock");
    g.equip_in_hand(p, gun, Grip::Both).unwrap();
    wound(&mut g, p, "right_arm", InjuryStage::Severed);
    assert!(!g.can_attack(p, target, Some(gun)));
    assert!(g.shoot(p, target).is_err());
    assert!(g.reload(p).is_err());
    assert_eq!(g.entities[&gun].carrier(), Some(p));
    assert!(g.equip_in_hand(p, gun, Grip::Right).is_err());
    g.equip_in_hand(p, gun, Grip::Left).unwrap();
    assert!(g.can_attack(p, target, Some(gun)));
    let syringe = item(&mut g, p, "item.syringe");
    assert!(g.equip_in_hand(p, syringe, Grip::Left).is_err());
    g.actor_mut(p).unwrap().hp.hp = 20;
    wound(&mut g, p, "left_arm", InjuryStage::Severed);
    assert!(g.equipment_choices(p, syringe).is_empty());
    assert_eq!(g.actor(p).unwrap().hp.hp, 20);
    assert_eq!(g.entities[&syringe].carrier(), Some(p));
}
#[test]
fn off_hand_damage_is_three_quarters_and_tracks_left_or_right_dominance() {
    for dominant in [Hand::Right, Hand::Left] {
        for kind in ["item.knife", "item.glock"] {
            let mut base = game();
            let p = base.player_id;
            base.actor_mut(p).unwrap().dominant_hand = dominant;
            let at = base.position(p).unwrap();
            let target = base
                .spawn("monster.ghoul_worker", Some(at.offset(1, 0)), None, 1)
                .unwrap();
            base.actor_mut(target).unwrap().hp = HP {
                hp: 10000,
                max_hp: 10000,
            };
            let weapon = item(&mut base, p, kind);
            let main = if dominant == Hand::Right {
                Grip::Right
            } else {
                Grip::Left
            };
            let other = if dominant == Hand::Right {
                Grip::Left
            } else {
                Grip::Right
            };
            let mut saw_damage = false;
            for seed in 0..64 {
                let mut normal = base.clone();
                let mut off = base.clone();
                normal.equip_in_hand(p, weapon, main).unwrap();
                off.equip_in_hand(p, weapon, other).unwrap();
                normal.rng = Rng::new(seed);
                off.rng = Rng::new(seed);
                normal.attack(p, target, Some(weapon)).unwrap();
                off.attack(p, target, Some(weapon)).unwrap();
                let damage = 10000 - normal.actor(target).unwrap().hp.hp;
                assert_eq!(10000 - off.actor(target).unwrap().hp.hp, damage * 3 / 4);
                saw_damage |= damage > 0;
            }
            assert!(saw_damage);
        }
    }
}
#[test]
fn pistol_grip_changes_range_and_accuracy_and_club_grip_changes_pv() {
    let mut g = game();
    let p = g.player_id;
    let gun = item(&mut g, p, "item.glock");
    let club = item(&mut g, p, "item.club");
    g.equip_in_hand(p, gun, Grip::Right).unwrap();
    let short = g.view().effective_range(p, gun);
    assert_eq!(g.view().one_hand_accuracy_penalty(p, gun), 1);
    let at = g.position(p).unwrap();
    let target = g
        .spawn("monster.ghoul_worker", Some(at.offset(5, 0)), None, 1)
        .unwrap();
    assert!(!g.can_attack(p, target, Some(gun)));
    g.equip_in_hand(p, gun, Grip::Both).unwrap();
    assert_eq!(g.view().effective_range(p, gun), short + 2);
    assert_eq!(g.view().one_hand_accuracy_penalty(p, gun), 0);
    assert!(g.can_attack(p, target, Some(gun)));
    g.equip_in_hand(p, club, Grip::Right).unwrap();
    let pv = g.view().effective_weapon_pv(p, club);
    g.equip_in_hand(p, club, Grip::Both).unwrap();
    assert_eq!(g.view().effective_weapon_pv(p, club), pv + 1);
    wound(&mut g, p, "left_arm", InjuryStage::Severed);
    assert!(!g.view().weapon_usable(p, club));
    g.equip_in_hand(p, club, Grip::Right).unwrap();
    assert_eq!(g.view().effective_weapon_pv(p, club), pv);
}

#[test]
fn one_handed_pistol_accuracy_penalty_changes_actual_hit_result() {
    let mut base = game();
    let p = base.player_id;
    let at = base.position(p).unwrap();
    let target = base
        .spawn("monster.ghoul_worker", Some(at.offset(1, 0)), None, 1)
        .unwrap();
    let gun = item(&mut base, p, "item.glock");
    let mut found = false;
    for seed in 0..256 {
        if Rng::new(seed).roll(20) != 7 {
            continue;
        }
        let mut one = base.clone();
        let mut two = base.clone();
        one.equip_in_hand(p, gun, Grip::Right).unwrap();
        two.equip_in_hand(p, gun, Grip::Both).unwrap();
        one.rng = Rng::new(seed);
        two.rng = Rng::new(seed);
        one.shoot(p, target).unwrap();
        two.shoot(p, target).unwrap();
        assert_eq!(one.actor(target).unwrap().hp.hp, 35);
        assert!(one.messages.iter().any(|s| s.contains("miss")));
        found |= two.actor(target).unwrap().hp.hp < 35;
    }
    assert!(found);
}

#[test]
fn equipment_choices_include_only_supported_grips_and_weapons_or_shields() {
    let mut g = game();
    let p = g.player_id;
    for (kind, expected) in [
        ("item.knife", vec![Some(Grip::Right), Some(Grip::Left)]),
        ("item.sks", vec![Some(Grip::Both)]),
        (
            "item.glock",
            vec![Some(Grip::Right), Some(Grip::Left), Some(Grip::Both)],
        ),
        ("item.bandage", vec![]),
        ("item.hammer", vec![]),
    ] {
        let id = item(&mut g, p, kind);
        assert_eq!(g.equipment_choices(p, id), expected, "{kind}");
        if expected.is_empty() {
            assert!(g.equip_auto(p, id).is_err());
        }
    }
    let shield = item(&mut g, p, "item.hammer");
    std::sync::Arc::make_mut(&mut g.entity_definitions)
        .get_mut("item.hammer")
        .unwrap()
        .common
        .tags
        .push("shield".into());
    assert_eq!(
        g.equipment_choices(p, shield),
        vec![Some(Grip::Right), Some(Grip::Left)]
    );
    g.equip_in_hand(p, shield, Grip::Left).unwrap();
    assert_eq!(g.actor(p).unwrap().inventory.left_hand, Some(shield));
    assert!(g.equip_in_hand(p, shield, Grip::Both).is_err());
}
