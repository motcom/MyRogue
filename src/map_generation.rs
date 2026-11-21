//! Convert shared Map Gen output to terrain using scenario-owned Lua rules.
use crate::model::{config::*, map::Map};
use std::{collections::BTreeMap, path::Path};

pub(crate) fn validate_debug_output(
    image: &map_gen::graph::Image,
    output: map_gen::graph::NodeId,
    path: &str,
) -> Result<()> {
    ensure(
        image.terrain.is_some(),
        format!(
            "選択したJSONの最終出力（ノード #{output}）に地形IDがありません。Map GenでTerrain Palette / Terrain MergeにつながるOutputノードを選び、「このノードを最終出力にする」で設定して保存してください: {path}"
        ),
    )
}

pub(crate) fn terrain_spawns(
    root: &Path,
    config: &MapGenerator,
    map: &Map,
    seed: u64,
    definitions: &BTreeMap<String, Definition>,
) -> Result<Vec<Spawn>> {
    let root = root.canonicalize()?;
    let path = root.join(&config.json).canonicalize()?;
    ensure(
        path.starts_with(&root),
        "map JSON path escaped project root",
    )?;
    let graph = map_gen::load(&path)?;
    let image = if let Some(coord) = config.tile {
        ensure(
            map.width as u32 == graph.resolution && map.height as u32 == graph.resolution,
            "タイル接続ではシナリオの幅・高さとJSONのresolutionを一致させてください",
        )?;
        map_gen::world::generate_tile(&graph, seed, &config.inputs, coord)?
    } else {
        map_gen::generate(&graph, seed, &config.inputs)?
    };
    if let Some(palette) = &image.terrain {
        ensure(
            config.terrain.is_empty(),
            "Terrain Palette付きJSONではLuaのterrain指定を省略してください",
        )?;
        ensure(
            config
                .ids
                .keys()
                .all(|id| palette.bands.iter().any(|b| &b.id == id)),
            "unknown terrain ID alias",
        )?;
        for band in &palette.bands {
            let id = config.ids.get(&band.id).unwrap_or(&band.id);
            ensure(
                definitions
                    .get(id)
                    .is_some_and(|d| matches!(d.category.as_str(), "map" | "static")),
                format!("unknown or non-map/static terrain ID: {id}"),
            )?;
        }
    } else {
        ensure(
            config.ids.is_empty(),
            "ID aliases require a Terrain Palette node",
        )?;
        ensure(
            !config.terrain.is_empty() && config.terrain.len() <= 32,
            "terrain requires 1..32 bands",
        )?;
        let mut previous = -1.0;
        for band in &config.terrain {
            ensure(
                band.max.is_finite() && (0.0..=1.0).contains(&band.max) && band.max > previous,
                "terrain bounds must increase within 0..1",
            )?;
            ensure(
                definitions
                    .get(&band.definition_id)
                    .is_some_and(|d| matches!(d.category.as_str(), "map" | "static")),
                format!(
                    "map terrain must reference a map/static definition: {}",
                    band.definition_id
                ),
            )?;
            previous = band.max;
        }
        ensure(previous == 1.0, "last terrain bound must be 1")?;
    }
    let mut spawns = Vec::new();
    // Sample cell centers across the complete image. Scenario boundaries stay solid.
    let margin = if config.tile.is_some() { 0 } else { 1 };
    for y in margin..map.height - margin {
        for x in margin..map.width - margin {
            let sx = (2 * x as usize + 1) * image.size / (2 * map.width as usize);
            let sy = (2 * y as usize + 1) * image.size / (2 * map.height as usize);
            let index = sy * image.size + sx;
            if image.alpha.as_ref().is_some_and(|alpha| alpha[index] == 0) {
                continue;
            }
            let value = image.pixels[index];
            let definition_id = if let Some(palette) = &image.terrain {
                let id = &palette.band(value).id;
                config.ids.get(id).unwrap_or(id).clone()
            } else {
                config
                    .terrain
                    .iter()
                    .find(|b| value <= b.max)
                    .ok_or("generated terrain value is outside 0..1")?
                    .definition_id
                    .clone()
            };
            spawns.push(Spawn {
                definition_id,
                position: Pos { x, y, z: 0 },
                quantity: 1,
            });
        }
    }
    Ok(spawns)
}

/// Initial population only: independent coordinate hashes keep combat RNG untouched.
impl crate::game::Game {
    pub(crate) fn populate_map(&mut self, seed: u64, player: Pos) -> Result<()> {
        use crate::model::entity::Entity;
        let bases: Vec<_> = self
            .entities
            .values()
            .filter_map(|e| {
                if let Entity::Map(base) = e {
                    Some((base.position, base.script.definition_id.clone()))
                } else {
                    None
                }
            })
            .collect();
        for (position, definition_id) in bases {
            if position.distance(player) <= 2 || !self.can_enter(position, 0) {
                continue;
            }
            // Explicit map-gen/scenario objects reserve their cells, including furniture.
            if self
                .at(position)
                .iter()
                .any(|id| !matches!(self.entities[id], Entity::Map(_)))
            {
                continue;
            }
            let rules = self.entity_definitions[&definition_id]
                .map
                .as_ref()
                .unwrap()
                .spawns
                .clone();
            for (index, rule) in rules.iter().enumerate() {
                let key = format!(
                    "{definition_id}:{}:{}:{}:{index}",
                    position.x, position.y, position.z
                );
                let raw = map_gen::recipe::derive_seed(seed ^ 0x504F_5055, &key);
                let unit = (raw >> 11) as f64 / (1u64 << 53) as f64;
                if unit < rule.chance {
                    self.spawn(&rule.definition_id, Some(position), None, rule.quantity)?;
                    if let Some(item_id) = &rule.with_item {
                        self.spawn(item_id, Some(position), None, 1)?;
                    }
                    break;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{game::Game, scripting::LuaRuntime};

    #[test]
    fn map_bases_replace_each_other_without_spawning_or_disturbing_objects() {
        use crate::model::entity::Entity;
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut game = Game::load(root).unwrap();
        let p = game.position(game.player_id).unwrap();
        let old = game
            .at(p)
            .into_iter()
            .find(|id| matches!(game.entities[id], Entity::Map(_)))
            .unwrap();
        let chair = game.spawn("obstacle.chair", Some(p), None, 1).unwrap();
        game.events();
        let road = game.set_map_entity("terrain.road", p).unwrap();
        assert!(!game.entities.contains_key(&old));
        assert!(game.at(p).contains(&chair));
        assert!(game.at(p).contains(&game.player_id));
        assert_eq!(
            game.at(p)
                .iter()
                .filter(|id| matches!(game.entities[id], Entity::Map(_)))
                .count(),
            1
        );
        assert_eq!(game.entity_snapshot(road).unwrap()["category"], "map");
        assert!(game.spawn("terrain.field", Some(p), None, 1).is_err());
        assert!(game.set_map_entity("obstacle.tree", p).is_err());
        game.set_map_entity("terrain.pond", p).unwrap();
        assert!(!game.can_enter(p, game.player_id));
        game.set_map_entity("terrain.field", p).unwrap();
        game.remove(chair);
        assert_eq!(game.movement_cost(p), Some(2.0));
    }

    #[test]
    fn population_uses_lua_rules_reserves_explicit_cells_and_preserves_combat_rng() {
        use crate::model::entity::Entity;
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut game = Game::load(root).unwrap();
        let player = game.position(game.player_id).unwrap();
        let removed: Vec<_> = game
            .entities
            .iter()
            .filter_map(|(id, e)| {
                (*id != game.player_id && !matches!(e, Entity::Map(_))).then_some(*id)
            })
            .collect();
        for id in removed {
            game.remove(id);
        }
        for y in 0..game.map.height {
            for x in 0..game.map.width {
                game.set_map_entity("terrain.field", Pos { x, y, z: 0 })
                    .unwrap();
            }
        }
        let map = std::sync::Arc::make_mut(&mut game.entity_definitions)
            .get_mut("terrain.field")
            .unwrap()
            .map
            .as_mut()
            .unwrap();
        let mut chief_rule = map
            .spawns
            .iter()
            .find(|rule| rule.definition_id == "monster.goblin_chief")
            .unwrap()
            .clone();
        chief_rule.chance = 1.0;
        map.spawns = vec![chief_rule];
        let chair_pos = Pos { x: 20, y: 20, z: 0 };
        let chair = game
            .spawn("obstacle.chair", Some(chair_pos), None, 1)
            .unwrap();
        let road = chair_pos.offset(1, 0);
        game.set_map_entity("terrain.road", road).unwrap();
        let mut expected_rng = game.rng.clone();
        let mut same = game.clone();
        game.populate_map(42, player).unwrap();
        same.populate_map(42, player).unwrap();
        let positions = |g: &Game, definition_id: &str| {
            g.entities
                .values()
                .filter(|e| e.script().definition_id == definition_id)
                .map(|e| e.map_position().unwrap())
                .collect::<Vec<_>>()
        };
        let chiefs = positions(&game, "monster.goblin_chief");
        assert_eq!(chiefs, positions(&same, "monster.goblin_chief"));
        assert_eq!(chiefs, positions(&game, "item.treasure_chest"));
        assert!(!chiefs.is_empty());
        assert!(
            chiefs
                .iter()
                .all(|p| p.distance(player) > 2 && *p != chair_pos && *p != road)
        );
        assert!(game.entities.contains_key(&chair));
        assert_eq!(game.rng.roll(1000), expected_rng.roll(1000));
    }

    #[test]
    fn spawned_destructibles_restore_the_declared_map_base_once() {
        use crate::model::entity::Entity;
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut game = Game::load(root).unwrap();
        let p = Pos { x: 10, y: 20, z: 0 };
        for (definition, expected) in [
            ("obstacle.tree", "terrain.field"),
            ("obstacle.conifer", "terrain.wasteland"),
        ] {
            game.set_map_entity("terrain.road", p).unwrap();
            let tree = game.spawn(definition, Some(p), None, 1).unwrap();
            game.damage(game.player_id, tree, 9999).unwrap();
            game.events();
            assert!(!game.entities.contains_key(&tree));
            let bases: Vec<_> = game
                .at(p)
                .into_iter()
                .filter(|id| matches!(game.entities[id], Entity::Map(_)))
                .collect();
            assert_eq!(bases.len(), 1);
            assert_eq!(game.definition(bases[0]).id, expected);
            assert!(
                game.at(p)
                    .iter()
                    .any(|id| game.definition(*id).id == "item.wood")
            );
            game.events();
            assert_eq!(
                game.at(p)
                    .iter()
                    .filter(|id| matches!(game.entities[id], Entity::Map(_)))
                    .count(),
                1
            );
        }
        game.set_map_entity("terrain.road", p).unwrap();
        let wall = game
            .spawn("obstacle.wooden_wall", Some(p), None, 1)
            .unwrap();
        game.damage(game.player_id, wall, 9999).unwrap();
        game.events();
        assert!(
            game.at(p)
                .iter()
                .any(|id| game.definition(*id).id == "terrain.road")
        );
    }

    #[test]
    fn forest_house_scenario_generates_structures_and_populates_fields() {
        use crate::model::entity::Entity;
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut game = Game::load_scenario(root, "data/scenarios/forest_houses.lua").unwrap();
        let player = game.position(game.player_id).unwrap();
        assert!(game.can_enter(player, game.player_id));
        let ids: std::collections::BTreeSet<_> = game
            .entities
            .values()
            .map(|e| e.script().definition_id.as_str())
            .collect();
        for id in [
            "obstacle.tree",
            "obstacle.conifer",
            "obstacle.wooden_wall",
            "obstacle.wooden_door",
            "obstacle.table",
            "obstacle.chair",
        ] {
            assert!(ids.contains(id), "missing {id}");
        }
        let goblins: Vec<_> = game
            .entities
            .iter()
            .filter(|(_, entity)| entity.script().definition_id.starts_with("monster.goblin_"))
            .map(|(id, _)| game.position(*id).unwrap())
            .collect();
        assert!(!goblins.is_empty());
        for position in goblins {
            assert!(position.distance(player) > 2);
            assert!(
                game.at(position)
                    .iter()
                    .any(|id| game.definition(*id).id == "terrain.field")
            );
        }
        for y in 0..64 {
            for x in 0..64 {
                let at = game.at(Pos { x, y, z: 0 });
                assert_eq!(
                    at.iter()
                        .filter(|id| matches!(game.entities[id], Entity::Map(_)))
                        .count(),
                    1
                );
                assert!(
                    at.iter()
                        .filter(|id| game.entities[id].blocks_movement())
                        .count()
                        <= 1
                );
            }
        }
        let first_new_id = game.next_id;
        for _ in 0..10 {
            game.advance_turn();
        }
        // Ammunition and loot can change during combat. Terrain and monsters populate only at load.
        assert!(
            game.entities
                .range(first_new_id..)
                .all(|(_, entity)| matches!(entity, Entity::Item(_)))
        );
    }

    #[test]
    fn cave_json_uses_merged_terrain_and_spawns_goblins_on_fields() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let path = "data/map_json/doukutu.json";
        let graph = map_gen::load(&root.join(path)).unwrap();
        let image = map_gen::generate(&graph, 42, &BTreeMap::new()).unwrap();
        let palette = image.terrain.as_ref().unwrap();
        assert!(
            palette
                .bands
                .iter()
                .any(|band| band.id == "obstacle.conifer")
        );
        let game = Game::load_map_json(root, path).unwrap();
        assert_eq!((game.map.width, game.map.height), (64, 64));
        let player = game.position(game.player_id).unwrap();
        assert!(game.can_enter(player, game.player_id));
        let goblins: Vec<_> = game
            .entities
            .values()
            .filter(|entity| entity.script().definition_id.starts_with("monster.goblin_"))
            .collect();
        assert!(!goblins.is_empty());
        for entity in goblins {
            let position = entity.map_position().unwrap();
            assert!(position.distance(player) > 2);
            assert!(
                game.at(position)
                    .iter()
                    .any(|id| game.definition(*id).id == "terrain.field")
            );
        }
    }

    #[test]
    fn debug_json_without_terrain_reports_output_setting() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let error = Game::load_map_json(root, "tool/map_gen/examples/recipe_inputs.json")
            .err()
            .expect("numeric output must be rejected")
            .to_string();
        assert!(error.contains("最終出力"), "{error}");
        assert!(error.contains("このノードを最終出力にする"), "{error}");
    }

    #[test]
    fn debug_json_preserves_full_resolution_terrain_and_finds_safe_spawn() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let path = "data/map_json/shape_tiles.json";
        let game = Game::load_map_json(root, path).unwrap();
        let graph = map_gen::load(&root.join(path)).unwrap();
        let size = graph.resolution as i32;
        assert_eq!((game.map.width, game.map.height), (size, size));
        let image = map_gen::generate(&graph, 42, &BTreeMap::new()).unwrap();
        let palette = image.terrain.as_ref().unwrap();
        let player = game.position(game.player_id).unwrap();
        assert!(game.can_enter(player, game.player_id));
        let distance = |p: Pos| (p.x - size / 2).pow(2) + (p.y - size / 2).pow(2);
        for y in 1..size - 1 {
            for x in 1..size - 1 {
                let p = Pos { x, y, z: 0 };
                let floor = palette.band(image.pixels[(y * size + x) as usize]).id == "stone_floor";
                let wall = game
                    .at(p)
                    .iter()
                    .any(|id| game.definition(*id).id == "obstacle.stone_wall");
                assert_eq!(!wall, floor, "{x},{y}");
                if floor {
                    assert!(distance(p) >= distance(player));
                }
            }
        }
        let camera = game.map_camera();
        assert_eq!((camera.columns, camera.rows), (size.min(70), size.min(38)));
        assert!(player.x >= camera.origin.x && player.x < camera.origin.x + camera.columns);
        assert!(player.y >= camera.origin.y && player.y < camera.origin.y + camera.rows);
        assert!(game.map_draw_position(player).is_some());
    }

    #[test]
    fn tile_scenario_uses_stitched_ids_including_unsealed_edges() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let runtime = LuaRuntime::load(root).unwrap();
        let game = Game::load_scenario(root, "data/scenarios/stitched_tiles.lua").unwrap();
        let mut config = runtime
            .scenario_file("data/scenarios/stitched_tiles.lua")
            .unwrap()
            .map_generator
            .unwrap();
        let graph = map_gen::load(&root.join(&config.json)).unwrap();
        for coordinate in [[0, 0], [1, 0], [-1, 1]] {
            config.tile = Some(coordinate);
            let image =
                map_gen::world::generate_tile(&graph, 42, &config.inputs, coordinate).unwrap();
            let spawns =
                terrain_spawns(root, &config, &game.map, 42, &runtime.definitions).unwrap();
            assert_eq!(spawns.len(), 32 * 32);
            for spawn in spawns {
                let index = (spawn.position.y * 32 + spawn.position.x) as usize;
                assert_eq!(
                    spawn.definition_id,
                    image.terrain.as_ref().unwrap().band(image.pixels[index]).id
                );
                if coordinate == [0, 0] {
                    assert!(
                        game.at(spawn.position)
                            .iter()
                            .any(|id| game.entities[id].script().definition_id
                                == spawn.definition_id)
                    );
                    if spawn.definition_id == "terrain.road" {
                        assert!(game.can_enter(spawn.position, game.player_id));
                    }
                }
            }
        }
        assert!(
            terrain_spawns(
                root,
                &config,
                &Map::new(33, 32, 1).unwrap(),
                42,
                &runtime.definitions
            )
            .is_err()
        );
    }

    #[test]
    fn multiple_palette_ids_generate_the_same_game_map_as_one_palette() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let game = Game::load_scenario(root, "data/scenarios/terrain_merge.lua").unwrap();
        let runtime = LuaRuntime::load(root).unwrap();
        let config = runtime
            .scenario_file("data/scenarios/terrain_merge.lua")
            .unwrap()
            .map_generator
            .unwrap();
        let spawns = terrain_spawns(root, &config, &game.map, 42, &runtime.definitions).unwrap();
        let reference = map_gen::generate(
            &map_gen::load(&root.join("tool/map_gen/examples/game_map_ids.json")).unwrap(),
            42,
            &BTreeMap::new(),
        )
        .unwrap();
        for spawn in spawns {
            let index = (spawn.position.y * 32 + spawn.position.x) as usize;
            assert_eq!(
                spawn.definition_id,
                reference
                    .terrain
                    .as_ref()
                    .unwrap()
                    .band(reference.pixels[index])
                    .id
            );
            assert!(
                game.at(spawn.position)
                    .iter()
                    .any(|id| game.entities[id].script().definition_id == spawn.definition_id)
            );
        }
    }

    #[test]
    fn shared_json_generates_game_terrain_and_preserves_boundary_and_spawn() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let game = Game::load_scenario(root, "data/scenarios/map_gen.lua").unwrap();
        let graph = map_gen::load(&root.join("tool/map_gen/examples/game_map.json")).unwrap();
        let image = map_gen::generate(&graph, 42, &BTreeMap::new()).unwrap();
        for y in 0..32 {
            for x in 0..32 {
                let p = Pos { x, y, z: 0 };
                let wall = x == 0
                    || y == 0
                    || x == 31
                    || y == 31
                    || image.pixels[(y * 32 + x) as usize] <= 0.5;
                assert_eq!(game.can_enter(p, game.player_id), !wall);
            }
        }
        assert_eq!(
            game.position(game.player_id),
            Some(Pos { x: 16, y: 16, z: 0 })
        );
    }

    #[test]
    fn json_ids_drive_terrain_without_lua_thresholds_and_aliases_are_checked() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let runtime = LuaRuntime::load(root).unwrap();
        let mut config = runtime
            .scenario_file("data/scenarios/map_gen_ids.lua")
            .unwrap()
            .map_generator
            .unwrap();
        let map = Map::new(32, 32, 1).unwrap();
        let generate =
            |config: &MapGenerator| terrain_spawns(root, config, &map, 42, &runtime.definitions);
        let a = generate(&config).unwrap();
        assert!(a.iter().any(|s| s.definition_id == "terrain.road"));
        assert!(a.iter().any(|s| s.definition_id == "obstacle.stone_wall"));
        let game = Game::load_scenario(root, "data/scenarios/map_gen_ids.lua").unwrap();
        assert_eq!(
            game.position(game.player_id),
            Some(Pos { x: 16, y: 16, z: 0 })
        );
        let mut graph = map_gen::load(&root.join(&config.json)).unwrap();
        let before = map_gen::generate(&graph, 42, &BTreeMap::new()).unwrap();
        for node in &mut graph.nodes {
            if let map_gen::graph::NodeKind::Terrain(palette) = &mut node.kind {
                for band in &mut palette.bands {
                    band.color = [0, 0, 0];
                }
            }
        }
        let after = map_gen::generate(&graph, 42, &BTreeMap::new()).unwrap();
        assert_eq!(before.pixels, after.pixels);
        for (a, b) in before.pixels.iter().zip(&after.pixels) {
            assert_eq!(
                before.terrain.as_ref().unwrap().band(*a).id,
                after.terrain.as_ref().unwrap().band(*b).id
            );
        }
        config
            .ids
            .insert("terrain.road".into(), "terrain.field".into());
        assert!(
            generate(&config)
                .unwrap()
                .iter()
                .all(|s| s.definition_id != "terrain.road")
        );
        config
            .ids
            .insert("terrain.road".into(), "player.survivor".into());
        assert!(generate(&config).is_err());
        config.ids.insert("terrain.road".into(), "missing".into());
        assert!(generate(&config).is_err());
        config.ids.clear();
        config.ids.insert("unknown".into(), "terrain.field".into());
        assert!(generate(&config).is_err());
    }

    #[test]
    fn partition_sample_creates_real_door_entities_from_json_ids() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let game = Game::load_scenario(root, "data/scenarios/partition_doors.lua").unwrap();
        let doors: Vec<_> = game
            .entities
            .iter()
            .filter(|(_, entity)| entity.script().definition_id == "obstacle.wooden_door")
            .collect();
        assert_eq!(doors.len(), 8);
        assert!(
            doors
                .iter()
                .all(|(id, _)| !game.can_enter(game.position(**id).unwrap(), game.player_id))
        );
        assert!(game.can_enter(game.position(game.player_id).unwrap(), game.player_id));
    }

    #[test]
    fn transparent_cells_skip_terrain_placement() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let runtime = LuaRuntime::load(root).unwrap();
        let mut config = runtime
            .scenario_file("data/scenarios/map_gen.lua")
            .unwrap()
            .map_generator
            .unwrap();
        config.json = "tool/map_gen/examples/black_to_alpha.json".into();
        let spawns = terrain_spawns(
            root,
            &config,
            &Map::new(32, 32, 1).unwrap(),
            42,
            &runtime.definitions,
        )
        .unwrap();
        assert!(!spawns.is_empty() && spawns.len() < 900);
        assert!(
            spawns
                .iter()
                .all(|spawn| spawn.definition_id == "terrain.road")
        );
    }

    #[test]
    fn lua_mapping_and_runtime_inputs_are_validated() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let runtime = LuaRuntime::load(root).unwrap();
        let mut config = runtime
            .scenario_file("data/scenarios/map_gen.lua")
            .unwrap()
            .map_generator
            .unwrap();
        let map = Map::new(37, 21, 1).unwrap();
        let generate =
            |config: &MapGenerator| terrain_spawns(root, config, &map, 42, &runtime.definitions);
        let a = generate(&config).unwrap();
        let b = generate(&config).unwrap();
        assert_eq!(a.len(), 35 * 19);
        assert_eq!(
            a.iter()
                .map(|s| (&s.definition_id, s.position))
                .collect::<Vec<_>>(),
            b.iter()
                .map(|s| (&s.definition_id, s.position))
                .collect::<Vec<_>>()
        );
        config.terrain[0].definition_id = "missing".into();
        assert!(generate(&config).is_err());
        config.terrain[0].definition_id = "player.survivor".into();
        assert!(generate(&config).is_err());
        config.terrain[0].definition_id = "obstacle.stone_wall".into();
        config.terrain[1].max = 0.4;
        assert!(generate(&config).is_err());
        config.terrain[1].max = 1.0;
        config.inputs.insert("unknown".into(), 1.0);
        assert!(generate(&config).is_err());
        config.inputs.clear();
        config.json = "../missing.json".into();
        assert!(generate(&config).is_err());
    }
}
