//! Convert shared Map Gen output to terrain using scenario-owned Lua rules.
use crate::model::{config::*, map::Map};
use std::{collections::BTreeMap, path::Path};

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
                definitions.get(id).is_some_and(|d| d.category == "static"),
                format!("unknown or non-static terrain ID: {id}"),
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
                    .is_some_and(|d| d.category == "static"),
                format!(
                    "map terrain must reference a static definition: {}",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{game::Game, scripting::LuaRuntime};

    #[test]
    fn debug_json_preserves_full_resolution_terrain_and_finds_safe_spawn() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let path = "data/map_json/shape_tiles.json";
        let game = Game::load_map_json(root, path).unwrap();
        assert_eq!((game.map.width, game.map.height), (256, 256));
        let graph = map_gen::load(&root.join(path)).unwrap();
        let image = map_gen::generate(&graph, 42, &BTreeMap::new()).unwrap();
        let palette = image.terrain.as_ref().unwrap();
        for y in 1..255 {
            for x in 1..255 {
                let p = Pos { x, y, z: 0 };
                let floor = palette.band(image.pixels[(y * 256 + x) as usize]).id == "stone_floor";
                assert_eq!(game.can_enter(p, game.player_id), floor, "{x},{y}");
            }
        }
        let player = game.position(game.player_id).unwrap();
        assert!(game.can_enter(player, game.player_id));
        let distance = |p: Pos| (p.x - 128).pow(2) + (p.y - 128).pow(2);
        assert!((1..255).all(|y| (1..255).all(|x| {
            let p = Pos { x, y, z: 0 };
            !game.can_enter(p, game.player_id) || distance(p) >= distance(player)
        })));
        let camera = game.map_camera();
        assert_eq!((camera.columns, camera.rows), (70, 38));
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
