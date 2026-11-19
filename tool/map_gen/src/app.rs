use crate::graph::*;
use eframe::egui::{
    self, Color32 as Color, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle,
    TextureOptions, Vec2,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Instant,
};
const BG: Color = Color::from_rgb(21, 24, 30);
const PANEL: Color = Color::from_rgb(29, 33, 40);
const ACCENT: Color = Color::from_rgb(237, 169, 83);
const GREEN: Color = Color::from_rgb(109, 195, 164);
type Images = BTreeMap<NodeId, Result<Arc<Image>>>;
type Work = (u64, Graph, Option<(u64, [i32; 2])>);
struct Completed {
    revision: u64,
    result: Result<Images>,
    neighborhood: Option<Result<Arc<Image>>>,
    millis: u128,
}
struct JsonLocation {
    directory: String,
    filename: String,
    error: String,
    opening: bool,
}
impl JsonLocation {
    fn new(path: &str) -> Self {
        let path = PathBuf::from(path);
        let absolute = if path.is_absolute() {
            path
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        };
        Self {
            directory: absolute
                .parent()
                .unwrap_or(std::path::Path::new("/"))
                .display()
                .to_string(),
            filename: absolute
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            error: String::new(),
            opening: false,
        }
    }
    fn show(&mut self, ctx: &egui::Context) -> (bool, Option<PathBuf>) {
        let mut open = true;
        let mut chosen = None;
        let mut cancel = false;
        egui::Window::new(if self.opening {
            "マップJSONを開く"
        } else {
            "マップJSONの保存先"
        })
        .open(&mut open)
        .default_width(500.0)
        .show(ctx, |ui| {
            ui.label("フォルダ（直接入力でも変更できます）");
            ui.text_edit_singleline(&mut self.directory);
            if ui.button("親フォルダへ").clicked()
                && let Some(parent) = std::path::Path::new(&self.directory).parent()
            {
                self.directory = parent.display().to_string();
            }
            match std::fs::read_dir(&self.directory) {
                Ok(entries) => {
                    let mut entries: Vec<_> = entries
                        .filter_map(|entry| entry.ok())
                        .filter(|entry| {
                            entry.path().is_dir()
                                || entry
                                    .path()
                                    .extension()
                                    .is_some_and(|s| s.eq_ignore_ascii_case("json"))
                        })
                        .collect();
                    entries.sort_by_key(|e| (!e.path().is_dir(), e.file_name()));
                    egui::ScrollArea::vertical()
                        .wheel_scroll_multiplier(scroll_multiplier(ui.ctx()))
                        .id_salt("recipe-folders")
                        .max_height(240.0)
                        .show(ui, |ui| {
                            for entry in entries {
                                let directory = entry.path().is_dir();
                                let name = entry.file_name().to_string_lossy().into_owned();
                                if ui
                                    .button(format!(
                                        "{} {name}",
                                        if directory {
                                            "[フォルダ]"
                                        } else {
                                            "[JSON]"
                                        }
                                    ))
                                    .clicked()
                                {
                                    if directory {
                                        self.directory = entry.path().display().to_string();
                                    } else {
                                        self.filename = name;
                                    }
                                }
                            }
                        });
                }
                Err(error) => {
                    ui.colored_label(ACCENT, error.to_string());
                }
            }
            ui.label("ファイル名");
            ui.text_edit_singleline(&mut self.filename);
            if !self.opening {
                ui.weak("同名JSONは上書きします。新しいフォルダは直接入力できます。");
            }
            ui.horizontal(|ui| {
                if ui
                    .button(if self.opening {
                        "このJSONを開く"
                    } else {
                        "ここへ保存"
                    })
                    .clicked()
                {
                    let filename = std::path::Path::new(&self.filename);
                    if self.directory.trim().is_empty()
                        || self.filename.trim().is_empty()
                        || filename.components().count() != 1
                        || !matches!(
                            filename.components().next(),
                            Some(std::path::Component::Normal(_))
                        )
                    {
                        self.error = "フォルダとファイル名を指定してください".into();
                    } else {
                        let mut path = PathBuf::from(&self.directory).join(filename);
                        if path.extension().is_none() {
                            path.set_extension("json");
                        }
                        if path
                            .extension()
                            .is_some_and(|s| s.eq_ignore_ascii_case("json"))
                        {
                            chosen = Some(path);
                        } else {
                            self.error = ".jsonのファイル名を指定してください".into();
                        }
                    }
                }
                cancel = ui.button("キャンセル").clicked();
            });
            ui.colored_label(ACCENT, &self.error);
        });
        (open && !cancel && chosen.is_none(), chosen)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum EdgeSlot {
    Image(usize),
    Parameter(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Edge {
    source: NodeId,
    target: NodeId,
    slot: EdgeSlot,
}
impl Edge {
    fn exists(&self, graph: &Graph) -> bool {
        graph
            .node(self.target)
            .is_some_and(|node| match &self.slot {
                EdgeSlot::Image(index) => node.inputs.get(*index) == Some(&Some(self.source)),
                EdgeSlot::Parameter(key) => node.parameters.get(key) == Some(&self.source),
            })
    }
    fn disconnect(&self, graph: &mut Graph) {
        if !self.exists(graph) {
            return;
        }
        let node = graph.node_mut(self.target).unwrap();
        match &self.slot {
            EdgeSlot::Image(index) => node.inputs[*index] = None,
            EdgeSlot::Parameter(key) => {
                node.parameters.remove(key);
            }
        }
    }
}

pub struct Editor {
    graph: Graph,
    selected: NodeId,
    selection: BTreeSet<NodeId>,
    selected_edge: Option<Edge>,
    marquee_start: Option<Pos2>,
    pan_drag: bool,
    zoom: f32,
    pan: Vec2,
    pending_add: Option<NodeKind>,
    fit: bool,
    wire: Option<NodeId>,
    revision: u64,
    signal: Arc<AtomicU64>,
    sender: mpsc::Sender<Work>,
    receiver: mpsc::Receiver<Completed>,
    completed: Option<Completed>,
    thumbnails: BTreeMap<NodeId, TextureHandle>,
    preview: Option<TextureHandle>,
    preview_key: Option<(u64, NodeId)>,
    preview_node: Option<NodeId>,
    path: String,
    png: String,
    recipe_seed: u64,
    recipe_sample: bool,
    neighbors: bool,
    tile_coordinate: [i32; 2],
    recipe_location: Option<JsonLocation>,
    message: String,
    search: String,
    undo: Vec<Graph>,
    redo: Vec<Graph>,
    edit_start: Option<Graph>,
    skip_history: bool,
}
impl Editor {
    pub fn new(cc: &eframe::CreationContext<'_>, graph: Graph, path: Option<String>) -> Self {
        let mut editor = Self::with_context(&cc.egui_ctx, graph);
        if let Some(path) = path {
            editor.path = path;
        }
        editor
    }
    fn with_context(ctx: &egui::Context, graph: Graph) -> Self {
        style(ctx);
        font(ctx);
        let (sender, rx) = mpsc::channel::<Work>();
        let (tx, receiver) = mpsc::channel();
        let signal = Arc::new(AtomicU64::new(1));
        let worker_signal = signal.clone();
        let context = ctx.clone();
        std::thread::spawn(move || {
            while let Ok(mut work) = rx.recv() {
                while let Ok(latest) = rx.try_recv() {
                    work = latest;
                }
                let (revision, graph, neighborhood_request) = work;
                let start = Instant::now();
                let cancelled = || worker_signal.load(Ordering::Relaxed) != revision;
                let result = graph.render(&cancelled);
                let neighborhood = neighborhood_request.map(|(seed, coord)| {
                    map_gen::world::generate_neighborhood(
                        &graph,
                        seed,
                        &BTreeMap::new(),
                        coord,
                        &cancelled,
                    )
                });
                if worker_signal.load(Ordering::Relaxed) != revision {
                    continue;
                }
                if tx
                    .send(Completed {
                        revision,
                        result,
                        neighborhood,
                        millis: start.elapsed().as_millis(),
                    })
                    .is_err()
                {
                    break;
                }
                context.request_repaint();
            }
        });
        let output = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("output");
        let selected = graph.output;
        let app = Self {
            graph,
            selected,
            selection: BTreeSet::from([selected]),
            selected_edge: None,
            marquee_start: None,
            pan_drag: false,
            zoom: 1.0,
            pan: Vec2::new(20.0, 20.0),
            pending_add: None,
            fit: false,
            wire: None,
            revision: 1,
            signal,
            sender,
            receiver,
            completed: None,
            thumbnails: BTreeMap::new(),
            preview: None,
            preview_key: None,
            preview_node: None,
            path: output.join("map.json").display().to_string(),
            png: output.join("terrain.png").display().to_string(),
            recipe_seed: 42,
            recipe_sample: false,
            neighbors: false,
            tile_coordinate: [0, 0],
            recipe_location: None,
            message: String::new(),
            search: String::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            edit_start: None,
            skip_history: false,
        };
        let _ = app.sender.send((1, app.graph.clone(), None));
        app
    }
    fn generation_error(&mut self, error: String) {
        self.message = error.clone();
        self.completed = Some(Completed {
            revision: self.revision,
            result: Err(error),
            neighborhood: None,
            millis: 0,
        });
        self.thumbnails.clear();
        self.preview_key = None;
    }
    fn request(&mut self) {
        self.revision += 1;
        self.signal.store(self.revision, Ordering::Relaxed);
        if let Err(e) = self.graph.validate() {
            self.generation_error(e);
            return;
        }
        let graph = if self.recipe_sample {
            match self
                .graph
                .resolve_parameters(Some(self.recipe_seed), &BTreeMap::new())
            {
                Ok(graph) => graph,
                Err(error) => {
                    self.generation_error(error);
                    return;
                }
            }
        } else {
            self.graph.clone()
        };
        if self
            .sender
            .send((
                self.revision,
                if self.neighbors {
                    self.graph.clone()
                } else {
                    graph
                },
                self.neighbors
                    .then_some((self.recipe_seed, self.tile_coordinate)),
            ))
            .is_err()
        {
            self.message = "生成ワーカーが終了しました".into();
        }
    }
    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(result) = self.receiver.try_recv() {
            if result.revision != self.revision {
                continue;
            }
            self.thumbnails.clear();
            if let Ok(images) = &result.result {
                for (&id, image) in images {
                    if let Ok(image) = image {
                        let bytes = rgba(image);
                        let rgb_image =
                            image::RgbaImage::from_raw(image.size as u32, image.size as u32, bytes)
                                .unwrap();
                        let thumb = image::imageops::resize(
                            &rgb_image,
                            96,
                            96,
                            image::imageops::FilterType::Triangle,
                        );
                        self.thumbnails.insert(
                            id,
                            ctx.load_texture(
                                format!("node-{id}"),
                                egui::ColorImage::from_rgba_unmultiplied([96, 96], thumb.as_raw()),
                                TextureOptions::LINEAR,
                            ),
                        );
                    }
                }
            }
            self.completed = Some(result);
            self.preview_key = None;
        }
    }
    fn current(&self) -> bool {
        self.completed
            .as_ref()
            .is_some_and(|c| c.revision == self.revision)
    }
    fn image(&self, id: NodeId) -> Option<&Arc<Image>> {
        self.completed
            .as_ref()?
            .result
            .as_ref()
            .ok()?
            .get(&id)?
            .as_ref()
            .ok()
    }
    fn preview_image(&self, id: NodeId) -> Option<&Arc<Image>> {
        if self.neighbors {
            self.completed
                .as_ref()?
                .neighborhood
                .as_ref()?
                .as_ref()
                .ok()
        } else {
            self.image(id)
        }
    }
    fn add(&mut self, kind: NodeKind) {
        // Defer until the canvas has its current size and navigation transform.
        self.pending_add = Some(kind);
    }
    fn add_at_center(&mut self, kind: NodeKind, canvas: Rect) {
        let position = (canvas.size() * 0.5 - self.pan) / self.zoom - Vec2::new(180.0, 290.0) * 0.5;
        match self.graph.add(kind, position.into()) {
            Ok(id) => {
                self.selected = id;
                self.selection = BTreeSet::from([id]);
                self.selected_edge = None;
                self.message = "出力端子 → 入力端子をドラッグ、または順にクリックして接続".into();
            }
            Err(e) => self.message = e,
        }
    }
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.colored_label(
                ACCENT,
                egui::RichText::new("▣  MAP GEN").strong().size(20.0),
            );
            ui.label(egui::RichText::new("NODE MATERIAL LAB").weak().size(10.0));
            ui.separator();
            if ui.button("デモに戻す").clicked() {
                self.graph = Graph::default();
                self.selected = self.graph.output;
                self.selection = BTreeSet::from([self.selected]);
                self.selected_edge = None;
                self.fit = true;
            }
            let undo = ui
                .add_enabled(!self.undo.is_empty(), egui::Button::new("↶ Undo"))
                .clicked();
            let redo = ui
                .add_enabled(!self.redo.is_empty(), egui::Button::new("↷ Redo"))
                .clicked();
            let text_focus = ui.ctx().egui_wants_keyboard_input();
            let redo = redo
                || (!text_focus
                    && ui.input_mut(|i| {
                        i.consume_key(
                            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                            egui::Key::Z,
                        )
                    }));
            let undo = undo
                || (!text_focus
                    && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)));
            if undo {
                if let Some(previous) = self.undo.pop() {
                    self.redo.push(self.graph.clone());
                    self.graph = previous;
                    self.selected_edge = None;
                    self.skip_history = true;
                    self.edit_start = None;
                }
            } else if redo && let Some(next) = self.redo.pop() {
                self.undo.push(self.graph.clone());
                self.graph = next;
                self.selected_edge = None;
                self.skip_history = true;
                self.edit_start = None;
            }
            if ui.button("100%").clicked() {
                self.pan *= 1.0 / self.zoom;
                self.zoom = 1.0;
            }
            if ui.button("全体表示").clicked() {
                self.fit = true;
            }
            ui.separator();
            ui.label("解像度");
            egui::ComboBox::from_id_salt("resolution")
                .selected_text(format!("{}²", self.graph.resolution))
                .show_ui(ui, |ui| {
                    for size in [64, 128, 256, 512, 1024] {
                        ui.selectable_value(
                            &mut self.graph.resolution,
                            size,
                            format!("{size} × {size}"),
                        );
                    }
                });
            ui.label("範囲");
            ui.add(
                egui::DragValue::new(&mut self.graph.extent)
                    .range(1.0..=4096.0)
                    .speed(1.0),
            );
        });
    }
    fn library(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.label(egui::RichText::new("NODE LIBRARY").strong().color(ACCENT));
        ui.add_space(8.0);
        ui.text_edit_singleline(&mut self.search);
        ui.add_space(10.0);
        ui.add_space(16.0);
        ui.separator();
        ui.strong("マップを保存 / 開く");
        ui.weak("1つのJSONで再編集とゲーム内生成に使えます。");
        ui.label("JSONファイル（フォルダ＋ファイル名）");
        ui.text_edit_singleline(&mut self.path);
        ui.weak("任意の絶対パスを入力できます。親フォルダは作成します。");
        if ui.button("保存先を選ぶ…").clicked() {
            self.recipe_location = Some(JsonLocation::new(&self.path));
        }
        if ui.button("JSONを保存").clicked() {
            self.message = match crate::save(std::path::Path::new(&self.path), &self.graph) {
                Ok(()) => format!("マップJSONを保存: {}", self.path),
                Err(error) => error.to_string(),
            };
        }
        if ui.button("JSONを開く…").clicked() {
            let mut dialog = JsonLocation::new(&self.path);
            dialog.opening = true;
            self.recipe_location = Some(dialog);
        }
        ui.weak("ノード位置・未使用の枝・数値入力も保存します。");
        ui.weak("同名JSONは上書きします。PNGは作りません。");
        let mut changed = ui
            .checkbox(&mut self.recipe_sample, "実行時の値で試す")
            .changed();
        ui.horizontal(|ui| {
            ui.label("実行Seed");
            changed |= ui
                .add(egui::DragValue::new(&mut self.recipe_seed))
                .changed();
            if ui.small_button("+1").clicked() {
                self.recipe_seed = self.recipe_seed.wrapping_add(1);
                changed = true;
            }
        });
        if changed {
            self.request();
        }
        let query = self.search.trim().to_lowercase();
        for group in [
            "数値入力",
            "形状・ノイズ",
            "配置・通路",
            "壁・入口",
            "領域処理",
            "画像加工",
            "地形ID・出力",
        ] {
            let nodes: Vec<_> = NodeKind::catalog()
                .into_iter()
                .filter(|kind| {
                    library_group(kind) == group
                        && (query.is_empty()
                            || kind.title().to_lowercase().contains(&query)
                            || group.contains(&query))
                })
                .collect();
            if nodes.is_empty() {
                continue;
            }
            let mut header = egui::CollapsingHeader::new(format!("{group} ({})", nodes.len()))
                .default_open(true);
            if !query.is_empty() {
                header = header.open(Some(true));
            }
            header.show(ui, |ui| {
                for kind in nodes {
                    if ui
                        .add_sized(
                            [ui.available_width(), 32.0],
                            egui::Button::new(format!("＋ {}", kind.title())),
                        )
                        .clicked()
                    {
                        self.add(kind);
                    }
                }
            });
        }
        ui.add_space(20.0);
        ui.separator();
        ui.label(egui::RichText::new("接続は出力 → 入力\n端子クリックでも接続\n右クリックで接続解除\n\nノードのどこでもドラッグで移動
ダブルクリックでプレビュー\n背景の左ドラッグで矩形選択\n2本指・中ボタンでパン\nピンチ・ホイールでズーム\n線をクリックして選択\nDelete: ノード/接続削除\nEsc: 接続キャンセル").weak().size(11.0));
    }
    fn inspector(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().wheel_scroll_multiplier(scroll_multiplier(ui.ctx())).show(ui, |ui| {
            ui.add_space(10.0);
            ui.label(egui::RichText::new("PROPERTIES").color(ACCENT).strong());
            ui.add_space(10.0);
            let input_nodes: Vec<_> = self.graph.nodes.iter().filter_map(|n| {
                if let NodeKind::Input(input) = &n.kind { Some((n.id, input.key.clone())) } else { None }
            }).collect();
            let Some(node) = self.graph.node_mut(self.selected) else {
                ui.label("ノードを選択してください");
                return;
            };
            ui.heading(node.kind.title());
            ui.label(
                egui::RichText::new(format!("NODE / {:03}", node.id))
                    .weak()
                    .size(11.0),
            );
            ui.text_edit_singleline(&mut node.name);
            ui.add_space(12.0);
            match &mut node.kind {
                NodeKind::Input(input) => {
                    ui.label("入力名（外部から指定するキー）");
                    ui.text_edit_singleline(&mut input.key);
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut input.mode, InputMode::Constant, "定数");
                        ui.selectable_value(&mut input.mode, InputMode::Range, "Min / Max");
                    });
                    if ui.checkbox(&mut input.integer, "整数").changed() && input.integer {
                        input.value = input.value.round(); input.min = input.min.round();
                        input.max = input.max.round(); input.preview = input.preview.round();
                    }
                    let integer = input.integer;
                    let number = |ui: &mut egui::Ui, label: &str, value: &mut f64| {
                        ui.horizontal(|ui| { ui.label(label); ui.add(egui::DragValue::new(value).speed(if integer { 1.0 } else { 0.01 }).max_decimals(if integer { 0 } else { 6 }).range(-1e9..=1e9)); });
                    };
                    match input.mode {
                        InputMode::Constant => number(ui, "定数", &mut input.value),
                        InputMode::Range => {
                            number(ui, "Min", &mut input.min);
                            number(ui, "Max", &mut input.max);
                            ui.checkbox(&mut input.random, "実行時に範囲内でランダム");
                            number(ui, "仮の入力値 / 既定値", &mut input.preview);
                        }
                    }
                    if !input.validate() { ui.colored_label(ACCENT, "入力名・Min/Max・仮の値を確認してください。"); }
                    ui.weak("使うノードを選び、PARAMETER INPUTSでこの入力を割り当てます。実行時の値で試す：OFFなら仮の値、ONならSeedに従います。");
                }
                NodeKind::Noise(noise) => {
                    combo(
                        ui,
                        "Noise type",
                        &mut noise.kind,
                        &[
                            NoiseKind::Simplex,
                            NoiseKind::Perlin,
                            NoiseKind::Value,
                            NoiseKind::Cellular,
                            NoiseKind::White,
                        ],
                    );
                    ui.horizontal(|ui| {
                        ui.label("Seed");
                        ui.add(egui::DragValue::new(&mut noise.seed));
                        if ui.small_button("+1").clicked() {
                            noise.seed = noise.seed.wrapping_add(1);
                        }
                    });
                    slider(ui, &mut noise.frequency, 0.0001..=1.0, "Frequency", true);
                    ui.add_enabled_ui(
                        !matches!(noise.kind, NoiseKind::Cellular | NoiseKind::White),
                        |ui| {
                            ui.label("Fractal / FBM");
                            ui.add(egui::Slider::new(&mut noise.octaves, 1..=8).text("Octaves"));
                            slider(ui, &mut noise.gain, 0.0..=1.0, "Gain", false);
                            slider(ui, &mut noise.lacunarity, 1.0..=4.0, "Lacunarity", false);
                        },
                    );
                }
                NodeKind::Shape(s) => {
                    combo(ui, "Shape type", &mut s.kind, &ShapeKind::ALL);
                    slider(ui, &mut s.scale_x, 0.01..=1.0, "Scale X", false);
                    slider(ui, &mut s.scale_y, 0.01..=1.0, "Scale Y", false);
                    slider(ui, &mut s.rotation, -180.0..=180.0, "Rotation / deg", false);
                    slider(ui, &mut s.softness, 0.0..=0.5, "Softness", false);
                }
                NodeKind::TileSampler(t) => {
                    ui.add(egui::Slider::new(&mut t.count_x, 1..=64).text("Count X"));
                    ui.add(egui::Slider::new(&mut t.count_y, 1..=64).text("Count Y"));
                    slider(ui, &mut t.scale, 0.01..=1.0, "Scale", false);
                    slider(ui, &mut t.scale_x, 0.01..=1.0, "Scale X", false);
                    slider(ui, &mut t.scale_y, 0.01..=1.0, "Scale Y", false);
                    slider(ui, &mut t.scale_random, 0.0..=1.0, "Scale random", false);
                    slider(
                        ui,
                        &mut t.position_random_x,
                        0.0..=1.0,
                        "Position random X",
                        false,
                    );
                    slider(
                        ui,
                        &mut t.position_random_y,
                        0.0..=1.0,
                        "Position random Y",
                        false,
                    );
                    slider(ui, &mut t.rotation, -180.0..=180.0, "Rotation / deg", false);
                    slider(
                        ui,
                        &mut t.rotation_random,
                        0.0..=180.0,
                        "Rotation random / deg",
                        false,
                    );
                    ui.horizontal(|ui| {
                        ui.label("Seed");
                        ui.add(egui::DragValue::new(&mut t.seed));
                        if ui.small_button("+1").clicked() {
                            t.seed = t.seed.wrapping_add(1);
                        }
                    });
                    ui.weak(
                        "Sourceを各セルに配置。端は反対側へ回り込み、重なりは最大値で合成します。",
                    );
                }
                NodeKind::ConnectRegions(c) => {
                    ui.add(egui::Slider::new(&mut c.width, 1..=64).text("Width / px"));
                    slider(ui, &mut c.distortion, 0.0..=0.25, "Distortion", false);
                    slider(ui, &mut c.frequency, 0.5..=8.0, "Bend frequency", false);
                    slider(ui, &mut c.cutoff, 0.01..=1.0, "Region cutoff", false);
                    ui.horizontal(|ui| {
                        ui.label("Seed"); ui.add(egui::DragValue::new(&mut c.seed));
                        if ui.small_button("+1").clicked() { c.seed = c.seed.wrapping_add(1); }
                    });
                    ui.checkbox(&mut c.include_source, "元のブロックも出力");
                    ui.weak("白い領域を検出して全体を接続。歪み0で直線。太さは出力ピクセル単位、歪みは画像の一辺に対する割合です。");
                }
                NodeKind::TerrainMerge { cutoff } => {
                    slider(ui, cutoff, 0.0..=1.0, "Mask cutoff", false);
                    ui.label("地形ID付きのBackgroundへForegroundを重ねます。前景の透明部分は背景を残します。");
                    ui.weak("任意のMaskで前景を使う範囲を指定。IDと色を混ぜず、1マスにつき1つを選びます。同じIDは同じ色にしてください。境界接続方法・幅はForegroundを採用します。");
                }
                NodeKind::Terrain(palette) => {
                    ui.weak("値の上限・地形ID・表示色。上限は昇順、最後は1。IDは重複不可。");
                    let mut remove = None;
                    for (index, band) in palette.bands.iter_mut().enumerate() {
                        ui.push_id(index, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(format!("#{}", index + 1));
                                if ui.small_button("削除").clicked() { remove = Some(index); }
                            });
                            ui.add(egui::DragValue::new(&mut band.max).range(0.0..=1.0).speed(0.005).prefix("上限 "));
                            ui.text_edit_singleline(&mut band.id);
                            ui.color_edit_button_srgb(&mut band.color);
                            egui::ComboBox::from_id_salt("seam-mode").selected_text(band.seam.mode.label()).show_ui(ui, |ui| {
                                for mode in map_gen::terrain::SeamMode::ALL { ui.selectable_value(&mut band.seam.mode, mode, mode.label()); }
                            });
                            ui.add(egui::DragValue::new(&mut band.seam.width).range(1..=128).prefix("境界幅（マス） "));
                            ui.weak("3×3生成と本編のタイル座標指定で適用。道は同じIDの近い道へ接続。");
                        });
                        ui.separator();
                    }
                    if let Some(index) = remove { palette.bands.remove(index); }
                    if palette.bands.len() < 32 && ui.button("地形を追加").clicked() {
                        let previous = palette.bands.iter().rev().nth(1).map_or(0.0, |b| b.max);
                        if let Some(last) = palette.bands.last_mut() { last.max = (last.max + previous) * 0.5; }
                        let mut index = palette.bands.len();
                        while palette.bands.iter().any(|b| b.id == format!("terrain_{index}")) { index += 1; }
                        palette.bands.push(map_gen::terrain::TerrainBand { seam: Default::default(), max: 1.0, id: format!("terrain_{index}"), color: [160,160,160] });
                    }
                    if !palette.validate() { ui.colored_label(ACCENT, "上限・IDを確認してください（昇順・最終値1・ID一意）。"); }
                    ui.weak("出力は地形ID付き。画像加工はこのノードの前、後段はOutput・Black to Alpha・Terrain Mergeへ接続します。");
                }
                NodeKind::Map(node) => map_properties(ui, node),
                NodeKind::Constant { value } => slider(ui, value, 0.0..=1.0, "Value", false),
                NodeKind::Blend { mode, opacity } => {
                    combo(ui, "Blend mode", mode, &BlendMode::ALL);
                    slider(ui, opacity, 0.0..=1.0, "Opacity", false);
                    ui.label("Mask未接続は白（効果100%）です。");
                }
                NodeKind::Levels {
                    contrast,
                    brightness,
                    gamma,
                } => {
                    slider(ui, contrast, 0.0..=4.0, "Contrast", false);
                    slider(ui, brightness, -1.0..=1.0, "Brightness", false);
                    slider(ui, gamma, 0.1..=4.0, "Gamma", false);
                }
                NodeKind::Threshold { cutoff, softness } => {
                    slider(ui, cutoff, 0.0..=1.0, "Cutoff", false);
                    slider(ui, softness, 0.0..=0.5, "Softness", false);
                }
                NodeKind::Blur { radius } => {
                    ui.add(egui::Slider::new(radius, 0..=32).text("Radius / px"));
                }
                NodeKind::Warp { strength } => {
                    slider(ui, strength, 0.0..=0.25, "Strength", false);
                    ui.label("変位画像で斜め45度へ歪めます。");
                }
                NodeKind::BlackToAlpha => {
                    ui.label("RGBが完全な黒（0,0,0）のピクセルを透明にします。既存の透明度・地形IDは保持します。");
                    ui.weak("画像加工の最後に置き、Outputへ接続してください。Terrain Paletteの前後にも置けます。");
                }
                NodeKind::Invert => {
                    ui.label("入力画像の値を反転します。");
                }
                NodeKind::Output => {
                    ui.label("接続した画像を出力します。");
                }
            }
            let mut bindings = Vec::new();
            let fields = crate::recipe::numeric_parameters(&node.kind);
            if !fields.is_empty() {
                ui.separator();
                ui.strong("PARAMETER INPUTS");
                ui.weak("入力ノードを割り当てると、その数値設定を置き換えます。");
                for key in fields {
                    let mut selected = node.parameters.get(&key).copied();
                    let before = selected;
                    let label = input_nodes.iter().find(|(id, _)| Some(*id) == selected).map_or("直接設定", |(_, key)| key.as_str());
                    egui::ComboBox::from_id_salt(("parameter", node.id, &key)).selected_text(label).show_ui(ui, |ui| {
                        ui.selectable_value(&mut selected, None, "直接設定");
                        for (id, key) in &input_nodes { ui.selectable_value(&mut selected, Some(*id), key); }
                    });
                    ui.label(&key);
                    if before != selected { bindings.push((key, selected)); }
                }
            }
            ui.add_space(10.0);
            ui.separator();
            ui.label("INPUTS");
            for (index, label) in node.kind.inputs().iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(*label);
                    let source = node.inputs[index];
                    ui.weak(source.map_or_else(|| "—".into(), |id| format!("#{id}")));
                    if source.is_some() && ui.small_button("×").clicked() {
                        node.inputs[index] = None;
                    }
                });
            }
            ui.add_space(10.0);
            let delete = ui.button("ノードを削除").clicked();
            if ui.button("このノードを最終出力にする").clicked() {
                self.graph.output = self.selected;
            }
            for (key, source) in bindings {
                if let Err(error) = self.graph.bind_parameter(self.selected, &key, source) { self.message = error; }
            }
            if delete {
                self.graph.remove(self.selected);
                self.selected = self.graph.output;
            }
            ui.add_space(12.0);
            ui.separator();
            ui.collapsing("画像を書き出す（任意）", |ui| {
                ui.label(if self.neighbors { "PNG / 隣接3×3" } else { "PNG / 最終出力" });
                ui.text_edit_singleline(&mut self.png);
                if ui
                    .add_enabled(
                        self.current() && self.preview_image(self.graph.output).is_some(),
                        egui::Button::new("PNGを書き出す"),
                    )
                    .clicked()
                {
                    let path = std::path::Path::new(&self.png);
                    let result =
                        crate::export(path, self.preview_image(self.graph.output).unwrap());
                    self.message = match result {
                        Ok(()) => "PNGを書き出しました".into(),
                        Err(e) => e.to_string(),
                    };
                }
                ui.weak("保存・書出しは同名ファイルを上書きします。");
            });
        });
    }
    fn preview_panel(&mut self, ui: &mut egui::Ui) {
        let id = self.preview_node.unwrap_or(self.graph.output);
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(ACCENT, "2D VIEW");
            ui.separator();
            let mut changed = ui.checkbox(&mut self.neighbors, "隣接3×3（9枚）").changed();
            if self.neighbors {
                ui.label("中心タイル X / Y");
                for v in &mut self.tile_coordinate {
                    changed |= ui
                        .add(egui::DragValue::new(v).range(-1000000..=1000000))
                        .changed();
                }
                ui.weak("最終出力・実行Seedを使用");
            }
            if changed {
                self.preview_key = None;
                self.request();
            }
            if ui.button("選択ノード").clicked() {
                self.preview_node = Some(self.selected);
            }
            if ui.button("最終出力").clicked() {
                self.preview_node = None;
            }
            ui.weak("ノードをダブルクリックで表示");
            ui.label(format!("NODE #{id}"));
            if let Some(c) = &self.completed {
                ui.weak(format!("{} ms", c.millis));
            }
            if !self.current() {
                ui.colored_label(ACCENT, "再計算中… 前回の結果を表示");
            }
        });
        let key = (self.completed.as_ref().map_or(0, |c| c.revision), id);
        if self.preview_key != Some(key) {
            self.preview_key = Some(key);
            self.preview = self.preview_image(id).map(|image| {
                let bytes = rgba(image);
                ui.ctx().load_texture(
                    "large-preview",
                    egui::ColorImage::from_rgba_unmultiplied([image.size, image.size], &bytes),
                    TextureOptions::NEAREST,
                )
            });
        }
        // A normal horizontal row starts with only one line of height. Reserve
        // the whole panel body before sizing the image so it cannot be clipped.
        let body_size = ui.available_size();
        ui.allocate_ui_with_layout(
            body_size,
            egui::Layout::left_to_right(egui::Align::Min),
            |ui| {
                if let Some(texture) = &self.preview {
                    let side = body_size.y.min((body_size.x - 200.0).max(0.0)).max(0.0);
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
                    checkerboard(ui.painter(), rect, 16.0);
                    ui.painter().image(
                        texture.id(),
                        rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color::WHITE,
                    );
                    if self.neighbors {
                        for i in 0..=3 {
                            let t = i as f32 / 3.0;
                            ui.painter().line_segment(
                                [
                                    rect.min + Vec2::new(rect.width() * t, 0.0),
                                    rect.min + Vec2::new(rect.width() * t, rect.height()),
                                ],
                                Stroke::new(1.0, ACCENT),
                            );
                            ui.painter().line_segment(
                                [
                                    rect.min + Vec2::new(0.0, rect.height() * t),
                                    rect.min + Vec2::new(rect.width(), rect.height() * t),
                                ],
                                Stroke::new(1.0, ACCENT),
                            );
                        }
                        for y in 0..3 {
                            for x in 0..3 {
                                ui.painter().text(
                                    rect.min
                                        + Vec2::new(
                                            rect.width() * (x as f32 + 0.05) / 3.0,
                                            rect.height() * (y as f32 + 0.05) / 3.0,
                                        ),
                                    egui::Align2::LEFT_TOP,
                                    format!(
                                        "{},{}",
                                        self.tile_coordinate[0] + x - 1,
                                        self.tile_coordinate[1] + y - 1
                                    ),
                                    FontId::monospace(12.0),
                                    Color::WHITE,
                                );
                            }
                        }
                    }
                    if let Some(pos) = response.hover_pos()
                        && let Some(image) = self.preview_image(id)
                    {
                        let rel = (pos - response.rect.min) / response.rect.size();
                        let x = ((rel.x * image.size as f32) as usize).min(image.size - 1);
                        let y = ((rel.y * image.size as f32) as usize).min(image.size - 1);
                        let value = image.pixels[y * image.size + x];
                        let terrain_id = image
                            .terrain
                            .as_ref()
                            .map_or("", |p| p.band(value).id.as_str());
                        response.on_hover_text(format!("({x}, {y}) = {value:.6} {terrain_id}"));
                    }
                }
                ui.vertical(|ui| {
                    if let Some(image) = self.preview_image(id) {
                        let min = image.pixels.iter().copied().fold(1.0_f32, f32::min);
                        let max = image.pixels.iter().copied().fold(0.0_f32, f32::max);
                        ui.strong(self.graph.node(id).map_or("Unknown", |n| n.name.as_str()));
                        ui.label(format!(
                            "{} × {} · scalar / terrain ID",
                            image.size, image.size
                        ));
                        ui.weak(format!("MIN  {min:.4}\nMAX  {max:.4}"));
                        ui.add_space(10.0);
                        ui.weak("Terrain Paletteで値を色と地形IDへ分類できます。");
                    } else if let Some(c) = &self.completed {
                        let error = if self.neighbors {
                            c.neighborhood
                                .as_ref()
                                .and_then(|r| r.as_ref().err().map(String::as_str))
                        } else {
                            match &c.result {
                                Err(e) => Some(e.as_str()),
                                Ok(images) => images
                                    .get(&id)
                                    .and_then(|i| i.as_ref().err().map(String::as_str)),
                            }
                        };
                        if let Some(error) = error {
                            ui.colored_label(Color::from_rgb(239, 129, 124), error);
                        }
                    }
                });
            },
        );
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, BG);
        if self.fit && !self.graph.nodes.is_empty() {
            let min = self
                .graph
                .nodes
                .iter()
                .fold(Vec2::splat(f32::INFINITY), |a, n| {
                    a.min(Vec2::from(n.position))
                });
            let max = self
                .graph
                .nodes
                .iter()
                .fold(Vec2::splat(f32::NEG_INFINITY), |a, n| {
                    a.max(Vec2::from(n.position) + Vec2::new(180.0, 290.0))
                });
            let size = max - min;
            self.zoom = ((rect.width() - 50.0) / size.x)
                .min((rect.height() - 50.0) / size.y)
                .clamp(0.25, 1.3);
            self.pan = (rect.size() - size * self.zoom) * 0.5 - min * self.zoom;
            self.fit = false;
        }
        // Primary drag selects; middle drag pans. Node/port widgets take priority.
        if response.dragged_by(egui::PointerButton::Middle) {
            self.pan += response.drag_delta();
        }
        if response.drag_started_by(egui::PointerButton::Primary) {
            self.pan_drag = ui.input(|i| i.modifiers.alt);
            if !self.pan_drag {
                self.marquee_start = ui.input(|i| i.pointer.press_origin());
                self.selection.clear();
                self.selected_edge = None;
                self.wire = None;
            }
        }
        if response.dragged_by(egui::PointerButton::Primary) && self.pan_drag {
            self.pan += response.drag_delta();
        }
        if !ui.input(|i| i.pointer.primary_down()) {
            self.pan_drag = false;
        }
        // Use the event source to distinguish trackpad motion from mouse wheels.
        // contains_pointer also permits navigation while hovering a node or port.
        if response.contains_pointer() {
            let (pan, factor) = ui.input(|i| {
                let mut pan = Vec2::ZERO;
                let mut factor = 1.0;
                for event in &i.events {
                    match event {
                        egui::Event::MouseWheel {
                            unit,
                            delta,
                            source,
                            ..
                        } => {
                            let trackpad = matches!(
                                source,
                                egui::MouseWheelSource::Trackpad | egui::MouseWheelSource::Momentum
                            ) || (*source == egui::MouseWheelSource::Unknown
                                && *unit == egui::MouseWheelUnit::Point);
                            if trackpad {
                                pan += *delta;
                            } else {
                                let points = match unit {
                                    egui::MouseWheelUnit::Point => delta.y,
                                    egui::MouseWheelUnit::Line => delta.y * 40.0,
                                    egui::MouseWheelUnit::Page => delta.y * rect.height(),
                                };
                                factor *= (points * 0.002).exp();
                            }
                        }
                        egui::Event::Zoom(scale) => factor *= scale,
                        _ => {}
                    }
                }
                (pan, factor)
            });
            self.pan += pan;
            if factor != 1.0 && factor.is_finite() && factor > 0.0 {
                let old = self.zoom;
                self.zoom = (old * factor).clamp(0.25, 1.8);
                if let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) {
                    let local = pointer - rect.min;
                    self.pan = local - (local - self.pan) * (self.zoom / old);
                }
            }
        }
        response.context_menu(|ui| {
            ui.label("Add node");
            for kind in NodeKind::catalog() {
                if ui.button(kind.title()).clicked() {
                    self.add(kind);
                    ui.close();
                }
            }
        });
        let step = 24.0 * self.zoom;
        let x0 = rect.left() + self.pan.x.rem_euclid(step);
        let y0 = rect.top() + self.pan.y.rem_euclid(step);
        let dot = Color::from_rgb(49, 54, 65);
        for x in 0..(rect.width() / step) as i32 + 1 {
            for y in 0..(rect.height() / step) as i32 + 1 {
                painter.circle_filled(
                    Pos2::new(x0 + x as f32 * step, y0 + y as f32 * step),
                    0.8,
                    dot,
                );
            }
        }
        if let Some(kind) = self.pending_add.take() {
            self.add_at_center(kind, rect);
        }
        let origin = rect.min + self.pan;
        let z = self.zoom;
        let position = |node: &Node| origin + Vec2::from(node.position) * z;
        let out = |node: &Node| position(node) + Vec2::new(180.0, 62.0) * z;
        let input =
            |node: &Node, i: usize| position(node) + Vec2::new(0.0, 214.0 + i as f32 * 18.0) * z;
        let mut edges = Vec::new();
        for node in &self.graph.nodes {
            for (index, source) in node.inputs.iter().enumerate() {
                if let Some(source) = source.and_then(|id| self.graph.node(id)) {
                    edges.push((
                        Edge {
                            source: source.id,
                            target: node.id,
                            slot: EdgeSlot::Image(index),
                        },
                        out(source),
                        input(node, index),
                        GREEN,
                    ));
                }
            }
            for (key, source) in &node.parameters {
                if let Some(source) = self.graph.node(*source) {
                    edges.push((
                        Edge {
                            source: source.id,
                            target: node.id,
                            slot: EdgeSlot::Parameter(key.clone()),
                        },
                        out(source),
                        position(node) + Vec2::new(0.0, 15.0) * z,
                        ACCENT,
                    ));
                }
            }
        }
        let pointer = ui.input(|i| i.pointer.hover_pos());
        // Only canvas clicks select wires: node bodies and ports take priority.
        if response.clicked_by(egui::PointerButton::Primary) {
            self.selected_edge = pointer.and_then(|pointer| {
                edges
                    .iter()
                    .filter_map(|(edge, a, b, _)| {
                        let distance = curve_distance(*a, *b, z, pointer);
                        (distance <= 7.0).then_some((edge, distance))
                    })
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(edge, _)| edge.clone())
            });
        }
        for (edge, a, b, color) in &edges {
            let selected = self.selected_edge.as_ref() == Some(edge);
            if selected {
                curve(&painter, *a, *b, Color::WHITE, z, 5.0);
            }
            curve(&painter, *a, *b, *color, z, 2.0);
        }
        let mut connect_to = None;
        let mut disconnect = None;
        let mut movement = None;
        let mut wire_started = false;
        for node in &self.graph.nodes {
            let top = position(node);
            let bounds = Rect::from_min_size(top, Vec2::new(180.0, 290.0) * z);
            if !rect.intersects(bounds) {
                continue;
            }
            let selected = self.selection.contains(&node.id);
            painter.rect_filled(bounds, 6.0 * z, Color::from_rgb(37, 42, 51));
            painter.rect_stroke(
                bounds,
                6.0 * z,
                Stroke::new(
                    if selected { 2.0 } else { 1.0 },
                    if selected {
                        ACCENT
                    } else {
                        Color::from_rgb(62, 69, 82)
                    },
                ),
                StrokeKind::Inside,
            );
            let header = Rect::from_min_size(top, Vec2::new(180.0, 30.0) * z);
            painter.rect_filled(
                header,
                5.0 * z,
                match node.kind.category() {
                    "GENERATORS" => Color::from_rgb(53, 78, 79),
                    "COMPOSITE" => Color::from_rgb(76, 61, 91),
                    "OUTPUT" => Color::from_rgb(104, 78, 44),
                    _ => Color::from_rgb(56, 68, 90),
                },
            );
            let mut title = egui::text::LayoutJob::simple(
                node.name.clone(),
                FontId::proportional(18.0 * z),
                Color::WHITE,
                160.0 * z,
            );
            title.wrap.max_rows = 1;
            let title = painter.layout_job(title);
            painter.galley(
                top + Vec2::new(10.0 * z, (30.0 * z - title.size().y) * 0.5),
                title,
                Color::WHITE,
            );
            let body_response = ui.interact(
                bounds,
                ui.scope_id().with(("node", node.id)),
                Sense::click_and_drag(),
            );
            if body_response.clicked()
                || body_response.drag_started_by(egui::PointerButton::Primary)
            {
                self.selected_edge = None;
                self.selected = node.id;
                if !self.selection.contains(&node.id) {
                    self.selection = BTreeSet::from([node.id]);
                }
            }
            if body_response.double_clicked() {
                self.preview_node = Some(node.id);
                self.preview_key = None;
            }
            if body_response.dragged_by(egui::PointerButton::Primary) {
                movement = Some((node.id, body_response.drag_delta() / z));
            }
            body_response.on_hover_text(&node.name);
            let image_rect =
                Rect::from_min_size(top + Vec2::new(10.0, 38.0) * z, Vec2::new(160.0, 160.0) * z);
            checkerboard(&painter, image_rect, 10.0 * z);
            if let NodeKind::Input(input) = &node.kind {
                painter.text(
                    image_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!(
                        "{}\n{}",
                        input.key,
                        input
                            .resolve(self.recipe_sample.then_some(self.recipe_seed), None)
                            .unwrap_or(input.preview_value())
                    ),
                    FontId::monospace(14.0 * z),
                    ACCENT,
                );
            } else if let Some(texture) = self.thumbnails.get(&node.id) {
                painter.image(
                    texture.id(),
                    image_rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color::WHITE,
                );
            } else {
                painter.text(
                    image_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "NO INPUT",
                    FontId::monospace(14.0 * z),
                    Color::GRAY,
                );
            }
            let output = out(node);
            painter.circle_filled(output, 5.5 * z, GREEN);
            let r = Rect::from_center_size(output, Vec2::splat(20.0 * z));
            let port = ui.interact(
                r,
                ui.scope_id().with(("out", node.id)),
                Sense::click_and_drag(),
            );
            if port.clicked() || port.drag_started() {
                self.selected_edge = None;
                self.wire = Some(node.id);
                wire_started = true;
            }
            port.on_hover_text("出力: ドラッグまたはクリックで接続");
            for (index, label) in node.kind.inputs().iter().enumerate() {
                let p = input(node, index);
                let connected = node.inputs[index].is_some();
                painter.circle_filled(
                    p,
                    5.5 * z,
                    if connected {
                        GREEN
                    } else {
                        Color::from_rgb(80, 87, 101)
                    },
                );
                painter.text(
                    p + Vec2::new(11.0, 0.0) * z,
                    egui::Align2::LEFT_CENTER,
                    label,
                    FontId::proportional(14.0 * z),
                    Color::from_gray(200),
                );
                let r = Rect::from_center_size(p, Vec2::splat(22.0 * z));
                let port = ui.interact(
                    r,
                    ui.scope_id().with(("input", node.id, index)),
                    Sense::click(),
                );
                if port.secondary_clicked() {
                    disconnect = Some((node.id, index));
                }
                if self.wire.is_some()
                    && (port.clicked()
                        || ui.input(|i| i.pointer.any_released())
                            && pointer.is_some_and(|p| r.contains(p)))
                {
                    connect_to = Some((node.id, index));
                }
                port.on_hover_text("入力: 出力端子から接続 / 右クリックで解除");
            }
            if node.id == self.graph.output {
                painter.text(
                    top + Vec2::new(12.0, 276.0) * z,
                    egui::Align2::LEFT_CENTER,
                    "● FINAL OUTPUT",
                    FontId::monospace(13.0 * z),
                    ACCENT,
                );
            }
        }
        if let Some(start) = self.marquee_start {
            if let Some(pointer) = pointer {
                let area = Rect::from_two_pos(start, pointer);
                painter.rect_filled(area, 0.0, ACCENT.gamma_multiply(0.12));
                painter.rect_stroke(area, 0.0, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
                self.selection = self
                    .graph
                    .nodes
                    .iter()
                    .filter(|node| {
                        let bounds =
                            Rect::from_min_size(position(node), Vec2::new(180.0, 290.0) * z);
                        area.intersects(bounds)
                    })
                    .map(|node| node.id)
                    .collect();
                if let Some(&id) = self.selection.first() {
                    self.selected = id;
                }
            }
            if !ui.input(|i| i.pointer.primary_down()) {
                self.marquee_start = None;
            }
        }
        if let Some((_, delta)) = movement {
            // Clamp the shared delta, keeping relative positions intact at the limits.
            let mut delta = delta;
            for node in self
                .graph
                .nodes
                .iter()
                .filter(|n| self.selection.contains(&n.id))
            {
                delta.x = delta
                    .x
                    .clamp(-100000.0 - node.position[0], 100000.0 - node.position[0]);
                delta.y = delta
                    .y
                    .clamp(-100000.0 - node.position[1], 100000.0 - node.position[1]);
            }
            for node in self
                .graph
                .nodes
                .iter_mut()
                .filter(|n| self.selection.contains(&n.id))
            {
                node.position[0] += delta.x;
                node.position[1] += delta.y;
            }
        }
        if let Some((id, index)) = disconnect {
            self.graph.node_mut(id).unwrap().inputs[index] = None;
        }
        if let Some((id, index)) = connect_to
            && let Some(source) = self.wire.take()
        {
            self.message = match self.graph.connect(source, id, index) {
                Ok(()) => "接続しました".into(),
                Err(e) => e,
            };
        }
        if let Some(source) = self.wire.and_then(|id| self.graph.node(id))
            && let Some(pointer) = pointer
        {
            curve(&painter, out(source), pointer, ACCENT, z, 2.0);
        }
        if response.clicked_by(egui::PointerButton::Primary) && !wire_started {
            self.wire = None;
            self.selection.clear();
        }
        if !ui.ctx().egui_wants_keyboard_input() {
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.wire = None;
                self.marquee_start = None;
                self.selected_edge = None;
            }
            if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
            {
                if let Some(edge) = self.selected_edge.take() {
                    edge.disconnect(&mut self.graph);
                } else {
                    for id in std::mem::take(&mut self.selection) {
                        self.graph.remove(id);
                    }
                }
                self.selected = self.graph.output;
                self.wire = None;
            }
        }
        painter.text(
            rect.left_top() + Vec2::new(14.0, 12.0),
            egui::Align2::LEFT_TOP,
            format!(
                "GRAPH  /  {} NODES  /  {:.0}%  /  {} SELECTED",
                self.graph.nodes.len(),
                self.zoom * 100.0,
                if self.selected_edge.is_some() {
                    "1 EDGE".into()
                } else {
                    format!("{} NODES", self.selection.len())
                }
            ),
            FontId::monospace(11.0),
            Color::from_gray(135),
        );
    }
}
impl Editor {
    fn render_ui(&mut self, ui: &mut egui::Ui) {
        self.poll(ui.ctx());
        self.skip_history = false;
        let before = self.graph.clone();
        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.weak("BRACKET-NOISE  /  CPU GRAPH");
                ui.separator();
                ui.label(&self.message);
            });
        });
        egui::Panel::left("library")
            .default_size(175.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .wheel_scroll_multiplier(scroll_multiplier(ui.ctx()))
                    .show(ui, |ui| self.library(ui));
            });
        egui::Panel::right("inspector")
            .default_size(280.0)
            .show(ui, |ui| self.inspector(ui));
        // Reset the old content-sized panel state once with the new ID.
        // Keep enough room above it to operate the graph on smaller windows.
        let preview_max_height = (ui.available_height() - 220.0).max(240.0);
        egui::Panel::bottom("preview_full_image")
            .default_size(760.0)
            .min_size(240.0)
            .max_size(preview_max_height)
            .resizable(true)
            .show(ui, |ui| self.preview_panel(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.canvas(ui));
        if let Some(dialog) = &mut self.recipe_location {
            let opening = dialog.opening;
            let (open, chosen) = dialog.show(ui.ctx());
            if let Some(path) = chosen {
                if opening {
                    match crate::load(&path) {
                        Ok(graph) => {
                            self.graph = graph;
                            self.path = path.display().to_string();
                            self.selected = self.graph.output;
                            self.selection = BTreeSet::from([self.selected]);
                            self.selected_edge = None;
                            self.marquee_start = None;
                            self.fit = true;
                            self.wire = None;
                            self.preview_node = None;
                            self.request();
                            self.message = "マップJSONを開きました".into();
                        }
                        Err(error) => self.message = error.to_string(),
                    }
                } else {
                    self.path = path.display().to_string();
                    self.message = match crate::save(&path, &self.graph) {
                        Ok(()) => format!("マップJSONを保存: {}", path.display()),
                        Err(error) => error.to_string(),
                    };
                }
            }
            if !open {
                self.recipe_location = None;
            }
        }
        if self.graph != before {
            if !self.skip_history {
                if self.edit_start.is_none() {
                    self.edit_start = Some(before.clone());
                }
                self.redo.clear();
            }
            if !self.graph.same_computation(&before) {
                self.request();
            }
        }
        if !ui.input(|i| i.pointer.any_down())
            && let Some(start) = self.edit_start.take()
        {
            self.undo.push(start);
            if self.undo.len() > 100 {
                self.undo.remove(0);
            }
        }
        self.selection.retain(|id| self.graph.node(*id).is_some());
        if self
            .selected_edge
            .as_ref()
            .is_some_and(|edge| !edge.exists(&self.graph))
        {
            self.selected_edge = None;
        }
        if self.graph.node(self.selected).is_none() {
            self.selected = self.graph.output;
        }
        if self
            .preview_node
            .is_some_and(|id| self.graph.node(id).is_none())
        {
            self.preview_node = None;
        }
    }
}
impl eframe::App for Editor {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render_ui(ui);
    }
}
impl Drop for Editor {
    fn drop(&mut self) {
        self.signal.fetch_add(1, Ordering::Relaxed);
    }
}
fn checkerboard(painter: &egui::Painter, rect: Rect, cell: f32) {
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    for y in 0..(rect.height() / cell).ceil() as usize {
        for x in 0..(rect.width() / cell).ceil() as usize {
            let tile = Rect::from_min_size(
                rect.min + Vec2::new(x as f32 * cell, y as f32 * cell),
                Vec2::splat(cell),
            );
            painter.rect_filled(
                tile,
                0.0,
                Color::from_gray(if (x + y) % 2 == 0 { 50 } else { 75 }),
            );
        }
    }
}

fn library_group(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Input(_) => "数値入力",
        NodeKind::Noise(_) | NodeKind::Shape(_) | NodeKind::Constant { .. } => "形状・ノイズ",
        NodeKind::TileSampler(_)
        | NodeKind::ConnectRegions(_)
        | NodeKind::Map(MapNode::Scatter { .. } | MapNode::ExtraConnections { .. }) => "配置・通路",
        NodeKind::Map(
            MapNode::Outline { .. } | MapNode::Entrances { .. } | MapNode::Partition { .. },
        ) => "壁・入口",
        NodeKind::Map(_) => "領域処理",
        NodeKind::Terrain(_) | NodeKind::TerrainMerge { .. } | NodeKind::Output => "地形ID・出力",
        _ => "画像加工",
    }
}

// Keep trackpad and momentum scrolling natural; reverse only mouse-wheel input.
fn scroll_multiplier(ctx: &egui::Context) -> Vec2 {
    let trackpad = ctx.input(|input| {
        input.events.iter().rev().find_map(|event| {
            if let egui::Event::MouseWheel { unit, source, .. } = event {
                Some(
                    matches!(
                        source,
                        egui::MouseWheelSource::Trackpad | egui::MouseWheelSource::Momentum
                    ) || (*source == egui::MouseWheelSource::Unknown
                        && *unit == egui::MouseWheelUnit::Point),
                )
            } else {
                None
            }
        })
    });
    // Retain the source while egui applies smoothed scrolling between events.
    let natural = ctx.data_mut(|data| {
        let id = egui::Id::unique("scroll-input-is-trackpad");
        if let Some(trackpad) = trackpad {
            data.insert_temp(id, trackpad);
        }
        data.get_temp::<bool>(id).unwrap_or(true)
    });
    Vec2::splat(if natural { 1.0 } else { -1.0 })
}

fn curve_points(a: Pos2, b: Pos2, zoom: f32) -> [Pos2; 4] {
    let dx = ((a.x - b.x).abs() * 0.45).max(45.0 * zoom);
    [a, a + Vec2::new(dx, 0.0), b - Vec2::new(dx, 0.0), b]
}
fn curve_distance(a: Pos2, b: Pos2, zoom: f32, pointer: Pos2) -> f32 {
    let [a, c, d, b] = curve_points(a, b, zoom);
    let mut previous = a;
    let mut distance = f32::INFINITY;
    // Sample in screen coordinates so the click tolerance stays usable at every zoom.
    let steps = ((a.distance(c) + c.distance(d) + d.distance(b)) / 4.0)
        .ceil()
        .max(16.0) as usize;
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        let u = 1.0 - t;
        let point = Pos2::ZERO
            + a.to_vec2() * u.powi(3)
            + c.to_vec2() * (3.0 * u * u * t)
            + d.to_vec2() * (3.0 * u * t * t)
            + b.to_vec2() * t.powi(3);
        let segment = point - previous;
        let fraction = ((pointer - previous).dot(segment) / segment.length_sq().max(f32::EPSILON))
            .clamp(0.0, 1.0);
        distance = distance.min(pointer.distance(previous + segment * fraction));
        previous = point;
    }
    distance
}
fn curve(p: &egui::Painter, a: Pos2, b: Pos2, color: Color, zoom: f32, width: f32) {
    p.add(egui::epaint::CubicBezierShape::from_points_stroke(
        curve_points(a, b, zoom),
        false,
        Color::TRANSPARENT,
        Stroke::new(width * zoom, color),
    ));
}
fn slider(
    ui: &mut egui::Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    label: &str,
    log: bool,
) {
    ui.add(egui::Slider::new(value, range).text(label).logarithmic(log));
}
fn combo<T: Copy + PartialEq + std::fmt::Debug>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    all: &[T],
) {
    egui::ComboBox::from_label(label)
        .selected_text(format!("{value:?}"))
        .show_ui(ui, |ui| {
            for &option in all {
                ui.selectable_value(value, option, format!("{option:?}"));
            }
        });
}
fn map_properties(ui: &mut egui::Ui, node: &mut MapNode) {
    match node {
        MapNode::Morphology { mode, radius, .. } => {
            combo(ui, "Mode", mode, &[MorphMode::Dilate, MorphMode::Erode]);
            ui.add(egui::Slider::new(radius, 0..=64).text("Radius / px"));
            ui.weak("白い領域を円形距離で拡大 / 縮小します。画像外は黒です。");
        }
        MapNode::Outline { width, .. } => {
            ui.add(egui::Slider::new(width, 1..=64).text("Wall width / px"));
            ui.weak("白い領域の外側に壁のマスクを生成します。");
        }
        MapNode::Distance { range, inside, .. } => {
            slider(ui, range, 1.0..=1024.0, "Distance range / px", false);
            ui.checkbox(inside, "白い領域の内側の距離");
            ui.weak("ON：壁までの距離。OFF：白い領域までの距離。Rangeで白に到達します。");
        }
        MapNode::Scatter {
            count,
            min_distance,
            radius,
            seed,
            ..
        } => {
            ui.add(egui::Slider::new(count, 0..=4096).text("Count"));
            slider(
                ui,
                min_distance,
                1.0..=1024.0,
                "Min center distance / px",
                false,
            );
            ui.add(egui::Slider::new(radius, 0..=32).text("Point radius / px"));
            map_seed(ui, seed);
            ui.weak("白い領域内へ間隔を空けて配置。半径0で1ピクセル。置ける数が足りない場合は少なくなります。");
        }
        MapNode::Entrances {
            rectangle,
            depth,
            offset,
            merge,
            width,
            spacing,
            ..
        } => {
            ui.checkbox(rectangle, "矩形モード（壁の向きに自動回転）");
            if *rectangle {
                ui.add(egui::Slider::new(depth, 1..=64).text("Depth / px"));
                ui.checkbox(merge, "向かい合う入口を統合");
            }
            slider(ui, offset, 0.0..=64.0, "Outward offset / px", false);
            ui.add(egui::Slider::new(width, 1..=64).text("Marker width / px"));
            slider(ui, spacing, 0.0..=1024.0, "Min spacing / px", false);
            ui.weak("Roomsの縁とCorridorsが重なる場所に入口候補を配置。通路は部屋の内側まで接続してください。");
        }
        MapNode::ExtraConnections {
            count,
            width,
            distortion,
            frequency,
            seed,
            include_source,
            ..
        } => {
            ui.add(egui::Slider::new(count, 0..=64).text("Extra paths"));
            ui.add(egui::Slider::new(width, 1..=64).text("Width / px"));
            slider(ui, distortion, 0.0..=0.25, "Distortion", false);
            slider(ui, frequency, 0.5..=8.0, "Bend frequency", false);
            map_seed(ui, seed);
            ui.checkbox(include_source, "部屋と既存の道も出力");
            ui.weak("基本の木構造以外の接続を追加。元画像だけで埋まる候補は除きます。最大512領域、追加本数は上限です。");
        }
        MapNode::SelectRegions {
            min_area,
            max_area,
            largest_only,
            ..
        } => {
            ui.add(
                egui::DragValue::new(min_area)
                    .range(1..=1048576)
                    .prefix("Min area / px: "),
            );
            ui.add(
                egui::DragValue::new(max_area)
                    .range(1..=1048576)
                    .prefix("Max area / px: "),
            );
            ui.checkbox(largest_only, "面積条件に合う最大の領域だけ残す");
        }
        MapNode::Cellular {
            iterations,
            birth_limit,
            survival_limit,
            ..
        } => {
            ui.add(egui::Slider::new(iterations, 0..=64).text("Iterations"));
            ui.add(egui::Slider::new(birth_limit, 0..=8).text("Birth neighbors"));
            ui.add(egui::Slider::new(survival_limit, 0..=8).text("Survival neighbors"));
            ui.weak("周囲8マスの白い数が閾値以上なら白にします。画像外は黒です。");
        }
        MapNode::Partition {
            min_size,
            depth,
            wall_width,
            seed,
            ..
        } => {
            ui.add(egui::Slider::new(min_size, 2..=512).text("Min child size / px"));
            ui.add(egui::Slider::new(depth, 0..=8).text("Split depth"));
            ui.add(egui::Slider::new(wall_width, 1..=32).text("Divider width / px"));
            map_seed(ui, seed);
            ui.weak(
                "各領域を矩形で再帰分割し、仕切りを黒く抜きます。後段のConnect Regionsで通路を追加できます。",
            );
        }
        MapNode::Reachable {
            start_x, start_y, ..
        } => {
            ui.add(egui::Slider::new(start_x, 0..=1023).text("Start X / px"));
            ui.add(egui::Slider::new(start_y, 0..=1023).text("Start Y / px"));
            ui.weak("指定した白い開始点から上下左右に到達できる領域だけ出力。開始点が黒なら空になります。");
        }
    }
    let cutoff = match node {
        MapNode::Morphology { cutoff, .. }
        | MapNode::Outline { cutoff, .. }
        | MapNode::Distance { cutoff, .. }
        | MapNode::Scatter { cutoff, .. }
        | MapNode::Entrances { cutoff, .. }
        | MapNode::ExtraConnections { cutoff, .. }
        | MapNode::SelectRegions { cutoff, .. }
        | MapNode::Cellular { cutoff, .. }
        | MapNode::Partition { cutoff, .. }
        | MapNode::Reachable { cutoff, .. } => cutoff,
    };
    slider(ui, cutoff, 0.01..=1.0, "Mask cutoff", false);
}
fn map_seed(ui: &mut egui::Ui, seed: &mut u64) {
    ui.horizontal(|ui| {
        ui.label("Seed");
        ui.add(egui::DragValue::new(seed));
        if ui.small_button("+1").clicked() {
            *seed = seed.wrapping_add(1);
        }
    });
}

fn style(ctx: &egui::Context) {
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = PANEL;
    style.visuals.window_fill = PANEL;
    style.visuals.extreme_bg_color = BG;
    style.visuals.selection.bg_fill = Color::from_rgb(103, 79, 46);
    style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    ctx.set_style_of(egui::Theme::Dark, style);
    ctx.set_theme(egui::ThemePreference::Dark);
}
fn font(ctx: &egui::Context) {
    let paths = [
        "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "C:\\Windows\\Fonts\\meiryo.ttc",
    ];
    let bytes = std::env::var_os("MAP_GEN_FONT")
        .and_then(|p| std::fs::read(p).ok())
        .or_else(|| paths.iter().find_map(|p| std::fs::read(p).ok()));
    if let Some(bytes) = bytes {
        let mut fonts = egui::FontDefinitions::default();
        fonts
            .font_data
            .insert("japanese".into(), egui::FontData::from_owned(bytes).into());
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push("japanese".into());
        }
        ctx.set_fonts(fonts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(events: Vec<egui::Event>) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1380.0, 880.0))),
            events,
            ..Default::default()
        }
    }
    fn canvas_frame(ctx: &egui::Context, app: &mut Editor, events: Vec<egui::Event>) {
        ctx.run_ui(input(events), |ui| app.canvas(ui))
            .drop_without_applying_deltas();
    }
    fn click(ctx: &egui::Context, app: &mut Editor, pos: Pos2) {
        for pressed in [true, false] {
            canvas_frame(
                ctx,
                app,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }
    #[test]
    fn neighborhood_worker_preview_and_export_use_the_same_atlas() {
        let ctx = egui::Context::default();
        let graph = crate::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/stitched_tiles.json"),
        )
        .unwrap();
        let mut app = Editor::with_context(&ctx, graph);
        app.neighbors = true;
        app.tile_coordinate = [-1, 1];
        app.request();
        let latest = loop {
            let c = app
                .receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            if c.revision == app.revision {
                break c;
            }
        };
        app.completed = Some(latest);
        let expected = map_gen::world::generate_neighborhood(
            &app.graph,
            42,
            &BTreeMap::new(),
            [-1, 1],
            &|| false,
        )
        .unwrap();
        assert_eq!(
            app.preview_image(app.graph.output).unwrap().pixels,
            expected.pixels
        );
        // The node thumbnails retain the original single-tile dimensions.
        assert_eq!(app.image(app.graph.output).unwrap().size, 32);
        assert_eq!(app.preview_image(app.graph.output).unwrap().size, 96);
        let output = ctx.run_ui(input(vec![]), |ui| app.render_ui(ui));
        assert!(app.preview.is_some());
        output.drop_without_applying_deltas();
        let path = std::env::temp_dir().join(format!("map-gen-nine-{}.png", std::process::id()));
        crate::export(&path, app.preview_image(app.graph.output).unwrap()).unwrap();
        let png = image::open(&path).unwrap().to_rgba8();
        assert_eq!(png.dimensions(), (96, 96));
        assert_eq!(png.as_raw(), &rgba(&expected));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn input_properties_save_location_and_seed_preview_render() {
        let ctx = egui::Context::default();
        let mut graph = Graph {
            resolution: 32,
            ..Default::default()
        };
        let input_node = graph
            .add(NodeKind::Input(NumberInput::default()), [0.0, 0.0])
            .unwrap();
        graph.node_mut(input_node).unwrap().kind = NodeKind::Input(NumberInput {
            key: "contrast".into(),
            mode: InputMode::Range,
            min: 0.2,
            max: 0.8,
            preview: 0.4,
            random: true,
            ..Default::default()
        });
        graph
            .bind_parameter(4, "contrast", Some(input_node))
            .unwrap();
        let connector = graph
            .add(NodeKind::ConnectRegions(Default::default()), [800.0, 0.0])
            .unwrap();
        graph.connect(graph.output, connector, 0).unwrap();
        graph.output = connector;
        let mut map_nodes = Vec::new();
        for settings in MapNode::catalog() {
            let ports = settings.inputs().len();
            let id = graph.add(NodeKind::Map(settings), [1000.0, 0.0]).unwrap();
            for port in 0..ports {
                graph.connect(connector, id, port).unwrap();
            }
            map_nodes.push(id);
        }
        let palette_node = graph
            .add(NodeKind::Terrain(Default::default()), [1260.0, 0.0])
            .unwrap();
        graph.connect(connector, palette_node, 0).unwrap();
        map_nodes.push(palette_node);
        let merge = graph
            .add(NodeKind::TerrainMerge { cutoff: 0.5 }, [1500.0, 0.0])
            .unwrap();
        graph.connect(palette_node, merge, 0).unwrap();
        graph.connect(palette_node, merge, 1).unwrap();
        map_nodes.push(merge);
        let mut app = Editor::with_context(&ctx, graph);
        app.recipe_sample = true;
        app.recipe_seed = 123;
        app.request();
        loop {
            let result = app
                .receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            if result.revision == app.revision {
                app.completed = Some(result);
                break;
            }
        }
        let expected = app
            .graph
            .resolve_parameters(Some(123), &BTreeMap::new())
            .unwrap()
            .render(&|| false)
            .unwrap();
        assert_eq!(
            app.image(app.graph.output).unwrap().pixels,
            expected[&app.graph.output].as_ref().unwrap().pixels
        );
        for selected in [input_node, 4, connector].into_iter().chain(map_nodes) {
            app.selected = selected;
            let output = ctx.run_ui(input(vec![]), |ui| app.render_ui(ui));
            assert!(output.shapes.len() > 50);
            output.drop_without_applying_deltas();
        }
        let path = std::env::temp_dir().join("mapgen-dialog.recipe.json");
        app.recipe_location = Some(JsonLocation::new(&path.display().to_string()));
        ctx.run_ui(input(vec![]), |ui| app.render_ui(ui))
            .drop_without_applying_deltas();
        assert!(app.recipe_location.is_some());
        assert!(app.current());
    }
    #[test]
    fn trackpad_pans_and_pinches_while_mouse_wheel_still_zooms() {
        let ctx = egui::Context::default();
        let mut app = Editor::with_context(&ctx, Graph::default());
        app.zoom = 1.0;
        app.pan = Vec2::new(40.0, 40.0);
        let pointer = Pos2::new(140.0, 180.0); // Over a node, not just background.
        canvas_frame(&ctx, &mut app, vec![egui::Event::PointerMoved(pointer)]);
        let graph = app.graph.clone();
        let selection = app.selection.clone();
        let revision = app.revision;
        let wheel = |unit, source, delta| egui::Event::MouseWheel {
            unit,
            source,
            delta,
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        };
        let delta = Vec2::new(25.0, -15.0);
        for source in [
            egui::MouseWheelSource::Trackpad,
            egui::MouseWheelSource::Momentum,
            egui::MouseWheelSource::Unknown,
        ] {
            let pan = app.pan;
            canvas_frame(
                &ctx,
                &mut app,
                vec![wheel(egui::MouseWheelUnit::Point, source, delta)],
            );
            assert_eq!(app.pan, pan + delta);
            assert_eq!(app.zoom, 1.0);
        }
        // No extra drift on a frame with no new events.
        let pan = app.pan;
        canvas_frame(&ctx, &mut app, vec![]);
        assert_eq!(app.pan, pan);
        let anchor = (pointer.to_vec2() - app.pan) / app.zoom;
        canvas_frame(&ctx, &mut app, vec![egui::Event::Zoom(1.25)]);
        assert_eq!(app.zoom, 1.25);
        let projected = app.pan + anchor * app.zoom;
        assert!((projected - pointer.to_vec2()).length() < 0.01);
        canvas_frame(&ctx, &mut app, vec![egui::Event::Zoom(0.8)]);
        assert!((app.zoom - 1.0).abs() < 0.001);
        canvas_frame(
            &ctx,
            &mut app,
            vec![wheel(
                egui::MouseWheelUnit::Line,
                egui::MouseWheelSource::Wheel,
                Vec2::new(0.0, 1.0),
            )],
        );
        assert!(app.zoom > 1.0); // Only the mouse wheel direction is reversed.
        assert!((app.pan + anchor * app.zoom - pointer.to_vec2()).length() < 0.01);
        let zoom = app.zoom;
        canvas_frame(
            &ctx,
            &mut app,
            vec![wheel(
                egui::MouseWheelUnit::Line,
                egui::MouseWheelSource::Unknown,
                Vec2::new(0.0, -1.0),
            )],
        );
        assert!(app.zoom < zoom);
        assert_eq!(app.graph, graph);
        assert_eq!(app.selection, selection);
        assert_eq!(app.revision, revision);
        // Navigation outside the graph must not affect its view.
        let view = (app.pan, app.zoom);
        canvas_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(Pos2::new(-10.0, -10.0)),
                wheel(
                    egui::MouseWheelUnit::Point,
                    egui::MouseWheelSource::Trackpad,
                    delta,
                ),
                egui::Event::Zoom(1.5),
            ],
        );
        assert_eq!((app.pan, app.zoom), view);
    }
    #[test]
    fn marquee_selects_intersecting_nodes_and_drag_moves_the_group() {
        let ctx = egui::Context::default();
        let mut app = Editor::with_context(&ctx, Graph::default());
        app.fit = false;
        app.zoom = 1.0;
        app.pan = Vec2::new(40.0, 40.0);
        canvas_frame(&ctx, &mut app, vec![]);
        let button = |pos, button, pressed| egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        // Only overlap a small part of each node; neither is fully enclosed.
        let start = Pos2::new(50.0, 100.0);
        let end = Pos2::new(100.0, 450.0);
        let pan_before = app.pan;
        canvas_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(start),
                button(start, egui::PointerButton::Primary, true),
            ],
        );
        canvas_frame(&ctx, &mut app, vec![egui::Event::PointerMoved(end)]);
        canvas_frame(
            &ctx,
            &mut app,
            vec![button(end, egui::PointerButton::Primary, false)],
        );
        assert_eq!(app.selection, BTreeSet::from([1, 2]));
        assert_eq!(app.pan, pan_before);
        let before = app.graph.clone();
        let from = Pos2::new(140.0, 180.0);
        let delta = Vec2::new(35.0, 25.0);
        canvas_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(from),
                button(from, egui::PointerButton::Primary, true),
            ],
        );
        canvas_frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(from + delta)],
        );
        canvas_frame(
            &ctx,
            &mut app,
            vec![button(from + delta, egui::PointerButton::Primary, false)],
        );
        for node in &app.graph.nodes {
            let old = before.node(node.id).unwrap();
            let expected = Vec2::from(old.position) + if node.id <= 2 { delta } else { Vec2::ZERO };
            assert_eq!(Vec2::from(node.position), expected);
            assert_eq!(node.inputs, old.inputs);
        }
        assert!(before.same_computation(&app.graph));
        assert_eq!(app.selection, BTreeSet::from([1, 2]));
        // Background middle drag pans without changing the selection or graph.
        let from = Pos2::new(1200.0, 700.0);
        let graph_before = app.graph.clone();
        canvas_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(from),
                button(from, egui::PointerButton::Middle, true),
            ],
        );
        canvas_frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(from + delta)],
        );
        canvas_frame(
            &ctx,
            &mut app,
            vec![button(from + delta, egui::PointerButton::Middle, false)],
        );
        assert_eq!(app.pan, pan_before + delta);
        assert_eq!(app.graph, graph_before);
        assert_eq!(app.selection, BTreeSet::from([1, 2]));
        click(&ctx, &mut app, Pos2::new(1200.0, 700.0));
        assert!(app.selection.is_empty());
    }
    fn key_event(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }
    #[test]
    fn clicking_edges_deletes_only_the_selected_connection_at_different_zooms() {
        for zoom in [0.4, 1.0, 2.0] {
            for parameter in [false, true] {
                let ctx = egui::Context::default();
                let mut graph = Graph {
                    resolution: 32,
                    ..Default::default()
                };
                graph.nodes.retain(|n| n.id == 1 || n.id == 4);
                graph.output = 4;
                graph.node_mut(1).unwrap().position = [20.0, 20.0];
                graph.node_mut(4).unwrap().position = [600.0, 20.0];
                graph.connect(1, 4, 0).unwrap();
                let number = graph
                    .add(NodeKind::Input(NumberInput::default()), [20.0, 400.0])
                    .unwrap();
                graph.bind_parameter(4, "contrast", Some(number)).unwrap();
                let before = graph.clone();
                let mut app = Editor::with_context(&ctx, graph);
                app.fit = false;
                app.pan = Vec2::new(20.0, 20.0);
                app.zoom = zoom;
                canvas_frame(&ctx, &mut app, vec![]);
                let edge = Edge {
                    source: if parameter { number } else { 1 },
                    target: 4,
                    slot: if parameter {
                        EdgeSlot::Parameter("contrast".into())
                    } else {
                        EdgeSlot::Image(0)
                    },
                };
                let source = app.graph.node(edge.source).unwrap();
                let a = Pos2::ZERO
                    + app.pan
                    + (Vec2::from(source.position) + Vec2::new(180.0, 62.0)) * zoom;
                let b = Pos2::ZERO
                    + app.pan
                    + (Vec2::new(600.0, 20.0)
                        + Vec2::new(0.0, if parameter { 15.0 } else { 214.0 }))
                        * zoom;
                // At t=0.5 these symmetric control points lie at the endpoint midpoint.
                click(&ctx, &mut app, a.lerp(b, 0.5) + Vec2::new(0.0, 3.0));
                assert_eq!(app.selected_edge, Some(edge.clone()));
                assert!(app.selection.is_empty());
                assert_eq!(app.graph, before);
                canvas_frame(
                    &ctx,
                    &mut app,
                    vec![key_event(
                        if parameter {
                            egui::Key::Backspace
                        } else {
                            egui::Key::Delete
                        },
                        egui::Modifiers::NONE,
                    )],
                );
                assert!(!edge.exists(&app.graph));
                assert_eq!(app.graph.nodes.len(), before.nodes.len());
                assert_eq!(
                    app.graph.node(4).unwrap().inputs[0],
                    if parameter { Some(1) } else { None }
                );
                assert_eq!(
                    app.graph
                        .node(4)
                        .unwrap()
                        .parameters
                        .contains_key("contrast"),
                    !parameter
                );
                // Selecting a node switches Delete back to node selection.
                let node_center = Pos2::ZERO + app.pan + Vec2::new(90.0, 80.0) * zoom;
                click(&ctx, &mut app, node_center);
                assert_eq!(app.selected_edge, None);
                assert_eq!(app.selection, BTreeSet::from([1]));
            }
        }
    }
    #[test]
    fn edge_deletion_uses_editor_undo_and_redo() {
        let ctx = egui::Context::default();
        let mut app = Editor::with_context(
            &ctx,
            Graph {
                resolution: 32,
                ..Default::default()
            },
        );
        ctx.run_ui(input(vec![]), |ui| app.render_ui(ui))
            .drop_without_applying_deltas();
        let before = app.graph.clone();
        app.selection.clear();
        app.selected_edge = Some(Edge {
            source: 1,
            target: 3,
            slot: EdgeSlot::Image(0),
        });
        ctx.run_ui(
            input(vec![key_event(egui::Key::Delete, egui::Modifiers::NONE)]),
            |ui| app.render_ui(ui),
        )
        .drop_without_applying_deltas();
        let deleted = app.graph.clone();
        assert_eq!(deleted.node(3).unwrap().inputs[0], None);
        assert_eq!(app.undo.last(), Some(&before));
        ctx.run_ui(
            input(vec![key_event(egui::Key::Z, egui::Modifiers::COMMAND)]),
            |ui| app.render_ui(ui),
        )
        .drop_without_applying_deltas();
        assert_eq!(app.graph, before);
        let mut release = key_event(egui::Key::Z, egui::Modifiers::COMMAND);
        if let egui::Event::Key { pressed, .. } = &mut release {
            *pressed = false;
        }
        ctx.run_ui(input(vec![release]), |ui| app.render_ui(ui))
            .drop_without_applying_deltas();
        ctx.run_ui(
            input(vec![key_event(
                egui::Key::Z,
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            )]),
            |ui| app.render_ui(ui),
        )
        .drop_without_applying_deltas();
        assert_eq!(app.graph, deleted);
    }
    #[test]
    fn clicking_output_and_input_rewires_the_actual_canvas() {
        let ctx = egui::Context::default();
        let mut app = Editor::with_context(&ctx, Graph::default());
        app.fit = false;
        app.zoom = 1.0;
        app.pan = Vec2::new(40.0, 40.0);
        canvas_frame(&ctx, &mut app, vec![]);
        click(&ctx, &mut app, Pos2::new(250.0, 142.0));
        assert_eq!(app.wire, Some(1));
        click(&ctx, &mut app, Pos2::new(590.0, 374.0));
        assert_eq!(app.graph.node(4).unwrap().inputs[0], Some(1));
        assert_eq!(app.wire, None);
        app.graph.validate().unwrap();
    }
    #[test]
    fn body_double_click_pins_preview_and_body_drag_moves_the_node() {
        let ctx = egui::Context::default();
        let mut app = Editor::with_context(&ctx, Graph::default());
        app.fit = false;
        app.zoom = 1.0;
        app.pan = Vec2::new(40.0, 40.0);
        canvas_frame(&ctx, &mut app, vec![]);
        let first = Pos2::new(140.0, 180.0);
        click(&ctx, &mut app, first);
        click(&ctx, &mut app, first);
        assert_eq!(app.preview_node, Some(1));
        let other = Pos2::new(390.0, 240.0);
        click(&ctx, &mut app, other);
        assert_eq!(app.selected, 3);
        assert_eq!(app.preview_node, Some(1));
        let before = app.graph.node(3).unwrap().position;
        canvas_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(other),
                egui::Event::PointerButton {
                    pos: other,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        let destination = other + Vec2::new(30.0, 30.0);
        canvas_frame(&ctx, &mut app, vec![egui::Event::PointerMoved(destination)]);
        canvas_frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerButton {
                pos: destination,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let after = app.graph.node(3).unwrap().position;
        assert_eq!(after, [before[0] + 30.0, before[1] + 30.0]);
        assert_eq!(app.pan, Vec2::new(40.0, 40.0));
        assert_eq!(app.preview_node, Some(1));
    }
    #[test]
    fn full_editor_layout_renders_and_worker_uses_the_latest_graph() {
        let ctx = egui::Context::default();
        let mut app = Editor::with_context(&ctx, Graph::default());
        if let NodeKind::Noise(n) = &mut app.graph.nodes[0].kind {
            n.seed = 999;
        }
        app.request();
        let latest = loop {
            let result = app
                .receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            if result.revision == app.revision {
                break result;
            }
        };
        let want = app.graph.render(&|| false).unwrap();
        assert_eq!(
            latest.result.as_ref().unwrap()[&app.graph.output]
                .as_ref()
                .unwrap()
                .pixels,
            want[&app.graph.output].as_ref().unwrap().pixels
        );
        app.completed = Some(latest);
        let output = ctx.run_ui(input(vec![]), |ui| app.render_ui(ui));
        assert!(output.shapes.len() > 50);
        assert!(app.current());
        output.drop_without_applying_deltas();
        // Check the actual image mesh against its clip rect, including a second
        // frame that reloads the panel's saved height and a smaller window.
        for size in [
            Vec2::new(1380.0, 880.0),
            Vec2::new(1380.0, 880.0),
            Vec2::new(1000.0, 700.0),
        ] {
            let mut raw = input(vec![]);
            raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, size));
            let output = ctx.run_ui(raw, |ui| app.render_ui(ui));
            let texture_id = app.preview.as_ref().unwrap().id();
            let primitives = ctx.tessellate(output.shapes.clone(), output.pixels_per_point);
            let image_shape = primitives
                .iter()
                .find_map(|shape| {
                    if let egui::epaint::Primitive::Mesh(mesh) = &shape.primitive
                        && mesh.texture_id == texture_id
                    {
                        Some((mesh.calc_bounds(), shape.clip_rect))
                    } else {
                        None
                    }
                })
                .expect("2D VIEW image must be drawn");
            let (image_rect, clip) = image_shape;
            assert!((image_rect.width() - image_rect.height()).abs() < 1.0);
            assert!(
                image_rect.height() > 300.0,
                "preview too small: {image_rect:?}"
            );
            assert!(
                clip.expand(1.0).contains_rect(image_rect),
                "preview clipped: {image_rect:?} outside {clip:?}"
            );
            output.drop_without_applying_deltas();
        }
    }
}
