//! Serializable DAG and GUI-independent image nodes.
use bracket_noise::prelude::{FastNoise, FractalType, NoiseType};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
pub type NodeId = u32;
pub type Result<T> = std::result::Result<T, String>;
#[derive(Clone, Debug)]
pub struct Image {
    pub size: usize,
    pub pixels: Vec<f32>,
}
#[derive(Clone, Copy)]
pub struct RenderContext {
    pub size: usize,
    pub extent: f32,
}
/// Common contract. A node owns its parameters; the graph owns connections and evaluation order.
pub trait ImageNode {
    fn title(&self) -> &'static str;
    fn inputs(&self) -> &'static [&'static str];
    fn evaluate(
        &self,
        inputs: &[Arc<Image>],
        context: RenderContext,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Image>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoiseKind {
    Simplex,
    Perlin,
    Value,
    Cellular,
    White,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Noise {
    pub kind: NoiseKind,
    pub seed: u64,
    pub frequency: f32,
    pub octaves: i32,
    pub gain: f32,
    pub lacunarity: f32,
}
impl Default for Noise {
    fn default() -> Self {
        Self {
            kind: NoiseKind::Simplex,
            seed: 42,
            frequency: 0.025,
            octaves: 4,
            gain: 0.5,
            lacunarity: 2.0,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Add,
    Subtract,
    Min,
    Max,
}
impl BlendMode {
    pub const ALL: [Self; 8] = [
        Self::Normal,
        Self::Multiply,
        Self::Screen,
        Self::Overlay,
        Self::Add,
        Self::Subtract,
        Self::Min,
        Self::Max,
    ];
    pub fn apply(self, a: f32, b: f32) -> f32 {
        match self {
            Self::Normal => b,
            Self::Multiply => a * b,
            Self::Screen => 1.0 - (1.0 - a) * (1.0 - b),
            Self::Overlay if a <= 0.5 => 2.0 * a * b,
            Self::Overlay => 1.0 - 2.0 * (1.0 - a) * (1.0 - b),
            Self::Add => (a + b).min(1.0),
            Self::Subtract => (a - b).max(0.0),
            Self::Min => a.min(b),
            Self::Max => a.max(b),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum NodeKind {
    Noise(Noise),
    Constant {
        value: f32,
    },
    Blend {
        mode: BlendMode,
        opacity: f32,
    },
    Levels {
        contrast: f32,
        brightness: f32,
        gamma: f32,
    },
    Invert,
    Threshold {
        cutoff: f32,
        softness: f32,
    },
    Blur {
        radius: u32,
    },
    Warp {
        strength: f32,
    },
    Output,
}
impl NodeKind {
    pub fn catalog() -> Vec<Self> {
        vec![
            Self::Noise(Noise::default()),
            Self::Constant { value: 0.5 },
            Self::Blend {
                mode: BlendMode::Overlay,
                opacity: 0.6,
            },
            Self::Levels {
                contrast: 1.0,
                brightness: 0.0,
                gamma: 1.0,
            },
            Self::Invert,
            Self::Threshold {
                cutoff: 0.5,
                softness: 0.05,
            },
            Self::Blur { radius: 2 },
            Self::Warp { strength: 0.04 },
            Self::Output,
        ]
    }
    pub fn category(&self) -> &'static str {
        match self {
            Self::Noise(_) | Self::Constant { .. } => "GENERATORS",
            Self::Blend { .. } | Self::Warp { .. } => "COMPOSITE",
            Self::Output => "OUTPUT",
            _ => "FILTERS",
        }
    }
    pub fn validate(&self) -> bool {
        match self {
            Self::Noise(n) => {
                finite(n.frequency, 0.0001, 1.0)
                    && (1..=8).contains(&n.octaves)
                    && finite(n.gain, 0.0, 1.0)
                    && finite(n.lacunarity, 1.0, 4.0)
            }
            Self::Constant { value } => finite(*value, 0.0, 1.0),
            Self::Blend { opacity, .. } => finite(*opacity, 0.0, 1.0),
            Self::Levels {
                contrast,
                brightness,
                gamma,
            } => {
                finite(*contrast, 0.0, 4.0)
                    && finite(*brightness, -1.0, 1.0)
                    && finite(*gamma, 0.1, 4.0)
            }
            Self::Threshold { cutoff, softness } => {
                finite(*cutoff, 0.0, 1.0) && finite(*softness, 0.0, 0.5)
            }
            Self::Blur { radius } => *radius <= 32,
            Self::Warp { strength } => finite(*strength, 0.0, 0.25),
            _ => true,
        }
    }
}
fn finite(v: f32, min: f32, max: f32) -> bool {
    v.is_finite() && (min..=max).contains(&v)
}
impl ImageNode for NodeKind {
    fn title(&self) -> &'static str {
        match self {
            Self::Noise(_) => "Noise",
            Self::Constant { .. } => "Uniform",
            Self::Blend { .. } => "Blend",
            Self::Levels { .. } => "Levels",
            Self::Invert => "Invert",
            Self::Threshold { .. } => "Threshold",
            Self::Blur { .. } => "Blur",
            Self::Warp { .. } => "Directional Warp",
            Self::Output => "Output",
        }
    }
    fn inputs(&self) -> &'static [&'static str] {
        match self {
            Self::Noise(_) | Self::Constant { .. } => &[],
            Self::Blend { .. } => &["Background", "Foreground", "Mask (optional)"],
            Self::Warp { .. } => &["Source", "Displacement"],
            _ => &["Source"],
        }
    }
    fn evaluate(
        &self,
        inputs: &[Arc<Image>],
        context: RenderContext,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Image> {
        let required = if matches!(self, Self::Blend { .. }) {
            2
        } else {
            self.inputs().len()
        };
        if inputs.len() < required || inputs.len() > self.inputs().len() {
            return Err(format!("{}: input count mismatch", self.title()));
        }
        let size = context.size;
        if inputs
            .iter()
            .any(|i| i.size != size || i.pixels.len() != size * size)
        {
            return Err("画像の寸法が一致しません".into());
        }
        if !self.validate() {
            return Err("ノードのパラメータが範囲外です".into());
        }
        let mut noise = FastNoise::seeded(0);
        if let Self::Noise(n) = self {
            noise = FastNoise::seeded(n.seed);
            noise.set_noise_type(match n.kind {
                NoiseKind::Simplex => NoiseType::SimplexFractal,
                NoiseKind::Perlin => NoiseType::PerlinFractal,
                NoiseKind::Value => NoiseType::ValueFractal,
                NoiseKind::Cellular => NoiseType::Cellular,
                NoiseKind::White => NoiseType::WhiteNoise,
            });
            noise.set_fractal_type(FractalType::FBM);
            noise.set_frequency(n.frequency);
            noise.set_fractal_octaves(n.octaves);
            noise.set_fractal_gain(n.gain);
            noise.set_fractal_lacunarity(n.lacunarity);
        }
        // Separable box filter: O(radius * size²), not O(radius² * size²).
        let mut horizontal = Vec::new();
        if let Self::Blur { radius } = self {
            horizontal.resize(size * size, 0.0);
            let r = *radius as isize;
            for y in 0..size {
                if cancelled() {
                    return Err("cancelled".into());
                }
                for x in 0..size {
                    horizontal[y * size + x] = (-r..=r)
                        .map(|d| {
                            inputs[0].pixels
                                [y * size + (x as isize + d).clamp(0, size as isize - 1) as usize]
                        })
                        .sum::<f32>()
                        / (2 * r + 1) as f32;
                }
            }
        }
        let mut pixels = Vec::with_capacity(size * size);
        for y in 0..size {
            if cancelled() {
                return Err("cancelled".into());
            }
            for x in 0..size {
                let index = y * size + x;
                let source = || inputs[0].pixels[index];
                let value = match self {
                    Self::Noise(_) => {
                        let raw = noise.get_noise(
                            x as f32 / (size - 1) as f32 * context.extent,
                            y as f32 / (size - 1) as f32 * context.extent,
                        );
                        (raw + 1.0) * 0.5
                    }
                    Self::Constant { value } => *value,
                    Self::Blend { mode, opacity } => {
                        let a = source();
                        let mask = inputs.get(2).map_or(1.0, |m| m.pixels[index]);
                        a + opacity * mask * (mode.apply(a, inputs[1].pixels[index]) - a)
                    }
                    Self::Levels {
                        contrast,
                        brightness,
                        gamma,
                    } => ((source() - 0.5) * contrast + 0.5 + brightness)
                        .clamp(0.0, 1.0)
                        .powf(1.0 / gamma),
                    Self::Invert => 1.0 - source(),
                    Self::Threshold { cutoff, softness } => {
                        if *softness == 0.0 {
                            if source() >= *cutoff { 1.0 } else { 0.0 }
                        } else {
                            let t =
                                ((source() - cutoff + softness) / (2.0 * softness)).clamp(0.0, 1.0);
                            t * t * (3.0 - 2.0 * t)
                        }
                    }
                    Self::Blur { radius } => {
                        let r = *radius as isize;
                        (-r..=r)
                            .map(|d| {
                                horizontal[(y as isize + d).clamp(0, size as isize - 1) as usize
                                    * size
                                    + x]
                            })
                            .sum::<f32>()
                            / (2 * r + 1) as f32
                    }
                    Self::Warp { strength } => {
                        let d = (inputs[1].pixels[index] - 0.5) * 2.0 * strength * size as f32;
                        bilinear(&inputs[0], x as f32 + d, y as f32 + d)
                    }
                    Self::Output => source(),
                };
                if !value.is_finite() {
                    return Err("ノイズが非有限値になりました".into());
                }
                pixels.push(value.clamp(0.0, 1.0));
            }
        }
        Ok(Image { size, pixels })
    }
}
fn bilinear(image: &Image, x: f32, y: f32) -> f32 {
    let max = (image.size - 1) as f32;
    let x = x.clamp(0.0, max);
    let y = y.clamp(0.0, max);
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(image.size - 1);
    let y1 = (y0 + 1).min(image.size - 1);
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let a =
        image.pixels[y0 * image.size + x0] * (1.0 - tx) + image.pixels[y0 * image.size + x1] * tx;
    let b =
        image.pixels[y1 * image.size + x0] * (1.0 - tx) + image.pixels[y1 * image.size + x1] * tx;
    a * (1.0 - ty) + b * ty
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: NodeId,
    pub name: String,
    pub position: [f32; 2],
    pub kind: NodeKind,
    pub inputs: Vec<Option<NodeId>>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Graph {
    pub schema_version: u32,
    pub resolution: u32,
    pub extent: f32,
    pub nodes: Vec<Node>,
    pub output: NodeId,
}
impl Default for Graph {
    fn default() -> Self {
        let mut g = Self {
            schema_version: 1,
            resolution: 256,
            extent: 256.0,
            nodes: Vec::new(),
            output: 0,
        };
        let a = g
            .add(
                NodeKind::Noise(Noise {
                    frequency: 0.012,
                    ..Default::default()
                }),
                [30.0, 40.0],
            )
            .unwrap();
        let b = g
            .add(
                NodeKind::Noise(Noise {
                    kind: NoiseKind::Perlin,
                    seed: 73,
                    frequency: 0.06,
                    ..Default::default()
                }),
                [30.0, 380.0],
            )
            .unwrap();
        let blend = g
            .add(
                NodeKind::Blend {
                    mode: BlendMode::Overlay,
                    opacity: 0.65,
                },
                [290.0, 120.0],
            )
            .unwrap();
        let levels = g
            .add(
                NodeKind::Levels {
                    contrast: 1.8,
                    brightness: -0.02,
                    gamma: 1.0,
                },
                [550.0, 120.0],
            )
            .unwrap();
        let output = g.add(NodeKind::Output, [810.0, 120.0]).unwrap();
        g.connect(a, blend, 0).unwrap();
        g.connect(b, blend, 1).unwrap();
        g.connect(blend, levels, 0).unwrap();
        g.connect(levels, output, 0).unwrap();
        g.output = output;
        g
    }
}
impl Graph {
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }
    pub fn add(&mut self, kind: NodeKind, position: [f32; 2]) -> Result<NodeId> {
        if self.nodes.len() >= 64 {
            return Err("最大64ノードです".into());
        }
        let id = self
            .nodes
            .iter()
            .map(|n| n.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("node ID overflow")?;
        self.nodes.push(Node {
            id,
            name: kind.title().into(),
            position,
            inputs: vec![None; kind.inputs().len()],
            kind,
        });
        if self.nodes.len() == 1 {
            self.output = id;
        }
        Ok(id)
    }
    pub fn connect(&mut self, source: NodeId, target: NodeId, input: usize) -> Result<()> {
        if self.node(source).is_none() {
            return Err("接続元がありません".into());
        }
        let target_node = self.node_mut(target).ok_or("接続先がありません")?;
        let slot = target_node
            .inputs
            .get_mut(input)
            .ok_or("入力ポートがありません")?;
        let old = *slot;
        *slot = Some(source);
        if let Err(error) = self.validate() {
            self.node_mut(target).unwrap().inputs[input] = old;
            return Err(error);
        }
        Ok(())
    }
    pub fn remove(&mut self, id: NodeId) {
        self.nodes.retain(|n| n.id != id);
        for n in &mut self.nodes {
            for slot in &mut n.inputs {
                if *slot == Some(id) {
                    *slot = None;
                }
            }
        }
        if self.output == id {
            self.output = self.nodes.last().map_or(0, |n| n.id);
        }
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != 1
            || !(16..=1024).contains(&self.resolution)
            || !finite(self.extent, 1.0, 4096.0)
            || self.nodes.len() > 64
        {
            return Err("グラフの形式・解像度・範囲が不正です".into());
        }
        let ids: BTreeSet<_> = self.nodes.iter().map(|n| n.id).collect();
        if ids.len() != self.nodes.len() || (!self.nodes.is_empty() && !ids.contains(&self.output))
        {
            return Err("出力またはノードIDが不正です".into());
        }
        for n in &self.nodes {
            if n.name.len() > 256
                || !n.kind.validate()
                || n.inputs.len() != n.kind.inputs().len()
                || n.position.iter().any(|v| !finite(*v, -100000.0, 100000.0))
                || n.inputs.iter().flatten().any(|id| !ids.contains(id))
            {
                return Err(format!("ノード {} の設定・接続が不正です", n.id));
            }
        }
        let mut done = BTreeSet::new();
        let mut visiting = BTreeSet::new();
        fn visit(
            id: NodeId,
            g: &Graph,
            visiting: &mut BTreeSet<NodeId>,
            done: &mut BTreeSet<NodeId>,
        ) -> Result<()> {
            if done.contains(&id) {
                return Ok(());
            }
            if !visiting.insert(id) {
                return Err("循環する接続は作れません".into());
            }
            for &input in g.node(id).unwrap().inputs.iter().flatten() {
                visit(input, g, visiting, done)?;
            }
            visiting.remove(&id);
            done.insert(id);
            Ok(())
        }
        for n in &self.nodes {
            visit(n.id, self, &mut visiting, &mut done)?;
        }
        Ok(())
    }
    /// Node movement/renaming doesn't invalidate the expensive image cache.
    pub fn same_computation(&self, other: &Self) -> bool {
        self.resolution == other.resolution
            && self.extent == other.extent
            && self.output == other.output
            && self.nodes.len() == other.nodes.len()
            && self
                .nodes
                .iter()
                .zip(&other.nodes)
                .all(|(a, b)| a.id == b.id && a.kind == b.kind && a.inputs == b.inputs)
    }
    pub fn render(
        &self,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<BTreeMap<NodeId, Result<Arc<Image>>>> {
        self.validate()?;
        let mut cache = BTreeMap::new();
        fn eval(
            id: NodeId,
            g: &Graph,
            cache: &mut BTreeMap<NodeId, Result<Arc<Image>>>,
            cancelled: &dyn Fn() -> bool,
        ) -> Result<Arc<Image>> {
            if cancelled() {
                return Err("cancelled".into());
            }
            if let Some(result) = cache.get(&id) {
                return result.clone();
            }
            let node = g.node(id).ok_or("ノードがありません")?;
            let result = (|| {
                let mut inputs = Vec::new();
                for (i, slot) in node.inputs.iter().enumerate() {
                    if let Some(input) = slot {
                        inputs.push(eval(*input, g, cache, cancelled)?);
                    } else if matches!(node.kind, NodeKind::Blend { .. }) && i == 2 {
                        break;
                    } else {
                        return Err(format!(
                            "{}: {} が未接続です",
                            node.name,
                            node.kind.inputs()[i]
                        ));
                    }
                }
                node.kind
                    .evaluate(
                        &inputs,
                        RenderContext {
                            size: g.resolution as usize,
                            extent: g.extent,
                        },
                        cancelled,
                    )
                    .map(Arc::new)
            })();
            cache.insert(id, result.clone());
            result
        }
        for node in &self.nodes {
            if cancelled() {
                return Err("cancelled".into());
            }
            let _ = eval(node.id, self, &mut cache, cancelled);
        }
        Ok(cache)
    }
}
pub fn rgb(image: &Image, terrain: bool) -> Vec<u8> {
    image
        .pixels
        .iter()
        .flat_map(|&v| {
            if terrain {
                if v < 0.38 {
                    [34, 70, 93]
                } else if v < 0.43 {
                    [171, 152, 106]
                } else if v < 0.7 {
                    [94, 122, 80]
                } else if v < 0.86 {
                    [134, 128, 115]
                } else {
                    [220, 221, 213]
                }
            } else {
                let v = (v * 255.0).round() as u8;
                [v; 3]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn image(v: f32) -> Arc<Image> {
        Arc::new(Image {
            size: 16,
            pixels: vec![v; 256],
        })
    }
    fn evaluate(kind: NodeKind, values: &[f32]) -> Image {
        kind.evaluate(
            &values.iter().map(|&v| image(v)).collect::<Vec<_>>(),
            RenderContext {
                size: 16,
                extent: 128.0,
            },
            &|| false,
        )
        .unwrap()
    }
    #[test]
    fn blend_formulas_and_mask() {
        for (mode, a, b, want) in [
            (BlendMode::Multiply, 0.4, 0.7, 0.28),
            (BlendMode::Screen, 0.4, 0.7, 0.82),
            (BlendMode::Overlay, 0.2, 0.8, 0.32),
            (BlendMode::Overlay, 0.8, 0.2, 0.68),
        ] {
            assert!((mode.apply(a, b) - want).abs() < 0.00001);
        }
        let n = NodeKind::Blend {
            mode: BlendMode::Multiply,
            opacity: 0.5,
        };
        assert!((evaluate(n.clone(), &[0.4, 0.7]).pixels[0] - 0.34).abs() < 0.00001);
        assert_eq!(evaluate(n, &[0.4, 0.7, 0.0]).pixels[0], 0.4);
    }
    #[test]
    fn cycle_is_rejected_without_losing_previous_connection() {
        let mut g = Graph::default();
        let before = g.clone();
        assert!(g.connect(g.output, 1, 0).is_err());
        assert_eq!(g, before);
        assert!(g.connect(g.output, 3, 0).is_err());
        assert_eq!(g, before);
    }
    #[test]
    fn render_deterministic_and_shared_branches_match() {
        let g = Graph::default();
        let a = g.render(&|| false).unwrap();
        let b = g.render(&|| false).unwrap();
        for n in &g.nodes {
            assert_eq!(
                a[&n.id].as_ref().unwrap().pixels,
                b[&n.id].as_ref().unwrap().pixels
            );
        }
        let mut changed = g.clone();
        if let NodeKind::Noise(noise) = &mut changed.nodes[0].kind {
            noise.seed += 1;
        }
        assert_ne!(
            a[&g.output].as_ref().unwrap().pixels,
            changed.render(&|| false).unwrap()[&g.output]
                .as_ref()
                .unwrap()
                .pixels
        );
        assert!(g.render(&|| true).is_err());
    }
    #[test]
    fn delete_disconnects_and_invalid_graph_is_rejected() {
        let mut g = Graph::default();
        g.remove(1);
        g.validate().unwrap();
        assert!(g.render(&|| false).unwrap()[&g.output].is_err());
        g.nodes[0].inputs.push(Some(999));
        assert!(g.validate().is_err());
    }
    #[test]
    fn filters_preserve_flat_fields_and_zero_warp() {
        for n in [
            NodeKind::Blur { radius: 4 },
            NodeKind::Levels {
                contrast: 1.0,
                brightness: 0.0,
                gamma: 1.0,
            },
        ] {
            assert!(
                evaluate(n, &[0.4])
                    .pixels
                    .iter()
                    .all(|&v| (v - 0.4).abs() < 0.00001)
            );
        }
        assert_eq!(evaluate(NodeKind::Invert, &[0.4]).pixels[0], 0.6);
        assert_eq!(
            evaluate(
                NodeKind::Threshold {
                    cutoff: 0.5,
                    softness: 0.0
                },
                &[0.4]
            )
            .pixels[0],
            0.0
        );
        assert_eq!(
            evaluate(NodeKind::Warp { strength: 0.0 }, &[0.4, 0.8]).pixels[0],
            0.4
        );
    }
    #[test]
    fn serialized_graph_and_layout_changes() {
        let g = Graph::default();
        assert_eq!(
            g,
            serde_json::from_str::<Graph>(&serde_json::to_string(&g).unwrap()).unwrap()
        );
        let mut moved = g.clone();
        moved.nodes[0].position = [55.0, 60.0];
        assert!(g.same_computation(&moved));
        moved.resolution = 0;
        assert!(moved.validate().is_err());
    }
}
