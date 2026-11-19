//! Terminal-only debug picker, before the graphical game starts.
use my_rogue::model::config::Result;
use std::{
    io::{self, Write},
    path::Path,
};

pub enum Selection {
    Test,
    Normal,
    Json(String),
    Quit,
}

fn json_maps(root: &Path) -> Result<Vec<String>> {
    let directory = root.join("data/map_json");
    let entries = match std::fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("json"))
        {
            paths.push(
                path.strip_prefix(root)?
                    .to_str()
                    .ok_or("JSONパスをUTF-8で表せません")?
                    .to_owned(),
            );
        }
    }
    paths.sort();
    Ok(paths)
}

pub fn select_map(root: &Path) -> Result<Selection> {
    let maps = json_maps(root)?;
    println!("起動するマップを選択してください（デバッグ）");
    println!("  0: Entityテストマップ（既定）");
    for (index, path) in maps.iter().enumerate() {
        println!("  {}: {}", index + 1, path);
    }
    println!("  n: 通常マップ / q: 終了");
    loop {
        print!("番号 [0]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            return Ok(Selection::Quit);
        }
        match input.trim() {
            "" | "0" => return Ok(Selection::Test),
            "n" => return Ok(Selection::Normal),
            "q" => return Ok(Selection::Quit),
            number => {
                if let Ok(index) = number.parse::<usize>()
                    && let Some(path) = index.checked_sub(1).and_then(|i| maps.get(i))
                {
                    return Ok(Selection::Json(path.clone()));
                }
                println!("一覧の番号、n、qを入力してください。");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_user_map() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(
            json_maps(root)
                .unwrap()
                .contains(&"data/map_json/shape_tiles.json".to_owned())
        );
    }
}
