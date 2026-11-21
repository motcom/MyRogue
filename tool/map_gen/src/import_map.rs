//! Import an editable child recipe as a portable snapshot and place it inside a region.
use crate::graph::{Graph, Image, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportMap {
    /// Source is informational; graph is the saved, portable snapshot. Reload is explicit.
    pub json: String,
    #[serde(default)]
    pub graph: Option<Box<Graph>>,
    pub seed: u64,
    pub count: u32,
    pub width: u32,
    pub height: u32,
    /// Normalized x, y, width, height of the allowed placement region.
    pub region: [f32; 4],
    #[serde(skip)]
    pub error: String,
}
impl Default for ImportMap {
    fn default() -> Self {
        Self {
            json: String::new(),
            graph: None,
            seed: 42,
            count: 1,
            width: 10,
            height: 10,
            region: [0.0, 0.0, 1.0, 1.0],
            error: String::new(),
        }
    }
}
impl ImportMap {
    pub fn validate(&self) -> bool {
        let [x, y, w, h] = self.region;
        (1..=32).contains(&self.count)
            && (1..=1024).contains(&self.width)
            && (1..=1024).contains(&self.height)
            && self
                .region
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            && w > 0.0
            && h > 0.0
            && x + w <= 1.00001
            && y + h <= 1.00001
    }
    pub fn import(&mut self, path: &Path) -> Result<()> {
        let graph = crate::load(path).map_err(|e| e.to_string())?;
        validate_nested(&graph, 1, &mut 0)?;
        self.graph = Some(Box::new(graph));
        self.json = path.display().to_string();
        self.error.clear();
        Ok(())
    }
    pub fn render(
        &self,
        background: &Image,
        mask: Option<&Image>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Image> {
        if !self.validate() {
            return Err("Import Map: 配置パラメータが不正です".into());
        }
        let graph = self
            .graph
            .as_ref()
            .ok_or("Import Map: JSONのパスを入力して「読み込み」を押してください")?;
        validate_nested(graph, 1, &mut 0)?;
        if background.terrain.is_none() || mask.is_some_and(|m| m.terrain.is_some()) {
            return Err("Import Map: Backgroundは地形ID、Maskは濃淡画像を接続してください".into());
        }
        let size = background.size;
        let [rx, ry, rw, rh] = self.region;
        let x0 = (rx * size as f32).round() as usize;
        let y0 = (ry * size as f32).round() as usize;
        let x1 = ((rx + rw) * size as f32).round() as usize;
        let y1 = ((ry + rh) * size as f32).round() as usize;
        let width = self.width as usize;
        let height = self.height as usize;
        if x1 > size || y1 > size || x1 < x0 + width || y1 < y0 + height {
            return Err("Import Map: 配置領域に家の幅・高さが収まりません".into());
        }
        let mut output = background.clone();
        let mut occupied = vec![false; size * size];
        let mut placed = 0;
        for attempt in 0..self.count * 128 {
            if cancelled() {
                return Err("cancelled".into());
            }
            if placed == self.count {
                break;
            }
            let random =
                |axis: &str| crate::recipe::derive_seed(self.seed, &format!("{axis}:{attempt}"));
            let x = x0 + (random("x") % (x1 - x0 - width + 1) as u64) as usize;
            let y = y0 + (random("y") % (y1 - y0 - height + 1) as u64) as usize;
            let allowed = (y..y + height).all(|py| {
                (x..x + width).all(|px| {
                    let i = py * size + px;
                    !occupied[i]
                        && mask.is_none_or(|m| {
                            m.pixels[i] >= 0.5 && m.alpha.as_ref().is_none_or(|a| a[i] > 0)
                        })
                })
            });
            if !allowed {
                continue;
            }
            let seed = crate::recipe::derive_seed(self.seed, &format!("instance:{placed}"));
            let executable =
                crate::recipe::Recipe::from_graph(graph)?.instantiate(seed, &BTreeMap::new())?;
            let child = executable
                .render(cancelled)?
                .remove(&executable.output)
                .ok_or("Import Map: 子JSONに最終出力がありません")??;
            if child.terrain.is_none() {
                return Err("Import Map: 子JSONにはTerrain Paletteが必要です".into());
            }
            let mut front = Image {
                size,
                terrain: child.terrain.clone(),
                pixels: vec![0.0; size * size],
                alpha: Some(vec![0; size * size]),
            };
            for py in 0..height {
                if cancelled() {
                    return Err("cancelled".into());
                }
                for px in 0..width {
                    let from = ((2 * py + 1) * child.size / (2 * height)) * child.size
                        + (2 * px + 1) * child.size / (2 * width);
                    let to = (y + py) * size + x + px;
                    front.pixels[to] = child.pixels[from];
                    front.alpha.as_mut().unwrap()[to] =
                        child.alpha.as_ref().map_or(255, |a| a[from]);
                    occupied[to] = true;
                }
            }
            output = crate::terrain::merge(&output, &front, None, 0.5, cancelled)?;
            placed += 1;
        }
        // A fully excluded region is valid and leaves the parent unchanged.
        Ok(output)
    }
}
fn validate_nested(graph: &Graph, depth: usize, total: &mut usize) -> Result<()> {
    *total += graph.nodes.len();
    if depth > 4 || *total > 2048 {
        return Err("Import Map: 入れ子は4階層・合計2048ノードまでです".into());
    }
    graph.validate()?;
    for node in &graph.nodes {
        if let crate::graph::NodeKind::Import(imported) = &node.kind
            && let Some(child) = &imported.graph
        {
            validate_nested(child, depth + 1, total)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        graph::*,
        terrain::{TerrainBand, TerrainPalette},
    };
    use std::sync::Arc;

    fn flat_graph(id: &str, color: [u8; 3]) -> Graph {
        let mut graph = Graph {
            nodes: vec![],
            output: 0,
            resolution: 16,
            ..Default::default()
        };
        let uniform = graph
            .add(NodeKind::Constant { value: 1.0 }, [0.0, 0.0])
            .unwrap();
        let terrain = graph
            .add(
                NodeKind::Terrain(TerrainPalette {
                    bands: vec![TerrainBand {
                        id: id.into(),
                        max: 1.0,
                        color,
                        seam: Default::default(),
                    }],
                }),
                [200.0, 0.0],
            )
            .unwrap();
        graph.connect(uniform, terrain, 0).unwrap();
        graph.output = terrain;
        graph
    }
    fn terrain_id(image: &Image, i: usize) -> &str {
        &image.terrain.as_ref().unwrap().band(image.pixels[i]).id
    }
    fn setup() -> (ImportMap, Arc<Image>) {
        let background = crate::generate(
            &flat_graph("terrain.field", [94, 122, 80]),
            1,
            &BTreeMap::new(),
        )
        .unwrap();
        let imported = ImportMap {
            graph: Some(Box::new(flat_graph("obstacle.wooden_wall", [75, 74, 69]))),
            width: 3,
            height: 4,
            count: 2,
            region: [0.25, 0.25, 0.5, 0.5],
            ..Default::default()
        };
        (imported, background)
    }
    #[test]
    fn import_places_non_overlapping_instances_only_inside_region_and_repeats_by_seed() {
        let (imported, background) = setup();
        let output = imported.render(&background, None, &|| false).unwrap();
        assert_eq!(
            output.pixels,
            imported
                .render(&background, None, &|| false)
                .unwrap()
                .pixels
        );
        assert_eq!(
            (0..256)
                .filter(|&i| terrain_id(&output, i) == "obstacle.wooden_wall")
                .count(),
            24
        );
        for i in 0..256 {
            if terrain_id(&output, i) != "terrain.field" {
                assert!((4..12).contains(&(i % 16)) && (4..12).contains(&(i / 16)));
            }
        }
        let mut different = imported.clone();
        different.seed += 1;
        assert_ne!(
            output.pixels,
            different
                .render(&background, None, &|| false)
                .unwrap()
                .pixels
        );
        assert!(output.alpha.unwrap().iter().all(|&a| a == 255));
    }
    #[test]
    fn import_respects_whole_footprint_mask_and_rejects_invalid_inputs() {
        let (mut imported, background) = setup();
        let mut mask = Image {
            size: 16,
            pixels: vec![0.0; 256],
            terrain: None,
            alpha: None,
        };
        assert_eq!(
            imported
                .render(&background, Some(&mask), &|| false)
                .unwrap()
                .pixels,
            background.pixels
        );
        for y in 0..16 {
            for x in 8..16 {
                mask.pixels[y * 16 + x] = 1.0;
            }
        }
        imported.region = [0.0, 0.0, 1.0, 1.0];
        let output = imported
            .render(&background, Some(&mask), &|| false)
            .unwrap();
        assert!((0..256).all(|i| i % 16 >= 8 || terrain_id(&output, i) == "terrain.field"));
        assert!(imported.render(&background, None, &|| true).is_err());
        imported.width = 17;
        assert!(imported.render(&background, None, &|| false).is_err());
        imported.width = 3;
        imported.graph = None;
        assert!(imported.render(&background, None, &|| false).is_err());
        imported.graph = Some(Box::new(Graph {
            resolution: 16,
            ..Default::default()
        }));
        assert!(imported.render(&background, None, &|| false).is_err());
    }
    #[test]
    fn imported_json_is_portable_and_nested_recipes_are_bounded() {
        let (mut imported, background) = setup();
        let path =
            std::env::temp_dir().join(format!("map-import-child-{}.json", std::process::id()));
        crate::save(&path, imported.graph.as_ref().unwrap()).unwrap();
        imported.import(&path).unwrap();
        let output = imported.render(&background, None, &|| false).unwrap();
        std::fs::remove_file(&path).unwrap();
        let saved = serde_json::to_string(&NodeKind::Import(imported.clone())).unwrap();
        let NodeKind::Import(restored) = serde_json::from_str(&saved).unwrap() else {
            panic!()
        };
        assert_eq!(
            output.pixels,
            restored
                .render(&background, None, &|| false)
                .unwrap()
                .pixels
        );
        let mut nested = *imported.graph.take().unwrap();
        for _ in 0..5 {
            let mut outer = flat_graph("terrain.field", [94, 122, 80]);
            let source = outer.output;
            let id = outer
                .add(
                    NodeKind::Import(ImportMap {
                        graph: Some(Box::new(nested)),
                        ..Default::default()
                    }),
                    [400.0, 0.0],
                )
                .unwrap();
            outer.connect(source, id, 0).unwrap();
            outer.output = id;
            nested = outer;
        }
        assert!(validate_nested(&nested, 1, &mut 0).is_err());
    }
    #[test]
    fn sample_house_composes_furniture_and_procedural_parent() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let graph = crate::load(&root.join("examples/forest_houses.json")).unwrap();
        let first = crate::generate(&graph, 42, &BTreeMap::new()).unwrap();
        let ids: std::collections::BTreeSet<_> = (0..first.pixels.len())
            .map(|i| terrain_id(&first, i))
            .collect();
        for id in [
            "terrain.field",
            "terrain.road",
            "obstacle.wooden_wall",
            "obstacle.wooden_door",
            "obstacle.table",
            "obstacle.chair",
        ] {
            assert!(ids.contains(id), "missing {id}");
        }
        assert_eq!(
            first.pixels,
            crate::generate(&graph, 42, &BTreeMap::new())
                .unwrap()
                .pixels
        );
        assert_ne!(
            first.pixels,
            crate::generate(&graph, 43, &BTreeMap::new())
                .unwrap()
                .pixels
        );
    }
}
