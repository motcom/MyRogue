mod app;
mod entity_catalog;
use graph::*;
use map_gen::{graph, recipe};
use std::{error::Error, path::Path};
type AppResult<T> = std::result::Result<T, Box<dyn Error>>;
fn prepare(path: &Path) -> AppResult<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}
pub use map_gen::{load, save};
// Legacy CLI spelling; all saves now preserve the complete graph.
pub use map_gen::save as save_recipe;
pub fn export(path: &Path, image: &Image) -> AppResult<()> {
    if !path
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("png"))
    {
        return Err("PNGの保存先を指定してください".into());
    }
    prepare(path)?;
    image::save_buffer_with_format(
        path,
        &rgba(image),
        image.size as u32,
        image.size as u32,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )?;
    Ok(())
}
fn run() -> AppResult<()> {
    let mut args = std::env::args().skip(1);
    let mut file = None;
    let mut output = None;
    let mut terrain = false;
    let mut neighbors = false;
    let mut coordinate = [0i32; 2];
    let mut tile_requested = false;
    let mut recipe_output = None;
    let mut seed = None;
    let mut inputs = std::collections::BTreeMap::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--graph" => file = Some(args.next().ok_or("--graph PATH.json")?),
            "--export" => output = Some(args.next().ok_or("--export PATH.png")?),
            "--terrain" => terrain = true,
            "--neighbors" => neighbors = true,
            "--tile-x" => {
                tile_requested = true;
                coordinate[0] = args.next().ok_or("--tile-x INTEGER")?.parse()?;
            }
            "--tile-y" => {
                tile_requested = true;
                coordinate[1] = args.next().ok_or("--tile-y INTEGER")?.parse()?;
            }
            "--save" | "--export-recipe" => {
                recipe_output = Some(args.next().ok_or("--export-recipe PATH.json")?)
            }
            "--seed" => seed = Some(args.next().ok_or("--seed INTEGER")?.parse::<u64>()?),
            "--input" => {
                let entry = args.next().ok_or("--input NAME=VALUE")?;
                let (key, value) = entry.split_once('=').ok_or("--input NAME=VALUE")?;
                if inputs
                    .insert(key.to_string(), value.parse::<f64>()?)
                    .is_some()
                {
                    return Err("Duplicate --input".into());
                }
            }
            "--help" | "-h" => {
                println!(
                    "map_gen [--graph PATH.json] [--save PATH.json] [--export PATH.png [--terrain] [--seed INTEGER] [--input NAME=VALUE]... [--neighbors] [--tile-x X --tile-y Y]]\nNo arguments: open the node editor. Export: render without a GUI."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    let mut graph = match file.as_deref() {
        Some(file) => load(Path::new(&file))?,
        None => Graph::default(),
    };
    if terrain {
        let source = graph.output;
        let id = graph.add(NodeKind::Terrain(Default::default()), [700.0, 40.0])?;
        graph.connect(source, id, 0)?;
        graph.output = id;
    }
    if let Some(path) = recipe_output {
        if output.is_some() || seed.is_some() || !inputs.is_empty() || neighbors || tile_requested {
            return Err("--export-recipeは画像出力・実行入力と併用できません".into());
        }
        save_recipe(Path::new(&path), &graph)?;
        println!("Map JSON -> {path}");
        return Ok(());
    }
    if output.is_none() && (seed.is_some() || !inputs.is_empty() || neighbors || tile_requested) {
        return Err("--seed/--inputは--exportと併用してください".into());
    }
    if let Some(output) = output {
        if neighbors || tile_requested {
            let image = if neighbors {
                map_gen::world::generate_neighborhood(
                    &graph,
                    seed.unwrap_or(42),
                    &inputs,
                    coordinate,
                    &|| false,
                )?
            } else {
                map_gen::world::generate_tile(&graph, seed.unwrap_or(42), &inputs, coordinate)?
            };
            export(Path::new(&output), &image)?;
            println!("{}×{} -> {output}", image.size, image.size);
            return Ok(());
        }
        let graph = if seed.is_some() || !inputs.is_empty() {
            recipe::Recipe::from_graph(&graph)?.instantiate(seed.unwrap_or(42), &inputs)?
        } else {
            graph
        };
        let images = graph.render(&|| false)?;
        let image = images
            .get(&graph.output)
            .ok_or("No output node")?
            .as_ref()
            .map_err(|e| e.to_owned())?;
        export(Path::new(&output), image)?;
        println!(
            "{} nodes / {}×{} -> {}",
            graph.nodes.len(),
            image.size,
            image.size,
            output
        );
        return Ok(());
    }
    if terrain {
        return Err("--terrain requires --export; add a Terrain Palette node instead".into());
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1380.0, 880.0])
            .with_min_inner_size([1000.0, 700.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Map Gen · Node Material Lab",
        options,
        Box::new(move |cc| Ok(Box::new(app::Editor::new(cc, graph, file)))),
    )?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn map_json_saves_to_custom_nested_path_and_loads_without_png() {
        let directory = std::env::temp_dir().join(format!("map-gen-recipe-{}", std::process::id()));
        let path = directory.join("任意の場所/sub folder/terrain.recipe.json");
        let mut graph = Graph {
            resolution: 32,
            ..Default::default()
        };
        let input = graph
            .add(NodeKind::Input(NumberInput::default()), [0.0, 0.0])
            .unwrap();
        graph.node_mut(input).unwrap().kind = NodeKind::Input(NumberInput {
            key: "frequency".into(),
            mode: InputMode::Range,
            min: 0.005,
            max: 0.05,
            preview: 0.025,
            ..Default::default()
        });
        graph.bind_parameter(1, "frequency", Some(input)).unwrap();
        save_recipe(&path, &graph).unwrap();
        let original = std::fs::read_to_string(&path).unwrap();
        let saved: Graph = serde_json::from_str(&original).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(saved, graph);
        assert_eq!(loaded, graph);
        let pixels = |g: &Graph| {
            g.render(&|| false).unwrap()[&g.output]
                .as_ref()
                .unwrap()
                .pixels
                .clone()
        };
        assert_eq!(pixels(&graph), pixels(&loaded));
        assert!(!path.with_extension("png").exists());
        assert!(save_recipe(&path.with_extension("png"), &graph).is_err());
        graph.node_mut(1).unwrap().inputs.push(None);
        assert!(save_recipe(&path, &graph).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn black_to_alpha_json_and_png_keep_transparency() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/black_to_alpha.json");
        let graph = load(&path).unwrap();
        let image = map_gen::generate(&graph, 42, &std::collections::BTreeMap::new()).unwrap();
        assert!(image.alpha.as_ref().unwrap().contains(&0));
        assert!(image.alpha.as_ref().unwrap().contains(&255));
        let path = std::env::temp_dir().join(format!("black-alpha-{}.png", std::process::id()));
        export(&path, &image).unwrap();
        assert_eq!(
            image::open(&path).unwrap().to_rgba8().into_raw(),
            rgba(&image)
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn graph_and_png_roundtrip_reproduce_pixels() {
        let directory =
            std::env::temp_dir().join(format!("map-gen-roundtrip-{}", std::process::id()));
        let graph = Graph {
            resolution: 32,
            ..Default::default()
        };
        let path = directory.join("graph.json");
        save(&path, &graph).unwrap();
        let restored = load(&path).unwrap();
        assert_eq!(graph, restored);
        let generated = restored.render(&|| false).unwrap();
        let image = generated[&graph.output].as_ref().unwrap();
        let file = directory.join("gray.png");
        export(&file, image).unwrap();
        let png = image::open(file).unwrap().to_rgb8();
        assert_eq!(png.into_raw(), rgb(image));
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            &path,
            text.replace("\"schema_version\": 1", "\"schema_version\": 999"),
        )
        .unwrap();
        assert!(load(&path).is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
