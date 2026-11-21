use super::*;
use serde_json::Value;
mod controls;
pub(super) use controls::*;

const LETTERS: &[u8] = b"asdfghjklqwertyuiopzxcvbnm";
const CREATE_CATEGORIES: [&str; 7] = [
    "数値入力",
    "形状・ノイズ",
    "配置・通路",
    "壁・入口",
    "領域処理",
    "画像加工",
    "地形ID・出力",
];

#[derive(Clone, PartialEq, Eq)]
enum HintMode {
    Nodes,
    Edges,
    Create,
    Connectors(NodeId),
    Sources(NodeId, EdgeSlot),
    Plugs(NodeId),
    Properties,
}

#[derive(Default)]
pub(super) struct Keyboard {
    hints: Option<HintMode>,
    prefix: String,
    property: Option<(NodeId, usize)>,
    focus: Option<NodeId>,
    zoom_steps: i32,
    targets: Vec<Target>,
    controls: Vec<Control>,
    scroll_property: bool,
    create_category: usize,
    create_row: usize,
    hint_shift: bool,
    adding_nodes: bool,
}

#[derive(Clone)]
enum Target {
    Node(NodeId),
    Edge(Edge),
    Output(NodeId),
    Plug(NodeId, EdgeSlot),
    Property(usize),
}

fn hint_label(mut index: usize, count: usize) -> String {
    let mut width = 1;
    let mut capacity = LETTERS.len();
    while capacity < count {
        width += 1;
        capacity *= LETTERS.len();
    }
    let mut label = vec![LETTERS[0]; width];
    for character in label.iter_mut().rev() {
        *character = LETTERS[index % LETTERS.len()];
        index /= LETTERS.len();
    }
    String::from_utf8(label).unwrap()
}

fn choices(value: &str) -> Option<&'static [&'static str]> {
    [
        &["simplex", "perlin", "value", "cellular", "white"][..],
        &["circle", "square", "triangle", "diamond", "hexagon"],
        &[
            "normal", "multiply", "screen", "overlay", "add", "subtract", "min", "max",
        ],
        &["constant", "range"],
        &["dilate", "erode"],
        &["cut", "mosaic", "fill", "road"],
    ]
    .into_iter()
    .find(|options| options.contains(&value))
}

#[cfg(test)]
fn properties(kind: &NodeKind) -> Vec<(String, Value)> {
    fn collect(value: &Value, path: String, result: &mut Vec<(String, Value)>) {
        match value {
            Value::Object(fields) => {
                for (key, value) in fields {
                    if key != "type" && key != "operation" && key != "id" && key != "key" {
                        collect(value, format!("{path}/{key}"), result);
                    }
                }
            }
            Value::Array(items) => {
                for (index, value) in items.iter().enumerate() {
                    collect(value, format!("{path}/{index}"), result);
                }
            }
            Value::Number(_) | Value::Bool(_) => result.push((path, value.clone())),
            Value::String(text) if choices(text).is_some() => result.push((path, value.clone())),
            _ => {}
        }
    }
    let mut result = Vec::new();
    collect(
        &serde_json::to_value(kind).unwrap(),
        String::new(),
        &mut result,
    );
    result
}

fn adjusted(value: &Value, delta: i32) -> Option<Value> {
    match value {
        Value::Number(number) if number.is_f64() => {
            serde_json::Number::from_f64(number.as_f64()? + f64::from(delta)).map(Value::Number)
        }
        Value::Number(number) => {
            if let Some(integer) = number.as_u64() {
                integer
                    .checked_add_signed(i64::from(delta))
                    .map(Value::from)
            } else {
                number
                    .as_i64()?
                    .checked_add(i64::from(delta))
                    .map(Value::from)
            }
        }
        Value::Bool(_) => Some(Value::Bool(delta > 0)),
        Value::String(text) => {
            let options = choices(text)?;
            let index = options.iter().position(|option| *option == text)?;
            let next = (index as i32 + delta).rem_euclid(options.len() as i32) as usize;
            Some(Value::String(options[next].into()))
        }
        _ => None,
    }
}

impl Editor {
    fn hint_targets(&self) -> Vec<Target> {
        if self.keyboard.hints.is_some() {
            self.keyboard.targets.clone()
        } else {
            Vec::new()
        }
    }

    fn compute_hint_targets(&self) -> Vec<Target> {
        match self.keyboard.hints.clone() {
            Some(HintMode::Edges) => self
                .graph
                .nodes
                .iter()
                .flat_map(|node| {
                    let mut edges: Vec<_> = node
                        .inputs
                        .iter()
                        .enumerate()
                        .filter_map(|(index, source)| {
                            source.map(|source| {
                                Target::Edge(Edge {
                                    source,
                                    target: node.id,
                                    slot: EdgeSlot::Image(index),
                                })
                            })
                        })
                        .collect();
                    edges.extend(node.parameters.iter().map(|(key, source)| {
                        Target::Edge(Edge {
                            source: *source,
                            target: node.id,
                            slot: EdgeSlot::Parameter(key.clone()),
                        })
                    }));
                    edges
                })
                .collect(),
            Some(HintMode::Create) => Vec::new(),
            Some(HintMode::Connectors(id)) => self.graph.node(id).map_or_else(Vec::new, |node| {
                let mut targets = vec![Target::Output(id)];
                targets.extend(
                    (0..node.inputs.len()).map(|index| Target::Plug(id, EdgeSlot::Image(index))),
                );
                targets.extend(
                    crate::recipe::numeric_parameters(&node.kind)
                        .into_iter()
                        .map(|key| Target::Plug(id, EdgeSlot::Parameter(key))),
                );
                targets
            }),
            Some(HintMode::Sources(target, slot)) => self
                .graph
                .nodes
                .iter()
                .filter_map(|node| {
                    let mut graph = self.graph.clone();
                    let valid = match &slot {
                        EdgeSlot::Image(index) => graph.connect(node.id, target, *index),
                        EdgeSlot::Parameter(key) => {
                            graph.bind_parameter(target, key, Some(node.id))
                        }
                    };
                    (node.id != target && valid.is_ok()).then_some(Target::Output(node.id))
                })
                .collect(),
            Some(HintMode::Nodes) => self
                .graph
                .nodes
                .iter()
                .map(|node| Target::Node(node.id))
                .collect(),
            Some(HintMode::Properties) => (0..self.keyboard.controls.len())
                .map(Target::Property)
                .collect(),
            Some(HintMode::Plugs(source)) => {
                let numeric = self
                    .graph
                    .node(source)
                    .is_some_and(|node| matches!(node.kind, NodeKind::Input(_)));
                let mut targets = Vec::new();
                for node in &self.graph.nodes {
                    if node.id == source {
                        continue;
                    }
                    let slots = if numeric {
                        crate::recipe::numeric_parameters(&node.kind)
                            .into_iter()
                            .map(EdgeSlot::Parameter)
                            .collect::<Vec<_>>()
                    } else {
                        (0..node.inputs.len()).map(EdgeSlot::Image).collect()
                    };
                    for slot in slots {
                        let mut graph = self.graph.clone();
                        let valid = match &slot {
                            EdgeSlot::Image(index) => graph.connect(source, node.id, *index),
                            EdgeSlot::Parameter(key) => {
                                graph.bind_parameter(node.id, key, Some(source))
                            }
                        };
                        if valid.is_ok() {
                            targets.push(Target::Plug(node.id, slot));
                        }
                    }
                }
                targets
            }
            None => Vec::new(),
        }
    }

    fn begin_hints(&mut self, mode: HintMode) {
        let creating = mode == HintMode::Create;
        if creating {
            self.keyboard.create_category = 0;
            self.keyboard.create_row = 0;
        }
        if mode == HintMode::Edges {
            self.selection.clear();
        }
        let prompt = match &mode {
            HintMode::Nodes => "フォーカスするノード",
            HintMode::Edges => "選択するエッジ",
            HintMode::Create => "作成するノードの種類",
            HintMode::Connectors(_) => "このノードのコネクタ",
            HintMode::Plugs(_) => "接続先の入力端子",
            HintMode::Sources(..) => "接続元の出力端子",
            HintMode::Properties => "編集するプロパティ",
        };
        let overview = matches!(
            mode,
            HintMode::Nodes | HintMode::Edges | HintMode::Plugs(_) | HintMode::Sources(..)
        );
        self.keyboard.hints = Some(mode);
        self.keyboard.prefix.clear();
        self.keyboard.hint_shift = false;
        self.keyboard.adding_nodes = false;
        self.keyboard.property = None;
        self.wire = None;
        self.selected_edge = None;
        self.keyboard.targets = self.compute_hint_targets();
        if overview {
            self.fit = true;
        }
        if self.hint_targets().is_empty() && !creating {
            self.keyboard.hints = None;
            self.message = "選択できる対象がありません".into();
        } else if creating {
            self.message = "h/l: カテゴリ / j/k: 上下 / Enter: 作成 / Esc: 閉じる".into();
        } else {
            self.message = format!("{prompt}のヒントを入力 / Escでキャンセル");
        }
    }

    pub(super) fn keyboard_input(&mut self, ctx: &egui::Context) {
        if self.keyboard.adding_nodes
            && self.keyboard.hints == Some(HintMode::Nodes)
            && ctx.input(|input| !input.modifiers.shift && !input.events.iter().any(|event| matches!(event, egui::Event::Key { modifiers, .. } if modifiers.shift)))
        {
            self.keyboard.hints = None;
            self.keyboard.prefix.clear();
            self.keyboard.hint_shift = false;
            self.keyboard.adding_nodes = false;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Escape))
            && (self.keyboard.property.is_some() || self.keyboard.hints.is_some())
        {
            ctx.memory_mut(|memory| {
                if let Some(id) = memory.focused() {
                    memory.surrender_focus(id);
                }
            });
            self.keyboard.property = None;
            self.keyboard.hints = None;
            self.keyboard.prefix.clear();
            self.wire = None;
            self.selected_edge = None;
            ctx.input_mut(|input| {
                input.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
            });
            return;
        }
        if self
            .keyboard
            .property
            .is_some_and(|(id, _)| id != self.selected || !self.single_node_selected())
        {
            self.keyboard.property = None;
        }
        if self.recipe_location.is_some()
            || ctx.text_edit_focused()
            || egui::Popup::is_any_open(ctx)
        {
            self.keyboard.hints = None;
            return;
        }
        if ctx.input(|input| input.pointer.any_pressed())
            && self.keyboard.hints != Some(HintMode::Properties)
            && self.keyboard.hints != Some(HintMode::Create)
        {
            self.keyboard.hints = None;
        }
        let events = ctx.input(|input| input.events.clone());
        for event in events {
            let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            else {
                continue;
            };
            if modifiers.command || modifiers.ctrl || modifiers.alt {
                continue;
            }
            if key == egui::Key::Escape {
                self.release_property_focus(ctx);
                self.selection.clear();
                self.keyboard.focus = None;
                self.keyboard.hints = None;
                self.keyboard.prefix.clear();
                self.keyboard.property = None;
                self.wire = None;
                self.marquee_start = None;
                self.selected_edge = None;
                continue;
            }
            if self.keyboard.hints == Some(HintMode::Create) {
                ctx.input_mut(|input| {
                    input.consume_key(modifiers, key);
                });
                let items = self.create_items();
                match key {
                    egui::Key::H | egui::Key::L | egui::Key::Tab => {
                        let delta =
                            if key == egui::Key::H || (key == egui::Key::Tab && modifiers.shift) {
                                CREATE_CATEGORIES.len() - 1
                            } else {
                                1
                            };
                        self.keyboard.create_category =
                            (self.keyboard.create_category + delta) % CREATE_CATEGORIES.len();
                        self.keyboard.create_row = 0;
                    }
                    egui::Key::J | egui::Key::ArrowDown if !items.is_empty() => {
                        self.keyboard.create_row = (self.keyboard.create_row + 1) % items.len()
                    }
                    egui::Key::K | egui::Key::ArrowUp if !items.is_empty() => {
                        self.keyboard.create_row =
                            (self.keyboard.create_row + items.len() - 1) % items.len()
                    }
                    egui::Key::Enter => {
                        if let Some(kind) = items.get(self.keyboard.create_row) {
                            self.keyboard.hints = None;
                            self.add(kind.clone());
                        }
                    }
                    _ => {}
                }
                continue;
            }
            if self.keyboard.hints.is_some() {
                ctx.input_mut(|input| {
                    input.consume_key(modifiers, key);
                });
                if key == egui::Key::Backspace {
                    self.keyboard.prefix.pop();
                    if self.keyboard.prefix.is_empty() {
                        self.keyboard.hint_shift = false;
                    }
                    continue;
                }
                let letter = key.name().to_ascii_lowercase();
                if letter.len() != 1 || !LETTERS.contains(&letter.as_bytes()[0]) {
                    continue;
                }
                self.keyboard.prefix.push_str(&letter);
                self.keyboard.hint_shift |= modifiers.shift;
                let targets = self.hint_targets();
                let labels: Vec<_> = (0..targets.len())
                    .map(|index| hint_label(index, targets.len()))
                    .collect();
                if let Some(index) = labels
                    .iter()
                    .position(|label| *label == self.keyboard.prefix)
                {
                    let mode = self.keyboard.hints.clone();
                    let additive = self.keyboard.hint_shift;
                    self.keyboard.hints = None;
                    self.keyboard.prefix.clear();
                    self.keyboard.hint_shift = false;
                    match &targets[index] {
                        Target::Node(id) => {
                            self.selected = *id;
                            if additive {
                                self.selection.insert(*id);
                                self.keyboard.adding_nodes = true;
                                self.keyboard.hints = Some(HintMode::Nodes);
                                self.keyboard.focus = None;
                                self.message = format!(
                                    "{}ノード選択 / Shift中は追加選択 / Shiftを離すかEscで確定",
                                    self.selection.len()
                                );
                            } else {
                                self.selection = BTreeSet::from([*id]);
                                self.keyboard.focus = Some(*id);
                                self.message =
                                    "Tab: 作成 / c: 接続 / hjklyubn: 移動 / p: プロパティ".into();
                            }
                        }
                        Target::Edge(edge) => {
                            if edge.exists(&self.graph) {
                                self.selected_edge = Some(edge.clone());
                                self.selection.clear();
                                self.message = "Delete: 選択エッジを削除 / Esc: 選択解除".into();
                            }
                        }
                        Target::Output(id) => {
                            if let Some(HintMode::Sources(target, slot)) = mode {
                                self.connect_hint(*id, target, &slot);
                            } else {
                                self.begin_hints(HintMode::Plugs(*id));
                            }
                        }
                        Target::Plug(id, slot) => {
                            if matches!(mode, Some(HintMode::Connectors(_))) {
                                self.begin_hints(HintMode::Sources(*id, slot.clone()));
                            } else if let Some(HintMode::Plugs(source)) = mode {
                                self.connect_hint(source, *id, slot);
                            }
                        }
                        Target::Property(index) => {
                            self.keyboard.property = Some((self.selected, *index));
                            self.keyboard.scroll_property = true;
                            self.message =
                                "j/k: プロパティ移動 / h/l: 1目盛り / Shift: スライダー全長の1/10 / p: ヒント / Esc: 終了"
                                    .into();
                        }
                    }
                } else if !labels
                    .iter()
                    .any(|label| label.starts_with(&self.keyboard.prefix))
                {
                    self.keyboard.prefix.clear();
                    self.keyboard.hint_shift = false;
                }
                continue;
            }
            match key {
                egui::Key::F => self.begin_hints(HintMode::Nodes),
                egui::Key::E => self.begin_hints(HintMode::Edges),
                egui::Key::A => self.align_nodes(),
                egui::Key::Tab => {
                    self.release_property_focus(ctx);
                    ctx.input_mut(|input| {
                        input.consume_key(modifiers, key);
                    });
                    self.begin_hints(HintMode::Create);
                }
                egui::Key::C if self.single_node_selected() => {
                    self.begin_hints(HintMode::Connectors(self.selected))
                }
                egui::Key::P if self.single_node_selected() => {
                    ctx.input_mut(|input| {
                        input.consume_key(modifiers, key);
                    });
                    self.begin_hints(HintMode::Properties);
                }
                egui::Key::Q => {
                    self.fit = true;
                    self.keyboard.property = None;
                }
                egui::Key::Plus | egui::Key::Equals => self.keyboard.zoom_steps += 1,
                egui::Key::Minus => self.keyboard.zoom_steps -= 1,
                egui::Key::H
                | egui::Key::J
                | egui::Key::K
                | egui::Key::L
                | egui::Key::Y
                | egui::Key::U
                | egui::Key::B
                | egui::Key::N
                    if self.keyboard.property.is_none() =>
                {
                    self.move_nodes_or_pan(key, modifiers.shift);
                }
                egui::Key::J | egui::Key::K => {
                    ctx.input_mut(|input| {
                        input.consume_key(modifiers, key);
                        input
                            .events
                            .retain(|event| !matches!(event, egui::Event::Text(_)));
                    });
                    if let Some((id, index)) = self.keyboard.property {
                        let count = self.keyboard.controls.len();
                        if count > 0 {
                            let next = if key == egui::Key::J {
                                (index + 1) % count
                            } else {
                                (index + count - 1) % count
                            };
                            self.keyboard.property = Some((id, next));
                            self.keyboard.scroll_property = true;
                        }
                    }
                }
                egui::Key::H | egui::Key::L => {
                    ctx.input_mut(|input| {
                        input.consume_key(modifiers, key);
                        input
                            .events
                            .retain(|event| !matches!(event, egui::Event::Text(_)));
                    });
                    self.adjust_property(if key == egui::Key::L { 1 } else { -1 }, modifiers.shift);
                }
                _ => {}
            }
        }
    }

    fn connect_hint(&mut self, source: NodeId, target: NodeId, slot: &EdgeSlot) {
        let result = match slot {
            EdgeSlot::Image(index) => self.graph.connect(source, target, *index),
            EdgeSlot::Parameter(key) => self.graph.bind_parameter(target, key, Some(source)),
        };
        self.message = result.map_or_else(|error| error, |()| "接続しました".into());
    }

    fn single_node_selected(&self) -> bool {
        self.selection.len() == 1 && self.selection.contains(&self.selected)
    }

    fn align_nodes(&mut self) {
        let mut levels = BTreeMap::<NodeId, usize>::new();
        for _ in 0..self.graph.nodes.len() {
            let before = levels.len();
            for node in &self.graph.nodes {
                if levels.contains_key(&node.id) {
                    continue;
                }
                let dependencies: Vec<_> = node
                    .inputs
                    .iter()
                    .flatten()
                    .chain(node.parameters.values())
                    .collect();
                if dependencies.iter().all(|id| levels.contains_key(id)) {
                    let level = dependencies
                        .iter()
                        .filter_map(|id| levels.get(id))
                        .max()
                        .map_or(0, |level| level + 1);
                    levels.insert(node.id, level);
                }
            }
            if levels.len() == before {
                break;
            }
        }
        let mut columns = BTreeMap::<usize, Vec<NodeId>>::new();
        for node in &self.graph.nodes {
            columns
                .entry(levels.get(&node.id).copied().unwrap_or(0))
                .or_default()
                .push(node.id);
        }
        let rows = columns.values().map(Vec::len).max().unwrap_or(0);
        for (column, mut ids) in columns {
            ids.sort_by(|left, right| {
                self.graph.node(*left).unwrap().position[1]
                    .total_cmp(&self.graph.node(*right).unwrap().position[1])
                    .then(left.cmp(right))
            });
            let offset = (rows - ids.len()) as f32 * 180.0;
            for (row, id) in ids.into_iter().enumerate() {
                self.graph.node_mut(id).unwrap().position = [
                    40.0 + column as f32 * 260.0,
                    40.0 + offset + row as f32 * 360.0,
                ];
            }
        }
        self.keyboard.property = None;
        self.fit = true;
        self.message = "入力から出力へ、ノードを等間隔に整列しました（Undoで戻せます）".into();
    }

    fn move_nodes_or_pan(&mut self, key: egui::Key, large: bool) {
        let direction = match key {
            egui::Key::H => [-1.0, 0.0],
            egui::Key::J => [0.0, 1.0],
            egui::Key::K => [0.0, -1.0],
            egui::Key::L => [1.0, 0.0],
            egui::Key::Y => [-1.0, -1.0],
            egui::Key::U => [1.0, -1.0],
            egui::Key::B => [-1.0, 1.0],
            egui::Key::N => [1.0, 1.0],
            _ => return,
        };
        if self.selection.is_empty() {
            self.pan += Vec2::from(direction) * if large { 160.0 } else { 40.0 };
            return;
        }
        if !self.selection.contains(&self.selected) {
            return;
        }
        let step = if large { 40.0 } else { 8.0 };
        let mut delta = Vec2::from(direction) * step;
        for node in self
            .graph
            .nodes
            .iter()
            .filter(|node| self.selection.contains(&node.id))
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
            .filter(|node| self.selection.contains(&node.id))
        {
            for (position, offset) in node.position.iter_mut().zip([delta.x, delta.y]) {
                *position += offset;
            }
        }
    }

    fn adjust_property(&mut self, delta: i32, range_step: bool) {
        let Some((id, index)) = self.keyboard.property else {
            return;
        };
        let Some(node) = self.graph.node(id) else {
            return;
        };
        let Some(control) = self.keyboard.controls.get(index) else {
            return;
        };
        let path = &control.path;
        let mut json = serde_json::to_value(&node.kind).unwrap();
        let Some(value) = json.pointer(path) else {
            return;
        };
        if node.parameters.contains_key(path.trim_start_matches('/')) {
            self.message = "この値はNumber Input側で変更してください".into();
            return;
        }
        let range = control
            .range
            .filter(|(min, max)| (max - min).is_finite() && max > min);
        let next = if let Some((min, max)) = range.filter(|_| range_step && value.is_number()) {
            let span = max - min;
            let current = value.as_f64().unwrap();
            let integer =
                !value.is_f64() || matches!(&node.kind, NodeKind::Input(input) if input.integer);
            let mut next = if control.logarithmic && min > 0.0 {
                let log_min = min.ln();
                let log_span = max.ln() - log_min;
                let position = (current.clamp(min, max).ln() - log_min) / log_span;
                let position = (position + f64::from(delta) / 10.0).clamp(0.0, 1.0);
                if position <= 1e-6 {
                    min
                } else if position >= 1.0 - 1e-6 {
                    max
                } else {
                    (log_min + log_span * position).exp()
                }
            } else if integer {
                let mut index =
                    (((current - min) / span * 10.0).round() + f64::from(delta)).clamp(0.0, 10.0);
                let mut next = (min + span * index / 10.0).round();
                while index > 0.0 && index < 10.0 && (next - current) * f64::from(delta) <= 0.0 {
                    index += f64::from(delta);
                    next = (min + span * index / 10.0).round();
                }
                next
            } else {
                current + f64::from(delta) * span / 10.0
            };
            next = next.clamp(min, max);
            if (next - min).abs() <= span * 1e-6 {
                next = min;
            }
            if (next - max).abs() <= span * 1e-6 {
                next = max;
            }
            if value.is_f64() {
                serde_json::Number::from_f64(next).map(Value::Number)
            } else if next < 0.0 {
                Some(Value::from(next as i64))
            } else {
                Some(Value::from(next as u64))
            }
        } else if value.is_f64() {
            let mut next = value.as_f64().unwrap() + f64::from(delta) * control.step;
            if let Some((min, max)) = control.range {
                next = next.clamp(min, max);
            }
            serde_json::Number::from_f64(next).map(Value::Number)
        } else {
            adjusted(value, delta)
        };
        let Some(value) = next else {
            return;
        };
        *json.pointer_mut(path).unwrap() = value;
        let Ok(mut kind) = serde_json::from_value::<NodeKind>(json) else {
            return;
        };
        if let NodeKind::Input(input) = &mut kind
            && input.integer
        {
            input.value = input.value.round();
            input.min = input.min.round();
            input.max = input.max.round();
            input.preview = input.preview.round();
        }
        let mut graph = self.graph.clone();
        graph.node_mut(id).unwrap().kind = kind;
        if let Err(error) = graph.validate() {
            self.message = format!("変更できません（許容範囲）: {error}");
        } else {
            self.graph = graph;
        }
    }

    pub(super) fn keyboard_view(&mut self, rect: Rect) {
        if let Some(id) = self.keyboard.focus.take()
            && let Some(node) = self.graph.node(id)
        {
            self.fit = false;
            self.zoom = 1.0;
            self.pan = rect.size() * 0.5
                - (Vec2::from(node.position) + Vec2::new(90.0, 145.0)) * self.zoom;
        }
        let steps = std::mem::take(&mut self.keyboard.zoom_steps);
        if steps != 0 {
            let old = self.zoom;
            self.zoom = (old * 1.2_f32.powi(steps)).clamp(0.001, 1.8);
            self.pan = rect.size() * 0.5 - (rect.size() * 0.5 - self.pan) * (self.zoom / old);
        }
    }

    pub(super) fn keyboard_finish(&mut self, ctx: &egui::Context) {
        if self.keyboard.hints == Some(HintMode::Create) {
            let catalog = self.create_items();
            let mut chosen = None;
            egui::Modal::new(egui::Id::unique("create_node_modal")).show(ctx, |ui| {
                let font = egui::TextStyle::Button.resolve(ui.style());
                let category_width = CREATE_CATEGORIES
                    .iter()
                    .map(|category| {
                        ui.painter()
                            .layout_no_wrap((*category).into(), font.clone(), Color::WHITE)
                            .size()
                            .x
                            + ui.spacing().button_padding.x * 2.0
                    })
                    .sum::<f32>()
                    + ui.spacing().item_spacing.x * (CREATE_CATEGORIES.len() - 1) as f32;
                ui.set_width(
                    (category_width + 24.0)
                        .max(620.0)
                        .min((ctx.content_rect().width() - 40.0).max(200.0)),
                );
                ui.heading("CREATE NODE");
                ui.weak("h/l: カテゴリ切替 / j/k: 上下 / Enter: 作成 / Esc: 閉じる");
                egui::ScrollArea::horizontal()
                    .id_salt("create_categories")
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (index, category) in CREATE_CATEGORIES.iter().enumerate() {
                                if ui
                                    .selectable_label(
                                        self.keyboard.create_category == index,
                                        *category,
                                    )
                                    .clicked()
                                {
                                    self.keyboard.create_category = index;
                                    self.keyboard.create_row = 0;
                                }
                            }
                        })
                    });
                let row_height = (ui.text_style_height(&egui::TextStyle::Button)
                    + ui.spacing().button_padding.y * 2.0)
                    .max(ui.spacing().interact_size.y);
                let header_height = ui.cursor().top() - ui.min_rect().top();
                let list_height = (row_height * 10.0 + 4.0 * 9.0)
                    .min((ctx.content_rect().height() - header_height - 48.0).max(row_height));
                egui::ScrollArea::vertical()
                    .min_scrolled_height(list_height)
                    .max_height(list_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        for (index, kind) in catalog.iter().enumerate() {
                            let selected = index == self.keyboard.create_row;
                            let response = ui.selectable_label(selected, kind.title());
                            if response.clicked() {
                                chosen = Some(index);
                            }
                            if selected {
                                response.scroll_to_me(Some(egui::Align::Center));
                            }
                        }
                    });
            });
            if let Some(index) = chosen {
                self.keyboard.hints = None;
                self.add(catalog[index].clone());
            }
        }
        if matches!(
            self.keyboard.hints,
            Some(HintMode::Properties | HintMode::Create)
        ) {
            ctx.memory_mut(|memory| {
                if let Some(id) = memory.focused() {
                    memory.surrender_focus(id);
                }
            });
        }
    }

    fn create_items(&self) -> Vec<NodeKind> {
        NodeKind::catalog()
            .into_iter()
            .filter(|kind| library_group(kind) == CREATE_CATEGORIES[self.keyboard.create_category])
            .collect()
    }

    fn release_property_focus(&self, ctx: &egui::Context) {
        ctx.memory_mut(|memory| {
            if let Some(id) = memory.focused()
                && self
                    .keyboard
                    .controls
                    .iter()
                    .any(|control| control.id == id)
            {
                memory.surrender_focus(id);
            }
        });
    }

    pub(super) fn keyboard_hints(&self, painter: &egui::Painter, rect: Rect) {
        let targets = self.hint_targets();
        let mut occupied = Vec::<Rect>::new();
        for (index, target) in targets.iter().enumerate() {
            let (id, offset, suffix) = match target {
                Target::Edge(edge) => {
                    let (Some(source), Some(target)) =
                        (self.graph.node(edge.source), self.graph.node(edge.target))
                    else {
                        continue;
                    };
                    let input_offset = match &edge.slot {
                        EdgeSlot::Image(index) => Vec2::new(0.0, 214.0 + *index as f32 * 18.0),
                        EdgeSlot::Parameter(_) => Vec2::new(0.0, 15.0),
                    };
                    let center = (Vec2::from(source.position)
                        + Vec2::new(180.0, 62.0)
                        + Vec2::from(target.position)
                        + input_offset)
                        * 0.5;
                    (
                        edge.target,
                        center - Vec2::from(target.position),
                        String::new(),
                    )
                }
                Target::Output(id) => (*id, Vec2::new(180.0, 62.0), " Output".into()),
                Target::Node(id) => (*id, Vec2::new(8.0, 30.0), String::new()),
                Target::Plug(id, EdgeSlot::Image(slot)) => (
                    *id,
                    Vec2::new(0.0, 214.0 + *slot as f32 * 18.0),
                    format!(
                        " {}",
                        self.graph
                            .node(*id)
                            .and_then(|node| node.kind.inputs().get(*slot).copied())
                            .unwrap_or("Input")
                    ),
                ),
                Target::Plug(id, EdgeSlot::Parameter(key)) => {
                    let Some(node) = self.graph.node(*id) else {
                        continue;
                    };
                    let Some(slot) = crate::recipe::numeric_parameters(&node.kind)
                        .iter()
                        .position(|field| field == key)
                    else {
                        continue;
                    };
                    (
                        *id,
                        Vec2::new(0.0, 15.0 + slot as f32 * 18.0),
                        format!(" {key}"),
                    )
                }
                Target::Property(_) => continue,
            };
            let label = hint_label(index, targets.len());
            if !label.starts_with(&self.keyboard.prefix) {
                continue;
            }
            let Some(node) = self.graph.node(id) else {
                continue;
            };
            let position = rect.min + self.pan + (Vec2::from(node.position) + offset) * self.zoom;
            let galley =
                painter.layout_no_wrap(format!("{label}{suffix}"), FontId::monospace(14.0), BG);
            let mut bounds = Rect::from_min_size(position, galley.size()).expand(2.0);
            for _ in 0..targets.len() {
                if !occupied.iter().any(|other| other.intersects(bounds)) {
                    break;
                }
                bounds = bounds.translate(Vec2::new(0.0, bounds.height() + 2.0));
                if bounds.bottom() > rect.bottom() {
                    bounds = Rect::from_min_size(
                        Pos2::new(bounds.right() + 4.0, rect.top() + 30.0),
                        bounds.size(),
                    );
                }
            }
            occupied.push(bounds);
            if (bounds.min + Vec2::splat(2.0)).distance(position) > 3.0 {
                painter.line_segment([position, bounds.left_center()], Stroke::new(1.0, ACCENT));
            }
            painter.rect_filled(bounds, 2.0, ACCENT);
            painter.galley(bounds.min + Vec2::splat(2.0), galley, BG);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &egui::Context, editor: &mut Editor, events: Vec<egui::Event>) {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1380.0, 880.0))),
                events,
                ..Default::default()
            },
            |ui| editor.render_ui(ui),
        )
        .drop_without_applying_deltas();
    }

    fn press(ctx: &egui::Context, editor: &mut Editor, key: egui::Key) {
        press_modified(ctx, editor, key, egui::Modifiers::NONE);
    }

    fn press_modified(
        ctx: &egui::Context,
        editor: &mut Editor,
        key: egui::Key,
        modifiers: egui::Modifiers,
    ) {
        for pressed in [true, false] {
            frame(
                ctx,
                editor,
                vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers,
                }],
            );
        }
    }

    fn choose(ctx: &egui::Context, editor: &mut Editor, index: usize) {
        let label = hint_label(index, editor.hint_targets().len());
        for character in label.chars() {
            press(
                ctx,
                editor,
                egui::Key::from_name(&character.to_ascii_uppercase().to_string()).unwrap(),
            );
        }
    }

    fn choose_shifted(ctx: &egui::Context, editor: &mut Editor, index: usize) {
        let label = hint_label(index, editor.hint_targets().len());
        assert!(label.bytes().all(|letter| letter.is_ascii_lowercase()));
        for character in label.chars() {
            press_modified(
                ctx,
                editor,
                egui::Key::from_name(&character.to_ascii_uppercase().to_string()).unwrap(),
                egui::Modifiers::SHIFT,
            );
        }
    }

    #[test]
    fn hints_are_unique_and_prefix_free_for_large_graphs() {
        for count in [1, 24, 25, 64, 1000, 4096] {
            let labels: BTreeSet<_> = (0..count).map(|index| hint_label(index, count)).collect();
            assert_eq!(labels.len(), count);
            assert!(
                labels
                    .iter()
                    .all(|label| label.len() == labels.first().unwrap().len())
            );
        }
    }

    #[test]
    fn focus_connect_exact_plug_cancel_and_undo_through_ui() {
        let ctx = egui::Context::default();
        let mut editor = Editor::with_context(&ctx, Graph::default());
        frame(&ctx, &mut editor, vec![]);
        let original = editor.graph.clone();
        press(&ctx, &mut editor, egui::Key::F);
        choose(&ctx, &mut editor, 0);
        assert_eq!(editor.selected, original.nodes[0].id);
        assert_eq!(editor.selection, BTreeSet::from([editor.selected]));
        assert_eq!(editor.zoom, 1.0);
        press(&ctx, &mut editor, egui::Key::C);
        choose(&ctx, &mut editor, 0);
        let source = editor.selected;
        let index = editor.hint_targets().iter().position(|target| matches!(target,
            Target::Plug(id, EdgeSlot::Image(slot)) if original.node(*id).unwrap().inputs[*slot] != Some(source)
        )).unwrap();
        let Target::Plug(target, EdgeSlot::Image(slot)) = editor.hint_targets()[index] else {
            panic!()
        };
        choose(&ctx, &mut editor, index);
        assert_eq!(
            editor.graph.node(target).unwrap().inputs[slot],
            Some(source)
        );
        assert_eq!(editor.undo.last(), Some(&original));
        let connected = editor.graph.clone();
        press(&ctx, &mut editor, egui::Key::F);
        press(&ctx, &mut editor, egui::Key::Escape);
        assert!(editor.keyboard.hints.is_none());
        assert_eq!(editor.graph, connected);
        editor.selected = target;
        editor.selection = BTreeSet::from([target]);
        press(&ctx, &mut editor, egui::Key::C);
        choose(&ctx, &mut editor, 0);
        assert!(
            editor
                .hint_targets()
                .iter()
                .all(|candidate| !matches!(candidate, Target::Plug(id, _) if *id == source))
        );
    }

    #[test]
    fn properties_increment_move_validate_and_record_undo() {
        let ctx = egui::Context::default();
        let mut graph = Graph::default();
        let id = graph
            .add(
                NodeKind::Levels {
                    contrast: 1.0,
                    brightness: 0.0,
                    gamma: 1.0,
                },
                [0.0, 0.0],
            )
            .unwrap();
        let mut editor = Editor::with_context(&ctx, graph);
        editor.selected = id;
        editor.selection = BTreeSet::from([id]);
        frame(&ctx, &mut editor, vec![]);
        press(&ctx, &mut editor, egui::Key::P);
        let fields = editor.keyboard.controls.clone();
        let index = fields
            .iter()
            .position(|control| control.path == "/contrast")
            .unwrap();
        choose(&ctx, &mut editor, index);
        assert_eq!(editor.keyboard.property, Some((id, index)));
        let before = editor.graph.clone();
        press(&ctx, &mut editor, egui::Key::L);
        let NodeKind::Levels { contrast, .. } = editor.graph.node(id).unwrap().kind else {
            panic!()
        };
        assert!((contrast - 1.01).abs() < 0.00001);
        assert_eq!(editor.undo.last(), Some(&before));
        press(&ctx, &mut editor, egui::Key::H);
        assert_eq!(editor.graph, before);
        press(&ctx, &mut editor, egui::Key::J);
        assert_eq!(
            editor.keyboard.property,
            Some((id, (index + 1) % fields.len()))
        );
        press(&ctx, &mut editor, egui::Key::K);
        assert_eq!(editor.keyboard.property, Some((id, index)));
        if let NodeKind::Levels { contrast, .. } = &mut editor.graph.node_mut(id).unwrap().kind {
            *contrast = 0.005;
        }
        press(&ctx, &mut editor, egui::Key::H);
        assert!(matches!(
            editor.graph.node(id).unwrap().kind,
            NodeKind::Levels { contrast: 0.0, .. }
        ));
        assert!(editor.graph.validate().is_ok());
    }

    #[test]
    fn number_input_plugs_bind_parameters_and_avoid_invalid_types() {
        let ctx = egui::Context::default();
        let mut graph = Graph::default();
        let source = graph
            .add(
                NodeKind::Input(crate::recipe::NumberInput::default()),
                [0.0, 0.0],
            )
            .unwrap();
        let mut editor = Editor::with_context(&ctx, graph);
        editor.selected = source;
        editor.selection = BTreeSet::from([source]);
        frame(&ctx, &mut editor, vec![]);
        press(&ctx, &mut editor, egui::Key::C);
        choose(&ctx, &mut editor, 0);
        assert!(!editor.hint_targets().is_empty());
        assert!(
            editor
                .hint_targets()
                .iter()
                .all(|target| matches!(target, Target::Plug(_, EdgeSlot::Parameter(_))))
        );
        let Target::Plug(target, EdgeSlot::Parameter(key)) = editor.hint_targets()[0].clone()
        else {
            panic!()
        };
        choose(&ctx, &mut editor, 0);
        assert_eq!(
            editor.graph.node(target).unwrap().parameters.get(&key),
            Some(&source)
        );
        assert!(editor.graph.validate().is_ok());
    }

    #[test]
    fn overview_zoom_and_text_entry_do_not_conflict() {
        let ctx = egui::Context::default();
        let mut editor = Editor::with_context(&ctx, Graph::default());
        frame(&ctx, &mut editor, vec![]);
        let graph = editor.graph.clone();
        press(&ctx, &mut editor, egui::Key::Q);
        let zoom = editor.zoom;
        press(&ctx, &mut editor, egui::Key::Plus);
        assert!(editor.zoom > zoom);
        press(&ctx, &mut editor, egui::Key::Minus);
        assert!((editor.zoom - zoom).abs() < 0.0001);
        assert_eq!(editor.graph, graph);
        ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.text_edit_singleline(&mut editor.search).request_focus();
        })
        .drop_without_applying_deltas();
        press(&ctx, &mut editor, egui::Key::F);
        assert!(editor.keyboard.hints.is_none());
        assert_eq!(editor.graph, graph);
    }

    #[test]
    fn nested_properties_enum_and_exact_integer_steps() {
        let fields = properties(&NodeKind::Terrain(Default::default()));
        assert!(fields.iter().any(|(path, _)| path == "/bands/0/seam/mode"));
        assert!(fields.iter().any(|(path, _)| path == "/bands/0/color/0"));
        assert_eq!(
            adjusted(&Value::from(u64::MAX), -1),
            Some(Value::from(u64::MAX - 1))
        );
        assert_eq!(adjusted(&Value::from(u64::MAX), 1), None);
        assert_eq!(adjusted(&Value::from(0_u64), -1), None);
        assert_eq!(
            adjusted(&Value::from("circle"), 1),
            Some(Value::from("square"))
        );
        assert_eq!(adjusted(&Value::from(false), 1), Some(Value::from(true)));
    }

    #[test]
    fn mouse_selected_node_still_accepts_connection_shortcut() {
        let ctx = egui::Context::default();
        let mut editor = Editor::with_context(&ctx, Graph::default());
        let node = &editor.graph.nodes[0];
        let id = node.id;
        let position = Pos2::ZERO + editor.pan + Vec2::from(node.position) + Vec2::new(90.0, 90.0);
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1380.0, 880.0))),
                ..Default::default()
            },
            |ui| editor.canvas(ui),
        )
        .drop_without_applying_deltas();
        for pressed in [true, false] {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1380.0, 880.0))),
                    events: vec![
                        egui::Event::PointerMoved(position),
                        egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| editor.canvas(ui),
            )
            .drop_without_applying_deltas();
        }
        assert_eq!(editor.selected, id);
        press(&ctx, &mut editor, egui::Key::C);
        choose(&ctx, &mut editor, 0);
        assert!(editor.keyboard.hints == Some(HintMode::Plugs(id)));
        assert!(!editor.hint_targets().is_empty());
    }

    #[test]
    fn multi_letter_hints_focus_offscreen_nodes_and_overview_fits_extreme_positions() {
        let ctx = egui::Context::default();
        let mut graph = Graph::default();
        for index in 0..30 {
            graph
                .add(
                    NodeKind::Constant { value: 0.5 },
                    [index as f32 * 3000.0, 0.0],
                )
                .unwrap();
        }
        let id = graph.nodes.last().unwrap().id;
        let mut editor = Editor::with_context(&ctx, graph);
        frame(&ctx, &mut editor, vec![]);
        press(&ctx, &mut editor, egui::Key::F);
        assert!(editor.zoom < 0.02);
        let index = editor.graph.nodes.len() - 1;
        assert_eq!(hint_label(index, editor.hint_targets().len()).len(), 2);
        choose(&ctx, &mut editor, index);
        assert_eq!(editor.selected, id);
        assert_eq!(editor.zoom, 1.0);
        press(&ctx, &mut editor, egui::Key::Q);
        assert!(editor.zoom < 0.02);
        assert!(editor.pan.is_finite());
    }

    #[test]
    fn create_shortcut_selects_type_adds_at_center_and_can_cancel() {
        let ctx = egui::Context::default();
        let mut editor = Editor::with_context(&ctx, Graph::default());
        frame(&ctx, &mut editor, vec![]);
        let before = editor.graph.clone();
        press(&ctx, &mut editor, egui::Key::Tab);
        assert!(editor.keyboard.hints == Some(HintMode::Create));
        assert_eq!(editor.graph, before);
        press(&ctx, &mut editor, egui::Key::Escape);
        assert_eq!(editor.graph, before);
        press(&ctx, &mut editor, egui::Key::Tab);
        press(&ctx, &mut editor, egui::Key::Tab);
        let index = editor
            .create_items()
            .iter()
            .position(|kind| matches!(kind, NodeKind::Shape(_)))
            .unwrap();
        for _ in 0..index {
            press(&ctx, &mut editor, egui::Key::J);
        }
        press(&ctx, &mut editor, egui::Key::Enter);
        assert_eq!(editor.graph.nodes.len(), before.nodes.len() + 1);
        assert!(matches!(
            editor.graph.node(editor.selected).unwrap().kind,
            NodeKind::Shape(_)
        ));
        assert_eq!(editor.selection, BTreeSet::from([editor.selected]));
        assert_eq!(editor.undo.last(), Some(&before));
    }

    #[test]
    fn input_first_selects_exact_port_then_compatible_output_and_records_undo() {
        let ctx = egui::Context::default();
        let mut graph = Graph::default();
        let source = graph
            .add(NodeKind::Constant { value: 0.5 }, [0.0, 0.0])
            .unwrap();
        let target = graph
            .add(
                NodeKind::Blend {
                    mode: BlendMode::Normal,
                    opacity: 0.5,
                },
                [300.0, 0.0],
            )
            .unwrap();
        let mut editor = Editor::with_context(&ctx, graph);
        editor.selected = target;
        editor.selection = BTreeSet::from([target]);
        frame(&ctx, &mut editor, vec![]);
        let before = editor.graph.clone();
        press(&ctx, &mut editor, egui::Key::C);
        let index = editor
            .hint_targets()
            .iter()
            .position(
                |entry| matches!(entry, Target::Plug(id, EdgeSlot::Image(1)) if *id == target),
            )
            .unwrap();
        choose(&ctx, &mut editor, index);
        assert_eq!(editor.graph, before);
        assert!(editor.keyboard.hints == Some(HintMode::Sources(target, EdgeSlot::Image(1))));
        let index = editor
            .hint_targets()
            .iter()
            .position(|entry| matches!(entry, Target::Output(id) if *id == source))
            .unwrap();
        choose(&ctx, &mut editor, index);
        assert_eq!(editor.graph.node(target).unwrap().inputs[1], Some(source));
        assert_eq!(editor.graph.node(target).unwrap().inputs[0], None);
        assert_eq!(editor.undo.last(), Some(&before));
        let connected = editor.graph.clone();
        press(&ctx, &mut editor, egui::Key::C);
        choose(&ctx, &mut editor, 0);
        press(&ctx, &mut editor, egui::Key::Escape);
        assert_eq!(editor.graph, connected);
    }

    #[test]
    fn input_first_numeric_port_only_offers_valid_number_outputs() {
        let ctx = egui::Context::default();
        let mut graph = Graph::default();
        let source = graph
            .add(
                NodeKind::Input(crate::recipe::NumberInput::default()),
                [0.0, 0.0],
            )
            .unwrap();
        let target = graph
            .add(NodeKind::Constant { value: 0.5 }, [300.0, 0.0])
            .unwrap();
        let mut editor = Editor::with_context(&ctx, graph);
        editor.selected = target;
        editor.selection = BTreeSet::from([target]);
        frame(&ctx, &mut editor, vec![]);
        press(&ctx, &mut editor, egui::Key::C);
        let index = editor.hint_targets().iter().position(|entry| matches!(entry, Target::Plug(id, EdgeSlot::Parameter(key)) if *id == target && key == "value")).unwrap();
        choose(&ctx, &mut editor, index);
        assert!(
            editor
                .hint_targets()
                .iter()
                .all(|entry| matches!(entry, Target::Output(id) if *id == source))
        );
        choose(&ctx, &mut editor, 0);
        assert_eq!(
            editor.graph.node(target).unwrap().parameters.get("value"),
            Some(&source)
        );
        assert!(editor.graph.validate().is_ok());
    }

    #[test]
    fn shift_logarithmic_slider_moves_in_equal_tenths_of_track() {
        let ctx = egui::Context::default();
        let mut editor = Editor::with_context(&ctx, Graph::default());
        let id = editor
            .graph
            .nodes
            .iter()
            .find(|node| matches!(node.kind, NodeKind::Noise(_)))
            .unwrap()
            .id;
        editor.selected = id;
        editor.selection = BTreeSet::from([id]);
        if let NodeKind::Noise(noise) = &mut editor.graph.node_mut(id).unwrap().kind {
            noise.frequency = 0.0001;
        }
        frame(&ctx, &mut editor, vec![]);
        press(&ctx, &mut editor, egui::Key::P);
        let index = editor
            .keyboard
            .controls
            .iter()
            .position(|control| control.path == "/frequency")
            .unwrap();
        assert!(editor.keyboard.controls[index].logarithmic);
        choose(&ctx, &mut editor, index);
        let before = editor.graph.clone();
        for (key, indices) in [
            (egui::Key::L, (1..=10).collect::<Vec<_>>()),
            (egui::Key::H, (0..10).rev().collect()),
        ] {
            for index in indices {
                press_modified(&ctx, &mut editor, key, egui::Modifiers::SHIFT);
                let NodeKind::Noise(noise) = &editor.graph.node(id).unwrap().kind else {
                    panic!()
                };
                let position = (f64::from(noise.frequency).log10() + 4.0) / 4.0;
                assert!(
                    (position - f64::from(index) / 10.0).abs() < 1e-6,
                    "{index}: {position}"
                );
            }
        }
        assert_eq!(editor.undo.first(), Some(&before));
        press(&ctx, &mut editor, egui::Key::L);
        let NodeKind::Noise(noise) = &editor.graph.node(id).unwrap().kind else {
            panic!()
        };
        assert!((noise.frequency - 0.0002).abs() < 1e-7);
    }

    #[test]
    fn escape_releases_node_selection_then_direction_keys_pan() {
        let ctx = egui::Context::default();
        let mut editor = Editor::with_context(&ctx, Graph::default());
        frame(&ctx, &mut editor, vec![]);
        press(&ctx, &mut editor, egui::Key::F);
        choose(&ctx, &mut editor, 0);
        assert!(editor.single_node_selected());
        let graph = editor.graph.clone();
        press(&ctx, &mut editor, egui::Key::Escape);
        assert!(editor.selection.is_empty());
        assert!(editor.keyboard.focus.is_none());
        let pan = editor.pan;
        press(&ctx, &mut editor, egui::Key::L);
        assert_eq!(editor.pan, pan + Vec2::new(40.0, 0.0));
        assert_eq!(editor.graph, graph);
        press(&ctx, &mut editor, egui::Key::Delete);
        assert_eq!(editor.graph, graph);

        editor.selection = editor.graph.nodes.iter().map(|node| node.id).collect();
        press(&ctx, &mut editor, egui::Key::P);
        press(&ctx, &mut editor, egui::Key::Escape);
        assert!(editor.selection.is_empty());
        assert_eq!(editor.graph, graph);

        press(&ctx, &mut editor, egui::Key::F);
        choose(&ctx, &mut editor, 0);
        press(&ctx, &mut editor, egui::Key::P);
        press(&ctx, &mut editor, egui::Key::Escape);
        assert!(editor.single_node_selected());
        assert!(editor.keyboard.hints.is_none());
        press(&ctx, &mut editor, egui::Key::Escape);
        assert!(editor.selection.is_empty());
    }

    #[test]
    fn unselected_direction_keys_pan_without_editing_graph() {
        let ctx = egui::Context::default();
        let mut editor = Editor::with_context(&ctx, Graph::default());
        frame(&ctx, &mut editor, vec![]);
        editor.selection.clear();
        let graph = editor.graph.clone();
        let undo = editor.undo.clone();
        let revision = editor.revision;
        for zoom in [0.5, 2.0] {
            editor.zoom = zoom;
            for (key, direction) in [
                (egui::Key::H, [-1.0, 0.0]),
                (egui::Key::J, [0.0, 1.0]),
                (egui::Key::K, [0.0, -1.0]),
                (egui::Key::L, [1.0, 0.0]),
                (egui::Key::Y, [-1.0, -1.0]),
                (egui::Key::U, [1.0, -1.0]),
                (egui::Key::B, [-1.0, 1.0]),
                (egui::Key::N, [1.0, 1.0]),
            ] {
                for (modifiers, step) in [
                    (egui::Modifiers::NONE, 40.0),
                    (egui::Modifiers::SHIFT, 160.0),
                ] {
                    let pan = editor.pan;
                    press_modified(&ctx, &mut editor, key, modifiers);
                    assert_eq!(editor.pan, pan + Vec2::from(direction) * step);
                    assert_eq!(editor.graph, graph);
                    assert_eq!(editor.undo, undo);
                    assert_eq!(editor.revision, revision);
                }
            }
        }
        let pan = editor.pan;
        press_modified(&ctx, &mut editor, egui::Key::H, egui::Modifiers::CTRL);
        assert_eq!(editor.pan, pan);
        press(&ctx, &mut editor, egui::Key::Tab);
        press(&ctx, &mut editor, egui::Key::L);
        assert_eq!(editor.pan, pan);
        assert_eq!(editor.keyboard.create_category, 1);
    }

    #[test]
    fn eight_direction_movement_shift_bounds_and_property_mode_are_separate() {
        let ctx = egui::Context::default();
        let mut editor = Editor::with_context(&ctx, Graph::default());
        frame(&ctx, &mut editor, vec![]);
        press(&ctx, &mut editor, egui::Key::F);
        choose(&ctx, &mut editor, 0);
        let id = editor.selected;
        let revision = editor.revision;
        for (key, direction) in [
            (egui::Key::H, [-1.0, 0.0]),
            (egui::Key::J, [0.0, 1.0]),
            (egui::Key::K, [0.0, -1.0]),
            (egui::Key::L, [1.0, 0.0]),
            (egui::Key::Y, [-1.0, -1.0]),
            (egui::Key::U, [1.0, -1.0]),
            (egui::Key::B, [-1.0, 1.0]),
            (egui::Key::N, [1.0, 1.0]),
        ] {
            for (modifiers, step) in [(egui::Modifiers::NONE, 8.0), (egui::Modifiers::SHIFT, 40.0)]
            {
                let before = editor.graph.clone();
                let start = before.node(id).unwrap().position;
                press_modified(&ctx, &mut editor, key, modifiers);
                assert_eq!(
                    editor.graph.node(id).unwrap().position,
                    [
                        start[0] + direction[0] * step,
                        start[1] + direction[1] * step
                    ]
                );
                assert_eq!(editor.undo.last(), Some(&before));
                assert_eq!(editor.revision, revision);
            }
        }
        editor.graph.node_mut(id).unwrap().position = [99999.0, -99999.0];
        press_modified(&ctx, &mut editor, egui::Key::U, egui::Modifiers::SHIFT);
        assert_eq!(
            editor.graph.node(id).unwrap().position,
            [100000.0, -100000.0]
        );
        press(&ctx, &mut editor, egui::Key::P);
        choose(&ctx, &mut editor, 0);
        let position = editor.graph.node(id).unwrap().position;
        press(&ctx, &mut editor, egui::Key::J);
        press(&ctx, &mut editor, egui::Key::L);
        assert_eq!(editor.graph.node(id).unwrap().position, position);
    }

    #[test]
    fn focused_node_delete_removes_node_and_undo_restores_connections() {
        let ctx = egui::Context::default();
        let mut editor = Editor::with_context(&ctx, Graph::default());
        frame(&ctx, &mut editor, vec![]);
        let before = editor.graph.clone();
        press(&ctx, &mut editor, egui::Key::F);
        choose(&ctx, &mut editor, 0);
        let id = editor.selected;
        press(&ctx, &mut editor, egui::Key::Delete);
        assert!(editor.graph.node(id).is_none());
        assert!(
            editor
                .graph
                .nodes
                .iter()
                .all(|node| !node.inputs.contains(&Some(id)))
        );
        assert_eq!(editor.undo.last(), Some(&before));
        press_modified(&ctx, &mut editor, egui::Key::Z, egui::Modifiers::COMMAND);
        assert_eq!(editor.graph, before);
    }

    #[test]
    fn edge_hints_delete_only_chosen_image_or_parameter_edge_and_undo_restores_it() {
        for parameter in [false, true] {
            let ctx = egui::Context::default();
            let mut graph = Graph::default();
            let source = graph
                .add(
                    NodeKind::Input(crate::recipe::NumberInput::default()),
                    [0.0, 0.0],
                )
                .unwrap();
            let target = graph
                .add(NodeKind::Constant { value: 0.5 }, [300.0, 0.0])
                .unwrap();
            graph.bind_parameter(target, "value", Some(source)).unwrap();
            let mut editor = Editor::with_context(&ctx, graph);
            frame(&ctx, &mut editor, vec![]);
            let before = editor.graph.clone();
            press(&ctx, &mut editor, egui::Key::E);
            let targets = editor.hint_targets();
            let index = targets.iter().position(|entry| matches!(entry, Target::Edge(edge) if matches!(edge.slot, EdgeSlot::Parameter(_)) == parameter)).unwrap();
            let Target::Edge(edge) = targets[index].clone() else {
                panic!()
            };
            choose(&ctx, &mut editor, index);
            assert_eq!(editor.selected_edge, Some(edge.clone()));
            assert!(editor.selection.is_empty());
            assert_eq!(editor.graph, before);
            press(&ctx, &mut editor, egui::Key::Delete);
            let mut expected = before.clone();
            edge.disconnect(&mut expected);
            assert_eq!(editor.graph, expected);
            press_modified(&ctx, &mut editor, egui::Key::Z, egui::Modifiers::COMMAND);
            assert_eq!(editor.graph, before);
            press(&ctx, &mut editor, egui::Key::E);
            press(&ctx, &mut editor, egui::Key::Escape);
            assert!(editor.keyboard.hints.is_none());
            assert!(editor.selected_edge.is_none());
        }
    }

    #[test]
    fn alignment_is_spaced_dependency_ordered_idempotent_and_undoable_without_recompute() {
        let ctx = egui::Context::default();
        let mut graph = Graph::default();
        let source = graph
            .add(
                NodeKind::Input(crate::recipe::NumberInput::default()),
                [1000.0, 1000.0],
            )
            .unwrap();
        let target = graph
            .add(NodeKind::Constant { value: 0.5 }, [-1000.0, -1000.0])
            .unwrap();
        graph.bind_parameter(target, "value", Some(source)).unwrap();
        graph.nodes.reverse();
        let mut editor = Editor::with_context(&ctx, graph);
        frame(&ctx, &mut editor, vec![]);
        let before = editor.graph.clone();
        let revision = editor.revision;
        press(&ctx, &mut editor, egui::Key::A);
        assert!(editor.graph.validate().is_ok());
        assert!(editor.graph.same_computation(&before));
        assert_eq!(editor.revision, revision);
        assert_eq!(editor.undo.last(), Some(&before));
        for (index, node) in editor.graph.nodes.iter().enumerate() {
            for source in node.inputs.iter().flatten().chain(node.parameters.values()) {
                assert!(editor.graph.node(*source).unwrap().position[0] < node.position[0]);
            }
            let bounds = Rect::from_min_size(Pos2::from(node.position), Vec2::new(180.0, 290.0));
            for other in editor.graph.nodes.iter().skip(index + 1) {
                assert!(!bounds.intersects(Rect::from_min_size(
                    Pos2::from(other.position),
                    Vec2::new(180.0, 290.0)
                )));
            }
        }
        let aligned = editor.graph.clone();
        let history = editor.undo.len();
        press(&ctx, &mut editor, egui::Key::A);
        assert_eq!(editor.graph, aligned);
        assert_eq!(editor.undo.len(), history);
        press_modified(&ctx, &mut editor, egui::Key::Z, egui::Modifiers::COMMAND);
        assert_eq!(editor.graph, before);
    }

    #[test]
    fn empty_edge_selection_does_not_leave_a_node_armed_for_deletion() {
        let ctx = egui::Context::default();
        let mut graph = Graph::default();
        for node in &mut graph.nodes {
            node.inputs.fill(None);
        }
        let mut editor = Editor::with_context(&ctx, graph);
        frame(&ctx, &mut editor, vec![]);
        let before = editor.graph.clone();
        press(&ctx, &mut editor, egui::Key::E);
        assert!(editor.keyboard.hints.is_none());
        press(&ctx, &mut editor, egui::Key::Delete);
        assert_eq!(editor.graph, before);
    }

    #[test]
    fn tab_creation_cycles_categories_and_rows_without_editing_the_graph() {
        let ctx = egui::Context::default();
        let mut editor = Editor::with_context(&ctx, Graph::default());
        frame(&ctx, &mut editor, vec![]);
        let before = editor.graph.clone();
        press(&ctx, &mut editor, egui::Key::Tab);
        assert!(editor.keyboard.hints == Some(HintMode::Create));
        assert!(editor.keyboard.property.is_none());
        press(&ctx, &mut editor, egui::Key::L);
        assert_eq!(editor.keyboard.create_category, 1);
        press(&ctx, &mut editor, egui::Key::J);
        assert_eq!(editor.keyboard.create_row, 1);
        press(&ctx, &mut editor, egui::Key::H);
        assert_eq!(editor.keyboard.create_category, 0);
        assert_eq!(editor.keyboard.create_row, 0);
        press(&ctx, &mut editor, egui::Key::H);
        assert_eq!(editor.keyboard.create_category, CREATE_CATEGORIES.len() - 1);
        press(&ctx, &mut editor, egui::Key::L);
        assert_eq!(editor.keyboard.create_category, 0);
        for (category, category_name) in CREATE_CATEGORIES.iter().enumerate() {
            assert_eq!(editor.keyboard.create_category, category);
            let items = editor.create_items();
            assert!(!items.is_empty());
            assert!(
                items
                    .iter()
                    .all(|kind| library_group(kind) == *category_name)
            );
            press(&ctx, &mut editor, egui::Key::K);
            assert_eq!(editor.keyboard.create_row, items.len() - 1);
            press(&ctx, &mut editor, egui::Key::J);
            assert_eq!(editor.keyboard.create_row, 0);
            press(&ctx, &mut editor, egui::Key::Tab);
        }
        assert_eq!(editor.keyboard.create_category, 0);
        press_modified(&ctx, &mut editor, egui::Key::Tab, egui::Modifiers::SHIFT);
        assert_eq!(editor.keyboard.create_category, CREATE_CATEGORIES.len() - 1);
        press(&ctx, &mut editor, egui::Key::Escape);
        assert!(editor.keyboard.hints.is_none());
        assert_eq!(editor.graph, before);
    }

    #[test]
    fn hints_overlay_existing_controls_focus_and_adjust_by_the_slider_step() {
        fn text_positions(shape: &egui::epaint::Shape, result: &mut Vec<(Pos2, String)>) {
            match shape {
                egui::epaint::Shape::Text(text) => {
                    result.push((text.pos, text.galley.job.text.clone()))
                }
                egui::epaint::Shape::Vec(shapes) => {
                    for shape in shapes {
                        text_positions(shape, result);
                    }
                }
                _ => {}
            }
        }
        let ctx = egui::Context::default();
        let mut graph = Graph::default();
        let id = graph
            .add(NodeKind::Shape(Default::default()), [0.0, 0.0])
            .unwrap();
        let mut editor = Editor::with_context(&ctx, graph);
        editor.selected = id;
        editor.selection = BTreeSet::from([id]);
        frame(&ctx, &mut editor, vec![]);
        press(&ctx, &mut editor, egui::Key::P);
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1380.0, 880.0))),
                ..Default::default()
            },
            |ui| editor.render_ui(ui),
        );
        let mut texts = Vec::new();
        for shape in &output.shapes {
            text_positions(&shape.shape, &mut texts);
        }
        assert!(
            !texts
                .iter()
                .any(|(_, text)| text.contains("KEYBOARD PROPERTIES"))
        );
        let index = editor
            .keyboard
            .controls
            .iter()
            .position(|control| control.path == "/scale_x")
            .unwrap();
        let control = editor.keyboard.controls[index].clone();
        let hint = hint_label(index, editor.keyboard.controls.len());
        assert!(
            texts.iter().any(|(position, text)| text == &hint
                && position.distance(control.rect.left_top()) < 0.1)
        );
        output.drop_without_applying_deltas();
        choose(&ctx, &mut editor, index);
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(editor.keyboard.controls[index].id)
        );
        press(&ctx, &mut editor, egui::Key::L);
        let NodeKind::Shape(shape) = &editor.graph.node(id).unwrap().kind else {
            panic!()
        };
        assert!((shape.scale_x - 0.81).abs() < 0.00001);
        press(&ctx, &mut editor, egui::Key::ArrowRight);
        let NodeKind::Shape(shape) = &editor.graph.node(id).unwrap().kind else {
            panic!()
        };
        assert!((shape.scale_x - 0.82).abs() < 0.00001);
        press_modified(&ctx, &mut editor, egui::Key::Z, egui::Modifiers::COMMAND);
        let NodeKind::Shape(shape) = &editor.graph.node(id).unwrap().kind else {
            panic!()
        };
        assert!((shape.scale_x - 0.81).abs() < 0.00001);
        press_modified(
            &ctx,
            &mut editor,
            egui::Key::Z,
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        );
        let NodeKind::Shape(shape) = &editor.graph.node(id).unwrap().kind else {
            panic!()
        };
        assert!((shape.scale_x - 0.82).abs() < 0.00001);
        press(&ctx, &mut editor, egui::Key::J);
        let (_, index) = editor.keyboard.property.unwrap();
        assert_eq!(editor.keyboard.controls[index].path, "/scale_y");
        press(&ctx, &mut editor, egui::Key::Escape);
        assert!(editor.keyboard.property.is_none());
        let position = editor.graph.node(id).unwrap().position;
        press(&ctx, &mut editor, egui::Key::L);
        assert_eq!(
            editor.graph.node(id).unwrap().position[0],
            position[0] + 8.0
        );
    }

    #[test]
    fn actual_controls_cover_catalog_numeric_fields_and_nested_terrain() {
        for mut kind in NodeKind::catalog() {
            if let NodeKind::Map(MapNode::Entrances { rectangle, .. }) = &mut kind {
                *rectangle = true;
            }
            let ctx = egui::Context::default();
            let mut graph = Graph::default();
            let id = graph.add(kind.clone(), [0.0, 0.0]).unwrap();
            let mut editor = Editor::with_context(&ctx, graph);
            editor.selected = id;
            editor.selection = BTreeSet::from([id]);
            frame(&ctx, &mut editor, vec![]);
            for key in crate::recipe::numeric_parameters(&kind) {
                assert!(
                    editor
                        .keyboard
                        .controls
                        .iter()
                        .any(|control| control.path == format!("/{key}")),
                    "{}: {key}",
                    kind.title()
                );
            }
            if matches!(kind, NodeKind::Terrain(_)) {
                assert!(
                    editor
                        .keyboard
                        .controls
                        .iter()
                        .any(|control| control.path == "/bands/0/seam/width")
                );
                assert!(
                    editor
                        .keyboard
                        .controls
                        .iter()
                        .any(|control| control.path == "/bands/0/color/0")
                );
            }
        }
    }

    #[test]
    fn shifted_node_hints_accumulate_and_release_allows_group_move_delete_and_undo() {
        let ctx = egui::Context::default();
        let mut editor = Editor::with_context(&ctx, Graph::default());
        frame(&ctx, &mut editor, vec![]);
        press(&ctx, &mut editor, egui::Key::F);
        choose(&ctx, &mut editor, 0);
        let first = editor.selected;
        press(&ctx, &mut editor, egui::Key::F);
        choose_shifted(&ctx, &mut editor, 1);
        let second = editor.selected;
        choose_shifted(&ctx, &mut editor, 2);
        let third = editor.selected;
        assert_eq!(editor.selection, BTreeSet::from([first, second, third]));
        assert!(editor.keyboard.hints == Some(HintMode::Nodes));
        choose_shifted(&ctx, &mut editor, 1);
        assert_eq!(editor.selection.len(), 3);
        let before = editor.graph.clone();
        let revision = editor.revision;
        press(&ctx, &mut editor, egui::Key::L);
        assert!(editor.keyboard.hints.is_none());
        for node in &editor.graph.nodes {
            let old = before.node(node.id).unwrap();
            assert_eq!(
                node.position,
                [
                    old.position[0]
                        + if editor.selection.contains(&node.id) {
                            8.0
                        } else {
                            0.0
                        },
                    old.position[1]
                ]
            );
        }
        assert_eq!(editor.revision, revision);
        let moved = editor.graph.clone();
        press_modified(&ctx, &mut editor, egui::Key::U, egui::Modifiers::SHIFT);
        for id in &editor.selection {
            let old = moved.node(*id).unwrap();
            assert_eq!(
                editor.graph.node(*id).unwrap().position,
                [old.position[0] + 40.0, old.position[1] - 40.0]
            );
        }
        let before_delete = editor.graph.clone();
        let selection = editor.selection.clone();
        press(&ctx, &mut editor, egui::Key::Delete);
        assert!(selection.iter().all(|id| editor.graph.node(*id).is_none()));
        assert_eq!(
            editor.graph.nodes.len(),
            before_delete.nodes.len() - selection.len()
        );
        assert!(editor.graph.validate().is_ok());
        press_modified(&ctx, &mut editor, egui::Key::Z, egui::Modifiers::COMMAND);
        assert_eq!(editor.graph, before_delete);
    }

    #[test]
    fn multi_character_shift_selection_and_single_node_operations_are_exclusive() {
        let ctx = egui::Context::default();
        let mut graph = Graph::default();
        for index in 0..30 {
            graph
                .add(
                    NodeKind::Constant { value: 0.5 },
                    [index as f32 * 200.0, 0.0],
                )
                .unwrap();
        }
        let mut editor = Editor::with_context(&ctx, graph);
        frame(&ctx, &mut editor, vec![]);
        press(&ctx, &mut editor, egui::Key::F);
        choose(&ctx, &mut editor, 0);
        let first = editor.selected;
        press(&ctx, &mut editor, egui::Key::F);
        let index = editor.graph.nodes.len() - 1;
        assert_eq!(hint_label(index, editor.hint_targets().len()).len(), 2);
        choose_shifted(&ctx, &mut editor, index);
        assert_eq!(editor.selection.len(), 2);
        press(&ctx, &mut editor, egui::Key::Escape);
        let before = editor.graph.clone();
        for key in [egui::Key::P, egui::Key::C] {
            press(&ctx, &mut editor, key);
            assert!(editor.keyboard.hints.is_none());
            assert!(editor.keyboard.property.is_none());
            assert_eq!(editor.graph, before);
        }
        press(&ctx, &mut editor, egui::Key::F);
        choose(&ctx, &mut editor, 0);
        assert_eq!(editor.selection, BTreeSet::from([first]));
        press(&ctx, &mut editor, egui::Key::P);
        assert!(editor.keyboard.hints == Some(HintMode::Properties));
    }

    #[test]
    fn group_movement_clamps_one_shared_delta_and_preserves_spacing() {
        let ctx = egui::Context::default();
        let mut editor = Editor::with_context(&ctx, Graph::default());
        let first = editor.graph.nodes[0].id;
        let second = editor.graph.nodes[1].id;
        editor.graph.node_mut(first).unwrap().position = [99999.0, -99999.0];
        editor.graph.node_mut(second).unwrap().position = [99979.0, -99979.0];
        editor.selected = first;
        editor.selection = BTreeSet::from([first, second]);
        frame(&ctx, &mut editor, vec![]);
        press_modified(&ctx, &mut editor, egui::Key::U, egui::Modifiers::SHIFT);
        assert_eq!(
            editor.graph.node(first).unwrap().position,
            [100000.0, -100000.0]
        );
        assert_eq!(
            editor.graph.node(second).unwrap().position,
            [99980.0, -99980.0]
        );
    }

    #[test]
    fn shift_property_adjustment_divides_min_max_range_into_ten_steps() {
        let ctx = egui::Context::default();
        let mut graph = Graph::default();
        let shape = graph
            .add(NodeKind::Shape(Default::default()), [0.0, 0.0])
            .unwrap();
        let blur = graph
            .add(NodeKind::Blur { radius: 0 }, [300.0, 0.0])
            .unwrap();
        let mut editor = Editor::with_context(&ctx, graph);
        editor.selected = shape;
        editor.selection = BTreeSet::from([shape]);
        frame(&ctx, &mut editor, vec![]);
        press(&ctx, &mut editor, egui::Key::P);
        let index = editor
            .keyboard
            .controls
            .iter()
            .position(|control| control.path == "/scale_x")
            .unwrap();
        choose(&ctx, &mut editor, index);
        let before = editor.graph.clone();
        press_modified(&ctx, &mut editor, egui::Key::L, egui::Modifiers::SHIFT);
        let NodeKind::Shape(settings) = &editor.graph.node(shape).unwrap().kind else {
            panic!()
        };
        assert!((settings.scale_x - 0.899).abs() < 0.000001);
        assert_eq!(editor.undo.last(), Some(&before));
        press_modified(&ctx, &mut editor, egui::Key::H, egui::Modifiers::SHIFT);
        let NodeKind::Shape(settings) = &editor.graph.node(shape).unwrap().kind else {
            panic!()
        };
        assert!((settings.scale_x - 0.8).abs() < 0.000001);
        press(&ctx, &mut editor, egui::Key::L);
        let NodeKind::Shape(settings) = &editor.graph.node(shape).unwrap().kind else {
            panic!()
        };
        assert!((settings.scale_x - 0.81).abs() < 0.000001);
        if let NodeKind::Shape(settings) = &mut editor.graph.node_mut(shape).unwrap().kind {
            settings.scale_x = 0.01;
        }
        for _ in 0..10 {
            press_modified(&ctx, &mut editor, egui::Key::L, egui::Modifiers::SHIFT);
        }
        let NodeKind::Shape(settings) = &editor.graph.node(shape).unwrap().kind else {
            panic!()
        };
        assert_eq!(settings.scale_x, 1.0);
        for _ in 0..10 {
            press_modified(&ctx, &mut editor, egui::Key::H, egui::Modifiers::SHIFT);
        }
        let NodeKind::Shape(settings) = &editor.graph.node(shape).unwrap().kind else {
            panic!()
        };
        assert_eq!(settings.scale_x, 0.01);
        press(&ctx, &mut editor, egui::Key::Escape);
        editor.selected = blur;
        editor.selection = BTreeSet::from([blur]);
        frame(&ctx, &mut editor, vec![]);
        press(&ctx, &mut editor, egui::Key::P);
        choose(&ctx, &mut editor, 0);
        for _ in 0..10 {
            press_modified(&ctx, &mut editor, egui::Key::L, egui::Modifiers::SHIFT);
        }
        assert!(matches!(
            editor.graph.node(blur).unwrap().kind,
            NodeKind::Blur { radius: 32 }
        ));
    }

    #[test]
    fn creation_modal_keeps_categories_on_one_row_and_reserves_ten_item_rows() {
        fn collect(shape: &egui::epaint::Shape, clip: Rect, texts: &mut Vec<(String, Pos2, Rect)>) {
            match shape {
                egui::epaint::Shape::Text(text) => {
                    texts.push((text.galley.job.text.clone(), text.pos, clip))
                }
                egui::epaint::Shape::Vec(shapes) => {
                    for shape in shapes {
                        collect(shape, clip, texts);
                    }
                }
                _ => {}
            }
        }
        let ctx = egui::Context::default();
        let mut editor = Editor::with_context(&ctx, Graph::default());
        frame(&ctx, &mut editor, vec![]);
        press(&ctx, &mut editor, egui::Key::Tab);
        editor.keyboard.create_category = 5;
        frame(&ctx, &mut editor, vec![]);
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1380.0, 880.0))),
                ..Default::default()
            },
            |ui| editor.render_ui(ui),
        );
        let mut texts = Vec::new();
        for shape in &output.shapes {
            collect(&shape.shape, shape.clip_rect, &mut texts);
        }
        let categories: Vec<_> = CREATE_CATEGORIES
            .iter()
            .map(|category| {
                texts
                    .iter()
                    .rev()
                    .find(|(text, _, _)| text == category)
                    .unwrap()
            })
            .collect();
        assert!(categories.iter().all(
            |(_, position, clip)| (position.y - categories[0].1.y).abs() < 1.0
                && clip.contains(*position)
        ));
        let items = editor.create_items();
        let first = texts
            .iter()
            .rev()
            .find(|(text, _, _)| text == items[0].title())
            .unwrap();
        let second = texts
            .iter()
            .rev()
            .find(|(text, _, _)| text == items[1].title())
            .unwrap();
        let pitch = second.1.y - first.1.y;
        assert!(pitch > 0.0);
        assert!(
            first.2.height() >= pitch * 10.0 - 4.0,
            "list height {}, row pitch {}",
            first.2.height(),
            pitch
        );
        output.drop_without_applying_deltas();
    }

    #[test]
    fn c_selects_both_connector_directions_i_o_are_removed_and_only_tab_opens_creation() {
        let ctx = egui::Context::default();
        let mut editor = Editor::with_context(&ctx, Graph::default());
        frame(&ctx, &mut editor, vec![]);
        press(&ctx, &mut editor, egui::Key::F);
        choose(&ctx, &mut editor, 0);
        let before = editor.graph.clone();
        for key in [egui::Key::I, egui::Key::O] {
            press(&ctx, &mut editor, key);
            assert!(editor.keyboard.hints.is_none());
            assert_eq!(editor.graph, before);
        }
        press(&ctx, &mut editor, egui::Key::C);
        assert!(editor.keyboard.hints == Some(HintMode::Connectors(editor.selected)));
        assert!(
            editor
                .hint_targets()
                .iter()
                .any(|target| matches!(target, Target::Output(id) if *id == editor.selected))
        );
        assert!(
            editor
                .hint_targets()
                .iter()
                .any(|target| matches!(target, Target::Plug(id, _) if *id == editor.selected))
        );
        assert_eq!(editor.graph, before);
        press(&ctx, &mut editor, egui::Key::Escape);
        press(&ctx, &mut editor, egui::Key::Tab);
        assert!(editor.keyboard.hints == Some(HintMode::Create));
        assert_eq!(editor.graph, before);
    }
}
