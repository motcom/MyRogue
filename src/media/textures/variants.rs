//! Data-driven texture variants and cardinal road connections, independent of combat RNG.
use super::*;
use crate::{
    model::config::Pos,
    model::entity::{Entity, StaticRole},
};

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileRules {
    #[serde(default)]
    seed: u64,
    sets: BTreeMap<String, FileSet>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileSet {
    #[serde(default)]
    connect_group: Option<String>,
    #[serde(default)]
    background: Option<String>,
    variants: Vec<String>,
}
#[derive(Clone, Default)]
pub(super) struct TextureRules {
    seed: u64,
    sets: BTreeMap<String, TileSet>,
}
#[derive(Clone)]
struct TileSet {
    connect_group: Option<String>,
    // Each variant contains 256 neighbor combinations, or one non-connecting tile.
    indices: Vec<Vec<usize>>,
}

fn read_tile(directory: &Path, file: &str, allowed: &BTreeSet<[u8; 3]>) -> Result<RgbaImage> {
    let path = Path::new(file);
    ensure(
        !path.is_absolute()
            && path
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_))),
        "Invalid variant texture path",
    )?;
    let tile = image::open(directory.join(path))?.into_rgba8();
    ensure(
        tile.dimensions() == (TILE, TILE),
        "Variant texture must be 32x32",
    )?;
    validate_pixels(&tile, allowed)?;
    Ok(tile)
}

// N, E, S, W, NE, SE, SW, NW. Diagonals fill inside corners in wide roads/plazas.
const NEIGHBORS: [(i32, i32); 8] = [
    (0, -1),
    (1, 0),
    (0, 1),
    (-1, 0),
    (1, -1),
    (1, 1),
    (-1, 1),
    (-1, -1),
];

fn connected_tile(surface: &RgbaImage, background: &RgbaImage, mask: usize) -> RgbaImage {
    let mut tile = background.clone();
    for y in 0..TILE {
        for x in 0..TILE {
            let center_x = (8..24).contains(&x);
            let center_y = (8..24).contains(&y);
            let road = (center_x && center_y)
                || (center_x && y < 8 && mask & 1 != 0)
                || (x >= 24 && center_y && mask & 2 != 0)
                || (center_x && y >= 24 && mask & 4 != 0)
                || (x < 8 && center_y && mask & 8 != 0)
                || (x >= 24 && y < 8 && mask & 19 == 19)
                || (x >= 24 && y >= 24 && mask & 38 == 38)
                || (x < 8 && y >= 24 && mask & 76 == 76)
                || (x < 8 && y < 8 && mask & 137 == 137);
            if road {
                tile.put_pixel(x, y, *surface.get_pixel(x, y));
            }
        }
    }
    tile
}

fn variant_index(seed: u64, role: &str, p: Pos, count: usize) -> usize {
    let mut h = seed ^ 0xcbf29ce484222325;
    for byte in role
        .bytes()
        .chain(p.x.to_le_bytes())
        .chain(p.y.to_le_bytes())
        .chain(p.z.to_le_bytes())
    {
        h = (h ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    h ^= h >> 30;
    h = h.wrapping_mul(0xbf58476d1ce4e5b9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94d049bb133111eb);
    h ^= h >> 31;
    (h % count as u64) as usize
}

impl TextureRules {
    pub(super) fn load(
        directory: &Path,
        allowed: &BTreeSet<[u8; 3]>,
        base: &BTreeMap<String, usize>,
        image: &mut RgbaImage,
        start: usize,
    ) -> Result<Self> {
        let path = directory.join("texture_rules.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let file: FileRules = serde_json::from_slice(&std::fs::read(path)?)?;
        let mut rules = Self {
            seed: file.seed,
            sets: BTreeMap::new(),
        };
        let mut tiles = Vec::new();
        for (id, set) in file.sets {
            ensure(
                base.contains_key(&id),
                format!("Unknown texture rule ID: {id}"),
            )?;
            ensure(
                !set.variants.is_empty() && set.variants.len() <= 64,
                "Texture sets need 1..64 variants",
            )?;
            ensure(
                set.connect_group
                    .as_ref()
                    .is_none_or(|s| !s.trim().is_empty()),
                "Empty connection group",
            )?;
            ensure(
                set.connect_group.is_some() == set.background.is_some(),
                "Connecting textures require a background",
            )?;
            let background = if let Some(id) = &set.background {
                let index = base.get(id).ok_or("Unknown connection background ID")?;
                Some(
                    image::imageops::crop_imm(
                        image,
                        *index as u32 % COLUMNS * TILE,
                        *index as u32 / COLUMNS * TILE,
                        TILE,
                        TILE,
                    )
                    .to_image(),
                )
            } else {
                None
            };
            let mut indices = Vec::new();
            for path in set.variants {
                let tile = read_tile(directory, &path, allowed)?;
                let generated = if let Some(bg) = &background {
                    ensure(
                        tile.pixels().chain(bg.pixels()).all(|p| p[3] == 255),
                        "Road surfaces/backgrounds must be opaque",
                    )?;
                    (0..256)
                        .map(|mask| connected_tile(&tile, bg, mask))
                        .collect::<Vec<_>>()
                } else {
                    vec![tile]
                };
                let first = start + tiles.len();
                indices.push((first..first + generated.len()).collect());
                tiles.extend(generated);
                ensure(
                    start + tiles.len() <= 8192,
                    "Texture variant atlas exceeds 8192 cells",
                )?;
            }
            rules.sets.insert(
                id,
                TileSet {
                    connect_group: set.connect_group,
                    indices,
                },
            );
        }
        let height = ((start + tiles.len()) as u32).div_ceil(COLUMNS) * TILE;
        if height > image.height() {
            let mut expanded = RgbaImage::new(COLUMNS * TILE, height);
            image::imageops::replace(&mut expanded, image, 0, 0);
            *image = expanded;
        }
        for (i, tile) in tiles.iter().enumerate() {
            place(image, tile, start + i);
        }
        Ok(rules)
    }

    fn connection_mask(&self, game: &Game, p: Pos, group: &str) -> usize {
        NEIGHBORS
            .iter()
            .enumerate()
            .fold(0, |mask, (bit, &(dx, dy))| {
                let terrain = game
                    .at(p.offset(dx, dy))
                    .into_iter()
                    .filter_map(|id| {
                        let entity = &game.entities[&id];
                        if matches!(entity, Entity::Static(s) if matches!(s.role, StaticRole::Terrain)) {
                            Some(entity)
                        } else {
                            None
                        }
                    })
                    .max_by_key(|e| (game.definition(e.id()).common.appearance.layer, e.id()));
                let same = terrain
                    .and_then(|e| self.sets.get(&e.script().definition_id))
                    .and_then(|s| s.connect_group.as_deref())
                    == Some(group);
                if same { mask | (1 << bit) } else { mask }
            })
    }

    pub(super) fn index(&self, game: &Game, id: &str, p: Pos) -> Option<usize> {
        let set = self.sets.get(id)?;
        let role = set.connect_group.as_deref().unwrap_or(id);
        let variant = variant_index(self.seed, role, p, set.indices.len());
        let mask = set
            .connect_group
            .as_ref()
            .map_or(0, |g| self.connection_mask(game, p, g));
        Some(set.indices[variant][mask])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_connection_mask_opens_only_the_required_edges_and_plazas_are_solid() {
        let road = RgbaImage::from_pixel(TILE, TILE, Rgba([48, 42, 36, 255]));
        let field = RgbaImage::from_pixel(TILE, TILE, Rgba([30, 48, 42, 255]));
        for mask in 0..256 {
            let tile = connected_tile(&road, &field, mask);
            for (bit, x, y) in [(1, 16, 0), (2, 31, 16), (4, 16, 31), (8, 0, 16)] {
                assert_eq!(
                    tile.get_pixel(x, y),
                    if mask & bit != 0 {
                        road.get_pixel(x, y)
                    } else {
                        field.get_pixel(x, y)
                    },
                    "mask {mask}"
                );
            }
            assert_eq!(tile.get_pixel(16, 16), road.get_pixel(16, 16));
            assert!(tile.pixels().all(|p| p[3] == 255));
        }
        assert_eq!(connected_tile(&road, &field, 255), road);
    }

    #[test]
    fn neighbors_change_road_orientation_and_overlaid_water_breaks_the_connection() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut game = Game::load(root).unwrap();
        let catalog = TextureCatalog::load(root).unwrap();
        let p = Pos { x: 10, y: 20, z: 0 };
        let directions = NEIGHBORS;
        for (dx, dy) in directions {
            game.spawn("terrain.field", Some(p.offset(dx, dy)), None, 1)
                .unwrap();
        }
        game.spawn("terrain.road", Some(p), None, 1).unwrap();
        assert_eq!(catalog.rules.connection_mask(&game, p, "road"), 0);
        game.spawn("terrain.road", Some(p.offset(-1, 0)), None, 1)
            .unwrap();
        game.spawn("terrain.road", Some(p.offset(1, 0)), None, 1)
            .unwrap();
        assert_eq!(catalog.rules.connection_mask(&game, p, "road"), 10);
        let horizontal = catalog.rules.index(&game, "terrain.road", p).unwrap();
        game.spawn("terrain.pond", Some(p.offset(1, 0)), None, 1)
            .unwrap();
        assert_eq!(catalog.rules.connection_mask(&game, p, "road"), 8);
        assert_ne!(
            horizontal,
            catalog.rules.index(&game, "terrain.road", p).unwrap()
        );

        // Visual QA artifact uses the actual runtime atlas, not a separate implementation.
        let mut preview = RgbaImage::new(TILE * 4, TILE * 4);
        let tiles = &catalog.rules.sets["terrain.road"].indices[0];
        for (slot, mask) in [10, 5, 3, 6, 12, 9, 0, 1, 2, 4, 8, 7, 11, 13, 14, 255]
            .into_iter()
            .enumerate()
        {
            let index = tiles[mask];
            let tile = image::imageops::crop_imm(
                &catalog.image,
                index as u32 % COLUMNS * TILE,
                index as u32 / COLUMNS * TILE,
                TILE,
                TILE,
            )
            .to_image();
            image::imageops::replace(
                &mut preview,
                &tile,
                (slot % 4) as i64 * i64::from(TILE),
                (slot / 4) as i64 * i64::from(TILE),
            );
        }
        image::imageops::resize(
            &preview,
            TILE * 4 * 4,
            TILE * 4 * 4,
            image::imageops::FilterType::Nearest,
        )
        .save(root.join("target/road-connections-preview.png"))
        .unwrap();
    }

    #[test]
    fn adjacent_road_shapes_share_identical_edges_including_corners_and_plazas() {
        let road = RgbaImage::from_pixel(TILE, TILE, Rgba([48, 42, 36, 255]));
        let field = RgbaImage::from_pixel(TILE, TILE, Rgba([30, 48, 42, 255]));
        let cells: BTreeSet<(i32, i32)> = (0..9)
            .flat_map(|y| {
                (0..12).filter_map(move |x| {
                    if y == 4 || x == 3 || ((6..10).contains(&x) && (2..7).contains(&y)) {
                        Some((x, y))
                    } else {
                        None
                    }
                })
            })
            .collect();
        let mut preview = RgbaImage::new(TILE * 12, TILE * 9);
        let tiles: BTreeMap<_, _> = cells
            .iter()
            .map(|&(x, y)| {
                let mask = NEIGHBORS.iter().enumerate().fold(0, |m, (i, &(dx, dy))| {
                    if cells.contains(&(x + dx, y + dy)) {
                        m | (1 << i)
                    } else {
                        m
                    }
                });
                ((x, y), connected_tile(&road, &field, mask))
            })
            .collect();
        for y in 0..9 {
            for x in 0..12 {
                let tile = tiles.get(&(x, y)).unwrap_or(&field);
                image::imageops::replace(&mut preview, tile, i64::from(x * 32), i64::from(y * 32));
                if let Some(east) = tiles.get(&(x + 1, y))
                    && tiles.contains_key(&(x, y))
                {
                    for i in 0..TILE {
                        assert_eq!(tile.get_pixel(31, i), east.get_pixel(0, i));
                    }
                }
                if let Some(south) = tiles.get(&(x, y + 1))
                    && tiles.contains_key(&(x, y))
                {
                    for i in 0..TILE {
                        assert_eq!(tile.get_pixel(i, 31), south.get_pixel(i, 0));
                    }
                }
            }
        }
        image::imageops::resize(
            &preview,
            TILE * 12 * 2,
            TILE * 9 * 2,
            image::imageops::FilterType::Nearest,
        )
        .save(Path::new(env!("CARGO_MANIFEST_DIR")).join("target/road-layout-preview.png"))
        .unwrap();
    }

    #[test]
    fn multiple_registered_variants_load_and_selection_is_stable_across_reload() {
        let temp = std::env::temp_dir().join(format!("my-rogue-variants-{}", std::process::id()));
        std::fs::create_dir_all(&temp).unwrap();
        let allowed = BTreeSet::from([[48, 42, 36], [83, 65, 51], [30, 48, 42]]);
        RgbaImage::from_pixel(TILE, TILE, Rgba([48, 42, 36, 255]))
            .save(temp.join("a.png"))
            .unwrap();
        RgbaImage::from_pixel(TILE, TILE, Rgba([83, 65, 51, 255]))
            .save(temp.join("b.png"))
            .unwrap();
        std::fs::write(temp.join("texture_rules.json"), r#"{"seed":42,"sets":{"terrain.road":{"connect_group":"road","background":"terrain.field","variants":["a.png","b.png"]}}}"#).unwrap();
        let base = BTreeMap::from([("terrain.road".into(), 0), ("terrain.field".into(), 1)]);
        let mut atlas = RgbaImage::from_pixel(COLUMNS * TILE, TILE, Rgba([30, 48, 42, 255]));
        let rules = TextureRules::load(&temp, &allowed, &base, &mut atlas, 2).unwrap();
        let again = TextureRules::load(&temp, &allowed, &base, &mut atlas, 2).unwrap();
        let game = Game::load(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let mut selected = BTreeSet::new();
        for x in 0..32 {
            let p = Pos { x, y: 20, z: 0 };
            let index = rules.index(&game, "terrain.road", p).unwrap();
            assert_eq!(Some(index), again.index(&game, "terrain.road", p));
            selected.insert((index - 2) / 256);
        }
        assert_eq!(selected, BTreeSet::from([0, 1]));
        assert!(read_tile(&temp, "../a.png", &allowed).is_err());
        assert!(read_tile(&temp, "missing.png", &allowed).is_err());
        std::fs::remove_dir_all(temp).unwrap();
    }
}
