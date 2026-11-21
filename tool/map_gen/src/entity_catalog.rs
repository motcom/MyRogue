//! Editor-only lookup of map/static Entity IDs from a user-selected directory.
use mlua::{HookTriggers, Lua, LuaOptions, StdLib, Table, VmState};
use std::{
    cell::Cell,
    collections::BTreeMap,
    path::{Path, PathBuf},
    rc::Rc,
};

#[derive(Debug)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

pub struct Catalog {
    pub directory: String,
    pub scanned_directory: String,
    pub entries: Vec<Entry>,
    pub errors: Vec<String>,
}
impl Catalog {
    pub fn new() -> Self {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/entity_templates");
        let directory = directory.canonicalize().unwrap_or(directory);
        let mut catalog = Self {
            directory: directory.display().to_string(),
            scanned_directory: String::new(),
            entries: Vec::new(),
            errors: Vec::new(),
        };
        catalog.reload();
        catalog
    }
    pub fn reload(&mut self) {
        self.entries.clear();
        self.errors.clear();
        self.scanned_directory = self.directory.clone();
        let root = PathBuf::from(&self.directory);
        if !root.is_absolute() {
            self.errors
                .push("参照ルートは絶対パスで指定してください。".into());
            return;
        }
        let mut files = Vec::new();
        collect(&root, &mut files, &mut self.errors);
        files.sort();
        let mut entries = BTreeMap::new();
        for path in files {
            match read_entry(&path) {
                Ok(Some(entry)) => {
                    if let Some(previous) = entries.get(&entry.id) {
                        let previous: &Entry = previous;
                        self.errors.push(format!(
                            "ID重複: {} ({} / {})",
                            entry.id,
                            previous.path.display(),
                            path.display()
                        ));
                    } else {
                        entries.insert(entry.id.clone(), entry);
                    }
                }
                Ok(None) => {}
                Err(error) => self.errors.push(format!("{}: {error}", path.display())),
            }
        }
        self.entries = entries.into_values().collect();
    }
}
fn collect(root: &Path, files: &mut Vec<PathBuf>, errors: &mut Vec<String>) {
    let children = match std::fs::read_dir(root) {
        Ok(children) => children,
        Err(error) => {
            errors.push(format!("{}: {error}", root.display()));
            return;
        }
    };
    for child in children {
        let result = (|| -> std::io::Result<()> {
            let child = child?;
            let kind = child.file_type()?;
            let path = child.path();
            // Do not follow symlinks into cycles or outside the selected tree.
            if kind.is_dir() {
                collect(&path, files, errors);
            } else if kind.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("lua"))
            {
                files.push(path);
            }
            Ok(())
        })();
        if let Err(error) = result {
            errors.push(format!("{}: {error}", root.display()));
        }
    }
}
fn read_entry(path: &Path) -> Result<Option<Entry>, Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)?;
    // Evaluate returned tables, without filesystem/process/module APIs or globals.
    let lua = Lua::new_with(StdLib::NONE, LuaOptions::default())?;
    lua.set_memory_limit(8 * 1024 * 1024)?;
    let budget = Rc::new(Cell::new(100_000u32));
    lua.set_hook(
        HookTriggers::new().every_nth_instruction(1000),
        move |_, _| {
            let remaining = budget.get();
            if remaining < 1000 {
                return Err(mlua::Error::RuntimeError(
                    "Lua instruction budget exceeded".into(),
                ));
            }
            budget.set(remaining - 1000);
            Ok(VmState::Continue)
        },
    )?;
    let table: Table = lua
        .load(&source)
        .set_name(path.display().to_string())
        .set_environment(lua.create_table()?)
        .eval()?;
    if !matches!(
        table.get::<Option<String>>("category")?.as_deref(),
        Some("map" | "static")
    ) {
        return Ok(None);
    }
    let id: String = table.get("id")?;
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        return Err("不正なEntity ID".into());
    }
    let common: Table = table.get("common")?;
    Ok(Some(Entry {
        id,
        name: common.get("name")?,
        path: path.to_owned(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recursive_lookup_reads_returned_ids_and_reports_errors() {
        let root = std::env::temp_dir().join(format!("map-gen-catalog-{}", std::process::id()));
        std::fs::create_dir_all(root.join("nested")).unwrap();
        let write = |name: &str, source: &str| std::fs::write(root.join(name), source).unwrap();
        write(
            "first.lua",
            "-- id = 'fake'\nreturn { id = 'terrain.real', category = 'static', common = { name = '実在' } }",
        );
        write(
            "nested/other.LUA",
            "return { id = 'obstacle.other', category = 'static', common = { name = '別' } }",
        );
        write(
            "item.lua",
            "return { id = 'item.ignored', category = 'item' }",
        );
        write("bad.lua", "return {");
        write("loop.lua", "while true do end");
        write("external.lua", "return dofile('/tmp/should-not-be-read')");
        let mut catalog = Catalog {
            directory: root.display().to_string(),
            scanned_directory: String::new(),
            entries: vec![],
            errors: vec![],
        };
        catalog.reload();
        assert_eq!(
            catalog
                .entries
                .iter()
                .map(|e| e.id.as_str())
                .collect::<Vec<_>>(),
            ["obstacle.other", "terrain.real"]
        );
        assert_eq!(catalog.errors.len(), 3);
        write(
            "duplicate.lua",
            "return { id = 'terrain.real', category = 'static', common = { name = '重複' } }",
        );
        // Files are not re-read until the explicit reload event.
        assert_eq!(catalog.errors.len(), 3);
        catalog.reload();
        assert!(catalog.errors.iter().any(|e| e.contains("ID重複")));
        catalog.directory = root.join("missing").display().to_string();
        catalog.reload();
        assert!(catalog.entries.is_empty());
        assert!(!catalog.errors.is_empty());
        catalog.directory = "relative".into();
        catalog.reload();
        assert!(catalog.errors[0].contains("絶対パス"));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn default_catalog_matches_actual_map_and_static_templates() {
        let catalog = Catalog::new();
        assert!(catalog.errors.is_empty(), "{:?}", catalog.errors);
        assert!(
            catalog
                .entries
                .iter()
                .any(|e| e.id == "obstacle.stone_wall")
        );
        assert!(catalog.entries.iter().any(|e| e.id == "terrain.pond"));
        assert!(catalog.entries.iter().any(|e| e.id == "terrain.field"));
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/entity_tex");
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(root.join("manifest.json")).unwrap())
                .unwrap();
        let texture_ids: std::collections::BTreeSet<_> = manifest["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| matches!(entry["category"].as_str(), Some("map" | "static")))
            .map(|entry| entry["id"].as_str().unwrap())
            .collect();
        let catalog_ids: std::collections::BTreeSet<_> = catalog
            .entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(catalog_ids, texture_ids);
        assert!(catalog.entries.iter().all(|e| e.path.is_absolute()));
    }
}
