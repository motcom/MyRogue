//! Portable generation recipes and named numeric inputs. No GUI dependencies.
use crate::graph::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputMode {
    Constant,
    Range,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NumberInput {
    pub key: String,
    pub mode: InputMode,
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub preview: f64,
    pub random: bool,
    pub integer: bool,
}
impl Default for NumberInput {
    fn default() -> Self {
        Self {
            key: "input".into(),
            mode: InputMode::Constant,
            value: 0.5,
            min: 0.0,
            max: 1.0,
            preview: 0.5,
            random: false,
            integer: false,
        }
    }
}
pub fn derive_seed(seed: u64, key: &str) -> u64 {
    let mut hash = seed ^ 0xcbf29ce484222325;
    for b in key.bytes() {
        hash = (hash ^ b as u64).wrapping_mul(0x100000001b3);
    }
    hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d049bb133111eb);
    hash ^ (hash >> 31)
}
impl NumberInput {
    pub fn validate(&self) -> bool {
        !self.key.is_empty()
            && self.key.len() <= 64
            && self
                .key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            && [self.value, self.min, self.max, self.preview]
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 1e9)
            && match self.mode {
                InputMode::Constant => !self.integer || self.value.fract() == 0.0,
                InputMode::Range => {
                    self.min <= self.max
                        && (self.min..=self.max).contains(&self.preview)
                        && (!self.integer
                            || [self.min, self.max, self.preview]
                                .iter()
                                .all(|v| v.fract() == 0.0))
                }
            }
    }
    pub fn preview_value(&self) -> f64 {
        if self.mode == InputMode::Constant {
            self.value
        } else {
            self.preview
        }
    }
    pub fn resolve(&self, seed: Option<u64>, external: Option<f64>) -> Result<f64> {
        if !self.validate() {
            return Err(format!("Input {}: 名前・数値・Min/Maxが不正です", self.key));
        }
        if seed.is_none() {
            return Ok(self.preview_value());
        }
        if self.mode == InputMode::Constant {
            if external.is_some() {
                return Err(format!("{} は定数なので上書きできません", self.key));
            }
            return Ok(self.value);
        }
        if self.random {
            if external.is_some() {
                return Err(format!("{} はランダム入力なので上書きできません", self.key));
            }
            let raw = derive_seed(seed.unwrap(), &self.key);
            if self.integer {
                let count = (self.max - self.min + 1.0) as u64;
                return Ok(self.min + ((raw as u128 * count as u128) >> 64) as f64);
            }
            let unit = (raw >> 11) as f64 / (1u64 << 53) as f64;
            return Ok(self.min + (self.max - self.min) * unit);
        }
        let value = external.unwrap_or(self.preview);
        if !value.is_finite()
            || !(self.min..=self.max).contains(&value)
            || (self.integer && value.fract() != 0.0)
        {
            return Err(format!(
                "{} の入力が範囲外または整数ではありません",
                self.key
            ));
        }
        Ok(value)
    }
}
/// Scalar parameter names use the same stable field keys as node JSON.
pub fn numeric_parameters(kind: &NodeKind) -> Vec<String> {
    if matches!(kind, NodeKind::Input(_)) {
        return vec![];
    }
    serde_json::to_value(kind)
        .unwrap()
        .as_object()
        .unwrap()
        .iter()
        .filter(|(_, v)| v.is_number())
        .map(|(key, _)| key.clone())
        .collect()
}
pub fn apply_parameter(kind: &NodeKind, key: &str, value: f64) -> Result<NodeKind> {
    if !value.is_finite() || value.abs() > 1e9 {
        return Err(format!("{key}: 入力値が範囲外です"));
    }
    let mut json = serde_json::to_value(kind).map_err(|e| e.to_string())?;
    let old = json
        .get(key)
        .filter(|v| v.is_number())
        .ok_or_else(|| format!("数値パラメータがありません: {key}"))?;
    let number = if old.is_f64() {
        serde_json::Number::from_f64(value)
    } else if value.fract() != 0.0 {
        None
    } else if old.as_i64().is_some_and(|n| n < 0) || value < 0.0 {
        Some((value as i64).into())
    } else {
        Some((value as u64).into())
    }
    .ok_or_else(|| format!("{key}: 有効な数値/整数を指定してください"))?;
    json[key] = number.into();
    let result: NodeKind = serde_json::from_value(json).map_err(|e| e.to_string())?;
    if !result.validate() {
        return Err(format!("{key}: 接続した入力がノードの許容範囲外です"));
    }
    Ok(result)
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeNode {
    pub id: NodeId,
    pub kind: NodeKind,
    pub inputs: Vec<Option<NodeId>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, NodeId>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub recipe_version: u32,
    pub resolution: u32,
    pub extent: f32,
    pub output: NodeId,
    pub nodes: Vec<RecipeNode>,
}
impl Recipe {
    pub fn from_graph(graph: &Graph) -> Result<Self> {
        graph.validate()?;
        let mut needed = BTreeSet::new();
        fn collect(id: NodeId, graph: &Graph, needed: &mut BTreeSet<NodeId>) -> Result<()> {
            if !needed.insert(id) {
                return Ok(());
            }
            let node = graph.node(id).ok_or("最終出力がありません")?;
            for (index, slot) in node.inputs.iter().enumerate() {
                match slot {
                    Some(id) => collect(*id, graph, needed)?,
                    None if matches!(
                        node.kind,
                        NodeKind::Blend { .. } | NodeKind::TerrainMerge { .. }
                    ) && index == 2 => {}
                    None => return Err(format!("{}: 必須入力が未接続です", node.name)),
                }
            }
            for &source in node.parameters.values() {
                collect(source, graph, needed)?;
            }
            Ok(())
        }
        if graph
            .node(graph.output)
            .is_some_and(|n| matches!(n.kind, NodeKind::Input(_)))
        {
            return Err("レシピの最終出力には画像ノードを指定してください".into());
        }
        collect(graph.output, graph, &mut needed)?;
        Ok(Self {
            recipe_version: 1,
            resolution: graph.resolution,
            extent: graph.extent,
            output: graph.output,
            nodes: graph
                .nodes
                .iter()
                .filter(|n| needed.contains(&n.id))
                .map(|n| RecipeNode {
                    id: n.id,
                    kind: n.kind.clone(),
                    inputs: n.inputs.clone(),
                    parameters: n.parameters.clone(),
                })
                .collect(),
        })
    }
    pub fn graph(&self) -> Result<Graph> {
        if self.recipe_version != 1 {
            return Err("未対応のレシピバージョンです".into());
        }
        let graph = Graph {
            schema_version: 1,
            resolution: self.resolution,
            extent: self.extent,
            output: self.output,
            nodes: self
                .nodes
                .iter()
                .enumerate()
                .map(|(i, n)| Node {
                    id: n.id,
                    name: n.kind.title().into(),
                    position: [(i % 4) as f32 * 260.0, (i / 4) as f32 * 340.0],
                    kind: n.kind.clone(),
                    inputs: n.inputs.clone(),
                    parameters: n.parameters.clone(),
                })
                .collect(),
        };
        Self::from_graph(&graph)?;
        Ok(graph)
    }
    pub fn instantiate(&self, seed: u64, inputs: &BTreeMap<String, f64>) -> Result<Graph> {
        self.graph()?.resolve_parameters(Some(seed), inputs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn graph_with_input() -> (Graph, NodeId, NodeId) {
        let mut graph = Graph {
            resolution: 64,
            ..Default::default()
        };
        let source = graph
            .add(NodeKind::Shape(Shape::default()), [0.0, 0.0])
            .unwrap();
        let tile = graph
            .add(NodeKind::TileSampler(TileSampler::default()), [260.0, 0.0])
            .unwrap();
        let input = graph
            .add(NodeKind::Input(NumberInput::default()), [0.0, 340.0])
            .unwrap();
        graph.node_mut(input).unwrap().kind = NodeKind::Input(NumberInput {
            key: "count".into(),
            mode: InputMode::Range,
            min: 2.0,
            max: 8.0,
            preview: 4.0,
            integer: true,
            ..Default::default()
        });
        graph.connect(source, tile, 0).unwrap();
        graph.bind_parameter(tile, "count_x", Some(input)).unwrap();
        graph.output = tile;
        (graph, tile, input)
    }
    #[test]
    fn constant_range_random_modes_and_integer_validation() {
        let constant = NumberInput {
            value: 0.75,
            ..Default::default()
        };
        assert_eq!(constant.resolve(Some(42), None).unwrap(), 0.75);
        assert!(constant.resolve(Some(42), Some(0.5)).is_err());
        let range = NumberInput {
            mode: InputMode::Range,
            min: -2.0,
            max: 3.0,
            preview: 0.4,
            ..Default::default()
        };
        assert_eq!(range.resolve(None, None).unwrap(), 0.4);
        assert_eq!(range.resolve(Some(42), Some(2.0)).unwrap(), 2.0);
        assert_eq!(range.resolve(Some(42), None).unwrap(), 0.4);
        assert!(range.resolve(Some(42), Some(4.0)).is_err());
        assert!(range.resolve(Some(42), Some(f64::NAN)).is_err());
        let random = NumberInput {
            random: true,
            ..range.clone()
        };
        assert_eq!(random.resolve(None, None).unwrap(), 0.4); // Preview ignores runtime randomness.
        assert_eq!(
            random.resolve(Some(42), None).unwrap(),
            random.resolve(Some(42), None).unwrap()
        );
        assert_ne!(
            random.resolve(Some(42), None).unwrap(),
            random.resolve(Some(43), None).unwrap()
        );
        assert!(random.resolve(Some(42), Some(1.0)).is_err());
        let integer = NumberInput {
            min: 2.0,
            max: 6.0,
            preview: 4.0,
            integer: true,
            ..random
        };
        let mut results = BTreeSet::new();
        for seed in 0..100 {
            let value = integer.resolve(Some(seed), None).unwrap();
            assert_eq!(value.fract(), 0.0);
            assert!((2.0..=6.0).contains(&value));
            results.insert(value as i32);
        }
        assert_eq!(results.len(), 5);
        assert!(
            !NumberInput {
                min: 7.0,
                ..integer.clone()
            }
            .validate()
        );
        assert!(
            !NumberInput {
                preview: 4.5,
                ..integer.clone()
            }
            .validate()
        );
        assert_eq!(
            NumberInput {
                min: 4.0,
                max: 4.0,
                ..integer
            }
            .resolve(Some(12), None)
            .unwrap(),
            4.0
        );
    }
    #[test]
    fn recipe_roundtrip_prunes_layout_and_binds_external_values() {
        let (graph, tile, input) = graph_with_input();
        let recipe = Recipe::from_graph(&graph).unwrap();
        assert_eq!(recipe.nodes.len(), 3); // Original demo nodes are unrelated.
        let json = serde_json::to_string_pretty(&recipe).unwrap();
        assert!(
            serde_json::from_str::<serde_json::Value>(&json).unwrap()["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .all(|n| n.get("position").is_none())
        );
        let copy: Recipe = serde_json::from_str(&json).unwrap();
        assert_eq!(copy, recipe);
        let preview = graph.render(&|| false).unwrap();
        let default = copy.instantiate(42, &BTreeMap::new()).unwrap();
        assert!(
            matches!(&default.node(tile).unwrap().kind, NodeKind::TileSampler(t) if t.count_x == 4)
        );
        let changed = copy
            .instantiate(42, &BTreeMap::from([("count".into(), 7.0)]))
            .unwrap();
        assert!(
            matches!(&changed.node(tile).unwrap().kind, NodeKind::TileSampler(t) if t.count_x == 7)
        );
        let pixels = |g: &Graph| {
            g.render(&|| false).unwrap()[&tile]
                .as_ref()
                .unwrap()
                .pixels
                .clone()
        };
        assert_ne!(preview[&tile].as_ref().unwrap().pixels, pixels(&changed));
        assert_eq!(
            pixels(&changed),
            pixels(
                &copy
                    .instantiate(42, &BTreeMap::from([("count".into(), 7.0)]))
                    .unwrap()
            )
        );
        assert!(
            copy.instantiate(42, &BTreeMap::from([("unknown".into(), 7.0)]))
                .is_err()
        );
        assert!(
            copy.instantiate(42, &BTreeMap::from([("count".into(), 2.5)]))
                .is_err()
        );
        let mut invalid = graph.clone();
        if let NodeKind::Input(n) = &mut invalid.node_mut(input).unwrap().kind {
            n.max = 65.0;
        }
        assert!(invalid.validate().is_err());
        let mut removed = graph.clone();
        removed.remove(input);
        assert!(removed.node(tile).unwrap().parameters.is_empty());
        removed.validate().unwrap();
        assert!(removed.render(&|| false).unwrap()[&tile].is_ok());
    }
    #[test]
    fn invalid_bindings_restore_previous_and_recipes_reject_broken_outputs() {
        let (mut graph, tile, input) = graph_with_input();
        let bindings = graph.node(tile).unwrap().parameters.clone();
        assert!(graph.bind_parameter(tile, "seed", Some(tile)).is_err());
        assert!(graph.bind_parameter(tile, "unknown", Some(input)).is_err());
        assert_eq!(graph.node(tile).unwrap().parameters, bindings);
        assert!(graph.connect(input, tile, 0).is_err());
        graph.node_mut(tile).unwrap().inputs[0] = None;
        assert!(Recipe::from_graph(&graph).is_err());
        let mut graph = Graph::default();
        let a = graph
            .add(NodeKind::Input(NumberInput::default()), [0.0, 0.0])
            .unwrap();
        let b = graph
            .add(NodeKind::Input(NumberInput::default()), [0.0, 0.0])
            .unwrap();
        graph.node_mut(b).unwrap().kind = graph.node(a).unwrap().kind.clone();
        assert!(graph.validate().is_err());
        let (graph, _, _) = graph_with_input();
        let mut recipe = Recipe::from_graph(&graph).unwrap();
        recipe.recipe_version = 2;
        assert!(recipe.graph().is_err());
    }
}
