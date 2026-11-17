mod app;
mod graph;
use graph::*;
use std::{error::Error, path::Path};
type AppResult<T> = std::result::Result<T, Box<dyn Error>>;
fn prepare(path: &Path) -> AppResult<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}
pub fn save(path: &Path, graph: &Graph) -> AppResult<()> {
    graph.validate()?;
    prepare(path)?;
    std::fs::write(path, serde_json::to_string_pretty(graph)?)?;
    Ok(())
}
pub fn load(path: &Path) -> AppResult<Graph> {
    let graph: Graph = serde_json::from_slice(&std::fs::read(path)?)?;
    graph.validate()?;
    Ok(graph)
}
pub fn export(path: &Path, image: &Image, terrain: bool) -> AppResult<()> {
    if !path
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("png"))
    {
        return Err("PNGの保存先を指定してください".into());
    }
    prepare(path)?;
    image::save_buffer_with_format(
        path,
        &rgb(image, terrain),
        image.size as u32,
        image.size as u32,
        image::ColorType::Rgb8,
        image::ImageFormat::Png,
    )?;
    Ok(())
}
fn run() -> AppResult<()> {
    let mut args = std::env::args().skip(1);
    let mut file = None;
    let mut output = None;
    let mut terrain = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--graph" => file = Some(args.next().ok_or("--graph PATH.json")?),
            "--export" => output = Some(args.next().ok_or("--export PATH.png")?),
            "--terrain" => terrain = true,
            "--help" | "-h" => {
                println!(
                    "map_gen [--graph PATH.json] [--export PATH.png [--terrain]]\nNo arguments: open the node editor. Export: render without a GUI."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    let graph = match file {
        Some(file) => load(Path::new(&file))?,
        None => Graph::default(),
    };
    if let Some(output) = output {
        let images = graph.render(&|| false)?;
        let image = images
            .get(&graph.output)
            .ok_or("No output node")?
            .as_ref()
            .map_err(|e| e.to_owned())?;
        export(Path::new(&output), image, terrain)?;
        save(&Path::new(&output).with_extension("json"), &graph)?;
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
        return Err("--terrain requires --export; use the GUI preview toggle instead".into());
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
        Box::new(move |cc| Ok(Box::new(app::Editor::new(cc, graph)))),
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
        for terrain in [true, false] {
            let file = directory.join(if terrain { "terrain.png" } else { "gray.png" });
            export(&file, image, terrain).unwrap();
            let png = image::open(file).unwrap().to_rgb8();
            assert_eq!(png.into_raw(), rgb(image, terrain));
        }
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
