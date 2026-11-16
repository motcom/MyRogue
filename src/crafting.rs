//! 材料・道具・収納を参照する複数ターン制作。完了時に成否と出来を確定する。
use crate::{config::*, entity::*, game::Game, inspect::wrap_description, japanese::JapanesePrint};
use bracket_lib::prelude as bl;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemAmount {
    pub kind: String,
    pub quantity: u32,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillRequirement {
    pub skill: String,
    pub recommended_level: u32,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolRequirement {
    #[serde(rename = "function_name")]
    pub function: ItemProp,
    pub level: u32,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub ingredients: Vec<ItemAmount>,
    pub required_tools: Vec<ToolRequirement>,
    pub required_facilities: Vec<FacilityKind>,
    pub required_skills: Vec<SkillRequirement>,
    pub resource_costs: Vec<ResourceAmount>,
    pub outputs: Vec<ItemAmount>,
    pub work_turns: u32,
    pub quality_enabled: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Supply {
    Item(Id),
    Stored { container: Id, index: usize },
}
#[derive(Clone, Debug)]
pub struct ItemSelection {
    pub item: Supply,
    pub kind: String,
    pub quantity: u32,
}
#[derive(Clone, Debug)]
pub struct ToolSelection {
    pub item: Supply,
    pub kind: String,
}
#[derive(Clone, Debug)]
pub struct ResourceSelection {
    pub source: Id,
    pub resource: ResourceAmount,
}
#[derive(Clone, Debug)]
pub struct CraftJob {
    pub recipe: String,
    pub completed_turns: u32,
    pub selected_ingredients: Vec<ItemSelection>,
    pub selected_tools: Vec<ToolSelection>,
    pub selected_facilities: Vec<Id>,
    pub selected_resources: Vec<ResourceSelection>,
    pub success_rate: u32,
    pub quality_modifier: i32,
}
pub struct CraftingSystem;
impl CraftingSystem {
    pub fn cancel(actor: &mut MoveEntity) {
        actor.crafting = None;
    }
}

pub fn tool_name(function: &ItemProp) -> &'static str {
    match function {
        ItemProp::Cutting => "切断",
        ItemProp::WoodCutting => "伐採・製材",
        ItemProp::Hammering => "打撃",
        ItemProp::Drilling => "穴あけ",
        ItemProp::Fastening => "締結",
        ItemProp::Prying => "こじ開け",
        ItemProp::Digging => "掘削",
        ItemProp::Sewing => "裁縫",
        ItemProp::Cooking => "調理",
        ItemProp::Butchering => "解体",
        ItemProp::MetalCutting => "金属切断",
        ItemProp::Grinding => "研削",
        ItemProp::Welding => "溶接",
    }
}
pub fn tool_level(item: &ItemDefinition, function: &ItemProp) -> u32 {
    if item.tool_functions.contains(function) {
        item.tool_levels.get(function).copied().unwrap_or(1)
    } else {
        0
    }
}
pub fn quality_title(quality: i32) -> &'static str {
    match quality {
        -2 => "粗悪な",
        -1 => "不出来な",
        1 => "良質な",
        2 => "見事な",
        3 => "傑作の",
        _ => "",
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CraftRules {
    pub success_base: i32,
    pub skill_step: i32,
    pub success_min: u32,
    pub success_max: u32,
    pub tool_step: u32,
    pub tool_cap: i32,
    pub material_step: i32,
    pub material_cap: i32,
    pub quality_thresholds: [i32; 5],
    pub failure_material_percent: u32,
}
impl CraftRules {
    pub fn quality(&self, score: i32) -> i32 {
        self.quality_thresholds
            .iter()
            .position(|limit| score <= *limit)
            .unwrap_or(5) as i32
            - 2
    }
    pub fn validate(&self) -> Result<()> {
        ensure(
            (0..=100).contains(&self.success_base)
                && (0..=100).contains(&self.skill_step)
                && (1..=100).contains(&self.success_min)
                && (self.success_min..=100).contains(&self.success_max)
                && self.tool_step <= 100
                && (0..=100).contains(&self.tool_cap)
                && (0..=100).contains(&self.material_step)
                && (0..=100).contains(&self.material_cap)
                && self.failure_material_percent <= 100
                && self
                    .quality_thresholds
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
                && self
                    .quality_thresholds
                    .iter()
                    .all(|value| (-1000..=1000).contains(value)),
            "制作の確率・出来設定が不正です",
        )
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CraftConfig {
    pub rules: CraftRules,
    pub recipes: Vec<Recipe>,
}

impl Game {
    pub fn display_name(&self, id: Id) -> String {
        let name = &self.definition(id).common.name;
        if let Some(Entity::Item(i)) = self.entities.get(&id)
            && i.quality != 0
        {
            format!("{}{name} {:+}", quality_title(i.quality), i.quality)
        } else {
            name.clone()
        }
    }
    fn craft_reachable(&self, actor: Id, id: Id) -> bool {
        self.entities.get(&id).is_some_and(|e| {
            e.alive()
                && (e.carrier() == Some(actor)
                    || e.map_position().is_some_and(|p| {
                        self.position(actor).is_some_and(|q| {
                            q.distance(p) <= 1
                                && self.sight(q, p)
                                && !self.at(p).iter().any(|id| self.entities[id].blocks_sight())
                        })
                    }))
        })
    }
    pub fn craft_supplies(&self, actor: Id) -> Vec<Supply> {
        let mut supplies = Vec::new();
        let Some(a) = self.actor(actor) else {
            return supplies;
        };
        let mut ids = a.inventory.items.clone();
        ids.extend(self.entities.iter().filter_map(|(id, e)| {
            (matches!(e, Entity::Item(_))
                && e.carrier() != Some(actor)
                && self.craft_reachable(actor, *id))
            .then_some(*id)
        }));
        for id in ids {
            let Some(Entity::Item(i)) = self.entities.get(&id) else {
                continue;
            };
            if !self.craft_reachable(actor, id) {
                continue;
            }
            supplies.push(Supply::Item(id));
            if self
                .definition(id)
                .common
                .tags
                .iter()
                .any(|t| t == "container")
            {
                supplies.extend(
                    i.contents
                        .iter()
                        .enumerate()
                        .filter(|(_, e)| e.quantity > 0)
                        .map(|(index, _)| Supply::Stored {
                            container: id,
                            index,
                        }),
                );
            }
        }
        supplies
    }
    fn supply_info(&self, actor: Id, supply: &Supply) -> Option<(String, u32, i32)> {
        match supply {
            Supply::Item(id) => {
                if !self.craft_reachable(actor, *id) {
                    return None;
                }
                let Entity::Item(i) = self.entities.get(id)? else {
                    return None;
                };
                (i.quantity > 0 && (i.max_taikyudo == 0 || i.taikyudo > 0))
                    .then(|| (i.kind.clone(), i.quantity, i.quality))
            }
            Supply::Stored { container, index } => {
                if !self.craft_reachable(actor, *container)
                    || !self
                        .definition(*container)
                        .common
                        .tags
                        .iter()
                        .any(|t| t == "container")
                {
                    return None;
                }
                let Entity::Item(i) = self.entities.get(container)? else {
                    return None;
                };
                let entry = i.contents.get(*index)?;
                (entry.quantity > 0).then(|| (entry.definition_id.clone(), entry.quantity, 0))
            }
        }
    }
    fn supply_tool_level(&self, actor: Id, supply: &Supply, function: &ItemProp) -> u32 {
        if let Supply::Item(id) = supply
            && !self.view().item_hand_usable(actor, *id)
        {
            return 0;
        }
        self.supply_info(actor, supply)
            .and_then(|(kind, _, _)| self.entity_definitions.get(&kind)?.item.as_ref())
            .map_or(0, |i| tool_level(i, function))
    }
    pub(crate) fn validate_recipe(&self, r: &Recipe) -> Result<()> {
        ensure(
            !r.ingredients.is_empty()
                && !r.required_tools.is_empty()
                && !r.outputs.is_empty()
                && (1..=1000).contains(&r.work_turns)
                && r.ingredients.len() <= 32
                && r.required_tools.len() <= 13
                && r.outputs.len() <= 32
                && !r.id.is_empty()
                && !r.name.is_empty()
                && r.name.chars().count() <= 64
                && !r.name.chars().any(char::is_control),
            "材料・道具・完成品・作業時間が必要です",
        )?;
        ensure(
            r.resource_costs.is_empty(),
            "消費資源は材料として指定してください",
        )?;
        for amount in r.ingredients.iter().chain(&r.outputs) {
            ensure(
                (1..=1000).contains(&amount.quantity)
                    && self
                        .entity_definitions
                        .get(&amount.kind)
                        .is_some_and(|d| d.item.is_some()),
                "制作の材料・完成品定義が不正です",
            )?;
        }
        let skills = serde_json::to_value(Skill::default())?;
        ensure(
            r.required_skills
                .iter()
                .all(|s| skills.get(&s.skill).is_some() && s.recommended_level <= 100),
            "unknown crafting skill or recommended level",
        )?;
        ensure(
            r.required_tools.iter().all(|t| (1..=20).contains(&t.level)),
            "道具の要求レベルが不正です",
        )
    }
    pub fn craft_plan(&self, actor: Id, recipe: &Recipe) -> Result<CraftJob> {
        self.validate_recipe(recipe)?;
        let a = self.actor(actor).ok_or("actor missing")?;
        ensure(self.entities[&actor].alive(), "dead crafter")?;
        ensure(
            a.hand_usable(Hand::Left) || a.hand_usable(Hand::Right),
            "使用できる手がありません",
        )?;
        let supplies = self.craft_supplies(actor);
        let mut job = CraftJob {
            recipe: recipe.id.clone(),
            completed_turns: 0,
            selected_ingredients: vec![],
            selected_tools: vec![],
            selected_facilities: vec![],
            selected_resources: vec![],
            success_rate: 0,
            quality_modifier: 0,
        };
        let mut reserved = BTreeMap::<Supply, u32>::new();
        // 工具を先に選び、材料と同じ品を消費しない。
        let mut tool_bonus = self.crafting_rules.tool_cap;
        for req in &recipe.required_tools {
            let supply = supplies
                .iter()
                .filter(|s| self.supply_tool_level(actor, s, &req.function) >= req.level)
                .max_by_key(|s| self.supply_tool_level(actor, s, &req.function))
                .ok_or_else(|| {
                    format!(
                        "{}Lv{}の道具が足りません",
                        tool_name(&req.function),
                        req.level
                    )
                })?;
            let (kind, _, _) = self.supply_info(actor, supply).unwrap();
            let extra = self.supply_tool_level(actor, supply, &req.function) - req.level;
            tool_bonus = tool_bonus.min(
                (extra * self.crafting_rules.tool_step).min(self.crafting_rules.tool_cap as u32)
                    as i32,
            );
            job.selected_tools.push(ToolSelection {
                item: supply.clone(),
                kind,
            });
        }
        let mut material_bonus = self.crafting_rules.material_cap;
        for ingredient in &recipe.ingredients {
            let mut remaining = ingredient.quantity;
            for supply in &supplies {
                if job.selected_tools.iter().any(|t| &t.item == supply) {
                    continue;
                }
                let Some((kind, count, quality)) = self.supply_info(actor, supply) else {
                    continue;
                };
                if kind != ingredient.kind {
                    continue;
                }
                // 収納品の入った箱そのものは材料にしない。
                if let Supply::Item(id) = supply
                    && matches!(&self.entities[id],Entity::Item(i) if i.contents.iter().any(|e|e.quantity>0))
                {
                    continue;
                }
                let quantity =
                    remaining.min(count.saturating_sub(*reserved.get(supply).unwrap_or(&0)));
                if quantity > 0 {
                    job.selected_ingredients.push(ItemSelection {
                        item: supply.clone(),
                        kind,
                        quantity,
                    });
                    *reserved.entry(supply.clone()).or_default() += quantity;
                    material_bonus =
                        material_bonus.min((quality * self.crafting_rules.material_step).clamp(
                            -self.crafting_rules.material_cap,
                            self.crafting_rules.material_cap,
                        ));
                    remaining -= quantity;
                }
                if remaining == 0 {
                    break;
                }
            }
            ensure(
                remaining == 0,
                format!(
                    "{}が{}個足りません",
                    self.entity_definitions[&ingredient.kind].common.name, remaining
                ),
            )?;
        }
        for facility in &recipe.required_facilities {
            let id = self
                .entities
                .iter()
                .find(|(id, e)| {
                    self.craft_reachable(actor, **id)
                        && e.map_position().is_some()
                        && self
                            .definition(**id)
                            .r#static
                            .as_ref()
                            .is_some_and(|s| s.facilities.contains(facility))
                })
                .map(|(id, _)| *id)
                .ok_or("近くに製作設備がありません")?;
            job.selected_facilities.push(id);
        }
        let skills = serde_json::to_value(&a.skill)?;
        let mut delta = None;
        for req in &recipe.required_skills {
            let value = skills
                .get(&req.skill)
                .and_then(|v| v.as_u64())
                .ok_or("制作スキルの指定が不正です")?;
            let difference =
                (value as i64 - i64::from(req.recommended_level)).clamp(-100, 100) as i32;
            delta = Some(delta.map_or(difference, |old: i32| old.min(difference)));
        }
        job.quality_modifier =
            self.crafting_rules.skill_step * delta.unwrap_or(0) + tool_bonus + material_bonus;
        job.success_rate = (self.crafting_rules.success_base + job.quality_modifier).clamp(
            self.crafting_rules.success_min as i32,
            self.crafting_rules.success_max as i32,
        ) as u32;
        Ok(job)
    }
    fn validate_job(&self, actor: Id, recipe: &Recipe, job: &CraftJob) -> Result<()> {
        self.validate_recipe(recipe)?;
        let a = self.actor(actor).ok_or("actor missing")?;
        ensure(
            self.entities[&actor].alive()
                && (a.hand_usable(Hand::Left) || a.hand_usable(Hand::Right)),
            "制作できる状態ではありません",
        )?;
        let mut counts = BTreeMap::<Supply, u32>::new();
        for selection in &job.selected_ingredients {
            *counts.entry(selection.item.clone()).or_default() += selection.quantity;
            ensure(
                self.supply_info(actor, &selection.item)
                    .is_some_and(|(kind, quantity, _)| {
                        kind == selection.kind && quantity >= counts[&selection.item]
                    }),
                "選んだ材料が不足したか手が届かなくなりました",
            )?;
        }
        ensure(
            job.selected_tools.len() == recipe.required_tools.len(),
            "制作道具の指定が変わりました",
        )?;
        for (selection, req) in job.selected_tools.iter().zip(&recipe.required_tools) {
            ensure(
                self.supply_info(actor, &selection.item)
                    .is_some_and(|(kind, _, _)| kind == selection.kind)
                    && self.supply_tool_level(actor, &selection.item, &req.function) >= req.level,
                "選んだ道具が使えないか手が届かなくなりました",
            )?;
        }
        for (id, facility) in job
            .selected_facilities
            .iter()
            .zip(&recipe.required_facilities)
        {
            ensure(
                self.craft_reachable(actor, *id)
                    && self
                        .definition(*id)
                        .r#static
                        .as_ref()
                        .is_some_and(|s| s.facilities.contains(facility)),
                "選んだ設備に手が届きません",
            )?;
        }
        Ok(())
    }
    pub fn start_craft(&mut self, actor: Id, recipe: &str) -> Result<()> {
        ensure(
            self.actor(actor).ok_or("actor missing")?.crafting.is_none(),
            "すでに製作中です",
        )?;
        let recipe = self
            .recipes
            .iter()
            .find(|r| r.id == recipe)
            .ok_or("unknown recipe")?;
        let job = self.craft_plan(actor, recipe)?;
        self.actor_mut(actor).unwrap().crafting = Some(job);
        Ok(())
    }
    fn consume_supply(&mut self, supply: &Supply, kind: &str, quantity: u32) -> Result<()> {
        match supply {
            Supply::Item(id) => self.consume(*id, quantity),
            Supply::Stored { container, index } => {
                let Some(Entity::Item(i)) = self.entities.get_mut(container) else {
                    return Err("収納がありません".into());
                };
                let entry = i
                    .contents
                    .get_mut(*index)
                    .ok_or("収納内容が変更されました")?;
                ensure(
                    entry.definition_id == kind && entry.quantity >= quantity,
                    "収納内の材料が不足しています",
                )?;
                entry.quantity -= quantity;
                Ok(())
            }
        }
    }
    pub fn continue_craft(&mut self, actor: Id) -> Result<bool> {
        let mut job = self
            .actor(actor)
            .and_then(|a| a.crafting.clone())
            .ok_or("製作していません")?;
        let recipe = self
            .recipes
            .iter()
            .find(|r| r.id == job.recipe)
            .ok_or("unknown recipe")?
            .clone();
        self.validate_job(actor, &recipe, &job)?;
        job.completed_turns += 1;
        if job.completed_turns < recipe.work_turns {
            self.actor_mut(actor).unwrap().crafting = Some(job);
            self.practice_recipe_skills(actor, &recipe);
            return Ok(false);
        }
        let before = self.clone();
        let result = (|| -> Result<bool> {
            let success = self.rng.roll(100) <= job.success_rate;
            let quality = if success && recipe.quality_enabled {
                self.crafting_rules
                    .quality(self.rng.roll(100) as i32 + job.quality_modifier)
            } else {
                0
            };
            // 失敗時も材料枠単位の半量を失う。別スタックごとの切上げで損失を増やさない。
            let mut loss = BTreeMap::<String, u32>::new();
            for ingredient in &recipe.ingredients {
                *loss.entry(ingredient.kind.clone()).or_default() += if success {
                    ingredient.quantity
                } else {
                    (ingredient.quantity * self.crafting_rules.failure_material_percent)
                        .div_ceil(100)
                };
            }
            for selection in &job.selected_ingredients {
                let remaining = loss.get_mut(&selection.kind).unwrap();
                let quantity = selection.quantity.min(*remaining);
                if quantity > 0 {
                    self.consume_supply(&selection.item, &selection.kind, quantity)?;
                    *remaining -= quantity;
                }
            }
            if success {
                let position = self.position(actor).ok_or("actor missing")?;
                for output in &recipe.outputs {
                    let definition = &self.entity_definitions[&output.kind];
                    let limit = definition.item.as_ref().unwrap().stack_limit;
                    let container = definition.common.tags.iter().any(|t| t == "container");
                    let mut remaining = output.quantity;
                    while remaining > 0 {
                        let mut quantity = remaining.min(limit);
                        let id = self.spawn(&output.kind, Some(position), None, quantity)?;
                        if let Entity::Item(i) = self.entities.get_mut(&id).unwrap() {
                            i.quality = quality;
                            if container {
                                i.contents.clear();
                            } // 新造の箱に初期配置用の中身を複製しない。
                            if i.max_taikyudo > 0 {
                                i.max_taikyudo = ((u64::from(i.max_taikyudo)
                                    * (100 + quality * 10) as u64)
                                    / 100)
                                    .max(1) as u32;
                                i.taikyudo = i.max_taikyudo;
                            }
                        }
                        let fit = self.pickup_capacity(actor, id)?;
                        let destination = if fit > 0 {
                            quantity = fit;
                            if let Entity::Item(item) = self.entities.get_mut(&id).unwrap() {
                                item.quantity = quantity;
                            }
                            self.pickup(actor, id)?;
                            "所持品に入れました"
                        } else {
                            "足元に置きました"
                        };
                        self.log(format!(
                            "{} x{}を制作して{}",
                            self.display_name(id),
                            quantity,
                            destination
                        ));
                        remaining -= quantity;
                    }
                }
            } else {
                self.log(format!("制作に失敗しました: {}", recipe.name));
            }
            self.actor_mut(actor).unwrap().crafting = None;
            self.practice_recipe_skills(actor, &recipe);
            Ok(true)
        })();
        if result.is_err() {
            *self = before;
        }
        result
    }
    fn practice_recipe_skills(&mut self, actor: Id, recipe: &Recipe) {
        let skills: std::collections::BTreeSet<crate::progression::SkillKind> = recipe
            .required_skills
            .iter()
            .filter_map(|s| serde_json::from_value(serde_json::json!(s.skill)).ok())
            .collect();
        for skill in skills {
            self.practice_skill(actor, skill);
        }
    }
    fn supply_label(&self, supply: &Supply) -> String {
        match supply {
            Supply::Item(id) => format!(
                "{}（{}）",
                self.display_name(*id),
                if self.entities[id].carrier().is_some() {
                    "所持品"
                } else {
                    "近く"
                }
            ),
            Supply::Stored { container, index } => {
                let Entity::Item(i) = &self.entities[container] else {
                    return String::new();
                };
                format!(
                    "{}（{}の中）",
                    self.entity_definitions[&i.contents[*index].definition_id]
                        .common
                        .name,
                    self.display_name(*container)
                )
            }
        }
    }
    fn player_craft_plan(&self, recipe: &Recipe) -> Result<CraftJob> {
        if let Some(job) = self
            .actor(self.player_id)
            .and_then(|a| a.crafting.as_ref())
            .filter(|j| j.recipe == recipe.id)
        {
            self.validate_job(self.player_id, recipe, job)
                .map(|_| job.clone())
        } else {
            self.craft_plan(self.player_id, recipe)
        }
    }

    pub(crate) fn render_crafting(&self, ctx: &mut bl::BTerm) {
        crate::theme::draw_box(
            ctx,
            2,
            4,
            75,
            39,
            crate::theme::TEXT,
            crate::theme::BACKGROUND,
        );
        ctx.print_ja(4, 5, "制作: j/k:選択  Enter:作業  c:中止  Esc:閉じる");
        ctx.print_ja(4, 6, "所持品・足元/隣接8マス・宝箱の中の材料と道具を使用");
        let start = self.selection.saturating_sub(13);
        for (index, r) in self.recipes.iter().enumerate().skip(start).take(14) {
            let y = 8 + (index - start) as i32;
            let selected = index == self.selection;
            let color = if self.player_craft_plan(r).is_err() {
                bl::RGB::named(crate::theme::DISABLED)
            } else if selected {
                bl::RGB::named(crate::theme::ACCENT)
            } else {
                bl::RGB::named(crate::theme::TEXT)
            };
            ctx.print_ja_color(
                4,
                y,
                bl::RGB::named(crate::theme::ACCENT),
                if selected { ">" } else { " " },
            );
            ctx.print_ja_color(6, y, color, format!("{} / {}ターン", r.name, r.work_turns));
        }
        let Some(recipe) = self.recipes.get(self.selection) else {
            return;
        };
        let mut lines = vec![];
        for ingredient in &recipe.ingredients {
            lines.push(format!(
                "材料: {} x{}",
                self.entity_definitions[&ingredient.kind].common.name, ingredient.quantity
            ));
        }
        lines.push(format!(
            "道具: {}",
            recipe
                .required_tools
                .iter()
                .map(|t| format!("{}Lv{}", tool_name(&t.function), t.level))
                .collect::<Vec<_>>()
                .join(" / ")
        ));
        if !recipe.required_facilities.is_empty() {
            lines.push("設備: 近くの作業面が必要".into());
        }
        lines.push(format!(
            "推奨技能: {}",
            recipe
                .required_skills
                .iter()
                .map(|s| format!(
                    "{} {}",
                    match s.skill.as_str() {
                        "wood_craft" => "木工",
                        "metal_craft" => "金属加工",
                        "saihou" => "裁縫",
                        "cooking" => "調理",
                        _ => "電子工作",
                    },
                    s.recommended_level
                ))
                .collect::<Vec<_>>()
                .join(" / ")
        ));
        let active = self.actor(self.player_id).and_then(|a| a.crafting.as_ref());
        match self.player_craft_plan(recipe) {
            Ok(plan) => {
                lines.push(format!(
                    "成功率{}% / 作業 {}/{}",
                    plan.success_rate, plan.completed_turns, recipe.work_turns
                ));
                let mut odds = [0; 6];
                for roll in 1..=100 {
                    odds[(self.crafting_rules.quality(roll + plan.quality_modifier) + 2)
                        as usize] += 1;
                }
                lines.push(if recipe.quality_enabled {
                    format!(
                        "完成時: -2 {}% / -1 {}% / 標準 {}% / +1 {}% / +2 {}% / +3 {}%",
                        odds[0], odds[1], odds[2], odds[3], odds[4], odds[5]
                    )
                } else {
                    "完成時の出来: 標準（消耗品の効果補正は今後調整）".into()
                });
                lines.push(format!(
                    "使用元: {}",
                    plan.selected_ingredients
                        .iter()
                        .map(|s| self.supply_label(&s.item))
                        .chain(
                            plan.selected_tools
                                .iter()
                                .map(|s| self.supply_label(&s.item))
                        )
                        .collect::<Vec<_>>()
                        .join(" / ")
                ));
            }
            Err(error) => lines.push(format!("制作不可/一時停止: {error}")),
        }
        if let Some(job) = active {
            let name = self
                .recipes
                .iter()
                .find(|r| r.id == job.recipe)
                .map_or("制作中", |r| r.name.as_str());
            lines.push(format!(
                "進行中: {name} {}ターン作業済み",
                job.completed_turns
            ));
        }
        lines.push("失敗時は材料の半分（切上げ）を失う。道具は消費しない。".into());
        for (row, line) in lines
            .iter()
            .flat_map(|line| wrap_description(line, 71))
            .take(19)
            .enumerate()
        {
            ctx.print_ja(4, 23 + row as i32, line);
        }
    }
}
