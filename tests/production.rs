use my_rogue::{combat::Rng, crafting::*, entity::*, game::Game, scripting::Action};
use std::path::Path;
fn arena() -> Game {
    let mut g = Game::load(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let remove:Vec<_>=g.entities.iter().filter_map(|(id,e)|(*id!=g.player_id && !matches!(e,Entity::Static(s) if matches!(s.role,StaticRole::Terrain) && !s.blocks_movement)).then_some(*id)).collect();
    for id in remove {
        g.remove(id);
    }
    let p = g.position(g.player_id).unwrap();
    g.spawn("facility.workbench", Some(p), None, 1).unwrap();
    g
}
fn knife(g: &mut Game) -> Id {
    g.spawn("item.knife", None, Some(g.player_id), 1).unwrap()
}
fn storage(g: &mut Game) -> Id {
    let p = g.position(g.player_id).unwrap();
    g.spawn("item.crafting_storage", Some(p.offset(1, 1)), None, 1)
        .unwrap()
}
fn seed_for(rate: u32, modifier: i32, quality: Option<i32>, success: bool) -> u64 {
    (0..100000)
        .find(|seed| {
            let mut rng = Rng::new(*seed);
            (rng.roll(100) <= rate) == success
                && quality.is_none_or(|q| {
                    let score = rng.roll(100) as i32 + modifier;
                    let expected = match score {
                        ..=10 => -2,
                        11..=30 => -1,
                        31..=75 => 0,
                        76..=90 => 1,
                        91..=100 => 2,
                        _ => 3,
                    };
                    q == expected
                })
        })
        .unwrap()
}
fn stored(g: &Game, id: Id, kind: &str) -> u32 {
    let Entity::Item(i) = &g.entities[&id] else {
        panic!()
    };
    i.contents
        .iter()
        .filter(|e| e.definition_id == kind)
        .map(|e| e.quantity)
        .sum()
}
fn finish(g: &mut Game) {
    let p = g.player_id;
    while !g.continue_craft(p).unwrap() {}
}
#[test]
fn every_lua_recipe_can_complete_using_nearby_closed_storage() {
    let recipes = arena().recipes;
    assert_eq!(recipes.len(), 12);
    for recipe in recipes {
        let mut g = arena();
        knife(&mut g);
        let chest = storage(&mut g);
        let p = g.player_id;
        let plan = g.craft_plan(p, &recipe).unwrap();
        assert!(
            plan.selected_ingredients
                .iter()
                .all(|s| matches!(s.item,Supply::Stored{container,..} if container==chest))
        );
        g.rng = Rng::new(seed_for(
            plan.success_rate,
            plan.quality_modifier,
            None,
            true,
        ));
        let original: std::collections::BTreeSet<_> = g.entities.keys().copied().collect();
        g.start_craft(p, &recipe.id).unwrap();
        finish(&mut g);
        for output in &recipe.outputs {
            let items: Vec<_> = g
                .entities
                .iter()
                .filter(|(id, _)| !original.contains(id))
                .filter_map(|(_, e)| match e {
                    Entity::Item(i)
                        if i.kind == output.kind
                            && (matches!(i.location, ItemLocation::Ground(_))
                                || e.carrier() == Some(p)) =>
                    {
                        Some(i)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(
                items.iter().map(|i| i.quantity).sum::<u32>(),
                output.quantity,
                "{}",
                recipe.id
            );
            if output.kind == "item.treasure_chest" {
                assert!(items[0].contents.is_empty());
            }
        }
        for input in &recipe.ingredients {
            let initial = if input.kind == "item.wood" || input.kind == "item.metal_stock" {
                80
            } else {
                match input.kind.as_str() {
                    "item.thread" => 60,
                    "item.wood_handle" => 20,
                    "item.raw_meat" | "item.dirty_water" => 10,
                    _ => 40,
                }
            };
            assert_eq!(stored(&g, chest, &input.kind), initial - input.quantity);
        }
        assert!(g.actor(p).unwrap().crafting.is_none());
    }
}
#[test]
fn ground_supplies_work_but_distant_and_other_actors_inventory_do_not() {
    let mut g = arena();
    let p = g.player_id;
    let pos = g.position(p).unwrap();
    g.spawn("item.knife", Some(pos.offset(-1, 1)), None, 1)
        .unwrap();
    let wood = g
        .spawn("item.wood", Some(pos.offset(1, 0)), None, 2)
        .unwrap();
    assert!(g.craft_plan(p, &g.recipes[0]).is_ok());
    g.remove(wood);
    g.spawn("item.wood", Some(pos.offset(2, 0)), None, 2)
        .unwrap();
    let other = g
        .spawn("monster.ghoul_worker", Some(pos.offset(0, 1)), None, 1)
        .unwrap();
    g.spawn("item.wood", None, Some(other), 1).unwrap();
    assert!(g.craft_plan(p, &g.recipes[0]).is_err());
}
#[test]
fn tool_level_is_a_gate_but_low_skill_keeps_a_chance() {
    let mut g = arena();
    let p = g.player_id;
    let chest = storage(&mut g);
    let r = g
        .recipes
        .iter()
        .find(|r| r.id == "craft.mace")
        .unwrap()
        .clone();
    let plan = g.craft_plan(p, &r).unwrap();
    assert_eq!(plan.success_rate, 30);
    let Entity::Item(i) = g.entities.get_mut(&chest).unwrap() else {
        panic!()
    };
    for entry in &mut i.contents {
        if entry.definition_id == "item.forging_hammer" {
            entry.definition_id = "item.hammer".into();
            entry.quantity = 2;
        }
    }
    assert!(
        g.craft_plan(p, &r)
            .unwrap_err()
            .to_string()
            .contains("打撃Lv2")
    );
    g.actor_mut(p).unwrap().skill.metal_craft = 100;
    assert!(g.craft_plan(p, &r).is_err());
}
#[test]
fn disappearing_storage_pauses_without_consuming_turns_or_materials() {
    let mut g = arena();
    knife(&mut g);
    let chest = storage(&mut g);
    let p = g.player_id;
    g.start_craft(p, "craft.club").unwrap();
    g.pending_action = Some(Action::ContinueCraft);
    g.update();
    assert_eq!(g.turn, 1);
    g.move_to(p, g.position(p).unwrap().offset(-1, 0)).unwrap();
    let before = stored(&g, chest, "item.wood");
    g.pending_action = Some(Action::ContinueCraft);
    g.update();
    assert_eq!(g.turn, 1);
    assert_eq!(stored(&g, chest, "item.wood"), before);
    assert_eq!(
        g.actor(p)
            .unwrap()
            .crafting
            .as_ref()
            .unwrap()
            .completed_turns,
        1
    );
}
#[test]
fn failure_consumes_half_of_each_material_and_keeps_tools() {
    let mut g = arena();
    let chest = storage(&mut g);
    let p = g.player_id;
    let r = g
        .recipes
        .iter()
        .find(|r| r.id == "craft.mace")
        .unwrap()
        .clone();
    let plan = g.craft_plan(p, &r).unwrap();
    g.rng = Rng::new(seed_for(
        plan.success_rate,
        plan.quality_modifier,
        None,
        false,
    ));
    g.start_craft(p, &r.id).unwrap();
    finish(&mut g);
    assert_eq!(stored(&g, chest, "item.metal_stock"), 78);
    assert_eq!(stored(&g, chest, "item.wood_handle"), 19);
    assert_eq!(stored(&g, chest, "item.forging_hammer"), 1);
    assert!(
        !g.entities
            .values()
            .any(|e| matches!(e,Entity::Item(i) if i.kind=="item.mace"))
    );
}
#[test]
fn quality_names_damage_durability_and_standard_names_match() {
    for quality in [-2, -1, 0, 1, 2, 3] {
        let mut g = arena();
        knife(&mut g);
        storage(&mut g);
        let p = g.player_id;
        // +3 requires a positive modifier; +1 tool余裕で品質点を押し上げる。
        if quality == 3 {
            g.actor_mut(p).unwrap().skill.wood_craft = 1;
        }
        let plan = g.craft_plan(p, &g.recipes[0]).unwrap();
        g.rng = Rng::new(seed_for(
            plan.success_rate,
            plan.quality_modifier,
            Some(quality),
            true,
        ));
        g.start_craft(p, "craft.club").unwrap();
        finish(&mut g);
        let (id, item) = g
            .entities
            .iter()
            .find_map(|(id, e)| match e {
                Entity::Item(i) if i.kind == "item.club" => Some((*id, i)),
                _ => None,
            })
            .unwrap();
        assert_eq!(item.quality, quality);
        assert_eq!(g.weapon_damage(id).unwrap().bonus, 4 + quality);
        assert_eq!(item.max_taikyudo, (100 + quality * 10) as u32);
        if quality == 0 {
            assert_eq!(g.display_name(id), "棍棒");
        } else {
            assert!(g.display_name(id).contains(quality_title(quality)));
        }
        assert_eq!(g.entities[&id].carrier(), Some(p));
        g.drop_item(p, id).unwrap();
        g.pickup(p, id).unwrap();
        g.drop_item(p, id).unwrap();
        assert_eq!(g.weapon_damage(id).unwrap().bonus, 4 + quality);
    }
}
#[test]
fn invalid_output_does_not_consume_storage_or_rng_and_can_resume() {
    let mut g = arena();
    knife(&mut g);
    let chest = storage(&mut g);
    let p = g.player_id;
    g.start_craft(p, "craft.club").unwrap();
    assert!(!g.continue_craft(p).unwrap());
    assert!(!g.continue_craft(p).unwrap());
    let mut expected = g.rng.clone();
    g.recipes[0].outputs.push(ItemAmount {
        kind: "item.missing".into(),
        quantity: 1,
    });
    assert!(g.continue_craft(p).is_err());
    assert_eq!(stored(&g, chest, "item.wood"), 80);
    assert_eq!(g.rng.roll(100), expected.roll(100));
    g.recipes[0].outputs.pop();
    assert!(g.continue_craft(p).unwrap());
}

#[test]
fn storage_in_a_wall_and_destroyed_stored_tools_are_not_reachable() {
    let mut g = arena();
    let p = g.player_id;
    let chest = storage(&mut g);
    let position = g.position(chest).unwrap();
    let r = g
        .recipes
        .iter()
        .find(|r| r.id == "craft.mace")
        .unwrap()
        .clone();
    assert!(g.craft_plan(p, &r).is_ok());
    let wall = g
        .spawn("obstacle.stone_wall", Some(position), None, 1)
        .unwrap();
    assert!(g.craft_plan(p, &r).is_err());
    g.remove(wall);
    g.start_craft(p, &r.id).unwrap();
    let Entity::Item(i) = g.entities.get_mut(&chest).unwrap() else {
        panic!()
    };
    for entry in &mut i.contents {
        if entry.definition_id == "item.hand_drill" {
            entry.quantity = 0;
        }
    }
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
fn crafted_output_uses_inventory_and_falls_back_for_each_capacity_limit() {
    for limit in ["fits", "length", "mass", "volume"] {
        let mut g = arena();
        let tool = knife(&mut g);
        storage(&mut g);
        let p = g.player_id;
        let tool_data = g.definition(tool).item.as_ref().unwrap().clone();
        let output = g.entity_definitions["item.club"]
            .item
            .as_ref()
            .unwrap()
            .clone();
        let inventory = &mut g.actor_mut(p).unwrap().inventory;
        inventory.max_longsize = output.length_mm - u32::from(limit == "length");
        inventory.max_jyuuryou = tool_data.mass_g + output.mass_g - u32::from(limit == "mass");
        inventory.max_youryou =
            tool_data.volume_ml + output.volume_ml - u32::from(limit == "volume");
        let plan = g.craft_plan(p, &g.recipes[0]).unwrap();
        g.rng = Rng::new(seed_for(
            plan.success_rate,
            plan.quality_modifier,
            Some(0),
            true,
        ));
        g.start_craft(p, "craft.club").unwrap();
        finish(&mut g);
        let (id, item) = g
            .entities
            .iter()
            .find_map(|(id, e)| match e {
                Entity::Item(i) if i.kind == "item.club" => Some((*id, i)),
                _ => None,
            })
            .unwrap();
        if limit == "fits" {
            assert_eq!(item.location, ItemLocation::CarriedBy(p));
            assert!(g.actor(p).unwrap().inventory.items.contains(&id));
            assert!(!g.map.entities_at(g.position(p).unwrap()).contains(&id));
        } else {
            assert_eq!(item.location, ItemLocation::Ground(g.position(p).unwrap()));
            assert!(g.map.entities_at(g.position(p).unwrap()).contains(&id));
            assert!(!g.actor(p).unwrap().inventory.items.contains(&id));
        }
        assert_eq!(item.quality, 0);
        assert!(g.actor(p).unwrap().crafting.is_none());
        assert_eq!(g.turn, 0); // 完成品の受け取り自体では追加の時間を使わない。
    }
}
#[test]
fn batch_output_puts_only_the_fitting_quantity_in_inventory() {
    for limit in ["mass", "volume"] {
        let mut g = arena();
        let tool = knife(&mut g);
        storage(&mut g);
        let p = g.player_id;
        let tool_data = g.definition(tool).item.as_ref().unwrap().clone();
        let output = g.entity_definitions["item.bandage"]
            .item
            .as_ref()
            .unwrap()
            .clone();
        let index = g
            .recipes
            .iter()
            .position(|r| r.id == "craft.bandage")
            .unwrap();
        g.recipes[index].outputs[0].quantity = 5;
        let inventory = &mut g.actor_mut(p).unwrap().inventory;
        if limit == "mass" {
            inventory.max_jyuuryou = tool_data.mass_g + output.mass_g * 2;
        } else {
            inventory.max_youryou = tool_data.volume_ml + output.volume_ml * 2;
        }
        let plan = g.craft_plan(p, &g.recipes[index]).unwrap();
        g.rng = Rng::new(seed_for(
            plan.success_rate,
            plan.quality_modifier,
            None,
            true,
        ));
        g.start_craft(p, "craft.bandage").unwrap();
        finish(&mut g);
        let mut carried = 0;
        let mut ground = 0;
        for e in g.entities.values() {
            if let Entity::Item(i) = e
                && i.kind == "item.bandage"
            {
                match i.location {
                    ItemLocation::CarriedBy(actor) if actor == p => carried += i.quantity,
                    ItemLocation::Ground(position) if Some(position) == g.position(p) => {
                        ground += i.quantity
                    }
                    _ => panic!("unexpected output location"),
                }
            }
        }
        assert_eq!((carried, ground), (2, 3));
    }
}
#[test]
fn consuming_carried_material_frees_capacity_before_receiving_output() {
    let mut g = arena();
    knife(&mut g);
    let p = g.player_id;
    g.spawn("item.cloth", None, Some(p), 1).unwrap();
    let mass: u32 = g
        .actor(p)
        .unwrap()
        .inventory
        .items
        .iter()
        .map(|id| g.definition(*id).item.as_ref().unwrap().mass_g)
        .sum();
    g.actor_mut(p).unwrap().inventory.max_jyuuryou = mass;
    let recipe = g.recipes.iter().find(|r| r.id == "craft.bandage").unwrap();
    let plan = g.craft_plan(p, recipe).unwrap();
    g.rng = Rng::new(seed_for(
        plan.success_rate,
        plan.quality_modifier,
        None,
        true,
    ));
    g.start_craft(p, "craft.bandage").unwrap();
    finish(&mut g);
    assert!(g.entities.values().any(|e|matches!(e,Entity::Item(i) if i.kind=="item.bandage" && i.quantity==2 && i.location==ItemLocation::CarriedBy(p))));
}
