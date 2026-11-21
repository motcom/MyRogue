//! Shared map graph format and evaluator. The editor is an optional binary.
pub mod connections;
pub mod graph;
pub mod import_map;
pub mod map_nodes;
pub mod patterns;
pub mod recipe;
pub mod terrain;
pub mod world;

use graph::{Graph, Image};
use std::{collections::BTreeMap, path::Path, sync::Arc};
pub type FileResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Read both complete map JSON and legacy compact recipes.
pub fn load(path: &Path) -> FileResult<Graph> {
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let graph: Graph = if value.get("recipe_version").is_some() {
        serde_json::from_value::<recipe::Recipe>(value)?.graph()?
    } else {
        serde_json::from_value(value)?
    };
    graph.validate()?;
    Ok(graph)
}

/// Save the editable source of truth, including unused branches and positions.
pub fn save(path: &Path, graph: &Graph) -> FileResult<()> {
    if !path
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("json"))
    {
        return Err("マップの保存先には.jsonを指定してください".into());
    }
    graph.validate()?;
    let json = serde_json::to_string_pretty(graph)?;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, json)?;
    Ok(())
}

/// Execute the output branch without modifying the editable graph.
pub fn generate(
    graph: &Graph,
    seed: u64,
    inputs: &BTreeMap<String, f64>,
) -> graph::Result<Arc<Image>> {
    let executable = recipe::Recipe::from_graph(graph)?.instantiate(seed, inputs)?;
    executable
        .render(&|| false)?
        .remove(&executable.output)
        .ok_or_else(|| "最終出力がありません".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph::*;

    #[test]
    fn one_json_preserves_editing_and_executes_only_output_branch() {
        let mut graph = Graph {
            resolution: 32,
            ..Default::default()
        };
        // Deliberately incomplete branch must survive editing and not prevent execution.
        let unused = graph.add(NodeKind::Output, [123.0, -456.0]).unwrap();
        graph.node_mut(unused).unwrap().name = "未使用の枝".into();
        let path =
            std::env::temp_dir().join(format!("map-gen-unified-{}.json", std::process::id()));
        save(&path, &graph).unwrap();
        let reopened = load(&path).unwrap();
        assert_eq!(reopened, graph);
        let inputs = BTreeMap::new();
        let before = generate(&graph, 123, &inputs).unwrap();
        let after = generate(&reopened, 123, &inputs).unwrap();
        assert_eq!(before.pixels, after.pixels);
        assert_ne!(
            before.pixels,
            generate(&reopened, 124, &inputs).unwrap().pixels
        );
        let recipe = recipe::Recipe::from_graph(&graph).unwrap();
        std::fs::write(&path, serde_json::to_string(&recipe).unwrap()).unwrap();
        let legacy = load(&path).unwrap();
        assert_eq!(
            before.pixels,
            generate(&legacy, 123, &inputs).unwrap().pixels
        );
        assert!(!legacy.nodes.iter().any(|n| n.id == unused));
        std::fs::remove_file(path).unwrap();
    }
}
