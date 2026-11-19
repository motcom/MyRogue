//! Explicit scalar-to-terrain classification. Colors are preview-only; IDs drive the game.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A categorical boundary rule: IDs and colors are never interpolated.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeamMode {
    #[default]
    Cut,
    Mosaic,
    Fill,
    Road,
}
impl SeamMode {
    pub const ALL: [Self; 4] = [Self::Cut, Self::Mosaic, Self::Fill, Self::Road];
    pub fn label(self) -> &'static str {
        match self {
            Self::Cut => "切断",
            Self::Mosaic => "モザイク",
            Self::Fill => "自然な塗りつなぎ",
            Self::Road => "近くの道へ接続",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Seam {
    pub mode: SeamMode,
    /// Boundary influence/search depth, measured in generated cells.
    pub width: u16,
}
impl Default for Seam {
    fn default() -> Self {
        Self {
            mode: SeamMode::Cut,
            width: 8,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainBand {
    #[serde(default)]
    pub seam: Seam,
    pub max: f32,
    pub id: String,
    pub color: [u8; 3],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainPalette {
    pub bands: Vec<TerrainBand>,
}
impl Default for TerrainPalette {
    fn default() -> Self {
        Self {
            bands: vec![
                TerrainBand {
                    seam: Seam::default(),
                    max: 0.38,
                    id: "terrain.pond".into(),
                    color: [34, 70, 93],
                },
                TerrainBand {
                    seam: Seam::default(),
                    max: 0.43,
                    id: "terrain.road".into(),
                    color: [171, 152, 106],
                },
                TerrainBand {
                    seam: Seam::default(),
                    max: 0.70,
                    id: "terrain.field".into(),
                    color: [94, 122, 80],
                },
                TerrainBand {
                    seam: Seam::default(),
                    max: 0.86,
                    id: "terrain.wasteland".into(),
                    color: [134, 128, 115],
                },
                TerrainBand {
                    seam: Seam::default(),
                    max: 1.0,
                    id: "obstacle.stone_wall".into(),
                    color: [220, 221, 213],
                },
            ],
        }
    }
}
impl TerrainPalette {
    pub fn validate(&self) -> bool {
        let mut previous = -1.0;
        let mut ids = BTreeSet::new();
        !self.bands.is_empty()
            && self.bands.len() <= 32
            && self.bands.iter().all(|band| {
                let valid = (1..=128).contains(&band.seam.width)
                    && band.max.is_finite()
                    && (0.0..=1.0).contains(&band.max)
                    && band.max > previous
                    && !band.id.is_empty()
                    && band.id.len() <= 128
                    && band
                        .id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                    && ids.insert(&band.id);
                previous = band.max;
                valid
            })
            && previous == 1.0
    }
    /// Call only with a validated palette and finite, normalized samples.
    pub fn band(&self, value: f32) -> &TerrainBand {
        self.bands
            .iter()
            .find(|band| value <= band.max)
            .unwrap_or_else(|| self.bands.last().unwrap())
    }
}

/// Categorical overlay: every pixel keeps exactly one terrain ID and its color.
pub fn merge(
    background: &crate::graph::Image,
    foreground: &crate::graph::Image,
    mask: Option<&crate::graph::Image>,
    cutoff: f32,
    cancelled: &dyn Fn() -> bool,
) -> crate::graph::Result<crate::graph::Image> {
    use std::{collections::BTreeMap, sync::Arc};
    let back = background
        .terrain
        .as_ref()
        .ok_or("BackgroundにはTerrain Paletteの出力が必要です")?;
    let front = foreground
        .terrain
        .as_ref()
        .ok_or("ForegroundにはTerrain Paletteの出力が必要です")?;
    if mask.is_some_and(|m| m.terrain.is_some()) {
        return Err("Maskには濃淡マスクを接続してください".into());
    }
    let mut lookup = BTreeMap::<String, usize>::new();
    let mut bands = Vec::<TerrainBand>::new();
    for band in back.bands.iter().chain(&front.bands) {
        if let Some(&index) = lookup.get(&band.id) {
            if bands[index].color != band.color {
                return Err(format!(
                    "同じ地形ID {} の色が異なります。色を揃えるか別のIDを使ってください",
                    band.id
                ));
            }
            // The foreground owns connection settings for a shared ID,
            // independently of pixel visibility and mask selection.
            bands[index].seam = band.seam;
        } else {
            if bands.len() == 256 {
                return Err("合成後の地形IDは最大256種類です".into());
            }
            lookup.insert(band.id.clone(), bands.len());
            bands.push(band.clone());
        }
    }
    let count = bands.len();
    for (index, band) in bands.iter_mut().enumerate() {
        band.max = (index + 1) as f32 / count as f32;
    }
    let mut pixels = Vec::with_capacity(background.pixels.len());
    let mut alpha = Vec::with_capacity(background.pixels.len());
    for index in 0..background.pixels.len() {
        if index % background.size == 0 && cancelled() {
            return Err("cancelled".into());
        }
        let visible = |image: &crate::graph::Image| image.alpha.as_ref().map_or(255, |a| a[index]);
        let foreground_selected = visible(foreground) > 0
            && mask.is_none_or(|m| visible(m) > 0 && m.pixels[index] >= cutoff);
        let (source, palette) = if foreground_selected {
            (foreground, front)
        } else {
            (background, back)
        };
        let id = &palette.band(source.pixels[index]).id;
        pixels.push((lookup[id] as f32 + 0.5) / count as f32);
        alpha.push(visible(source));
    }
    Ok(crate::graph::Image {
        size: background.size,
        pixels,
        alpha: Some(alpha),
        terrain: Some(Arc::new(TerrainPalette { bands })),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{graph::*, recipe::Recipe};
    use std::{collections::BTreeMap, sync::Arc};

    #[test]
    fn merged_palettes_roundtrip_and_execute_optional_mask_and_numeric_input() {
        let graph = crate::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/terrain_merge.json"),
        )
        .unwrap();
        let restored: Graph =
            serde_json::from_str(&serde_json::to_string(&graph).unwrap()).unwrap();
        assert_eq!(restored, graph);
        let image = crate::generate(&graph, 42, &BTreeMap::new()).unwrap();
        assert_eq!(
            image.pixels,
            crate::generate(&restored, 42, &BTreeMap::new())
                .unwrap()
                .pixels
        );
        let mut graph = restored;
        graph.connect(3, 7, 2).unwrap();
        let input = graph
            .add(
                NodeKind::Input(NumberInput {
                    key: "merge_cutoff".into(),
                    mode: InputMode::Range,
                    min: 0.0,
                    max: 1.0,
                    preview: 0.5,
                    ..Default::default()
                }),
                [0.0, 0.0],
            )
            .unwrap();
        if let NodeKind::Input(input) = &mut graph.node_mut(input).unwrap().kind {
            input.key = "merge_cutoff".into();
        }
        graph.bind_parameter(7, "cutoff", Some(input)).unwrap();
        let masked =
            crate::generate(&graph, 42, &BTreeMap::from([("merge_cutoff".into(), 0.5)])).unwrap();
        assert!(
            masked
                .pixels
                .iter()
                .all(|value| masked.terrain.as_ref().unwrap().band(*value).id == "terrain.road")
        );
    }

    #[test]
    fn merge_selects_ids_with_transparency_mask_and_repeated_layers() {
        let back = Arc::new(Image {
            size: 2,
            pixels: vec![0.0, 1.0, 0.0, 1.0],
            alpha: Some(vec![255, 255, 255, 0]),
            terrain: Some(Arc::new(TerrainPalette {
                bands: vec![
                    TerrainBand {
                        seam: Seam::default(),
                        max: 0.5,
                        id: "floor".into(),
                        color: [80, 80, 80],
                    },
                    TerrainBand {
                        seam: Seam::default(),
                        max: 1.0,
                        id: "wall".into(),
                        color: [100, 100, 100],
                    },
                ],
            })),
        });
        let front = Arc::new(Image {
            size: 2,
            pixels: vec![1.0, 0.0, 1.0, 0.0],
            alpha: Some(vec![0, 255, 255, 0]),
            terrain: Some(Arc::new(TerrainPalette {
                bands: vec![
                    TerrainBand {
                        seam: Seam::default(),
                        max: 0.25,
                        id: "wall".into(),
                        color: [100, 100, 100],
                    },
                    TerrainBand {
                        seam: Seam::default(),
                        max: 1.0,
                        id: "door".into(),
                        color: [150, 50, 30],
                    },
                ],
            })),
        });
        let mask = Arc::new(Image {
            size: 2,
            pixels: vec![1.0, 1.0, 0.4, 1.0],
            alpha: None,
            terrain: None,
        });
        let context = RenderContext {
            size: 2,
            extent: 2.0,
        };
        let node = NodeKind::TerrainMerge { cutoff: 0.5 };
        let ids = |image: &Image| {
            image
                .pixels
                .iter()
                .map(|value| image.terrain.as_ref().unwrap().band(*value).id.clone())
                .collect::<Vec<_>>()
        };
        let unmasked = node
            .evaluate(&[back.clone(), front.clone()], context, &|| false)
            .unwrap();
        assert_eq!(ids(&unmasked), ["floor", "wall", "door", "wall"]);
        assert_eq!(
            unmasked.alpha.as_deref(),
            Some([255, 255, 255, 0].as_slice())
        );
        let masked = node
            .evaluate(
                &[back.clone(), front.clone(), mask.clone()],
                context,
                &|| false,
            )
            .unwrap();
        assert_eq!(ids(&masked), ["floor", "wall", "floor", "wall"]);
        let chained = node
            .evaluate(&[Arc::new(masked), front.clone()], context, &|| false)
            .unwrap();
        assert_eq!(ids(&chained), ids(&unmasked));
        assert_eq!(rgb(&chained), rgb(&unmasked));
        let mut conflict = front.as_ref().clone();
        Arc::make_mut(conflict.terrain.as_mut().unwrap()).bands[0].color = [0, 0, 0];
        assert!(
            node.evaluate(&[back.clone(), Arc::new(conflict)], context, &|| false)
                .unwrap_err()
                .contains("色が異なります")
        );
        assert!(
            node.evaluate(&[back.clone(), mask], context, &|| false)
                .is_err()
        );
        assert!(node.evaluate(&[back, front], context, &|| true).is_err());
    }

    #[test]
    fn merge_adds_ids_and_foreground_overwrites_connection_settings() {
        use crate::graph::Image;
        let band = |id: &str, mode, width| TerrainBand {
            id: id.into(),
            max: 1.0,
            color: [80, 80, 80],
            seam: Seam { mode, width },
        };
        let image = |bands| Image {
            size: 2,
            pixels: vec![0.25; 4],
            alpha: None,
            terrain: Some(Arc::new(TerrainPalette { bands })),
        };
        let back = image(vec![band("floor", SeamMode::Mosaic, 4)]);
        let mut shared = band("floor", SeamMode::Fill, 12);
        shared.max = 0.5;
        let mut front = image(vec![shared, band("road", SeamMode::Road, 8)]);
        front.alpha = Some(vec![0; 4]);
        let mask = Image {
            size: 2,
            pixels: vec![0.0; 4],
            alpha: None,
            terrain: None,
        };
        let merged = merge(&back, &front, Some(&mask), 0.5, &|| false).unwrap();
        let palette = merged.terrain.as_ref().unwrap();
        assert_eq!(palette.bands.len(), 2);
        assert_eq!(palette.bands[0].id, "floor");
        assert_eq!(
            palette.bands[0].seam,
            Seam {
                mode: SeamMode::Fill,
                width: 12
            }
        );
        assert_eq!(palette.bands[1].id, "road");
        assert_eq!(
            palette.bands[1].seam,
            Seam {
                mode: SeamMode::Road,
                width: 8
            }
        );
        assert!(merged.pixels.iter().all(|v| palette.band(*v).id == "floor"));
        // In a chain, the last foreground wins, including switching back to Cut.
        let last = image(vec![band("floor", SeamMode::Cut, 2)]);
        let result = merge(&merged, &last, None, 0.5, &|| false).unwrap();
        assert_eq!(
            result.terrain.as_ref().unwrap().bands[0].seam,
            Seam {
                mode: SeamMode::Cut,
                width: 2
            }
        );
        let mut conflicting = last;
        Arc::make_mut(conflicting.terrain.as_mut().unwrap()).bands[0].color = [1, 2, 3];
        assert!(
            merge(&merged, &conflicting, None, 0.5, &|| false)
                .unwrap_err()
                .contains("色が異なります")
        );
    }

    #[test]
    fn palette_colors_ids_and_output_survive_json_and_runtime_inputs() {
        let palette = TerrainPalette {
            bands: vec![
                TerrainBand {
                    seam: Seam::default(),
                    max: 0.5,
                    id: "wall".into(),
                    color: [100, 100, 100],
                },
                TerrainBand {
                    seam: Seam::default(),
                    max: 1.0,
                    id: "floor".into(),
                    color: [100, 100, 100],
                },
            ],
        };
        assert!(palette.validate());
        let source = Arc::new(Image {
            alpha: None,
            size: 2,
            pixels: vec![0.0, 0.5, 0.5001, 1.0],
            terrain: None,
        });
        let context = RenderContext {
            size: 2,
            extent: 2.0,
        };
        let colored = Arc::new(
            NodeKind::Terrain(palette.clone())
                .evaluate(&[source], context, &|| false)
                .unwrap(),
        );
        assert_eq!(rgb(&colored), vec![100; 12]);
        let ids: Vec<_> = colored
            .pixels
            .iter()
            .map(|v| colored.terrain.as_ref().unwrap().band(*v).id.as_str())
            .collect();
        assert_eq!(ids, ["wall", "wall", "floor", "floor"]);
        assert!(
            NodeKind::Invert
                .evaluate(std::slice::from_ref(&colored), context, &|| false)
                .is_err()
        );
        let passed = NodeKind::Output
            .evaluate(&[colored], context, &|| false)
            .unwrap();
        assert_eq!(passed.terrain.as_deref(), Some(&palette));
        let mut graph = Graph {
            resolution: 32,
            ..Default::default()
        };
        let id = graph
            .add(NodeKind::Terrain(palette), [123.0, 456.0])
            .unwrap();
        graph.connect(graph.output, id, 0).unwrap();
        graph.output = id;
        let restored: Graph =
            serde_json::from_str(&serde_json::to_string(&graph).unwrap()).unwrap();
        assert_eq!(restored, graph);
        let image = crate::generate(&restored, 42, &BTreeMap::new()).unwrap();
        assert!(image.terrain.is_some());
        assert_eq!(
            Recipe::from_graph(&graph).unwrap(),
            Recipe::from_graph(&restored).unwrap()
        );
    }

    #[test]
    fn invalid_palettes_are_rejected() {
        let palette = TerrainPalette::default();
        assert!(palette.validate());
        let mut broken = palette.clone();
        broken.bands[0].max = f32::NAN;
        assert!(!broken.validate());
        let mut broken = palette.clone();
        broken.bands[1].max = 0.1;
        assert!(!broken.validate());
        let mut broken = palette.clone();
        broken.bands.last_mut().unwrap().max = 0.99;
        assert!(!broken.validate());
        let mut broken = palette.clone();
        broken.bands[0].id.clear();
        assert!(!broken.validate());
        let mut broken = palette;
        broken.bands[1].id = broken.bands[0].id.clone();
        assert!(!broken.validate());
        assert!(!TerrainPalette { bands: vec![] }.validate());
    }
}
