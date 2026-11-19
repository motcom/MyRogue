use crate::model::entity::{
    Controller, CreatureKind, FacilityKind, Grip, Hand, ItemProp, Psykick, ResourceKind, Skill,
    StaticRole, Status, Zairyou,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub fn ensure(ok: bool, message: impl Into<String>) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(message.into().into())
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
impl Pos {
    pub fn distance(self, other: Self) -> i32 {
        if self.z != other.z {
            i32::MAX
        } else {
            (i64::from(self.x) - i64::from(other.x))
                .abs()
                .max((i64::from(self.y) - i64::from(other.y)).abs())
                .min(i64::from(i32::MAX)) as i32
        }
    }
    pub fn offset(self, dx: i32, dy: i32) -> Self {
        Self {
            x: self.x.saturating_add(dx),
            y: self.y.saturating_add(dy),
            ..self
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Nutrition {
    pub protein: u32,
    pub carbohydrate: u32,
    pub vitamin: u32,
}
impl Nutrition {
    pub fn initial() -> Self {
        Self {
            protein: 50,
            carbohydrate: 50,
            vitamin: 50,
        }
    }
    pub fn add(&mut self, n: &Self) {
        self.protein = self.protein.saturating_add(n.protein).min(100);
        self.carbohydrate = self.carbohydrate.saturating_add(n.carbohydrate).min(100);
        self.vitamin = self.vitamin.saturating_add(n.vitamin).min(100);
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dice {
    pub dice: u32,
    pub sides: u32,
    pub bonus: i32,
}
impl Dice {
    pub fn validate(&self) -> Result<()> {
        ensure(
            (1..=20).contains(&self.dice)
                && (1..=100).contains(&self.sides)
                && self.bonus.unsigned_abs() <= 1000,
            "invalid damage dice",
        )
    }
}
impl std::fmt::Display for Dice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}d{}", self.dice, self.sides)?;
        if self.bonus != 0 {
            write!(f, "{:+}", self.bonus)?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Appearance {
    pub glyph: String,
    pub fg: [u8; 3],
    pub bg: [u8; 3],
    pub layer: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Collision {
    pub movement: bool,
    pub sight: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Durability {
    Flag(bool),
    Hp { max: u32 },
}
impl Durability {
    pub fn max(&self) -> Option<u32> {
        match self {
            Self::Hp { max } => Some(*max),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Common {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub appearance: Appearance,
    pub collision: Collision,
    pub durability: Durability,
    pub tags: Vec<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Combat {
    #[serde(default)]
    pub base_av: u32,
    #[serde(default)]
    pub dv_bonus: i32,
    #[serde(default)]
    pub accuracy_bonus: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attack {
    pub damage: Dice,
    pub range: u32,
    pub base_pv: u32,
    pub strength_cap: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weapon {
    #[serde(default)]
    pub skill: Option<crate::game::progression::SkillKind>,
    #[serde(default)]
    pub two_hand_pv_bonus: u32,
    pub hands: WeaponHands,
    #[serde(default)]
    pub one_hand_range_penalty: u32,
    #[serde(default)]
    pub one_hand_accuracy_penalty: u32,
    pub critical_value: u32,
    pub mode: String,
    pub damage: Dice,
    pub range: u32,
    pub base_pv: u32,
    pub strength_cap: u32,
    #[serde(default)]
    pub accuracy_bonus: i32,
    pub ammo_definition_id: Option<String>,
    #[serde(default)]
    pub ammo_per_attack: u32,
    #[serde(default)]
    pub magazine_capacity: Option<u32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeaponHands {
    One,
    Two,
    Either,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Armor {
    #[serde(default = "default_armor_slot")]
    pub slot: String,
    pub av: u32,
    pub dv: i32,
}
fn default_armor_slot() -> String {
    "body".into()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Use {
    pub kind: String,
    pub need: Option<String>,
    pub amount: Option<u32>,
    pub status_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NeedDefinition {
    pub max: u32,
    pub initial: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InventoryCapacity {
    pub max_length_mm: u32,
    pub max_volume_ml: u32,
    pub max_mass_g: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InitialItem {
    pub definition_id: String,
    pub quantity: u32,
    #[serde(default)]
    pub equipped: bool,
    #[serde(default)]
    pub grip: Option<Grip>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Senses {
    pub sight_range: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Binding {
    pub r#type: String,
    #[serde(default)]
    pub dx: i32,
    #[serde(default)]
    pub dy: i32,
    #[serde(default)]
    pub dz: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MovableDefinition {
    #[serde(default)]
    pub experience_reward: u32,
    #[serde(default)]
    pub dominant_hand: Hand,
    pub role: CreatureKind,
    pub controller: Controller,
    pub faction: String,
    pub hp: u32,
    pub body: Vec<String>,
    pub attributes: BTreeMap<String, u32>,
    pub skills: Skill,
    #[serde(default = "default_sanity")]
    pub sanity: u32,
    #[serde(default)]
    pub psykick: Vec<Psykick>,
    #[serde(default)]
    pub status: Vec<Status>,
    pub inventory: InventoryCapacity,
    pub senses: Senses,
    pub attack: Attack,
    pub combat: Combat,
    pub loadout: Vec<InitialItem>,
    pub needs: BTreeMap<String, NeedDefinition>,
    #[serde(default)]
    pub controls: BTreeMap<String, Binding>,
}
fn default_movement_cost() -> f32 {
    1.0
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StaticDefinition {
    pub role: StaticRole,
    #[serde(default = "default_movement_cost")]
    pub movement_cost: f32,
    pub facilities: Vec<FacilityKind>,
    pub resources: BTreeMap<ResourceKind, u32>,
    pub combat: Combat,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemDefinition {
    pub throwable: Option<Throwable>,
    #[serde(default)]
    pub contents: Vec<ContainerEntry>,
    pub length_mm: u32,
    pub volume_ml: u32,
    pub mass_g: u32,
    pub stack_limit: u32,
    pub materials: Vec<Zairyou>,
    pub tool_functions: Vec<ItemProp>,
    #[serde(default)]
    pub tool_levels: BTreeMap<ItemProp, u32>,
    pub resources: BTreeMap<ResourceKind, u32>,
    pub weapon: Option<Weapon>,
    pub armor: Option<Armor>,
    #[serde(rename = "use")]
    pub use_effect: Option<Use>,
    pub nutrition: Option<Nutrition>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Throwable {
    pub range: u32,
    pub radius: u32,
    pub power: u32,
    pub kind: BlastKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlastKind {
    Explosive,
    Flash,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerEntry {
    pub definition_id: String,
    pub quantity: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionDefinition {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub when_state: BTreeMap<String, serde_json::Value>,
    #[serde(default = "interaction_consumes_turn")]
    pub consumes_turn: bool,
}
fn interaction_consumes_turn() -> bool {
    true
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Definition {
    #[serde(default)]
    pub interactions: Vec<InteractionDefinition>,
    pub schema_version: u32,
    pub id: String,
    pub category: String,
    pub common: Common,
    pub initial_state: serde_json::Value,
    pub movable: Option<MovableDefinition>,
    pub r#static: Option<StaticDefinition>,
    pub item: Option<ItemDefinition>,
}
impl Definition {
    pub fn validate(&self) -> Result<()> {
        ensure(
            self.schema_version == 2,
            "expected entity schema_version = 2",
        )?;
        ensure(
            !self.id.is_empty() && !self.common.name.is_empty(),
            "empty id/name",
        )?;
        ensure(self.interactions.len() <= 32, "too many interactions")?;
        let mut ids = std::collections::BTreeSet::new();
        for interaction in &self.interactions {
            ensure(
                !interaction.id.is_empty() && ids.insert(&interaction.id),
                "invalid/duplicate interaction id",
            )?;
            ensure(
                !interaction.name.trim().is_empty()
                    && interaction.name.chars().count() <= 48
                    && !interaction.name.chars().any(char::is_control),
                "invalid interaction name",
            )?;
            ensure(
                interaction.when_state.iter().all(|(key, value)| {
                    !key.is_empty() && !value.is_array() && !value.is_object() && !value.is_null()
                }),
                "invalid interaction condition",
            )?;
        }
        let a = &self.common.appearance;
        ensure(
            a.glyph.len() == 1 && a.glyph.as_bytes()[0].is_ascii_graphic(),
            "glyph must be printable ASCII",
        )?;
        ensure(a.layer <= 100, "layer must be <=100")?;
        match self.common.durability {
            Durability::Flag(false) => {}
            Durability::Hp { max } => ensure(max > 0, "max durability must be positive")?,
            _ => return Err("durability must be false or {max > 0}".into()),
        }
        ensure(
            self.initial_state.is_object() || self.initial_state.is_array(),
            "initial_state must be a table",
        )?;
        ensure(
            [
                self.movable.is_some(),
                self.r#static.is_some(),
                self.item.is_some(),
            ]
            .iter()
            .filter(|x| **x)
            .count()
                == 1,
            "exactly one category table required",
        )?;
        match self.category.as_str() {
            "movable" => {
                let m = self.movable.as_ref().ok_or("missing movable")?;
                ensure(!m.faction.is_empty(), "empty faction")?;
                ensure(m.hp > 0, "HP must be positive")?;
                ensure(
                    (1..=1_000_000).contains(&m.sanity),
                    "sanity must be positive",
                )?;
                ensure(m.attributes.len() == 4, "four attributes required")?;
                let expected = [
                    "head",
                    "body",
                    "left_arm",
                    "right_arm",
                    "left_leg",
                    "right_leg",
                ];
                ensure(
                    m.body.len() == expected.len()
                        && expected
                            .iter()
                            .all(|name| m.body.iter().filter(|p| p.as_str() == *name).count() == 1),
                    "six unique body parts required",
                )?;
                for key in ["kinryoku", "subayasa", "kiyousa", "seishinryoku"] {
                    ensure(
                        m.attributes.get(key).is_some_and(|v| *v <= 100),
                        "invalid/missing attribute",
                    )?;
                }
                ensure(
                    m.needs.len() == 1 && m.needs.contains_key("hydration"),
                    "needs required",
                )?;
                ensure(
                    m.needs.values().all(|n| n.max > 0 && n.initial <= n.max),
                    "invalid need bounds",
                )?;
                m.attack.damage.validate()?;
                ensure(
                    m.attack.range == 1 && m.attack.base_pv <= 30 && m.attack.strength_cap <= 30,
                    "invalid innate attack",
                )?;
                ensure(m.experience_reward <= 100000, "experience reward too large")?;
                validate_combat(&m.combat)?;
                ensure(m.senses.sight_range <= 100, "sight range too large")?;
                ensure(
                    crate::game::progression::SkillKind::ALL
                        .into_iter()
                        .all(|skill| skill.value(&m.skills) <= 100),
                    "initial skill exceeds 100",
                )?;

                ensure(
                    m.loadout
                        .iter()
                        .all(|i| i.quantity > 0 && i.quantity <= 1000),
                    "invalid loadout quantity",
                )?;
                for b in m.controls.values() {
                    ensure(
                        matches!(
                            b.r#type.as_str(),
                            "move"
                                | "open_crafting"
                                | "open_inventory"
                                | "open_map"
                                | "shoot"
                                | "reload"
                                | "pickup"
                                | "open_inspect"
                                | "interact"
                                | "open"
                        ),
                        "unknown control",
                    )?;
                    if b.r#type == "move" {
                        ensure(
                            (-1..=1).contains(&b.dx)
                                && (-1..=1).contains(&b.dy)
                                && b.dz == 0
                                && (b.dx != 0 || b.dy != 0),
                            "invalid movement binding",
                        )?;
                    }
                }
            }
            "static" => {
                let s = self.r#static.as_ref().ok_or("missing static")?;
                ensure(
                    s.movement_cost.is_finite() && (0.01..=1000.0).contains(&s.movement_cost),
                    "static.movement_cost must be a finite number between 0.01 and 1000",
                )?;
                validate_combat(&s.combat)?;
            }
            "item" => {
                let i = self.item.as_ref().ok_or("missing item")?;
                ensure(
                    i.stack_limit > 0 && i.stack_limit <= 1000,
                    "invalid stack limit",
                )?;
                ensure(
                    i.tool_levels.iter().all(|(function, level)| {
                        i.tool_functions.contains(function) && (1..=20).contains(level)
                    }),
                    "tool_levels must name a tool function with level 1..20",
                )?;
                ensure(i.contents.len() <= 100, "too many container entries")?;
                ensure(
                    i.contents.is_empty() || i.stack_limit == 1,
                    "containers cannot stack",
                )?;
                for entry in &i.contents {
                    ensure(
                        (1..=1000).contains(&entry.quantity),
                        "invalid container quantity",
                    )?;
                }
                if let Some(t) = &i.throwable {
                    ensure(
                        (1..=20).contains(&t.range)
                            && t.radius == 2
                            && (1..=100).contains(&t.power)
                            && i.weapon.is_none(),
                        "invalid throwable",
                    )?;
                }
                if let Some(w) = &i.weapon {
                    w.damage.validate()?;
                    ensure(
                        w.two_hand_pv_bonus <= 30
                            && (w.two_hand_pv_bonus == 0
                                || (w.mode == "melee" && w.hands == WeaponHands::Either)),
                        "two_hand_pv_bonus requires an either-grip melee weapon",
                    )?;
                    ensure(
                        w.one_hand_accuracy_penalty <= 100 && w.one_hand_range_penalty < w.range,
                        "invalid one-hand penalty",
                    )?;
                    ensure(
                        w.hands == WeaponHands::Either
                            || (w.one_hand_accuracy_penalty == 0 && w.one_hand_range_penalty == 0),
                        "one-hand penalties require either grip",
                    )?;
                    ensure(
                        (2..=12).contains(&w.critical_value),
                        "critical_value must be 2..12",
                    )?;
                    ensure(
                        matches!(w.mode.as_str(), "melee" | "ranged")
                            && w.range > 0
                            && w.range <= 100
                            && w.base_pv <= 30
                            && w.strength_cap <= 30
                            && w.accuracy_bonus.unsigned_abs() <= 100,
                        "invalid weapon",
                    )?;
                    if w.mode == "ranged" {
                        ensure(
                            w.ammo_definition_id.is_some() && w.ammo_per_attack > 0,
                            "missing ammo",
                        )?;
                        if let Some(capacity) = w.magazine_capacity {
                            ensure(
                                capacity >= w.ammo_per_attack && capacity <= 1000,
                                "invalid magazine capacity",
                            )?;
                        }
                    } else {
                        ensure(
                            w.ammo_definition_id.is_none() && w.magazine_capacity.is_none(),
                            "invalid melee range/ammo",
                        )?;
                    }
                }
                if let Some(armor) = &i.armor {
                    ensure(
                        matches!(
                            armor.slot.as_str(),
                            "head" | "body" | "arms" | "hands" | "legs" | "feet"
                        ),
                        "invalid armor slot",
                    )?;
                    ensure(
                        armor.av <= 100 && armor.dv.unsigned_abs() <= 100,
                        "invalid armor AV/DV",
                    )?;
                }
                if let Some(u) = &i.use_effect {
                    ensure(
                        matches!(
                            u.kind.as_str(),
                            "heal" | "remove_status" | "restore_need" | "bandage" | "nutrition"
                        ),
                        "unknown use kind",
                    )?;
                    if u.kind == "restore_need" {
                        ensure(
                            i.nutrition.is_some() && matches!(u.need.as_deref(), Some("hydration")),
                            "food nutrition/need required",
                        )?;
                    }
                    if u.kind == "nutrition" {
                        ensure(i.nutrition.is_some(), "nutrition required")?;
                    } else if u.kind != "remove_status" {
                        ensure(u.amount.is_some_and(|n| n > 0), "positive amount required")?;
                    } else {
                        ensure(u.status_id.is_some(), "status id required")?;
                    }
                }
            }
            _ => return Err("unknown category".into()),
        }
        Ok(())
    }
}
fn validate_combat(c: &Combat) -> Result<()> {
    ensure(
        c.base_av <= 100
            && c.dv_bonus.unsigned_abs() <= 100
            && c.accuracy_bonus.unsigned_abs() <= 100,
        "invalid combat values",
    )
}
#[derive(Clone, Debug, Deserialize)]
pub struct Spawn {
    pub definition_id: String,
    pub position: Pos,
    #[serde(default = "one")]
    pub quantity: u32,
}
fn one() -> u32 {
    1
}
#[derive(Clone, Debug, Deserialize)]
pub struct Scenario {
    #[serde(default)]
    pub map_generator: Option<MapGenerator>,
    #[serde(default)]
    pub labels: Vec<MapLabel>,
    pub width: i32,
    pub height: i32,
    pub seed: u64,
    pub ground: String,
    pub boundary: String,
    pub player: Spawn,
    pub spawns: Vec<Spawn>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapGenerator {
    /// Optional world chunk coordinate. Enables stitched edges and omits the scenario perimeter wall.
    #[serde(default)]
    pub tile: Option<[i32; 2]>,
    pub json: String,
    #[serde(default)]
    pub inputs: BTreeMap<String, f64>,
    /// Ascending inclusive upper bounds; the last band must end at 1.
    #[serde(default)]
    pub terrain: Vec<MapTerrainBand>,
    /// Optional map ID aliases; otherwise IDs are Entity definition IDs directly.
    #[serde(default)]
    pub ids: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapTerrainBand {
    pub max: f32,
    pub definition_id: String,
}

fn default_sanity() -> u32 {
    100
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapLabel {
    pub position: Pos,
    pub text: String,
}
