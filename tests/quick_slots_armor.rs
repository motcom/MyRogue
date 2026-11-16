use my_rogue::{combat::Rng, config::Armor, entity::*, game::Game};
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
fn item(g: &mut Game, kind: &str, n: u32) -> Id {
    g.spawn(kind, None, Some(g.player_id), n).unwrap()
}
fn remove_kind(g: &mut Game, kind: &str) {
    let ids: Vec<_> = g
        .actor(g.player_id)
        .unwrap()
        .inventory
        .items
        .iter()
        .copied()
        .filter(|id| g.entities[id].script().definition_id == kind)
        .collect();
    for id in ids {
        g.remove(id);
    }
}
fn armor_kind(g: &mut Game, id: &str, slot: &str, av: u32, dv: i32) {
    let mut d = g.entity_definitions["item.leather_armor"].clone();
    d.id = id.into();
    d.item.as_mut().unwrap().armor = Some(Armor {
        slot: slot.into(),
        av,
        dv,
    });
    std::sync::Arc::make_mut(&mut g.entity_definitions).insert(id.into(), d);
}
#[test]
fn quick_slots_switch_weapons_and_keep_saved_grips_on_keys_one_and_zero() {
    let mut g = game();
    let p = g.player_id;
    let knife = item(&mut g, "item.knife", 1);
    let rifle = item(&mut g, "item.sks", 1);
    g.bind_quick_slot(knife, 0, Some(Grip::Left)).unwrap();
    g.bind_quick_slot(rifle, 9, Some(Grip::Both)).unwrap();
    assert_eq!(g.turn, 2);
    g.activate_quick_slot(0).unwrap();
    assert_eq!(g.actor(p).unwrap().inventory.grip(knife), Some(Grip::Left));
    assert_eq!(g.actor(p).unwrap().inventory.grip(rifle), None);
    assert_eq!(g.turn, 3);
    g.activate_quick_slot(0).unwrap();
    assert_eq!(g.turn, 3);
    g.activate_quick_slot(9).unwrap();
    assert_eq!(g.actor(p).unwrap().inventory.grip(rifle), Some(Grip::Both));
    assert_eq!(g.actor(p).unwrap().inventory.grip(knife), None);
    assert_eq!(g.turn, 4);
    assert_eq!(
        g.actor(p).unwrap().inventory.quick_slots[0],
        Some(QuickSlot::Equipment {
            item: knife,
            grip: Grip::Left
        })
    );
    assert_eq!(
        g.actor(p).unwrap().inventory.quick_slots[9],
        Some(QuickSlot::Equipment {
            item: rifle,
            grip: Grip::Both
        })
    );
}
#[test]
fn pistol_can_have_different_saved_grips_and_overwriting_a_slot_keeps_other_slots() {
    let mut g = game();
    let p = g.player_id;
    let gun = item(&mut g, "item.glock", 1);
    g.bind_quick_slot(gun, 1, Some(Grip::Right)).unwrap();
    g.bind_quick_slot(gun, 2, Some(Grip::Both)).unwrap();
    g.activate_quick_slot(1).unwrap();
    assert_eq!(g.view().effective_range(p, gun), 4);
    g.activate_quick_slot(2).unwrap();
    assert_eq!(g.view().effective_range(p, gun), 6);
    let bandage = item(&mut g, "item.bandage", 1);
    let turn = g.turn;
    g.bind_quick_slot(bandage, 1, None).unwrap();
    assert_eq!(g.turn, turn);
    assert!(matches!(
        g.actor(p).unwrap().inventory.quick_slots[1],
        Some(QuickSlot::Consumable { .. })
    ));
    assert_eq!(
        g.actor(p).unwrap().inventory.quick_slots[2],
        Some(QuickSlot::Equipment {
            item: gun,
            grip: Grip::Both
        })
    );
}
#[test]
fn bandage_shortcut_opens_selection_without_consuming_and_uses_another_stack_next_time() {
    let mut g = game();
    let p = g.player_id;
    remove_kind(&mut g, "item.bandage");
    let first = item(&mut g, "item.bandage", 1);
    let second = item(&mut g, "item.bandage", 2);
    g.actor_mut(p).unwrap().hp = HP {
        hp: 1000,
        max_hp: 1000,
    };
    g.actor_mut(p)
        .unwrap()
        .body
        .insert("left_arm".into(), InjuryStage::Bleeding);
    g.actor_mut(p)
        .unwrap()
        .body
        .insert("left_leg".into(), InjuryStage::HeavyBleeding);
    g.bind_quick_slot(first, 3, None).unwrap();
    g.activate_quick_slot(3).unwrap();
    assert_eq!(g.turn, 0);
    assert!(g.actor(p).unwrap().treatment.bandages.is_empty());
    g.use_bandage_on(first, "left_arm").unwrap();
    assert!(!g.entities.contains_key(&first));
    g.activate_quick_slot(3).unwrap();
    assert_eq!(g.turn, 1);
    g.use_bandage_on(second, "left_leg").unwrap();
    assert_eq!(g.actor(p).unwrap().treatment.bandages.len(), 2);
    assert!(matches!(
        g.actor(p).unwrap().inventory.quick_slots[3],
        Some(QuickSlot::Consumable { .. })
    ));
}
#[test]
fn consumable_shortcut_survives_running_out_and_resupply_and_uses_no_turn_when_empty() {
    let mut g = game();
    let p = g.player_id;
    remove_kind(&mut g, "item.bread");
    g.actor_mut(p).unwrap().nutrition.carbohydrate = 0;
    let bread = item(&mut g, "item.bread", 1);
    g.bind_quick_slot(bread, 5, None).unwrap();
    g.activate_quick_slot(5).unwrap();
    assert!(!g.entities.contains_key(&bread));
    let turn = g.turn;
    assert!(g.activate_quick_slot(5).is_err());
    assert_eq!(g.turn, turn);
    let fresh = item(&mut g, "item.bread", 1);
    g.activate_quick_slot(5).unwrap();
    assert!(!g.entities.contains_key(&fresh));
    assert_eq!(g.turn, turn + 1);
    assert!(g.quick_slot_summary(5).contains('0'));
}
#[test]
fn invalid_or_severed_hand_bindings_and_armor_assignment_do_not_change_state() {
    let mut g = game();
    let p = g.player_id;
    let gun = item(&mut g, "item.glock", 1);
    let armor = item(&mut g, "item.leather_armor", 1);
    g.actor_mut(p)
        .unwrap()
        .body
        .insert("left_arm".into(), InjuryStage::Severed);
    let before = g.entity_snapshot(p).unwrap();
    let mut rng = g.rng.clone();
    assert!(g.bind_quick_slot(gun, 10, Some(Grip::Right)).is_err());
    assert!(g.bind_quick_slot(gun, 0, Some(Grip::Both)).is_err());
    assert!(g.bind_quick_slot(armor, 0, None).is_err());
    assert!(g.bind_quick_slot(gun, 0, None).is_err());
    assert_eq!(g.entity_snapshot(p).unwrap(), before);
    assert_eq!(g.turn, 0);
    assert_eq!(g.rng.roll(1000), rng.roll(1000));
}
#[test]
fn dropping_or_removing_registered_weapons_clears_only_their_shortcuts() {
    let mut g = game();
    let p = g.player_id;
    let a = item(&mut g, "item.knife", 1);
    let b = item(&mut g, "item.sks", 1);
    g.bind_quick_slot(a, 0, Some(Grip::Right)).unwrap();
    g.bind_quick_slot(b, 1, Some(Grip::Both)).unwrap();
    g.drop_item(p, a).unwrap();
    assert!(g.actor(p).unwrap().inventory.quick_slots[0].is_none());
    assert!(g.actor(p).unwrap().inventory.quick_slots[1].is_some());
    g.remove(b);
    assert!(g.actor(p).unwrap().inventory.quick_slots[1].is_none());
    assert!(g.actor(p).unwrap().inventory.left_hand.is_none());
    assert!(g.actor(p).unwrap().inventory.right_hand.is_none());
}
#[test]
fn armor_uses_body_slots_replaces_same_slot_and_contributes_to_av_and_dv() {
    let mut g = game();
    let p = g.player_id;
    armor_kind(&mut g, "test.helmet", "head", 2, 1);
    armor_kind(&mut g, "test.mail", "body", 3, -1);
    let leather = item(&mut g, "item.leather_armor", 1);
    let helmet = item(&mut g, "test.helmet", 1);
    let mail = item(&mut g, "test.mail", 1);
    let hands = g.actor(p).unwrap().inventory.right_hand;
    let base = g.defense_values(p).unwrap();
    g.toggle_armor(p, leather).unwrap();
    assert_eq!(g.defense_values(p), Some((base.0 + 1, base.1)));
    g.toggle_armor(p, helmet).unwrap();
    assert_eq!(g.defense_values(p), Some((base.0 + 3, base.1 + 1)));
    g.toggle_armor(p, mail).unwrap();
    assert_eq!(g.defense_values(p), Some((base.0 + 5, base.1)));
    assert_eq!(g.actor(p).unwrap().inventory.armor["body"], mail);
    assert_eq!(g.actor(p).unwrap().inventory.right_hand, hands);
    g.toggle_armor(p, mail).unwrap();
    assert_eq!(g.defense_values(p), Some((base.0 + 2, base.1 + 1)));
    g.drop_item(p, helmet).unwrap();
    assert_eq!(g.defense_values(p), Some(base));
    g.toggle_armor(p, leather).unwrap();
    g.remove(leather);
    assert_eq!(g.defense_values(p), Some(base));
}
#[test]
fn armor_changes_actual_penetration_and_shields_need_a_usable_hand() {
    let mut g = game();
    let p = g.player_id;
    armor_kind(&mut g, "test.plate", "body", 100, 0);
    let target = g
        .spawn(
            "monster.ghoul_worker",
            Some(g.position(p).unwrap().offset(1, 0)),
            None,
            1,
        )
        .unwrap();
    let plate = g.spawn("test.plate", None, Some(target), 1).unwrap();
    let knife = item(&mut g, "item.knife", 1);
    g.equip_auto(p, knife).unwrap();
    let mut found = false;
    for seed in 0..32 {
        let mut bare = g.clone();
        let mut armored = g.clone();
        armored.toggle_armor(target, plate).unwrap();
        bare.rng = Rng::new(seed);
        armored.rng = Rng::new(seed);
        bare.attack(p, target, Some(knife)).unwrap();
        armored.attack(p, target, Some(knife)).unwrap();
        assert_eq!(armored.actor(target).unwrap().hp.hp, 35);
        found |= bare.actor(target).unwrap().hp.hp < 35;
    }
    assert!(found);
    let shield = item(&mut g, "item.hammer", 1);
    let defs = std::sync::Arc::make_mut(&mut g.entity_definitions);
    let d = defs.get_mut("item.hammer").unwrap();
    d.common.tags.push("shield".into());
    d.item.as_mut().unwrap().armor = Some(Armor {
        slot: "body".into(),
        av: 2,
        dv: 1,
    });
    let base = g.defense_values(p).unwrap();
    g.equip_in_hand(p, shield, Grip::Left).unwrap();
    assert_eq!(g.defense_values(p), Some((base.0 + 2, base.1 + 1)));
    g.actor_mut(p)
        .unwrap()
        .body
        .insert("left_arm".into(), InjuryStage::Severed);
    assert_eq!(g.defense_values(p), Some(base));
}
