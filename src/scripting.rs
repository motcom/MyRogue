use crate::{
    config::*,
    entity::{Grip, Hand, Id},
    game::Game,
};
use mlua::{Function, HookTriggers, Lua, LuaOptions, LuaSerdeExt, StdLib, Table, Value, VmState};
use serde::Deserialize;
use std::{
    cell::Cell,
    collections::BTreeMap,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    ToggleRunning,
    ZoomMap {
        delta: i32,
    },
    ScrollFooting {
        delta: i32,
    },
    GhoulTurn {
        behavior: crate::enemies::GhoulBehavior,
    },
    OpenItemTarget {
        item: Id,
    },
    MoveItemTarget {
        dx: i32,
        dy: i32,
    },
    ConfirmItemTarget,
    SelectRecipe {
        delta: i32,
    },
    SelectContainer {
        delta: i32,
    },
    ToggleContainerSelection,
    TakeContainerSelection,
    #[serde(rename = "wait")]
    WaitTurn,
    #[serde(rename = "move")]
    MoveTo {
        position: Pos,
    },
    Attack {
        target: Id,
    },
    Shoot {
        target: Id,
    },
    Reload,
    UseItem {
        item: Id,
    },
    Pickup {
        item: Id,
    },
    Drop {
        item: Id,
    },
    StartCraft {
        recipe: String,
    },
    ContinueCraft,
    CancelCraft,
    Quit,
    OpenInventory,
    BindQuickSlot {
        item: Id,
        slot: usize,
        grip: Option<Grip>,
    },
    ActivateQuickSlot {
        slot: usize,
    },
    ToggleArmor {
        item: Id,
    },
    SelectBandagePart {
        delta: i32,
    },
    BandagePart {
        item: Id,
        body_part: String,
    },
    OpenEquipment {
        item: Id,
    },
    SelectEquipment {
        delta: i32,
    },
    EquipInHand {
        item: Id,
        grip: Grip,
    },
    Unequip {
        item: Id,
    },
    ItemDetails {
        item: Id,
    },
    ScrollItemDetails {
        delta: i32,
    },
    BeginInteract,
    BeginOpen,
    OpenDirection {
        dx: i32,
        dy: i32,
    },
    InteractDirection {
        dx: i32,
        dy: i32,
    },
    OpenItemInteractions {
        item: Id,
    },
    SelectInteraction {
        delta: i32,
    },
    PerformInteraction {
        target: Id,
        interaction: String,
    },
    PickupNearby {
        item: Id,
    },
    OpenProperties,
    ScrollProperties {
        delta: i32,
    },
    OpenMap,
    OpenCrafting,
    OpenInspect,
    OpenTargeting,
    AimDirection {
        dx: i32,
        dy: i32,
    },
    CycleTarget {
        delta: i32,
    },
    FireSelected,
    MoveCursor {
        dx: i32,
        dy: i32,
    },
    SelectInspected {
        delta: i32,
    },
    ClosePanel,
    SelectInventory {
        delta: i32,
    },
    Equip {
        item: Id,
    },
    Message {
        message: String,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    OpenContainer {
        target: Id,
    },
    SetCollision {
        target: Id,
        movement: bool,
        sight: bool,
    },
    SetGlyph {
        target: Id,
        glyph: String,
    },
    Message {
        message: String,
    },
    Spawn {
        definition_id: String,
        position: Pos,
        quantity: u32,
    },
    WeaponAttack {
        attacker: Id,
        target: Id,
        weapon: Id,
    },
    Damage {
        target: Id,
        amount: u32,
    },
    PracticeSkill {
        target: Id,
        skill: crate::progression::SkillKind,
    },
    Heal {
        target: Id,
        amount: u32,
    },
    ApplyBandage {
        target: Id,
        body_part: String,
    },
    RemoveStatus {
        target: Id,
        status_id: String,
    },
    RestoreNeed {
        target: Id,
        need: String,
        amount: u32,
    },
    GainNutrition {
        target: Id,
        nutrition: Nutrition,
    },
    ConsumeItem {
        target: Id,
        quantity: u32,
    },
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptResult {
    pub state: Option<serde_json::Value>,
    #[serde(default)]
    pub effects: Vec<Effect>,
    pub action: Option<Action>,
}
impl ScriptResult {
    pub fn changed(&self) -> bool {
        self.state.is_some() || !self.effects.is_empty() || self.action.is_some()
    }
}
pub struct LuaRuntime {
    lua: Lua,
    hooks: BTreeMap<String, Table>,
    pub definitions: Arc<BTreeMap<String, Definition>>,
    fuel: Rc<Cell<usize>>,
    root: PathBuf,
}
fn to_lua(lua: &Lua, v: &impl serde::Serialize) -> mlua::Result<Value> {
    lua.to_value_with(
        v,
        mlua::serde::SerializeOptions::new()
            .serialize_none_to_null(false)
            .serialize_unit_to_null(false),
    )
}
impl LuaRuntime {
    pub fn load(root: &Path) -> Result<Self> {
        let root = root.canonicalize()?;
        let lua = Lua::new_with(
            StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8,
            LuaOptions::default(),
        )?;
        lua.set_memory_limit(32 * 1024 * 1024)?;
        let math: Table = lua.globals().get("math")?;
        math.set("random", Value::Nil)?;
        math.set("randomseed", Value::Nil)?;
        let fuel = Rc::new(Cell::new(100_000));
        let budget = fuel.clone();
        lua.set_hook(
            HookTriggers::new().every_nth_instruction(1000),
            move |_, _| {
                let n = budget.get();
                if n < 1000 {
                    return Err(mlua::Error::RuntimeError(
                        "Lua instruction budget exceeded".into(),
                    ));
                }
                budget.set(n - 1000);
                Ok(VmState::Continue)
            },
        )?;
        let mut runtime = Self {
            lua,
            hooks: BTreeMap::new(),
            definitions: Arc::new(BTreeMap::new()),
            fuel,
            root,
        };
        let manifest = runtime.eval("data/entity_templates/manifest.lua")?;
        let paths: Vec<String> = runtime.lua.from_value(manifest)?;
        ensure(
            !paths.is_empty() && paths.len() <= 1000,
            "manifest must contain 1..1000 definitions",
        )?;
        let mut definitions = BTreeMap::new();
        for path in paths {
            let table = match runtime.eval(&path)? {
                Value::Table(t) => t,
                _ => return Err(format!("{path}: expected a definition table").into()),
            };
            let hooks: Table = table.get("hooks")?;
            for pair in hooks.clone().pairs::<String, Value>() {
                let (name, value) = pair?;
                ensure(
                    matches!(
                        name.as_str(),
                        "on_spawn"
                            | "on_turn"
                            | "on_use"
                            | "on_damage"
                            | "on_destroy"
                            | "on_status"
                            | "on_ability"
                            | "on_interact"
                    ) && matches!(value, Value::Function(_)),
                    format!("{path}: invalid hook {name}"),
                )?;
            }
            table.set("hooks", Value::Nil)?;
            let definition: Definition = runtime
                .lua
                .from_value(Value::Table(table))
                .map_err(|e| format!("{path}: {e}"))?;
            definition.validate().map_err(|e| format!("{path}: {e}"))?;
            ensure(
                definition.interactions.is_empty() || hooks.contains_key("on_interact")?,
                format!("{path}: interactions require on_interact"),
            )?;
            ensure(
                !definitions.contains_key(&definition.id),
                format!("{path}: duplicate id {}", definition.id),
            )?;
            runtime.hooks.insert(definition.id.clone(), hooks);
            definitions.insert(definition.id.clone(), definition);
        }
        for d in definitions.values() {
            if let Some(m) = &d.movable {
                let mut occupied = [false; 2];
                let mut armor_slots = std::collections::BTreeSet::new();
                for entry in &m.loadout {
                    let _item = definitions
                        .get(&entry.definition_id)
                        .and_then(|d| d.item.as_ref())
                        .ok_or_else(|| {
                            format!(
                                "{}: loadout reference {} is missing/not an item",
                                d.id, entry.definition_id
                            )
                        })?;
                    if let Some(armor) =
                        crate::equipment::body_armor(&definitions[&entry.definition_id])
                    {
                        ensure(entry.grip.is_none(), "body armor cannot use hand grip")?;
                        if entry.equipped {
                            ensure(
                                armor_slots.insert(armor.slot.clone()),
                                "loadout armor slot conflict",
                            )?;
                        }
                    } else if entry.equipped || entry.grip.is_some() {
                        let hands =
                            crate::equipment::equipment_hands(&definitions[&entry.definition_id])
                                .ok_or("loadout item is not equippable")?;
                        let grip = entry.grip.unwrap_or(if hands != WeaponHands::One {
                            Grip::Both
                        } else if m.dominant_hand == Hand::Right {
                            Grip::Right
                        } else {
                            Grip::Left
                        });
                        ensure(
                            matches!(
                                (hands, grip),
                                (WeaponHands::Either, _)
                                    | (WeaponHands::Two, Grip::Both)
                                    | (WeaponHands::One, Grip::Right | Grip::Left)
                            ),
                            "invalid loadout grip",
                        )?;
                        for (index, used) in [grip != Grip::Left, grip != Grip::Right]
                            .into_iter()
                            .enumerate()
                        {
                            ensure(!used || !occupied[index], "loadout hand conflict")?;
                            occupied[index] |= used;
                        }
                    }
                    ensure(
                        !(entry.equipped || entry.grip.is_some()) || entry.quantity == 1,
                        "equipped loadout quantity must be one",
                    )?;
                }
            }
            if let Some(item) = &d.item {
                for entry in &item.contents {
                    let content = definitions
                        .get(&entry.definition_id)
                        .and_then(|d| d.item.as_ref())
                        .ok_or("container content must be an item")?;
                    ensure(
                        content.contents.is_empty(),
                        "nested containers are not supported",
                    )?;
                }
            }
            if let Some(w) = d.item.as_ref().and_then(|i| i.weapon.as_ref())
                && let Some(ammo) = &w.ammo_definition_id
            {
                let a = definitions
                    .get(ammo)
                    .ok_or_else(|| format!("{}: missing ammo {ammo}", d.id))?;
                ensure(
                    a.category == "item" && a.common.tags.iter().any(|t| t == "ammunition"),
                    "ammo reference is not ammunition",
                )?;
            }
        }
        runtime.definitions = Arc::new(definitions);
        Ok(runtime)
    }
    pub fn growth_rules(&self) -> Result<crate::progression::GrowthRules> {
        let value = self.eval("data/progression.lua")?;
        let rules: crate::progression::GrowthRules = self.lua.from_value(value)?;
        rules.validate()?;
        Ok(rules)
    }
    pub fn crafting_config(&self) -> Result<crate::crafting::CraftConfig> {
        let value = self.eval("data/production/recipes.lua")?;
        let config: crate::crafting::CraftConfig = self.lua.from_value(value)?;
        ensure(
            !config.recipes.is_empty() && config.recipes.len() <= 1000,
            "invalid recipe count",
        )?;
        config.rules.validate()?;
        Ok(config)
    }
    fn eval(&self, path: &str) -> Result<Value> {
        let path = self.root.join(path).canonicalize()?;
        ensure(
            path.starts_with(&self.root),
            "Lua path escaped project root",
        )?;
        let code = std::fs::read_to_string(&path)?;
        self.fuel.set(100_000);
        let env = self.lua.create_table()?;
        let meta = self.lua.create_table()?;
        meta.set("__index", self.lua.globals())?;
        env.set_metatable(Some(meta))?;
        Ok(self
            .lua
            .load(code)
            .set_name(path.to_string_lossy().as_ref())
            .set_environment(env)
            .eval()?)
    }
    pub fn scenario(&self) -> Result<Scenario> {
        self.scenario_file("data/scenario.lua")
    }
    pub(crate) fn scenario_file(&self, path: &str) -> Result<Scenario> {
        Ok(self.lua.from_value(self.eval(path)?)?)
    }
    pub fn has_hook(&self, definition: &str, hook: &str) -> bool {
        self.hooks
            .get(definition)
            .and_then(|t| t.get::<Option<Function>>(hook).ok())
            .flatten()
            .is_some()
    }
    pub fn call(
        &self,
        world: &Game,
        id: Id,
        hook: &str,
        event: &serde_json::Value,
    ) -> Result<ScriptResult> {
        let definition = &world
            .entities
            .get(&id)
            .ok_or("hook target missing")?
            .script()
            .definition_id;
        let Some(function) = self.hooks[definition].get::<Option<Function>>(hook)? else {
            return Ok(ScriptResult::default());
        };
        let snapshot = world.entity_snapshot(id).ok_or("missing snapshot")?;
        let w = Rc::new(world.read_snapshot());
        let ctx = self.lua.create_table()?;
        let view = w.clone();
        ctx.set(
            "entity",
            self.lua.create_function(move |lua, (_, id): (Value, Id)| {
                to_lua(lua, &view.view().entity_snapshot(id))
            })?,
        )?;
        let view = w.clone();
        ctx.set(
            "entities_at",
            self.lua
                .create_function(move |lua, (_, p): (Value, Value)| {
                    let p: Pos = lua.from_value(p)?;
                    to_lua(
                        lua,
                        &view
                            .view()
                            .at(p)
                            .iter()
                            .filter_map(|id| view.view().entity_snapshot(*id))
                            .collect::<Vec<_>>(),
                    )
                })?,
        )?;
        let view = w.clone();
        ctx.set(
            "nearest_hostile",
            self.lua
                .create_function(move |lua, (_, id, range): (Value, Id, u32)| {
                    let range = range.min(100);
                    to_lua(
                        lua,
                        &view
                            .view()
                            .nearest_hostile(id, range)
                            .and_then(|id| view.view().entity_snapshot(id)),
                    )
                })?,
        )?;
        let view = w.clone();
        ctx.set(
            "adjacent",
            self.lua
                .create_function(move |_, (_, a, b): (Value, Id, Id)| {
                    Ok(view
                        .view()
                        .position(a)
                        .zip(view.view().position(b))
                        .is_some_and(|(a, b)| a.distance(b) == 1))
                })?,
        )?;
        let view = w.clone();
        ctx.set(
            "can_reload",
            self.lua
                .create_function(move |_, (_, actor): (Value, Id)| {
                    Ok(view.view().reload_plan(actor).is_ok())
                })?,
        )?;
        let view = w.clone();
        ctx.set(
            "can_attack",
            self.lua.create_function(
                move |_, (_, a, b, weapon): (Value, Id, Id, Option<Id>)| {
                    Ok(view.view().can_attack(a, b, weapon))
                },
            )?,
        )?;
        let view = w.clone();
        ctx.set(
            "movement_cost",
            self.lua
                .create_function(move |lua, (_, position): (Value, Value)| {
                    let position: Pos = lua.from_value(position)?;
                    Ok(view.view().movement_cost(position))
                })?,
        )?;
        let view = w;
        ctx.set(
            "step_towards",
            self.lua
                .create_function(move |lua, (_, a, b): (Value, Id, Id)| {
                    to_lua(lua, &view.view().step_towards(a, b))
                })?,
        )?;
        self.fuel.set(100_000);
        let result: Value = function
            .call((
                to_lua(&self.lua, &snapshot)?,
                ctx,
                to_lua(&self.lua, event)?,
            ))
            .map_err(|e| format!("{definition} #{id} {hook}: {e}"))?;
        if matches!(result, Value::Nil) {
            return Ok(ScriptResult::default());
        }
        let result: ScriptResult = self.lua.from_value(result)?;
        ensure(result.effects.len() <= 100, "too many effects in one hook")?;
        if let Some(state) = &result.state {
            ensure(
                state.is_object() || state.is_array(),
                "state must be a table",
            )?;
            ensure(
                serde_json::to_vec(state)?.len() <= 64 * 1024,
                "state exceeds 64 KiB",
            )?;
        }
        Ok(result)
    }
}
