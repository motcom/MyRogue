use my_rogue::{config::*, entity::*, game::Game, scripting::Action};
use std::path::Path;
fn game() -> Game {
    Game::load(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap()
}
fn clear(g: &mut Game) {
    let ids: Vec<_> = g
        .entities
        .iter()
        .filter_map(|(id, e)| {
            ((matches!(e, Entity::Movable(_)) && *id != g.player_id)
                || matches!(e, Entity::Item(i) if matches!(i.location,ItemLocation::Ground(_))))
            .then_some(*id)
        })
        .collect();
    for id in ids {
        g.remove(id);
    }
}
fn pos(x: i32, y: i32) -> Pos {
    Pos { x, y, z: 0 }
}
fn held(g: &mut Game, kind: &str) -> Id {
    g.spawn(kind, None, Some(g.player_id), 1).unwrap()
}
#[test]
fn catalog_covers_documented_items_and_map_elements() {
    let g = game();
    for id in [
        "item.short_sword",
        "item.long_sword",
        "item.bastard_sword",
        "item.mace",
        "item.spear",
        "item.ak",
        "item.m16",
        "item.frag_grenade",
        "item.flash_grenade",
        "item.vitamins",
        "item.apple",
        "item.raw_meat",
        "item.cooked_meat",
        "item.rotten_meat",
        "item.carrot",
        "item.cabbage",
        "item.dirty_water",
        "item.clean_water",
        "terrain.wasteland",
        "obstacle.table",
        "obstacle.chair",
        "obstacle.wooden_wall",
        "obstacle.conifer",
        "obstacle.hedge",
        "obstacle.fence",
        "obstacle.lamp",
    ] {
        assert!(g.entity_definitions.contains_key(id), "{id}");
    }
    assert_eq!(g.movement_cost(pos(40, 25)), Some(4.0));
}
#[test]
fn explosion_falls_off_over_five_by_five_and_consumes_one() {
    let mut g = game();
    clear(&mut g);
    let grenade = held(&mut g, "item.frag_grenade");
    let mut targets = Vec::new();
    for x in [10, 11, 12, 13] {
        targets.push(
            g.spawn("monster.ghoul_brute", Some(pos(x, 20)), None, 1)
                .unwrap(),
        );
    }
    // Player must be close enough and have sight of the target.
    let p = g.player_id;
    g.move_to(p, pos(8, 11)).unwrap();
    for y in 12..=18 {
        g.move_to(p, pos(8, y)).unwrap();
    }
    g.throw_item(p, grenade, pos(10, 20)).unwrap();
    let hp: Vec<_> = targets
        .iter()
        .map(|id| g.actor(*id).unwrap().hp.hp)
        .collect();
    assert_eq!(hp, vec![96, 104, 112, 120]);
    assert!(!g.entities.contains_key(&grenade));
    assert_eq!(g.actor(p).unwrap().hp.hp, 52); // corner of 5x5 includes the thrower
}
#[test]
fn wall_blocks_blast_and_invalid_throw_keeps_item() {
    let mut g = game();
    clear(&mut g);
    let p = g.player_id;
    let grenade = held(&mut g, "item.frag_grenade");
    assert!(g.throw_item(p, grenade, pos(50, 30)).is_err());
    assert!(g.entities.contains_key(&grenade));
    let target = g
        .spawn("monster.ghoul_brute", Some(pos(12, 10)), None, 1)
        .unwrap();
    g.spawn("obstacle.stone_wall", Some(pos(11, 10)), None, 1)
        .unwrap();
    g.throw_item(p, grenade, pos(10, 10)).unwrap();
    assert_eq!(g.actor(target).unwrap().hp.hp, 120);
}
#[test]
fn flash_duration_falls_off_and_ai_recovers() {
    let mut g = game();
    clear(&mut g);
    let p = g.player_id;
    let grenade = held(&mut g, "item.flash_grenade");
    let target = g
        .spawn("monster.ghoul_runner", Some(pos(10, 10)), None, 1)
        .unwrap();
    g.throw_item(p, grenade, pos(10, 10)).unwrap();
    assert_eq!(g.actor(target).unwrap().stunned_until, 4);
    assert_eq!(g.actor(p).unwrap().stunned_until, 2);
    for _ in 0..3 {
        g.advance_turn();
        assert_eq!(g.position(target), Some(pos(10, 10)));
    }
    g.advance_turn();
    assert_ne!(g.position(target), Some(pos(10, 10)));
}
#[test]
fn spear_reaches_two_cells_but_cannot_cross_wall_or_use_severed_arm() {
    let mut g = game();
    clear(&mut g);
    let p = g.player_id;
    let spear = held(&mut g, "item.spear");
    g.equip_in_hand(p, spear, Grip::Both).unwrap();
    let target = g
        .spawn("monster.ghoul_brute", Some(pos(10, 10)), None, 1)
        .unwrap();
    assert!(g.can_attack(p, target, Some(spear)));
    let wall = g
        .spawn("obstacle.stone_wall", Some(pos(9, 10)), None, 1)
        .unwrap();
    assert!(!g.can_attack(p, target, Some(spear)));
    g.remove(wall);
    g.actor_mut(p)
        .unwrap()
        .body
        .insert("left_arm".into(), InjuryStage::Severed);
    assert!(!g.can_attack(p, target, Some(spear)));
}
#[test]
fn runner_takes_two_steps_and_severed_leg_preserves_delay() {
    let mut g = game();
    clear(&mut g);
    let id = g
        .spawn("monster.ghoul_runner", Some(pos(12, 10)), None, 1)
        .unwrap();
    g.advance_turn();
    assert_eq!(g.position(id).unwrap().distance(pos(12, 10)), 2);
    g.actor_mut(id)
        .unwrap()
        .body
        .insert("left_leg".into(), InjuryStage::Severed);
    let old = g.position(id).unwrap();
    g.advance_turn();
    assert!(g.position(id).unwrap().distance(old) <= 1);
}
#[test]
fn butcher_waits_every_other_turn_and_guard_has_shield() {
    let mut g = game();
    clear(&mut g);
    let b = g
        .spawn("monster.ghoul_butcher", Some(pos(12, 10)), None, 1)
        .unwrap();
    let guard = g
        .spawn("monster.ghoul_guard", Some(pos(13, 13)), None, 1)
        .unwrap();
    assert_eq!(g.defense_values(guard).unwrap().0, 6);
    g.advance_turn();
    assert_eq!(g.position(b), Some(pos(12, 10)));
    g.advance_turn();
    assert_ne!(g.position(b), Some(pos(12, 10)));
}
#[test]
fn howler_alerts_allies_with_last_seen_position() {
    let mut g = game();
    clear(&mut g);
    let h = g
        .spawn("monster.ghoul_howler", Some(pos(12, 10)), None, 1)
        .unwrap();
    let ally = g
        .spawn("monster.ghoul_runner", Some(pos(19, 10)), None, 1)
        .unwrap();
    g.advance_turn();
    assert_eq!(g.actor(ally).unwrap().alert_position, Some(pos(8, 10)));
    assert_eq!(g.actor(h).unwrap().alert_until, 7);
    assert_ne!(g.position(ally), Some(pos(19, 10)));
}
#[test]
fn stalker_waits_until_hostile_is_close() {
    let mut g = game();
    clear(&mut g);
    let s = g
        .spawn("monster.ghoul_stalker", Some(pos(12, 10)), None, 1)
        .unwrap();
    g.advance_turn();
    assert_eq!(g.position(s), Some(pos(12, 10)));
    g.move_to(g.player_id, pos(9, 10)).unwrap();
    g.advance_turn();
    assert_ne!(g.position(s), Some(pos(12, 10)));
}
#[test]
fn scavenger_picks_up_equips_and_reloads_rifle() {
    let mut g = game();
    clear(&mut g);
    let s = g
        .spawn("monster.ghoul_scavenger", Some(pos(12, 10)), None, 1)
        .unwrap();
    let rifle = g.spawn("item.sks", Some(pos(12, 10)), None, 1).unwrap();
    let ammo = g.spawn("ammo.rifle", Some(pos(12, 10)), None, 4).unwrap();
    g.advance_turn();
    assert_eq!(g.entities[&rifle].carrier(), Some(s));
    assert_eq!(g.actor(s).unwrap().inventory.grip(rifle), Some(Grip::Both));
    g.advance_turn();
    assert_eq!(g.entities[&ammo].carrier(), Some(s));
    g.advance_turn();
    assert!(matches!(&g.entities[&rifle],Entity::Item(i) if i.loaded_ammo==4));
}
#[test]
fn rifleman_reloads_then_fires_and_has_low_hp() {
    let mut g = game();
    clear(&mut g);
    let id = g
        .spawn("monster.ghoul_rifleman", Some(pos(12, 10)), None, 1)
        .unwrap();
    assert_eq!(g.actor(id).unwrap().hp.hp, 32);
    let w = g.actor(id).unwrap().inventory.equipped.unwrap();
    g.advance_turn();
    assert!(matches!(&g.entities[&w],Entity::Item(i) if i.loaded_ammo==10));
    g.advance_turn();
    assert!(matches!(&g.entities[&w],Entity::Item(i) if i.loaded_ammo==9));
}
#[test]
fn vitamins_work_at_full_carbohydrate_and_food_hazards_consume_one() {
    let mut g = game();
    clear(&mut g);
    let p = g.player_id;
    g.actor_mut(p).unwrap().nutrition.carbohydrate = 100;
    let vitamin = held(&mut g, "item.vitamins");
    assert!(g.use_item(vitamin).unwrap());
    assert_eq!(g.actor(p).unwrap().nutrition.vitamin, 60);
    let water = held(&mut g, "item.dirty_water");
    assert!(g.use_item(water).unwrap());
    assert_eq!(g.actor(p).unwrap().hp.hp, 58);
    assert!(!g.entities.contains_key(&water));
    g.actor_mut(p).unwrap().nutrition = Nutrition {
        protein: 100,
        carbohydrate: 100,
        vitamin: 100,
    };
    let food = held(&mut g, "item.apple");
    assert!(!g.use_item(food).unwrap());
    assert!(g.entities.contains_key(&food));
}
#[test]
fn crafting_rechecks_requirements_and_rolls_back_failed_output() {
    let mut g = game();
    clear(&mut g);
    let p = g.player_id;
    let wood = held(&mut g, "item.wood");
    g.move_to(p, pos(7, 10)).unwrap();
    g.start_craft(p, "craft.club").unwrap();
    assert!(!g.continue_craft(p).unwrap());
    assert!(!g.continue_craft(p).unwrap());
    g.recipes[0].outputs.push(my_rogue::crafting::ItemAmount {
        kind: "item.missing".into(),
        quantity: 1,
    });
    let count = g.entities.len();
    assert!(g.continue_craft(p).is_err());
    assert!(g.entities.contains_key(&wood));
    assert_eq!(g.entities.len(), count);
    assert_eq!(
        g.actor(p)
            .unwrap()
            .crafting
            .as_ref()
            .unwrap()
            .completed_turns,
        2
    );
    g.recipes[0].outputs.pop();
    assert!(g.continue_craft(p).unwrap());
    assert!(!g.entities.contains_key(&wood));
    assert!(g.actor(p).unwrap().crafting.is_none());
}
#[test]
fn crafting_requires_material_tool_and_nearby_workbench() {
    let mut g = game();
    clear(&mut g);
    let p = g.player_id;
    assert!(g.start_craft(p, "craft.club").is_err());
    held(&mut g, "item.wood");
    g.move_to(p, pos(9, 10)).unwrap();
    assert!(g.start_craft(p, "craft.club").is_err());
    g.move_to(p, pos(8, 10)).unwrap();
    g.move_to(p, pos(7, 10)).unwrap();
    g.start_craft(p, "craft.club").unwrap();
    let knife = g.actor(p).unwrap().inventory.equipped.unwrap();
    g.drop_item(p, knife).unwrap();
    // 足元の道具は制作に使える。削除されて初めて作業を止める。
    g.remove(knife);
    assert!(g.continue_craft(p).is_err());
    assert_eq!(
        g.actor(p)
            .unwrap()
            .crafting
            .as_ref()
            .unwrap()
            .completed_turns,
        0
    );
}
#[test]
fn input_drives_crafting_for_three_turns() {
    let mut g = game();
    clear(&mut g);
    held(&mut g, "item.wood");
    g.move_to(g.player_id, pos(7, 10)).unwrap();
    g.pending_action = Some(Action::StartCraft {
        recipe: "craft.club".into(),
    });
    g.update();
    for _ in 0..2 {
        g.pending_action = Some(Action::ContinueCraft);
        g.update();
    }
    assert_eq!(g.turn, 3);
    assert!(g.actor(g.player_id).unwrap().crafting.is_none());
}

#[test]
fn every_documented_food_can_be_used_from_inventory() {
    for (kind, carbohydrate, hydration, harm) in [
        ("item.apple", 15, 0, 0),
        ("item.raw_meat", 0, 0, 0),
        ("item.cooked_meat", 0, 0, 0),
        ("item.rotten_meat", 0, 0, 3),
        ("item.bread", 45, 0, 0),
        ("item.carrot", 10, 0, 0),
        ("item.cabbage", 12, 0, 0),
        ("item.mineral_water", 0, 40, 0),
        ("item.dirty_water", 0, 25, 3),
        ("item.clean_water", 0, 40, 0),
    ] {
        let mut g = game();
        clear(&mut g);
        let p = g.player_id;
        {
            let a = g.actor_mut(p).unwrap();
            a.nutrition.carbohydrate = 20;
            a.hydration.current = 20;
        }
        let item = g.spawn(kind, None, Some(p), 2).unwrap();
        g.pending_action = Some(Action::UseItem { item });
        g.update();
        assert_eq!(g.turn, 1, "{kind}");
        let a = g.actor(p).unwrap();
        assert_eq!(a.nutrition.carbohydrate, 20 + carbohydrate, "{kind}");
        assert_eq!(a.hydration.current, 20 + hydration, "{kind}");
        assert_eq!(a.hp.hp, 60 - harm + u32::from(harm > 0), "{kind}");
        assert!(
            matches!(&g.entities[&item],Entity::Item(i) if i.quantity==1),
            "{kind}"
        );
    }
}
#[test]
fn healing_items_work_via_registered_numbers_and_keep_no_effect_items() {
    let mut g = game();
    clear(&mut g);
    let p = g.player_id;
    g.actor_mut(p).unwrap().hp.hp = 40;
    let syringe = held(&mut g, "item.syringe");
    g.bind_quick_slot(syringe, 0, None).unwrap();
    g.activate_quick_slot(0).unwrap();
    assert_eq!(g.actor(p).unwrap().hp.hp, 56);
    assert_eq!(g.turn, 1);
    assert!(!g.entities.contains_key(&syringe));
    let vitamins = held(&mut g, "item.vitamins");
    g.bind_quick_slot(vitamins, 1, None).unwrap();
    g.activate_quick_slot(1).unwrap();
    assert_eq!(g.actor(p).unwrap().nutrition.vitamin, 60);
    assert_eq!(g.turn, 2);
    g.actor_mut(p).unwrap().hp.hp = 60;
    let unused = held(&mut g, "item.syringe");
    assert!(!g.use_item(unused).unwrap());
    assert_eq!(g.turn, 2);
    assert!(g.entities.contains_key(&unused));
}
