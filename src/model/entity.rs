use crate::model::config::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
pub type EntityId = u32;
pub type Id = EntityId;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LuaHookRef {
    pub hook_name: String,
}
impl LuaHookRef {
    pub fn new(name: &str) -> Self {
        Self {
            hook_name: name.into(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScriptBinding {
    #[serde(default)]
    pub glyph: Option<String>,
    pub definition_id: String,
    pub state: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HP {
    pub max_hp: u32,
    pub hp: u32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InjuryStage {
    #[default]
    Healthy,
    Bleeding,
    HeavyBleeding,
    Severed,
}
impl InjuryStage {
    pub fn next(self) -> Self {
        match self {
            Self::Healthy => Self::Bleeding,
            Self::Bleeding => Self::HeavyBleeding,
            Self::HeavyBleeding | Self::Severed => Self::Severed,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Healthy => "異常なし",
            Self::Bleeding => "出血",
            Self::HeavyBleeding => "大出血",
            Self::Severed => "切断",
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WoundTreatment {
    pub bandages: BTreeMap<String, u64>,
    pub stopped: BTreeSet<String>,
}
pub fn body_part_name(name: &str) -> &str {
    match name {
        "head" => "頭",
        "body" => "胴体",
        "left_arm" => "左腕",
        "right_arm" => "右腕",
        "left_leg" => "左脚",
        "right_leg" => "右脚",
        name => name,
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Skill {
    pub melee_weapon: u32,
    pub shield: u32,
    pub rifle: u32,
    pub bow: u32,
    pub touteki: u32,
    pub picking: u32,
    pub hacking: u32,
    pub wood_craft: u32,
    pub metal_craft: u32,
    pub denshi_craft: u32,
    pub saihou: u32,
    pub cooking: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Psykick {
    pub name: String,
    pub disc: String,
    pub shiyou_kaisu: u32,
    pub exe: LuaHookRef,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Status {
    pub name: String,
    pub enable: bool,
    pub value: u32,
    pub exe: LuaHookRef,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Meter {
    pub current: u32,
    pub max: u32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Hand {
    #[default]
    Right,
    Left,
}
impl Hand {
    pub fn part(self) -> &'static str {
        match self {
            Self::Right => "right_arm",
            Self::Left => "left_arm",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Right => "右手",
            Self::Left => "左手",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grip {
    Right,
    Left,
    Both,
}
impl Grip {
    pub fn label(self) -> &'static str {
        match self {
            Self::Right => "右手",
            Self::Left => "左手",
            Self::Both => "両手",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QuickSlot {
    Equipment { item: Id, grip: Grip },
    Consumable { definition_id: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inventory {
    pub max_longsize: u32,
    pub max_youryou: u32,
    pub max_jyuuryou: u32,
    pub items: Vec<EntityId>,
    pub equipped: Option<EntityId>,
    pub right_hand: Option<EntityId>,
    pub left_hand: Option<EntityId>,
    pub armor: BTreeMap<String, EntityId>,
    pub quick_slots: [Option<QuickSlot>; 10],
}
impl Inventory {
    pub fn grip(&self, item: Id) -> Option<Grip> {
        match (self.right_hand == Some(item), self.left_hand == Some(item)) {
            (true, true) => Some(Grip::Both),
            (true, false) => Some(Grip::Right),
            (false, true) => Some(Grip::Left),
            _ => None,
        }
    }
    pub fn forget_item(&mut self, item: Id) {
        self.unequip(item);
        for slot in &mut self.quick_slots {
            if matches!(slot,Some(QuickSlot::Equipment{item: id,..}) if *id == item) {
                *slot = None;
            }
        }
    }
    pub fn unequip(&mut self, item: Id) {
        self.armor.retain(|_, id| *id != item);
        if self.right_hand == Some(item) {
            self.right_hand = None;
        }
        if self.left_hand == Some(item) {
            self.left_hand = None;
        }
        if self.equipped == Some(item) {
            self.equipped = None;
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Controller {
    Input,
    Lua,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreatureKind {
    Player,
    Monster,
    Animal,
    Npc,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StaticRole {
    Terrain,
    Obstacle,
    Facility,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FacilityKind {
    WorkSurface,
    Clamping,
    Anvil,
    ForgeHeat,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Zairyou {
    Wood,
    Metal,
    Cloth,
    Leather,
    Stone,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemProp {
    Cutting,
    WoodCutting,
    Hammering,
    Drilling,
    Fastening,
    Prying,
    Digging,
    Sewing,
    Cooking,
    Butchering,
    MetalCutting,
    Grinding,
    Welding,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Fuel,
    Electricity,
    WeldingMaterial,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceAmount {
    pub kind: ResourceKind,
    pub amount: u32,
}
#[derive(Clone)]
pub struct MoveEntity {
    pub collision: Collision,
    pub script: ScriptBinding,
    pub controller: Controller,
    pub nutrition: Nutrition,
    pub carbohydrate_usage: u32,
    pub stamina: u32,
    pub max_stamina: u32,
    pub running_mode: bool,
    pub run_step_credit: bool,
    pub stamina_recovery_usage: u32,
    pub illness_turns: u32,
    pub illness_recovery_turns: u32,
    pub pneumonia_move_turns: u64,
    pub id: EntityId,
    pub kind: CreatureKind,
    pub position: Pos,
    pub hp: HP,
    pub dominant_hand: Hand,
    pub next_move_turn: u64,
    pub stunned_until: u64,
    pub alert_position: Option<Pos>,
    pub alert_until: u64,
    pub body: BTreeMap<String, InjuryStage>,
    pub treatment: Box<WoundTreatment>,
    pub hydration: Meter,
    pub sanity: u32,
    pub max_sanity: u32,
    pub ghoul: bool,
    pub status: Vec<Status>,
    pub kinryoku: u32,
    pub subayasa: u32,
    pub kiyousa: u32,
    pub seishinryoku: u32,
    pub skill: Box<Skill>,
    pub progression: Box<crate::game::progression::Progression>,
    pub psykick: Vec<Psykick>,
    // Keep the Entity enum compact as hand equipment grows.
    pub inventory: Box<Inventory>,
    pub crafting: Option<crate::game::crafting::CraftJob>,
}
impl MoveEntity {
    pub fn respiratory_status(&self) -> Option<&'static str> {
        ["pneumonia", "cold"].into_iter().find(|name| {
            self.status
                .iter()
                .any(|s| s.name == *name && s.enable && s.value > 0)
        })
    }

    pub fn illness_label(&self) -> &'static str {
        match self.respiratory_status() {
            Some("pneumonia") => "肺炎",
            Some("cold") => "風邪",
            _ => "正常",
        }
    }

    pub fn can_regenerate(&self) -> bool {
        self.nutrition.protein > 0.0 && self.respiratory_status().is_none()
    }

    pub fn hand_usable(&self, hand: Hand) -> bool {
        self.body
            .get(hand.part())
            .is_some_and(|s| *s != InjuryStage::Severed)
    }
    pub fn movement_turns(&self) -> u64 {
        match ["left_leg", "right_leg"]
            .iter()
            .filter(|p| self.body.get(**p) == Some(&InjuryStage::Severed))
            .count()
        {
            0 => 1,
            1 => 4,
            _ => 8,
        }
    }
    pub fn part_bleeding(&self, part: &str) -> u32 {
        if self.treatment.bandages.contains_key(part) || self.treatment.stopped.contains(part) {
            return 0;
        }
        match self.body.get(part) {
            Some(InjuryStage::Bleeding) => 1,
            Some(InjuryStage::HeavyBleeding) => 2,
            Some(InjuryStage::Severed) => 4,
            _ => 0,
        }
    }
    pub fn bleeding_damage(&self) -> u32 {
        self.body.keys().map(|part| self.part_bleeding(part)).sum()
    }
    pub fn wound_label(&self, part: &str) -> String {
        let Some(stage) = self.body.get(part) else {
            return "不明".into();
        };
        if self.treatment.stopped.contains(part) {
            return match stage {
                InjuryStage::Severed => "切断・止血済み",
                InjuryStage::HeavyBleeding => "重い傷・止血済み",
                InjuryStage::Bleeding => "傷・止血済み",
                InjuryStage::Healthy => "異常なし",
            }
            .into();
        }
        format!(
            "{}{}",
            stage.label(),
            if self.treatment.bandages.contains_key(part) {
                "・包帯中"
            } else {
                ""
            }
        )
    }
    pub fn advance_wound(&mut self, part: &str) -> Option<InjuryStage> {
        let stage = self.body.get_mut(part)?;
        *stage = stage.next();
        self.treatment.stopped.remove(part);
        Some(*stage)
    }
    pub fn needs(&self) -> BTreeMap<&str, Meter> {
        BTreeMap::from([("hydration", self.hydration.clone())])
    }

    pub fn restore_need(&mut self, need: &str, amount: u32) -> Result<()> {
        match need {
            "hydration" => {
                self.hydration.current = self
                    .hydration
                    .current
                    .saturating_add(amount)
                    .min(self.hydration.max)
            }
            _ => return Err("unknown need".into()),
        }
        Ok(())
    }
}
#[derive(Clone)]
pub struct MapEntity {
    pub script: ScriptBinding,
    pub id: EntityId,
    pub position: Pos,
    pub collision: Collision,
}
#[derive(Clone)]
pub struct StaticEntity {
    pub script: ScriptBinding,
    pub id: EntityId,
    pub role: StaticRole,
    pub name: String,
    pub blocks_movement: bool,
    pub blocks_sight: bool,
    pub destructible: bool,
    pub facilities: Vec<FacilityKind>,
    pub resources: Vec<ResourceAmount>,
    pub position: Pos,
    pub max_taikyudo: u32,
    pub taikyudo: u32,
    pub status: Vec<Status>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemLocation {
    Ground(Pos),
    CarriedBy(EntityId),
}
#[derive(Clone)]
pub struct ItemEntity {
    pub quality: i32,
    pub contents: Vec<ContainerEntry>,
    pub loaded_ammo: u32,
    pub collision: Collision,
    pub script: ScriptBinding,
    pub id: EntityId,
    pub kind: String,
    pub name: String,
    pub location: ItemLocation,
    pub quantity: u32,
    pub resources: Vec<ResourceAmount>,
    pub longsize: u32,
    pub youryou: u32,
    pub jyuuryou: u32,
    pub max_taikyudo: u32,
    pub taikyudo: u32,
    pub zairyou: Vec<Zairyou>,
    pub nouryoku: Vec<ItemProp>,
    pub status: Vec<Status>,
}
pub trait Update {
    fn update(&mut self);
}
impl Update for HP {
    fn update(&mut self) {
        self.hp = self.hp.min(self.max_hp);
    }
}
impl Update for Status {
    fn update(&mut self) {
        if !self.enable {
            self.value = 0;
        }
    }
}
impl Update for MoveEntity {
    fn update(&mut self) {
        self.hp.update();
        for s in &mut self.status {
            s.update();
        }
        self.nutrition.add(&FoodNutrition::default());
    }
}
impl Update for StaticEntity {
    fn update(&mut self) {
        for s in &mut self.status {
            s.update();
        }
        self.taikyudo = self.taikyudo.min(self.max_taikyudo);
    }
}
impl Update for ItemEntity {
    fn update(&mut self) {
        for s in &mut self.status {
            s.update();
        }
        self.taikyudo = self.taikyudo.min(self.max_taikyudo);
    }
}
#[derive(Clone)]
pub enum Entity {
    Map(MapEntity),
    Movable(MoveEntity),
    Static(StaticEntity),
    Item(ItemEntity),
}
impl Entity {
    pub fn id(&self) -> EntityId {
        match self {
            Self::Movable(e) => e.id,
            Self::Map(e) => e.id,
            Self::Static(e) => e.id,
            Self::Item(e) => e.id,
        }
    }
    pub fn script(&self) -> &ScriptBinding {
        match self {
            Self::Movable(e) => &e.script,
            Self::Map(e) => &e.script,
            Self::Static(e) => &e.script,
            Self::Item(e) => &e.script,
        }
    }
    pub fn script_mut(&mut self) -> &mut ScriptBinding {
        match self {
            Self::Movable(e) => &mut e.script,
            Self::Map(e) => &mut e.script,
            Self::Static(e) => &mut e.script,
            Self::Item(e) => &mut e.script,
        }
    }
    pub fn map_position(&self) -> Option<Pos> {
        match self {
            Self::Movable(e) => Some(e.position),
            Self::Map(e) => Some(e.position),
            Self::Static(e) => Some(e.position),
            Self::Item(e) => match e.location {
                ItemLocation::Ground(p) => Some(p),
                _ => None,
            },
        }
    }
    pub fn carrier(&self) -> Option<EntityId> {
        match self {
            Self::Item(e) => match e.location {
                ItemLocation::CarriedBy(id) => Some(id),
                _ => None,
            },
            _ => None,
        }
    }
    pub fn blocks_movement(&self) -> bool {
        match self {
            Self::Movable(e) => e.collision.movement,
            Self::Map(e) => e.collision.movement,
            Self::Static(e) => e.blocks_movement,
            Self::Item(e) => e.collision.movement,
        }
    }
    pub fn blocks_sight(&self) -> bool {
        match self {
            Self::Movable(e) => e.collision.sight,
            Self::Map(e) => e.collision.sight,
            Self::Static(e) => e.blocks_sight,
            Self::Item(e) => e.collision.sight,
        }
    }
    pub fn alive(&self) -> bool {
        match self {
            Self::Movable(e) => {
                e.hp.hp > 0
                    && ["head", "body"].iter().all(|name| {
                        e.body
                            .get(*name)
                            .is_some_and(|stage| *stage != InjuryStage::Severed)
                    })
            }
            Self::Map(_) => true,
            Self::Static(e) => !e.destructible || e.taikyudo > 0,
            Self::Item(e) => e.quantity > 0 && (e.max_taikyudo == 0 || e.taikyudo > 0),
        }
    }
    pub fn render(&self, ctx: &mut bracket_lib::prelude::BTerm, appearance: &Appearance) {
        if let Some(p) = self.map_position() {
            use bracket_lib::prelude::*;
            ctx.set(
                p.x + 1,
                p.y + 4,
                RGB::from_u8(appearance.fg[0], appearance.fg[1], appearance.fg[2]),
                RGB::from_u8(appearance.bg[0], appearance.bg[1], appearance.bg[2]),
                to_cp437(
                    self.script()
                        .glyph
                        .as_deref()
                        .unwrap_or(&appearance.glyph)
                        .chars()
                        .next()
                        .unwrap(),
                ),
            );
        }
    }
}
impl Update for Entity {
    fn update(&mut self) {
        match self {
            Self::Movable(e) => e.update(),
            Self::Map(_) => {}
            Self::Static(e) => e.update(),
            Self::Item(e) => e.update(),
        }
    }
}
