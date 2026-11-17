use crate::graph::*;
use eframe::egui::{
    self, Color32 as Color, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle,
    TextureOptions, Vec2,
};
use std::{
    collections::BTreeMap,
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
struct Completed {
    revision: u64,
    result: Result<Images>,
    millis: u128,
}
pub struct Editor {
    graph: Graph,
    selected: NodeId,
    zoom: f32,
    pan: Vec2,
    fit: bool,
    wire: Option<NodeId>,
    revision: u64,
    signal: Arc<AtomicU64>,
    sender: mpsc::Sender<(u64, Graph)>,
    receiver: mpsc::Receiver<Completed>,
    completed: Option<Completed>,
    thumbnails: BTreeMap<NodeId, TextureHandle>,
    preview: Option<TextureHandle>,
    preview_key: Option<(u64, NodeId, bool)>,
    terrain: bool,
    preview_node: Option<NodeId>,
    path: String,
    png: String,
    message: String,
    search: String,
    undo: Vec<Graph>,
    redo: Vec<Graph>,
    edit_start: Option<Graph>,
    skip_history: bool,
}
impl Editor {
    pub fn new(cc: &eframe::CreationContext<'_>, graph: Graph) -> Self {
        Self::with_context(&cc.egui_ctx, graph)
    }
    fn with_context(ctx: &egui::Context, graph: Graph) -> Self {
        style(ctx);
        font(ctx);
        let (sender, rx) = mpsc::channel::<(u64, Graph)>();
        let (tx, receiver) = mpsc::channel();
        let signal = Arc::new(AtomicU64::new(1));
        let worker_signal = signal.clone();
        let context = ctx.clone();
        std::thread::spawn(move || {
            while let Ok(mut work) = rx.recv() {
                while let Ok(latest) = rx.try_recv() {
                    work = latest;
                }
                let (revision, graph) = work;
                let start = Instant::now();
                let result = graph.render(&|| worker_signal.load(Ordering::Relaxed) != revision);
                if worker_signal.load(Ordering::Relaxed) != revision {
                    continue;
                }
                if tx
                    .send(Completed {
                        revision,
                        result,
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
            zoom: 0.8,
            pan: Vec2::new(20.0, 20.0),
            fit: true,
            wire: None,
            revision: 1,
            signal,
            sender,
            receiver,
            completed: None,
            thumbnails: BTreeMap::new(),
            preview: None,
            preview_key: None,
            terrain: true,
            preview_node: None,
            path: output.join("graph.json").display().to_string(),
            png: output.join("terrain.png").display().to_string(),
            message: String::new(),
            search: String::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            edit_start: None,
            skip_history: false,
        };
        let _ = app.sender.send((1, app.graph.clone()));
        app
    }
    fn request(&mut self) {
        self.revision += 1;
        self.signal.store(self.revision, Ordering::Relaxed);
        if let Err(e) = self.graph.validate() {
            self.message = e;
            return;
        }
        if self
            .sender
            .send((self.revision, self.graph.clone()))
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
                        let bytes = rgb(image, false);
                        let rgb_image =
                            image::RgbImage::from_raw(image.size as u32, image.size as u32, bytes)
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
                                egui::ColorImage::from_rgb([96, 96], thumb.as_raw()),
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
    fn add(&mut self, kind: NodeKind) {
        let offset = self.graph.nodes.len() as f32 * 12.0;
        match self.graph.add(kind, [160.0 + offset, 80.0 + offset]) {
            Ok(id) => {
                self.selected = id;
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
                self.fit = true;
            }
            let undo = ui
                .add_enabled(!self.undo.is_empty(), egui::Button::new("↶ Undo"))
                .clicked();
            let redo = ui
                .add_enabled(!self.redo.is_empty(), egui::Button::new("↷ Redo"))
                .clicked();
            let text_focus = ui.ctx().egui_wants_keyboard_input();
            let undo = undo
                || (!text_focus
                    && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)));
            let redo = redo
                || (!text_focus
                    && ui.input_mut(|i| {
                        i.consume_key(
                            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                            egui::Key::Z,
                        )
                    }));
            if undo {
                if let Some(previous) = self.undo.pop() {
                    self.redo.push(self.graph.clone());
                    self.graph = previous;
                    self.skip_history = true;
                    self.edit_start = None;
                }
            } else if redo && let Some(next) = self.redo.pop() {
                self.undo.push(self.graph.clone());
                self.graph = next;
                self.skip_history = true;
                self.edit_start = None;
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
        let mut category = "";
        for kind in NodeKind::catalog() {
            if !kind
                .title()
                .to_lowercase()
                .contains(&self.search.to_lowercase())
            {
                continue;
            }
            if category != kind.category() {
                category = kind.category();
                ui.add_space(15.0);
                ui.label(egui::RichText::new(category).weak().size(10.0));
            }
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
        ui.add_space(20.0);
        ui.separator();
        ui.label(egui::RichText::new("接続は出力 → 入力\n端子クリックでも接続\n右クリックで接続解除\n\nノードのどこでもドラッグで移動
ダブルクリックでプレビュー\n背景ドラッグでパン\nホイールでズーム\nDelete: ノード削除\nEsc: 接続キャンセル").weak().size(11.0));
    }
    fn inspector(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(10.0);
            ui.label(egui::RichText::new("PROPERTIES").color(ACCENT).strong());
            ui.add_space(10.0);
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
                NodeKind::Invert => {
                    ui.label("入力画像の値を反転します。");
                }
                NodeKind::Output => {
                    ui.label("接続した画像を出力します。");
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
            if delete {
                self.graph.remove(self.selected);
                self.selected = self.graph.output;
            }
            ui.add_space(12.0);
            ui.separator();
            ui.collapsing("SAVE / LOAD / EXPORT", |ui| {
                ui.label("Graph JSON");
                ui.text_edit_singleline(&mut self.path);
                ui.horizontal(|ui| {
                    if ui.button("保存").clicked() {
                        self.message =
                            match crate::save(std::path::Path::new(&self.path), &self.graph) {
                                Ok(()) => "グラフを保存しました".into(),
                                Err(e) => e.to_string(),
                            };
                    }
                    if ui.button("読込").clicked() {
                        match crate::load(std::path::Path::new(&self.path)) {
                            Ok(g) => {
                                self.graph = g;
                                self.selected = self.graph.output;
                                self.fit = true;
                                self.wire = None;
                                self.preview_node = None;
                                self.message = "グラフを読み込みました".into();
                            }
                            Err(e) => self.message = e.to_string(),
                        }
                    }
                });
                ui.label("PNG / 最終出力");
                ui.text_edit_singleline(&mut self.png);
                if ui
                    .add_enabled(
                        self.current() && self.image(self.graph.output).is_some(),
                        egui::Button::new("PNG + JSONを書出す"),
                    )
                    .clicked()
                {
                    let path = std::path::Path::new(&self.png);
                    let result =
                        crate::export(path, self.image(self.graph.output).unwrap(), self.terrain)
                            .and_then(|_| crate::save(&path.with_extension("json"), &self.graph));
                    self.message = match result {
                        Ok(()) => "PNGとグラフJSONを書き出しました".into(),
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
            if ui.button("選択ノード").clicked() {
                self.preview_node = Some(self.selected);
            }
            if ui.button("最終出力").clicked() {
                self.preview_node = None;
            }
            ui.weak("ノードをダブルクリックで表示");
            ui.checkbox(&mut self.terrain, "地形カラー");
            ui.label(format!("NODE #{id}"));
            if let Some(c) = &self.completed {
                ui.weak(format!("{} ms", c.millis));
            }
            if !self.current() {
                ui.colored_label(ACCENT, "再計算中… 前回の結果を表示");
            }
        });
        let key = (
            self.completed.as_ref().map_or(0, |c| c.revision),
            id,
            self.terrain,
        );
        if self.preview_key != Some(key) {
            self.preview_key = Some(key);
            self.preview = self.image(id).map(|image| {
                let bytes = rgb(image, self.terrain);
                ui.ctx().load_texture(
                    "large-preview",
                    egui::ColorImage::from_rgb([image.size, image.size], &bytes),
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
                    let response = ui.add(
                        egui::Image::new(texture).fit_to_exact_size(Vec2::splat(side)),
                    );
                    if let Some(pos) = response.hover_pos()
                        && let Some(image) = self.image(id)
                    {
                        let rel = (pos - response.rect.min) / response.rect.size();
                        let x = ((rel.x * image.size as f32) as usize).min(image.size - 1);
                        let y = ((rel.y * image.size as f32) as usize).min(image.size - 1);
                        response.on_hover_text(format!(
                            "({x}, {y}) = {:.6}", image.pixels[y * image.size + x]
                        ));
                    }
                }
                ui.vertical(|ui| {
                    if let Some(image) = self.image(id) {
                        let min = image.pixels.iter().copied().fold(1.0_f32, f32::min);
                        let max = image.pixels.iter().copied().fold(0.0_f32, f32::max);
                        ui.strong(self.graph.node(id).map_or("Unknown", |n| n.name.as_str()));
                        ui.label(format!("{} × {} · f32 grayscale", image.size, image.size));
                        ui.weak(format!("MIN  {min:.4}\nMAX  {max:.4}"));
                        ui.add_space(10.0);
                        ui.weak("値を合成してから地形へ分類します。\nノードのサムネイルは常に濃淡表示です。");
                    } else if let Some(c) = &self.completed {
                        let error = match &c.result {
                            Err(e) => Some(e.as_str()),
                            Ok(images) => images.get(&id).and_then(|i| i.as_ref().err().map(String::as_str)),
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
        if response.dragged() {
            self.pan += response.drag_delta();
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let old = self.zoom;
                self.zoom = (old * (-scroll * 0.002).exp()).clamp(0.25, 1.8);
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
        let origin = rect.min + self.pan;
        let z = self.zoom;
        let position = |node: &Node| origin + Vec2::from(node.position) * z;
        let out = |node: &Node| position(node) + Vec2::new(180.0, 62.0) * z;
        let input =
            |node: &Node, i: usize| position(node) + Vec2::new(0.0, 214.0 + i as f32 * 18.0) * z;
        for node in &self.graph.nodes {
            for (index, source) in node.inputs.iter().enumerate() {
                if let Some(source) = source.and_then(|id| self.graph.node(id)) {
                    curve(&painter, out(source), input(node, index), GREEN, z);
                }
            }
        }
        let mut connect_to = None;
        let mut disconnect = None;
        let mut movement = None;
        let mut wire_started = false;
        let pointer = ui.input(|i| i.pointer.hover_pos());
        for node in &self.graph.nodes {
            let top = position(node);
            let bounds = Rect::from_min_size(top, Vec2::new(180.0, 290.0) * z);
            if !rect.intersects(bounds) {
                continue;
            }
            let selected = node.id == self.selected;
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
            painter.text(
                top + Vec2::new(10.0, 15.0) * z,
                egui::Align2::LEFT_CENTER,
                &node.name,
                FontId::proportional(12.0 * z),
                Color::WHITE,
            );
            let body_response = ui.interact(
                bounds,
                ui.scope_id().with(("node", node.id)),
                Sense::click_and_drag(),
            );
            if body_response.clicked() || body_response.drag_started() {
                self.selected = node.id;
            }
            if body_response.double_clicked() {
                self.preview_node = Some(node.id);
                self.preview_key = None;
            }
            if body_response.dragged() {
                movement = Some((node.id, body_response.drag_delta() / z));
            }
            let image_rect =
                Rect::from_min_size(top + Vec2::new(10.0, 38.0) * z, Vec2::new(160.0, 160.0) * z);
            painter.rect_filled(image_rect, 2.0, Color::from_rgb(18, 20, 25));
            if let Some(texture) = self.thumbnails.get(&node.id) {
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
                    FontId::monospace(10.0 * z),
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
                    FontId::proportional(10.0 * z),
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
                    FontId::monospace(9.0 * z),
                    ACCENT,
                );
            }
        }
        if let Some((id, delta)) = movement
            && let Some(node) = self.graph.node_mut(id)
        {
            node.position[0] = (node.position[0] + delta.x).clamp(-100000.0, 100000.0);
            node.position[1] = (node.position[1] + delta.y).clamp(-100000.0, 100000.0);
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
            curve(&painter, out(source), pointer, ACCENT, z);
        }
        if response.clicked() && !wire_started {
            self.wire = None;
        }
        if !ui.ctx().egui_wants_keyboard_input() {
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.wire = None;
            }
            if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
            {
                self.graph.remove(self.selected);
                self.selected = self.graph.output;
                self.wire = None;
            }
        }
        painter.text(
            rect.left_top() + Vec2::new(14.0, 12.0),
            egui::Align2::LEFT_TOP,
            format!(
                "GRAPH  /  {} NODES  /  {:.0}%",
                self.graph.nodes.len(),
                self.zoom * 100.0
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
            .show(ui, |ui| self.library(ui));
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
fn curve(p: &egui::Painter, a: Pos2, b: Pos2, color: Color, zoom: f32) {
    let dx = ((a.x - b.x).abs() * 0.45).max(45.0 * zoom);
    p.add(egui::epaint::CubicBezierShape::from_points_stroke(
        [a, a + Vec2::new(dx, 0.0), b - Vec2::new(dx, 0.0), b],
        false,
        Color::TRANSPARENT,
        Stroke::new(2.0 * zoom, color),
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
