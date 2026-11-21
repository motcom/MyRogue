use super::*;
use egui::emath::Numeric;

#[derive(Clone)]
pub(super) struct Control {
    pub path: String,
    pub id: egui::Id,
    pub rect: Rect,
    pub clip: Rect,
    pub step: f64,
    pub range: Option<(f64, f64)>,
    pub native_focus: bool,
    pub logarithmic: bool,
}

#[derive(Clone, Default)]
struct Frame {
    json: Value,
    prefix: String,
    controls: Vec<Control>,
}

fn frame_id() -> egui::Id {
    egui::Id::unique("map_gen_property_controls")
}

pub(in super::super) fn begin_controls(ui: &egui::Ui, kind: &NodeKind) {
    ui.ctx().data_mut(|data| {
        data.insert_temp(
            frame_id(),
            Some(Frame {
                json: serde_json::to_value(kind).unwrap(),
                prefix: String::new(),
                controls: Vec::new(),
            }),
        )
    });
}

pub(in super::super) fn property_prefix(ui: &egui::Ui, prefix: String) {
    ui.ctx().data_mut(|data| {
        if let Some(frame) = data
            .get_temp_mut_or_default::<Option<Frame>>(frame_id())
            .as_mut()
        {
            frame.prefix = prefix;
        }
    });
}

pub(in super::super) fn record(
    ui: &egui::Ui,
    response: &egui::Response,
    key: &str,
    step: f64,
    range: Option<(f64, f64)>,
    native_focus: bool,
    logarithmic: bool,
) {
    if !ui.is_enabled() {
        return;
    }
    ui.ctx().data_mut(|data| {
        let Some(frame) = data
            .get_temp_mut_or_default::<Option<Frame>>(frame_id())
            .as_mut()
        else {
            return;
        };
        let path = format!("{}/{}", frame.prefix, key);
        if frame.json.pointer(&path).is_some() {
            frame.controls.push(Control {
                path,
                id: response.id,
                rect: response.rect,
                clip: ui.clip_rect(),
                step,
                range,
                native_focus,
                logarithmic,
            });
        }
    });
}

pub(in super::super) fn end_controls(ui: &mut egui::Ui, keyboard: &mut Keyboard, owner: NodeId) {
    let frame = ui.ctx().data_mut(|data| {
        data.get_temp_mut_or_default::<Option<Frame>>(frame_id())
            .take()
    });
    let Some(frame) = frame else {
        return;
    };
    let old_path = keyboard.property.and_then(|(id, index)| {
        (id == owner)
            .then(|| {
                keyboard
                    .controls
                    .get(index)
                    .map(|control| control.path.clone())
            })
            .flatten()
    });
    keyboard.controls = frame.controls;
    if let Some(path) = old_path {
        keyboard.property = keyboard
            .controls
            .iter()
            .position(|control| control.path == path)
            .map(|index| (owner, index));
    }
    let hinting = keyboard.hints == Some(HintMode::Properties);
    for (index, control) in keyboard.controls.iter().enumerate() {
        let selected = keyboard.property == Some((owner, index));
        if selected {
            if keyboard.scroll_property {
                ui.ctx().memory_mut(|memory| {
                    if let Some(id) = memory.focused() {
                        memory.surrender_focus(id);
                    }
                    if control.native_focus {
                        memory.request_focus(control.id);
                    }
                });
            }
            if keyboard.scroll_property {
                ui.scroll_to_rect(control.rect, Some(egui::Align::Center));
            }
            ui.painter().with_clip_rect(control.clip).rect_stroke(
                control.rect.expand(2.0),
                2.0,
                Stroke::new(2.0, ACCENT),
                StrokeKind::Outside,
            );
        }
        if hinting {
            let label = hint_label(index, keyboard.controls.len());
            if label.starts_with(&keyboard.prefix) {
                let painter = ui.painter().with_clip_rect(control.clip);
                let channel = if control.path.contains("/color/") {
                    control
                        .path
                        .rsplit('/')
                        .next()
                        .and_then(|channel| channel.parse::<usize>().ok())
                        .unwrap_or(0)
                } else {
                    0
                };
                let position = control.rect.left_top() + Vec2::new(channel as f32 * 22.0, 0.0);
                let galley = painter.layout_no_wrap(label, FontId::monospace(12.0), BG);
                painter.rect_filled(
                    Rect::from_min_size(position, galley.size()).expand(1.0),
                    2.0,
                    ACCENT,
                );
                painter.galley(position, galley, BG);
            }
        }
    }
    keyboard.scroll_property = false;
}

fn key(label: &str) -> String {
    match label {
        "Noise type" | "Shape type" => "kind".into(),
        "Blend mode" | "Mode" => "mode".into(),
        "Bend frequency" => "frequency".into(),
        "Distance range / px" => "range".into(),
        "Min center distance / px" => "min_distance".into(),
        "Point radius / px" => "radius".into(),
        "Wall width / px" | "Marker width / px" => "width".into(),
        "Outward offset / px" => "offset".into(),
        "Min spacing / px" => "spacing".into(),
        "Extra paths" => "count".into(),
        "Birth neighbors" => "birth_limit".into(),
        "Survival neighbors" => "survival_limit".into(),
        "Min child size / px" => "min_size".into(),
        "Split depth" => "depth".into(),
        "Divider width / px" => "wall_width".into(),
        "Mask cutoff" => "cutoff".into(),
        "Region cutoff" => "cutoff".into(),
        _ => label
            .split('/')
            .next()
            .unwrap_or(label)
            .trim()
            .to_lowercase()
            .replace(' ', "_"),
    }
}

pub(in super::super) struct PropertySlider<'value> {
    inner: egui::Slider<'value>,
    key: String,
    step: f64,
    range: (f64, f64),
    logarithmic: bool,
}
impl<'value> PropertySlider<'value> {
    pub fn new<Number: Numeric>(
        value: &'value mut Number,
        range: std::ops::RangeInclusive<Number>,
    ) -> Self {
        let bounds = (range.start().to_f64(), range.end().to_f64());
        let step = if Number::INTEGRAL || bounds.1 - bounds.0 > 8.0 {
            1.0
        } else if bounds.1 - bounds.0 <= 0.5 {
            0.005
        } else {
            0.01
        };
        Self {
            inner: egui::Slider::new(value, range)
                .step_by(step)
                .clamping(egui::SliderClamping::Edits)
                .max_decimals(if Number::INTEGRAL { 0 } else { 6 }),
            key: String::new(),
            step,
            range: bounds,
            logarithmic: false,
        }
    }
    pub fn text(mut self, label: &str) -> Self {
        self.key = key(label);
        self.inner = self.inner.text(label);
        self
    }
    pub fn logarithmic(mut self, logarithmic: bool) -> Self {
        self.logarithmic = logarithmic;
        self.inner = self.inner.logarithmic(logarithmic);
        if logarithmic {
            self.step = 0.0001;
            self.inner = self.inner.step_by(self.step);
        }
        self
    }
}
impl egui::Widget for PropertySlider<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let response = ui.add(self.inner);
        record(
            ui,
            &response,
            &self.key,
            self.step,
            Some(self.range),
            true,
            self.logarithmic,
        );
        response
    }
}

pub(in super::super) struct PropertyDrag<'value> {
    inner: egui::DragValue<'value>,
    key: String,
    step: f64,
    range: Option<(f64, f64)>,
}
impl<'value> PropertyDrag<'value> {
    pub fn new<Number: Numeric>(value: &'value mut Number) -> Self {
        Self {
            inner: egui::DragValue::new(value),
            key: "seed".into(),
            step: 1.0,
            range: Some((Number::MIN.to_f64(), Number::MAX.to_f64())),
        }
    }
    pub fn property(mut self, key: &str) -> Self {
        self.key = key.into();
        self
    }
    pub fn speed(mut self, speed: f64) -> Self {
        self.step = speed;
        self.inner = self.inner.speed(speed);
        self
    }
    pub fn max_decimals(mut self, decimals: usize) -> Self {
        self.inner = self.inner.max_decimals(decimals);
        self
    }
    pub fn range<Number: Numeric>(mut self, range: std::ops::RangeInclusive<Number>) -> Self {
        self.range = Some((range.start().to_f64(), range.end().to_f64()));
        self.inner = self.inner.range(range);
        self
    }
    pub fn prefix(mut self, prefix: &str) -> Self {
        self.inner = self.inner.prefix(prefix);
        self
    }
}
impl egui::Widget for PropertyDrag<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let response = ui.add(self.inner);
        record(
            ui, &response, &self.key, self.step, self.range, false, false,
        );
        response
    }
}

pub(in super::super) fn checkbox(
    ui: &mut egui::Ui,
    value: &mut bool,
    key: &str,
    label: &str,
) -> egui::Response {
    let response = ui.checkbox(value, label);
    record(ui, &response, key, 1.0, None, true, false);
    response
}

pub(in super::super) fn record_combo(ui: &egui::Ui, response: &egui::Response, label: &str) {
    record(ui, response, &key(label), 1.0, None, true, false);
}
