//! RGBA Entity tiles in an alpha-preserving sparse console below the Japanese UI.
use crate::{
    config::{Result, ensure},
    game::Game,
};
use bracket_lib::prelude as bl;
use image::{Rgba, RgbaImage};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Cursor,
    path::Path,
    sync::Arc,
};

pub const TILE: u32 = 32;
const FONT_TILE: u32 = 16;
const COLUMNS: u32 = 16;
const SHEET: &str = "resources/entity-runtime.png";

mod variants;
use variants::TextureRules;

#[derive(Deserialize)]
struct Manifest {
    tile_size: [u32; 2],
    entries: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    id: String,
    file: String,
}

#[derive(Deserialize)]
struct Palette {
    colors: Vec<PaletteColor>,
}
#[derive(Deserialize)]
struct PaletteColor {
    rgb: [u8; 3],
}

fn validate_pixels(tile: &RgbaImage, allowed: &BTreeSet<[u8; 3]>) -> Result<()> {
    let mut visible = false;
    for pixel in tile.pixels() {
        let [r, g, b, a] = pixel.0;
        ensure(a == 0 || a == 255, "Alpha must be 0 or 255")?;
        if a != 0 {
            visible = true;
            ensure(
                allowed.contains(&[r, g, b]),
                "RGB outside Ash & Rust 32 palette",
            )?;
        }
    }
    ensure(visible, "Empty texture")
}

pub struct TextureCatalog {
    indices: BTreeMap<String, usize>,
    image: RgbaImage,
    glyph_start: usize,
    rules: TextureRules,
}

#[derive(Clone)]
pub(crate) struct TextureRenderer {
    console: usize,
    indices: BTreeMap<String, usize>,
    glyph_start: usize,
    rules: TextureRules,
}

struct Sprite {
    destination: bl::Rect,
    index: usize,
    tint: bl::RGBA,
}

impl TextureCatalog {
    /// Read individual PNGs, so editing a texture takes effect on the next launch.
    pub fn load(root: &Path) -> Result<Self> {
        let palette: Palette = serde_json::from_slice(&std::fs::read(
            root.join("document/color_thema/palette.json"),
        )?)?;
        let allowed: BTreeSet<_> = palette.colors.into_iter().map(|c| c.rgb).collect();
        ensure(
            allowed.len() == 32,
            "Ash & Rust palette must contain 32 unique colors",
        )?;
        let directory = root.join("data/entity_tex");
        let manifest: Manifest =
            serde_json::from_slice(&std::fs::read(directory.join("manifest.json"))?)?;
        ensure(
            manifest.tile_size == [TILE, TILE],
            "Entity textures must be 32x32",
        )?;
        ensure(
            !manifest.entries.is_empty() && manifest.entries.len() <= 4096,
            "Invalid texture count",
        )?;
        let glyph_start = manifest.entries.len();
        let count = glyph_start + 258;
        let mut image = RgbaImage::new(COLUMNS * TILE, (count as u32).div_ceil(COLUMNS) * TILE);
        let mut indices = BTreeMap::new();
        for (index, entry) in manifest.entries.into_iter().enumerate() {
            ensure(
                indices.insert(entry.id.clone(), index).is_none(),
                format!("Duplicate texture ID: {}", entry.id),
            )?;
            let file = Path::new(&entry.file);
            ensure(
                !file.is_absolute()
                    && file
                        .components()
                        .all(|c| matches!(c, std::path::Component::Normal(_))),
                "Invalid texture path",
            )?;
            let path = directory.join(file);
            let tile = image::open(&path)
                .map_err(|e| format!("Texture {}: {e}", path.display()))?
                .into_rgba8();
            ensure(
                tile.dimensions() == (TILE, TILE),
                format!("Texture {} must be 32x32", path.display()),
            )?;
            validate_pixels(&tile, &allowed)
                .map_err(|e| format!("Texture {}: {e}", path.display()))?;
            place(&mut image, &tile, index);
        }
        // The original font is a 32-column atlas. Copy CP437 glyphs as transparent
        // sprites, allowing custom definitions and Lua glyph changes to share z-order.
        let font =
            image::load_from_memory(include_bytes!("assets/japanese16x16.png"))?.into_rgba8();
        let columns = font.width() / FONT_TILE;
        for glyph in 0..256 {
            let mut tile = image::imageops::crop_imm(
                &font,
                glyph % columns * FONT_TILE,
                glyph / columns * FONT_TILE,
                FONT_TILE,
                FONT_TILE,
            )
            .to_image();
            for pixel in tile.pixels_mut() {
                if pixel[0] < 26 && pixel[1] < 26 && pixel[2] < 26 {
                    *pixel = Rgba([0, 0, 0, 0]);
                }
            }
            let tile =
                image::imageops::resize(&tile, TILE, TILE, image::imageops::FilterType::Nearest);
            place(&mut image, &tile, glyph_start + glyph as usize);
        }
        // State markers occupy only part of a cell and preserve the terrain underneath.
        let c = image::imageops::crop_imm(
            &image,
            ((glyph_start + bl::to_cp437('c') as usize) as u32 % COLUMNS) * TILE,
            ((glyph_start + bl::to_cp437('c') as usize) as u32 / COLUMNS) * TILE,
            TILE,
            TILE,
        )
        .to_image();
        let mut marker = RgbaImage::new(TILE, TILE);
        image::imageops::replace(
            &mut marker,
            &image::imageops::resize(&c, 16, 16, image::imageops::FilterType::Nearest),
            16,
            0,
        );
        place(&mut image, &marker, glyph_start + 256);
        if let Some(index) = indices.get("obstacle.wooden_door") {
            let door = image::imageops::crop_imm(
                &image,
                (*index as u32 % COLUMNS) * TILE,
                (*index as u32 / COLUMNS) * TILE,
                TILE,
                TILE,
            )
            .to_image();
            let mut opened = RgbaImage::new(TILE, TILE);
            image::imageops::replace(
                &mut opened,
                &image::imageops::resize(&door, 8, TILE, image::imageops::FilterType::Nearest),
                0,
                0,
            );
            place(&mut image, &opened, glyph_start + 257);
        }
        let rules = TextureRules::load(
            &directory,
            &allowed,
            &indices,
            &mut image,
            glyph_start + 258,
        )?;
        Ok(Self {
            rules,
            indices,
            image,
            glyph_start,
        })
    }

    pub fn len(&self) -> usize {
        self.indices.len()
    }
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Register the RGBA atlas as a font before building the console.
    pub fn embed(&self) -> Result<()> {
        let mut bytes = Cursor::new(Vec::new());
        self.image
            .write_to(&mut bytes, image::ImageOutputFormat::Png)?;
        bl::EMBED.lock().add_resource(
            SHEET.into(),
            Box::leak(bytes.into_inner().into_boxed_slice()),
        );
        Ok(())
    }

    pub fn install(self, game: &mut Game, console: usize) {
        game.texture_renderer = Some(Arc::new(TextureRenderer {
            console,
            indices: self.indices,
            glyph_start: self.glyph_start,
            rules: self.rules,
        }));
    }
}

fn place(atlas: &mut RgbaImage, tile: &RgbaImage, index: usize) {
    image::imageops::replace(
        atlas,
        tile,
        (index as u32 % COLUMNS * TILE) as i64,
        (index as u32 / COLUMNS * TILE) as i64,
    );
}

impl TextureRenderer {
    fn sprites(&self, game: &Game, view: &BTreeSet<crate::config::Pos>) -> Vec<Sprite> {
        let mut entities: Vec<_> = game
            .entities
            .values()
            .filter(|e| e.map_position().is_some_and(|p| view.contains(&p)))
            .collect();
        entities.sort_by_key(|e| (game.definition(e.id()).common.appearance.layer, e.id()));
        let mut sprites = Vec::new();
        for entity in entities {
            let p = entity.map_position().unwrap();
            let appearance = &game.definition(entity.id()).common.appearance;
            let binding = entity.script();
            let base = self
                .rules
                .index(game, &binding.definition_id, p)
                .or_else(|| self.indices.get(&binding.definition_id).copied());
            let opened = binding.state.get("opened").and_then(|v| v.as_bool()) == Some(true);
            let door = binding.definition_id == "obstacle.wooden_door" && opened;
            let chest = binding.definition_id == "item.treasure_chest" && opened;
            let use_texture = base.is_some()
                && (binding
                    .glyph
                    .as_deref()
                    .is_none_or(|g| g == appearance.glyph)
                    || door
                    || chest);
            let (index, tint) = if use_texture {
                (base.unwrap(), bl::RGBA::from_f32(1.0, 1.0, 1.0, 1.0))
            } else {
                let glyph = binding
                    .glyph
                    .as_deref()
                    .unwrap_or(&appearance.glyph)
                    .chars()
                    .next()
                    .unwrap_or('?');
                (
                    self.glyph_start + bl::to_cp437(glyph) as usize,
                    bl::RGBA::from_u8(appearance.fg[0], appearance.fg[1], appearance.fg[2], 255),
                )
            };
            sprites.push(Sprite {
                index,
                tint,
                destination: bl::Rect::with_size(
                    (p.x + 1) * 32,
                    (p.y + 4) * 32,
                    if door { 8 } else { 32 },
                    32,
                ),
            });
            // Until an open-lid asset exists, preserve the texture and mark it with
            // the Lua-provided open glyph. The marker stays below actors/items above it.
            if chest && use_texture {
                sprites.push(Sprite {
                    index: self.glyph_start + bl::to_cp437('c') as usize,
                    tint: bl::RGBA::from_u8(229, 189, 99, 255),
                    destination: bl::Rect::with_size((p.x + 1) * 32 + 16, (p.y + 4) * 32, 16, 16),
                });
            }
        }
        sprites
    }

    pub(crate) fn render(
        &self,
        game: &Game,
        ctx: &mut bl::BTerm,
        view: &BTreeSet<crate::config::Pos>,
    ) {
        let ui = ctx.active_console;
        ctx.set_active_console(self.console);
        // Game::render already cleared/prepared this layer. Clearing it here
        // removes the zoom marker and leaves the sparse vertex cache stale.
        for sprite in self.sprites(game, view) {
            let world = crate::config::Pos {
                z: game.map_camera().origin.z,
                x: sprite.destination.x1 / TILE as i32 - 1,
                y: sprite.destination.y1 / TILE as i32 - 4,
            };
            let Some(p) = game.map_draw_position(world) else {
                continue;
            };
            ctx.set(
                p.x,
                p.y,
                sprite.tint,
                bl::RGBA::from_f32(0.0, 0.0, 0.0, 0.0),
                if sprite.destination.width() == 8 {
                    self.glyph_start + 257
                } else if sprite.destination.width() == 16 {
                    self.glyph_start + 256
                } else {
                    sprite.index
                } as bl::FontCharType,
            );
        }
        ctx.set_active_console(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::Entity;

    fn setup() -> (Game, TextureCatalog) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        (
            Game::load_test_map(root).unwrap(),
            TextureCatalog::load(root).unwrap(),
        )
    }
    fn renderer(catalog: &TextureCatalog) -> TextureRenderer {
        TextureRenderer {
            console: 0,
            indices: catalog.indices.clone(),
            glyph_start: catalog.glyph_start,
            rules: catalog.rules.clone(),
        }
    }

    #[test]
    fn all_definitions_have_textures_and_packing_preserves_every_rgba_pixel() {
        let (game, catalog) = setup();
        assert_eq!(catalog.len(), game.entity_definitions.len());
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/entity_tex");
        let manifest: Manifest =
            serde_json::from_slice(&std::fs::read(directory.join("manifest.json")).unwrap())
                .unwrap();
        for entry in manifest.entries {
            let index = catalog.indices[&entry.id];
            assert!(game.entity_definitions.contains_key(&entry.id));
            let original = image::open(directory.join(entry.file))
                .unwrap()
                .into_rgba8();
            let packed = image::imageops::crop_imm(
                &catalog.image,
                index as u32 % COLUMNS * TILE,
                index as u32 / COLUMNS * TILE,
                TILE,
                TILE,
            )
            .to_image();
            assert_eq!(original, packed, "{}", entry.id);
        }
    }

    #[test]
    fn open_state_tiles_leave_ground_visible_and_use_nearest_pixels() {
        let (_, catalog) = setup();
        let tile = |index: usize| {
            image::imageops::crop_imm(
                &catalog.image,
                index as u32 % COLUMNS * TILE,
                index as u32 / COLUMNS * TILE,
                TILE,
                TILE,
            )
            .to_image()
        };
        let opened = tile(catalog.glyph_start + 257);
        assert!(
            opened
                .enumerate_pixels()
                .all(|(x, _, p)| x < 8 || p[3] == 0)
        );
        assert!(opened.pixels().any(|p| p[3] == 255));
        let marker = tile(catalog.glyph_start + 256);
        assert!(
            marker
                .enumerate_pixels()
                .all(|(x, y, p)| (x >= 16 && y < 16) || p[3] == 0)
        );
        assert!(marker.pixels().any(|p| p[3] == 255));
    }

    #[test]
    fn sprites_obey_visibility_inventory_layer_order_and_glyph_overrides() {
        let (mut game, catalog) = setup();
        let renderer = renderer(&catalog);
        let p = game.position(game.player_id).unwrap();
        game.spawn("item.apple", Some(p), None, 1).unwrap();
        let view = BTreeSet::from([p]);
        let sprites = renderer.sprites(&game, &view);
        assert_eq!(
            sprites.last().unwrap().index,
            catalog.indices["player.survivor"]
        );
        assert_eq!(
            sprites.first().unwrap().index,
            catalog.rules.index(&game, "terrain.road", p).unwrap()
        );
        assert!(
            sprites
                .iter()
                .any(|s| s.index == catalog.indices["item.apple"])
        );
        assert!(
            !sprites
                .iter()
                .any(|s| s.index == catalog.indices["item.knife"])
        );
        assert!(renderer.sprites(&game, &BTreeSet::new()).is_empty());
        assert!(
            sprites
                .iter()
                .all(|s| s.destination.x1 == (p.x + 1) * 32 && s.destination.y1 == (p.y + 4) * 32)
        );
        let id = game.player_id;
        game.entities.get_mut(&id).unwrap().script_mut().glyph = Some("?".into());
        assert_eq!(
            renderer.sprites(&game, &view).last().unwrap().index,
            catalog.glyph_start + bl::to_cp437('?') as usize
        );
        game.entities
            .get_mut(&id)
            .unwrap()
            .script_mut()
            .definition_id = "custom.player".into();
        let mut definitions = (*game.entity_definitions).clone();
        definitions.insert(
            "custom.player".into(),
            definitions["player.survivor"].clone(),
        );
        game.entity_definitions = Arc::new(definitions);
        game.entities.get_mut(&id).unwrap().script_mut().glyph = None;
        assert_eq!(
            renderer.sprites(&game, &view).last().unwrap().index,
            catalog.glyph_start + bl::to_cp437('@') as usize
        );
    }

    #[test]
    fn opening_door_reveals_ground_and_chest_marker_stays_below_player() {
        let (mut game, catalog) = setup();
        let renderer = renderer(&catalog);
        let p = game.position(game.player_id).unwrap();
        let door = game
            .spawn("obstacle.wooden_door", Some(p.offset(1, 0)), None, 1)
            .unwrap();
        let chest = game.spawn("item.treasure_chest", Some(p), None, 1).unwrap();
        let view = BTreeSet::from([p, p.offset(1, 0)]);
        let width = |sprites: Vec<Sprite>| {
            sprites
                .into_iter()
                .find(|s| s.index == catalog.indices["obstacle.wooden_door"])
                .unwrap()
                .destination
                .width()
        };
        assert_eq!(width(renderer.sprites(&game, &view)), 32);
        game.interact(door, "open").unwrap();
        assert_eq!(width(renderer.sprites(&game, &view)), 8);
        game.interact(chest, "open").unwrap();
        let sprites = renderer.sprites(&game, &view);
        let marker = sprites
            .iter()
            .position(|s| s.index == catalog.glyph_start + bl::to_cp437('c') as usize)
            .unwrap();
        let player = sprites
            .iter()
            .position(|s| s.index == catalog.indices["player.survivor"])
            .unwrap();
        assert!(marker < player);
        game.interact(door, "close").unwrap();
        assert_eq!(width(renderer.sprites(&game, &view)), 32);
        assert!(matches!(game.entities[&chest], Entity::Item(_)));
    }

    #[test]
    fn rejects_out_of_palette_rgb_partial_alpha_and_empty_images() {
        let allowed = BTreeSet::from([[14, 17, 19]]);
        let mut tile = RgbaImage::from_pixel(TILE, TILE, Rgba([14, 17, 19, 255]));
        assert!(validate_pixels(&tile, &allowed).is_ok());
        tile.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        assert!(validate_pixels(&tile, &allowed).is_err());
        tile.put_pixel(0, 0, Rgba([14, 17, 19, 128]));
        assert!(validate_pixels(&tile, &allowed).is_err());
        tile.fill(0);
        assert!(validate_pixels(&tile, &allowed).is_err());
    }

    #[test]
    fn missing_or_wrong_size_texture_returns_an_error_before_gui_start() {
        let temp = std::env::temp_dir().join(format!("my-rogue-textures-{}", std::process::id()));
        let directory = temp.join("data/entity_tex");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::create_dir_all(temp.join("document/color_thema")).unwrap();
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("document/color_thema/palette.json"),
            temp.join("document/color_thema/palette.json"),
        )
        .unwrap();
        assert!(TextureCatalog::load(&temp).is_err());
        std::fs::write(
            directory.join("manifest.json"),
            r#"{"tile_size":[32,32],"entries":[{"id":"test","file":"test.png"}]}"#,
        )
        .unwrap();
        RgbaImage::new(8, 16)
            .save(directory.join("test.png"))
            .unwrap();
        assert!(
            TextureCatalog::load(&temp)
                .err()
                .unwrap()
                .to_string()
                .contains("32x32")
        );
        std::fs::remove_dir_all(temp).unwrap();
    }
}
