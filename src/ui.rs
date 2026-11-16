//! Game の input/update/render。input は pending_action の生成だけを行う。
use crate::{
    config::*,
    entity::*,
    game::{Game, Panel},
    japanese::{JapanesePrint, message},
    scripting::Action,
};
use bracket_lib::prelude as bl;
impl Game {
    pub fn input(&mut self, ctx: &bl::BTerm) {
        self.pending_action = None;
        use bl::VirtualKeyCode as K;
        // 文字イベントを使い、JISの@キーとUSのShift+2の両方に対応する。
        let mut properties = false;
        let mut close = false;
        let mut input_events = Vec::new();
        bl::INPUT.lock().for_each_message(|event| {
            input_events.push(event.clone());
            if matches!(event, bl::BEvent::Character { c: '@' }) {
                properties = true;
            }
            if matches!(event, bl::BEvent::CloseRequested) {
                close = true;
            }
        });
        let key = self.direction_input.key(
            &input_events,
            ctx.key,
            ctx.frame_time_ms,
            matches!(self.panel, Panel::World)
                && self.projectiles.is_empty()
                && !ctx.control
                && !ctx.alt
                && !ctx.shift,
        );
        if close {
            self.pending_action = Some(Action::Quit);
            return;
        }
        if ctx.key == Some(K::Q) && ctx.control {
            self.pending_action = Some(Action::Quit);
            return;
        }
        if !ctx.control && !ctx.alt {
            let delta = crate::camera::zoom_delta(&input_events, ctx.key, ctx.shift);
            if let Some(delta) = delta {
                self.pending_action = Some(Action::ZoomMap { delta });
                return;
            }
        }
        // 演出中もCtrl+Qで終了できる。
        if !self.projectiles.is_empty() {
            return;
        }
        if properties || ctx.key == Some(K::At) {
            self.pending_action = Some(if matches!(self.panel, Panel::Properties) {
                Action::ClosePanel
            } else {
                self.key_action('@')
            });
            return;
        }
        let Some(key) = key else { return };
        let action = if key == K::Escape {
            Action::ClosePanel
        } else if matches!(self.panel, Panel::Targeting { .. }) {
            let Some(action) = self.targeting_key_action(key, ctx.shift) else {
                return;
            };
            action
        } else if matches!(self.panel, Panel::ItemTarget { .. }) {
            if matches!(key, K::Return | K::F) {
                Action::ConfirmItemTarget
            } else {
                match self.inspect_key_action(key, ctx.shift) {
                    Some(Action::MoveCursor { dx, dy }) => Action::MoveItemTarget { dx, dy },
                    _ => return,
                }
            }
        } else if matches!(self.panel, Panel::Inspect { .. }) {
            let Some(action) = self.inspect_key_action(key, ctx.shift) else {
                return;
            };
            action
        } else if matches!(
            self.panel,
            Panel::InteractionDirection
                | Panel::InteractionMenu { .. }
                | Panel::OpenDirection
                | Panel::OpenMenu { .. }
                | Panel::ItemDetails { .. }
        ) {
            let Some(action) = self.interaction_key_action(key) else {
                return;
            };
            action
        } else if matches!(self.panel, Panel::Container { .. }) {
            match key {
                K::J | K::Down => Action::SelectContainer { delta: 1 },
                K::K | K::Up => Action::SelectContainer { delta: -1 },
                K::Space => Action::ToggleContainerSelection,
                K::Return => Action::TakeContainerSelection,
                _ => return,
            }
        } else if matches!(self.panel, Panel::Properties) {
            match key {
                K::J | K::Down => Action::ScrollProperties { delta: 1 },
                K::K | K::Up => Action::ScrollProperties { delta: -1 },
                K::PageDown => Action::ScrollProperties { delta: 20 },
                K::PageUp => Action::ScrollProperties { delta: -20 },
                _ => return,
            }
        } else if let Panel::Bandage { item } = self.panel {
            match key {
                K::J | K::Down => Action::SelectBandagePart { delta: 1 },
                K::K | K::Up => Action::SelectBandagePart { delta: -1 },
                K::Return => {
                    let choices = self.bandage_choices(self.player_id);
                    let Some(body_part) = choices.get(self.selection) else {
                        return;
                    };
                    Action::BandagePart {
                        item,
                        body_part: body_part.clone(),
                    }
                }
                _ => return,
            }
        } else if let Panel::Equipment { item } = self.panel {
            if !ctx.shift
                && !ctx.control
                && !ctx.alt
                && let Some(slot) = crate::quick_slots::number_slot(key)
            {
                Action::BindQuickSlot {
                    item,
                    slot,
                    grip: self
                        .equipment_choices(self.player_id, item)
                        .get(self.selection)
                        .copied()
                        .flatten(),
                }
            } else {
                match key {
                    K::J | K::Down => Action::SelectEquipment { delta: 1 },
                    K::K | K::Up => Action::SelectEquipment { delta: -1 },
                    K::Return => {
                        let choices = self.equipment_choices(self.player_id, item);
                        match choices.get(self.selection) {
                            Some(Some(grip)) => Action::EquipInHand { item, grip: *grip },
                            Some(None) => Action::Unequip { item },
                            None => return,
                        }
                    }
                    _ => return,
                }
            }
        } else if matches!(self.panel, Panel::Inventory) {
            let inventory = self.actor(self.player_id).map(|a| &a.inventory.items);
            let selected = inventory.and_then(|i| i.get(self.selection)).copied();
            match key {
                K::I => Action::ClosePanel,
                K::J | K::Down => Action::SelectInventory { delta: 1 },
                K::K | K::Up => Action::SelectInventory { delta: -1 },
                K::Return => selected
                    .map(|item| Action::UseItem { item })
                    .unwrap_or(Action::ClosePanel),
                K::W => {
                    let Some(item) = selected else {
                        return;
                    };
                    if crate::equipment::body_armor(self.definition(item)).is_some() {
                        Action::ToggleArmor { item }
                    } else if self.quick_bindable(item) {
                        Action::OpenEquipment { item }
                    } else {
                        return;
                    }
                }
                K::E => selected
                    .map(|item| Action::ItemDetails { item })
                    .unwrap_or(Action::ClosePanel),
                K::D => selected
                    .map(|item| Action::Drop { item })
                    .unwrap_or(Action::ClosePanel),
                _ => return,
            }
        } else if matches!(self.panel, Panel::Crafting) {
            match key {
                K::J | K::Down => Action::SelectRecipe { delta: 1 },
                K::K | K::Up => Action::SelectRecipe { delta: -1 },
                K::C => Action::CancelCraft,
                K::Return => {
                    if self
                        .actor(self.player_id)
                        .is_some_and(|a| a.crafting.is_some())
                    {
                        Action::ContinueCraft
                    } else if let Some(r) = self.recipes.get(self.selection) {
                        Action::StartCraft {
                            recipe: r.id.clone(),
                        }
                    } else {
                        return;
                    }
                }
                _ => return,
            }
        } else if matches!(self.panel, Panel::Map) {
            Action::ClosePanel
        } else if matches!(self.panel, Panel::World)
            && key == K::V
            && !ctx.control
            && !ctx.alt
            && !ctx.shift
        {
            Action::ToggleRunning
        } else if matches!(self.panel, Panel::World) && matches!(key, K::PageUp | K::PageDown) {
            Action::ScrollFooting {
                delta: self.footing_page_size() as i32 * if key == K::PageUp { -1 } else { 1 },
            }
        } else if !ctx.shift
            && !ctx.control
            && !ctx.alt
            && let Some(slot) = crate::quick_slots::number_slot(key)
        {
            Action::ActivateQuickSlot { slot }
        } else {
            let key = match key {
                K::H | K::Left => 'h',
                K::J | K::Down => 'j',
                K::K | K::Up => 'k',
                K::L | K::Right => 'l',
                K::Y => 'y',
                K::U => 'u',
                K::B => 'b',
                K::N => 'n',
                K::P => 'p',
                K::I => 'i',
                K::E => 'e',
                K::O => 'o',
                K::M => 'm',
                K::G => 'g',
                K::F => 'f',
                K::R => 'r',
                K::Semicolon => ';',
                K::T => 't',
                K::Period | K::Space => '.',
                _ => return,
            };
            self.key_action(key)
        };
        self.pending_action = Some(action);
    }
    pub(crate) fn key_action(&self, key: char) -> Action {
        if key == '@' {
            return Action::OpenProperties;
        }
        let Some(player) = self.actor(self.player_id) else {
            return Action::Quit;
        };
        let definition = self.definition(self.player_id).movable.as_ref().unwrap();
        if let Some(binding) = definition.controls.get(&key.to_string()) {
            return match binding.r#type.as_str() {
                "move" => Action::MoveTo {
                    position: player.position.offset(binding.dx, binding.dy),
                },
                "open_inventory" => Action::OpenInventory,
                "interact" => Action::BeginInteract,
                "open" => Action::BeginOpen,
                "open_map" => Action::OpenMap,
                "open_crafting" => Action::OpenCrafting,
                "shoot" => self.shoot_action(),
                "reload" => Action::Reload,
                "pickup" => self.pickup_action(),
                "open_inspect" => Action::OpenInspect,
                _ => Action::Message {
                    message: "Unsupported binding".into(),
                },
            };
        }
        match key {
            ';' => Action::OpenInspect,
            'e' => Action::BeginInteract,
            'o' => Action::BeginOpen,
            '.' => Action::WaitTurn,
            'g' => self.pickup_action(),
            'f' => self.shoot_action(),
            'r' => Action::Reload,
            'v' => Action::ToggleRunning,
            't' => self
                .entities
                .iter()
                .find(|(id, e)| {
                    matches!(e,Entity::Static(s) if s.destructible)
                        && e.map_position()
                            .is_some_and(|p| p.distance(player.position) == 1)
                        && self.can_attack(self.player_id, **id, None)
                })
                .map(|(target, _)| Action::Attack { target: *target })
                .unwrap_or(Action::Message {
                    message: "No attackable obstacle nearby".into(),
                }),
            _ => Action::Message {
                message: "Unknown key".into(),
            },
        }
    }
    fn pickup_action(&self) -> Action {
        self.position(self.player_id)
            .into_iter()
            .flat_map(|p| self.at(p))
            .find(|id| matches!(self.entities[id], Entity::Item(_)))
            .map(|item| Action::Pickup { item })
            .unwrap_or(Action::Message {
                message: "No item here".into(),
            })
    }
    fn shoot_action(&self) -> Action {
        let weapon = match self.targeting_weapon() {
            Ok(id) => id,
            Err(error) => {
                return Action::Message {
                    message: error.to_string(),
                };
            }
        };
        let definition = self
            .definition(weapon)
            .item
            .as_ref()
            .unwrap()
            .weapon
            .as_ref()
            .unwrap();
        if definition.magazine_capacity.is_some()
            && matches!(&self.entities[&weapon], Entity::Item(i) if i.loaded_ammo < definition.ammo_per_attack)
        {
            return Action::Message {
                message: "装填されていません。rでリロードしてください".into(),
            };
        }
        if self
            .shooting_targets()
            .is_ok_and(|targets| !targets.is_empty())
        {
            Action::OpenTargeting
        } else {
            Action::Message {
                message: "射程内に攻撃できる対象がいません".into(),
            }
        }
    }
    pub fn update(&mut self) {
        if !self.running {
            return;
        }
        if self.respawn_player() {
            return;
        }
        if let Some(action) = self.pending_action.take()
            && let Err(e) = self.update_action(action)
        {
            self.log(e.to_string());
        }
        for entity in self.entities.values_mut() {
            entity.update();
        }
        self.respawn_player();
    }
    fn update_action(&mut self, action: Action) -> Result<()> {
        let player = self.player_id;
        if self
            .actor(player)
            .is_some_and(|a| self.turn < a.stunned_until)
            && matches!(
                action,
                Action::WaitTurn
                    | Action::MoveTo { .. }
                    | Action::Attack { .. }
                    | Action::Shoot { .. }
                    | Action::Reload
                    | Action::Pickup { .. }
                    | Action::PickupNearby { .. }
                    | Action::Drop { .. }
                    | Action::Equip { .. }
                    | Action::EquipInHand { .. }
                    | Action::Unequip { .. }
                    | Action::ToggleArmor { .. }
                    | Action::ActivateQuickSlot { .. }
                    | Action::BandagePart { .. }
                    | Action::PerformInteraction { .. }
                    | Action::TakeContainerSelection
                    | Action::StartCraft { .. }
                    | Action::ContinueCraft
                    | Action::ConfirmItemTarget
                    | Action::FireSelected
                    | Action::UseItem { .. }
            )
        {
            self.log("閃光で行動できません");
            self.advance_turn();
            return Ok(());
        }
        match action {
            Action::ToggleRunning => self.toggle_running()?,
            Action::ZoomMap { delta } => {
                self.map_zoom = (i32::from(self.map_zoom) + delta.clamp(-1, 1)).clamp(1, 4) as u8;
            }
            Action::ScrollFooting { delta } => self.scroll_footing(delta)?,
            Action::GhoulTurn { .. } => return Err("AI action cannot be used by player".into()),
            Action::OpenItemTarget { item } => self.open_item_target(item)?,
            Action::MoveItemTarget { dx, dy } => self.move_item_target(dx, dy)?,
            Action::ConfirmItemTarget => self.confirm_item_target()?,
            Action::SelectRecipe { delta } => {
                ensure(matches!(self.panel, Panel::Crafting), "not crafting")?;
                if !self.recipes.is_empty() {
                    self.selection = (self.selection as i32 + delta)
                        .rem_euclid(self.recipes.len() as i32)
                        as usize;
                }
            }
            Action::Quit => self.running = false,
            Action::OpenInventory => {
                self.panel = Panel::Inventory;
                self.selection = 0;
            }
            Action::ItemDetails { item } => {
                ensure(
                    matches!(self.entities.get(&item), Some(Entity::Item(_)))
                        && self.entities[&item].carrier() == Some(player),
                    "item not held",
                )?;
                self.panel = Panel::ItemDetails { item };
                self.selection = 0;
            }
            Action::ScrollItemDetails { delta } => {
                if let Panel::ItemDetails { item } = self.panel {
                    ensure(self.entities.contains_key(&item), "item missing")?;
                    let max = crate::inspect::wrap_description(
                        &self.definition(item).common.description,
                        71,
                    )
                    .len()
                    .saturating_sub(28);
                    self.selection =
                        (self.selection as i64 + i64::from(delta)).clamp(0, max as i64) as usize;
                }
            }
            Action::SelectContainer { delta } => self.select_container(delta)?,
            Action::ToggleContainerSelection => self.toggle_container_selection()?,
            Action::TakeContainerSelection => self.take_container_selection()?,
            Action::BeginInteract => {
                self.panel = Panel::InteractionDirection;
                self.selection = 0;
            }
            Action::BeginOpen => {
                if let Some(target) = self.begin_open()? {
                    self.update_action(Action::PerformInteraction {
                        target,
                        interaction: "open".into(),
                    })?;
                }
            }
            Action::OpenDirection { dx, dy } => {
                if let Some(target) = self.choose_open_direction(dx, dy)? {
                    self.update_action(Action::PerformInteraction {
                        target,
                        interaction: "open".into(),
                    })?;
                }
            }
            Action::InteractDirection { dx, dy } => self.open_direction_interactions(dx, dy)?,
            Action::OpenItemInteractions { item } => {
                ensure(
                    self.entities
                        .get(&item)
                        .is_some_and(|e| e.carrier() == Some(player)),
                    "item not held",
                )?;
                self.panel = Panel::InteractionMenu {
                    position: self.position(player).ok_or("player missing")?,
                    carried: Some(item),
                };
                self.selection = 0;
            }
            Action::SelectInteraction { delta } => {
                ensure(
                    matches!(
                        self.panel,
                        Panel::InteractionMenu { .. } | Panel::OpenMenu { .. }
                    ),
                    "interaction menu is closed",
                )?;
                let len = self.interaction_choices().len();
                self.selection = (self.selection as i64 + i64::from(delta))
                    .clamp(0, len.saturating_sub(1) as i64)
                    as usize;
            }
            Action::PerformInteraction {
                target,
                interaction,
            } => {
                self.interact(target, &interaction)?;
            }
            Action::PickupNearby { item } => {
                ensure(
                    self.position(item)
                        .is_some_and(|p| self.visible_positions().contains(&p)),
                    "item is not here",
                )?;
                self.pickup_with_reach(player, item, 1)?;
                self.panel = Panel::World;
                self.advance_turn();
            }
            Action::OpenProperties => {
                self.panel = Panel::Properties;
                self.selection = 0;
            }
            Action::ScrollProperties { delta } => {
                let max = self.property_lines().len().saturating_sub(40);
                self.selection =
                    (self.selection as i64 + i64::from(delta)).clamp(0, max as i64) as usize;
            }
            Action::OpenMap => self.panel = Panel::Map,
            Action::OpenCrafting => {
                self.panel = Panel::Crafting;
                self.selection = 0;
            }
            Action::OpenInspect => {
                self.panel = Panel::Inspect {
                    position: self.position(player).ok_or("player missing")?,
                    selected: 0,
                };
            }
            Action::OpenTargeting => self.open_targeting()?,
            Action::AimDirection { dx, dy } => self.aim_direction(dx, dy)?,
            Action::CycleTarget { delta } => self.cycle_target(delta)?,
            Action::FireSelected => self.fire_selected()?,
            Action::MoveCursor { dx, dy } => self.move_cursor(dx, dy)?,
            Action::SelectInspected { delta } => self.select_inspected(delta)?,
            Action::ClosePanel => self.close_panel(),
            Action::Message { message } => self.log(message),
            Action::SelectInventory { delta } => {
                let len = self
                    .actor(player)
                    .map(|a| a.inventory.items.len())
                    .unwrap_or(0);
                self.selection = (self.selection as i32 + delta).max(0) as usize;
                self.selection = self.selection.min(len.saturating_sub(1));
            }
            Action::MoveTo { position } => {
                let p = self.position(player).ok_or("player missing")?;
                ensure(p.distance(position) == 1, "move must be one cell")?;
                self.player_move(position.x - p.x, position.y - p.y)?;
            }
            Action::Attack { target } => {
                self.attack_static(target)?;
            }
            Action::Shoot { target } => {
                self.shoot(player, target)?;
                self.events();
                self.advance_turn();
            }
            Action::Reload => {
                self.reload(player)?;
                self.advance_turn();
            }
            Action::SelectBandagePart { delta } => {
                let max = self.bandage_choices(player).len().saturating_sub(1);
                self.selection =
                    (self.selection as i64 + i64::from(delta)).clamp(0, max as i64) as usize;
            }
            Action::BandagePart { item, body_part } => {
                self.use_bandage_on(item, &body_part)?;
            }
            Action::UseItem { item } => {
                self.use_item(item)?;
            }
            Action::Pickup { item } => {
                self.pickup(player, item)?;
                self.advance_turn();
            }
            Action::Drop { item } => {
                self.drop_item(player, item)?;
                self.advance_turn();
            }
            Action::ToggleArmor { item } => {
                self.toggle_armor(player, item)?;
                self.log(format!(
                    "{}の着脱を切り替えました",
                    self.definition(item).common.name
                ));
                self.advance_turn();
            }
            Action::BindQuickSlot { item, slot, grip } => self.bind_quick_slot(item, slot, grip)?,
            Action::ActivateQuickSlot { slot } => self.activate_quick_slot(slot)?,
            Action::OpenEquipment { item } => {
                ensure(
                    self.entities
                        .get(&item)
                        .is_some_and(|e| e.carrier() == Some(player)),
                    "item not held",
                )?;
                ensure(
                    self.quick_bindable(item),
                    "この品は数字キーに登録できません",
                )?;
                let consumable = crate::equipment::equipment_hands(self.definition(item)).is_none();
                ensure(
                    consumable || !self.equipment_choices(player, item).is_empty(),
                    "使用できる手がありません",
                )?;
                self.panel = Panel::Equipment { item };
                let grip = self.default_quick_grip(item);
                self.selection = self
                    .equipment_choices(player, item)
                    .iter()
                    .position(|g| *g == grip)
                    .unwrap_or(0);
            }
            Action::SelectEquipment { delta } => {
                if let Panel::Equipment { item } = self.panel {
                    let max = self.equipment_choices(player, item).len().saturating_sub(1);
                    self.selection =
                        (self.selection as i64 + i64::from(delta)).clamp(0, max as i64) as usize;
                }
            }
            Action::Equip { item } => {
                self.equip_auto(player, item)?;
                self.log(format!(
                    "{}を装備しました",
                    self.definition(item).common.name
                ));
                self.advance_turn();
            }
            Action::EquipInHand { item, grip } => {
                self.equip_in_hand(player, item, grip)?;
                self.log(format!(
                    "{}を{}に装備しました",
                    self.definition(item).common.name,
                    grip.label()
                ));
                self.close_panel();
                self.advance_turn();
            }
            Action::Unequip { item } => {
                self.actor_mut(player).unwrap().inventory.unequip(item);
                self.close_panel();
                self.advance_turn();
            }
            Action::WaitTurn => self.advance_turn(),
            Action::StartCraft { recipe } => {
                self.start_craft(player, &recipe)?;
                self.continue_craft(player)?;
                self.advance_turn();
            }
            Action::ContinueCraft => {
                self.continue_craft(player)?;
                self.advance_turn();
            }
            Action::CancelCraft => crate::crafting::CraftingSystem::cancel(
                self.actor_mut(player).ok_or("player missing")?,
            ),
        }
        self.events();
        if matches!(self.panel, Panel::Inventory)
            && let Some(actor) = self.actor(player)
        {
            self.selection = self
                .selection
                .min(actor.inventory.items.len().saturating_sub(1));
        }
        Ok(())
    }
    /// Attach a separately registered console above the world for translucent popups.
    pub fn set_popup_console(&mut self, console: usize) {
        self.popup_console = Some(console);
    }
    pub fn render(&self, ctx: &mut bl::BTerm) {
        let world_console = ctx.active_console;
        if let Some(popup_console) = self.popup_console {
            ctx.set_active_console(popup_console);
            ctx.cls_bg(bl::RGBA::from_f32(0.0, 0.0, 0.0, 0.0));
            ctx.set_all_fg_alpha(0.0);
            ctx.set_active_console(world_console);
        }
        if self.map_console.is_some() || self.texture_renderer.is_some() {
            ctx.cls_bg(bl::RGBA::from_f32(0.0, 0.0, 0.0, 0.0));
            ctx.set_all_fg_alpha(0.0);
        } else {
            ctx.cls_bg(bl::RGB::named(crate::theme::BACKGROUND));
        }
        ctx.print_ja(
            1,
            0,
            format!(
                "Lua定義:{}  個体数:{}  ターン:{}",
                self.entity_definitions.len(),
                self.entities.len(),
                self.turn
            ),
        );
        if let Some(a) = self.actor(self.player_id) {
            let hp = &a.hp;
            ctx.print_ja(
                1,
                1,
                format!(
                    "HP:{}/{}  水分:{}  タンパク質:{}/100  炭水化物:{}/100  ビタミン:{}/100{}",
                    hp.hp,
                    hp.max_hp,
                    a.hydration.current,
                    a.nutrition.protein,
                    a.nutrition.carbohydrate,
                    a.nutrition.vitamin,
                    match a.respiratory_status() {
                        Some("pneumonia") => "  肺炎",
                        Some("cold") => "  風邪",
                        _ => "",
                    }
                ),
            );
            if let Some(id) = self.view().active_weapon(a.id, false) {
                let definition = self.definition(id);
                let ammo = match (
                    &self.entities[&id],
                    definition.item.as_ref().and_then(|i| i.weapon.as_ref()),
                ) {
                    (Entity::Item(item), Some(weapon)) => weapon
                        .magazine_capacity
                        .map(|capacity| format!("  装填:{}/{capacity}", item.loaded_ammo))
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                ctx.print_ja(
                    1,
                    2,
                    format!(
                        "{}  装備:{}  {}{ammo}",
                        self.combat_summary(self.player_id),
                        self.display_name(id),
                        self.held_weapon_summary(self.player_id, id)
                    ),
                );
            } else {
                ctx.print_ja(1, 2, self.combat_summary(self.player_id));
            }
        }
        if let Some(a) = self.actor(self.player_id) {
            ctx.print_ja(
                1,
                3,
                format!(
                    "v:速度切替  {}  スタミナ:{}/100  {}",
                    if a.running_mode { "走行" } else { "歩行" },
                    a.stamina,
                    self.level_progress_label()
                ),
            );
        }
        let view = self.visible_positions();
        if let Some(console) = self.map_overlay_console {
            self.prepare_map_console(ctx, console);
        }
        if let Some(console) = self.map_console {
            self.prepare_map_console(ctx, console);
        }
        if let Some(textures) = &self.texture_renderer {
            textures.render(self, ctx, &view);
        } else {
            let mut visible: Vec<_> = self
                .entities
                .values()
                .filter(|e| e.map_position().is_some_and(|p| view.contains(&p)))
                .collect();
            visible.sort_by_key(|e| (self.definition(e.id()).common.appearance.layer, e.id()));
            for e in visible {
                if let Some(p) = e.map_position().and_then(|p| self.map_draw_position(p)) {
                    let a = &self.definition(e.id()).common.appearance;
                    let glyph = e
                        .script()
                        .glyph
                        .as_deref()
                        .unwrap_or(&a.glyph)
                        .chars()
                        .next()
                        .unwrap();
                    ctx.set(
                        p.x,
                        p.y,
                        bl::RGB::from_u8(a.fg[0], a.fg[1], a.fg[2]),
                        bl::RGB::from_u8(a.bg[0], a.bg[1], a.bg[2]),
                        bl::to_cp437(glyph),
                    );
                }
            }
        }
        ctx.set_active_console(world_console);
        if let Some(console) = self.map_overlay_console {
            ctx.set_active_console(console);
        }
        for label in &self.map_labels {
            for (offset, c) in label.text.chars().enumerate() {
                let p = label.position.offset(offset as i32, 0);
                if view.contains(&p)
                    && !self
                        .at(p)
                        .iter()
                        .any(|id| self.definition(*id).common.appearance.layer > 0)
                    && let Some(p) = self.map_draw_position(p)
                {
                    ctx.print_ja_color(
                        p.x,
                        p.y,
                        bl::RGB::named(crate::theme::BORDER),
                        c.to_string(),
                    );
                }
            }
        }
        self.render_projectiles(ctx, &view);
        ctx.set_active_console(world_console);
        self.render_footing(ctx);
        ctx.print_ja(60, 0, format!("Zoom:{}x  +/-", self.map_zoom));
        for (row, commands) in [
            [
                "hjklyubn/矢印:移動",
                ".:待機",
                "g:拾う",
                "f:射撃",
                "r:装填",
                "t:伐採  o:開く",
            ],
            [
                "@:プロパティ",
                "i:所持品",
                "p:製作",
                "m:地図",
                "e:操作  ;:調査",
                "Ctrl+Q:終了",
            ],
        ]
        .iter()
        .enumerate()
        {
            for (column, command) in commands.iter().enumerate() {
                let x = [1, 23, 33, 43, 53, 65][column];
                ctx.print_ja(x, crate::theme::COMMAND_TOP + row as i32, command);
            }
        }
        for (row, message_text) in self
            .messages
            .iter()
            .skip(self.messages.len().saturating_sub(crate::theme::LOG_ROWS))
            .enumerate()
        {
            ctx.print_ja(1, crate::theme::LOG_TOP + row as i32, message(message_text));
        }
        for slot in 0..10 {
            let label = self.quick_slot_summary(slot);
            ctx.print_ja_color(
                (slot * 8) as i32,
                crate::theme::SLOTS_ROW,
                bl::RGB::named(if label == "未登録" {
                    crate::theme::MUTED
                } else {
                    crate::theme::ACCENT
                }),
                format!(
                    "{}:{}",
                    crate::quick_slots::slot_label(slot),
                    label.chars().take(5).collect::<String>()
                ),
            );
        }
        match self.panel {
            Panel::ItemTarget { item, position, .. } => {
                self.render_item_target(ctx, item, position)
            }
            Panel::Properties => self.render_properties(ctx),
            Panel::Equipment { item } => self.render_equipment(ctx, item),
            Panel::Bandage { item } => self.render_bandage_selection(ctx, item),
            Panel::ItemDetails { item } => self.render_item_details(ctx, item),
            Panel::Container { .. }
            | Panel::InteractionDirection
            | Panel::OpenDirection
            | Panel::OpenMenu { .. }
            | Panel::InteractionMenu { .. } => {
                if let Some(popup_console) = self.popup_console {
                    ctx.set_active_console(popup_console);
                    if matches!(self.panel, Panel::Container { .. }) {
                        self.render_container(ctx, 0.6);
                    } else {
                        self.render_interaction_panel(ctx, 0.6);
                    }
                    ctx.set_active_console(world_console);
                } else {
                    if matches!(self.panel, Panel::Container { .. }) {
                        self.render_container(ctx, 1.0);
                    } else {
                        self.render_interaction_panel(ctx, 1.0);
                    }
                }
            }
            Panel::Inventory => {
                crate::theme::draw_box(
                    ctx,
                    2,
                    3,
                    74,
                    31,
                    crate::theme::TEXT,
                    crate::theme::BACKGROUND,
                );
                ctx.print_ja_color(4, 4, bl::RGB::named(crate::theme::ACCENT), "所持品");
                ctx.print_ja(
                    4,
                    5,
                    "j/k:選択  Enter:使用  w:登録・装備  e:詳細  d:置く  Esc:戻る",
                );
                for (x, label) in [
                    (6, "名前・数字キー"),
                    (31, "個数"),
                    (34, "装備"),
                    (41, "装填"),
                    (52, "性能"),
                ] {
                    ctx.print_ja(x, 7, label);
                }
                if let Some(actor) = self.actor(self.player_id) {
                    let start = self.selection.saturating_sub(24);
                    for (row, (index, id)) in actor
                        .inventory
                        .items
                        .iter()
                        .enumerate()
                        .skip(start)
                        .take(25)
                        .enumerate()
                    {
                        let count = match &self.entities[id] {
                            Entity::Item(i) => i.quantity,
                            _ => 0,
                        };
                        let selected = index == self.selection;
                        let fg = bl::RGB::named(if selected {
                            crate::theme::ACCENT
                        } else {
                            crate::theme::TEXT
                        });
                        let y = 8 + row as i32;
                        ctx.print_ja_color(4, y, fg, if selected { ">" } else { " " });
                        let ammo = match (
                            &self.entities[id],
                            self.definition(*id)
                                .item
                                .as_ref()
                                .and_then(|i| i.weapon.as_ref()),
                        ) {
                            (Entity::Item(item), Some(weapon)) => weapon
                                .magazine_capacity
                                .map(|capacity| format!("{}/{capacity}", item.loaded_ammo))
                                .unwrap_or_default(),
                            _ => String::new(),
                        };
                        for (x, width, value) in [
                            (
                                6,
                                20,
                                format!("{}{}", self.display_name(*id), self.item_quick_keys(*id)),
                            ),
                            (27, 6, format!("{count:>6}")),
                            (
                                34,
                                6,
                                if actor.inventory.armor.values().any(|armor| armor == id) {
                                    "装着中".into()
                                } else {
                                    actor
                                        .inventory
                                        .grip(*id)
                                        .map(|g| {
                                            format!(
                                                "{}{}",
                                                g.label(),
                                                if self.view().item_hand_usable(actor.id, *id) {
                                                    ""
                                                } else {
                                                    "不可"
                                                }
                                            )
                                        })
                                        .unwrap_or_default()
                                },
                            ),
                            (41, 10, ammo),
                            (52, 22, self.combat_summary(*id)),
                        ] {
                            ctx.print_ja_color(
                                x,
                                y,
                                fg,
                                value.chars().take(width).collect::<String>(),
                            );
                        }
                    }
                }
            }
            Panel::Map => {
                ctx.print_ja_color(
                    2,
                    3,
                    bl::RGB::named(crate::theme::ACCENT),
                    "地図: 現在の視野を表示しています。何かキーを押すと戻ります",
                );
            }
            Panel::Crafting => self.render_crafting(ctx),
            Panel::Inspect { position, selected } => {
                self.render_inspection(ctx, position, selected)
            }
            Panel::Targeting { active_entity } => self.render_targeting(ctx, active_entity),
            Panel::World => {}
        }
    }
}
impl bl::GameState for Game {
    fn tick(&mut self, ctx: &mut bl::BTerm) {
        if !self.running {
            ctx.quitting = true;
            return;
        }
        self.input(ctx);
        self.update_frame(ctx.frame_time_ms);
        self.render(ctx);
        if !self.running {
            ctx.quitting = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_document_keys_generate_expected_actions() {
        let game = Game::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let p = game.position(game.player_id).unwrap();
        assert!(matches!(game.key_action('@'), Action::OpenProperties));
        for (key, dx, dy) in [
            ('h', -1, 0),
            ('j', 0, 1),
            ('k', 0, -1),
            ('l', 1, 0),
            ('y', -1, -1),
            ('u', 1, -1),
            ('b', -1, 1),
            ('n', 1, 1),
        ] {
            assert!(
                matches!(game.key_action(key), Action::MoveTo { position } if position == p.offset(dx, dy))
            );
        }
        assert!(matches!(game.key_action('r'), Action::Reload));
        assert!(matches!(game.key_action('v'), Action::ToggleRunning));
        assert!(matches!(game.key_action('i'), Action::OpenInventory));
        assert!(matches!(game.key_action('m'), Action::OpenMap));
        assert!(matches!(game.key_action('p'), Action::OpenCrafting));
        // 初期装備はナイフ。fで近接攻撃は行わない。
        assert!(matches!(game.key_action('f'), Action::Message { .. }));
    }

    #[test]
    fn pickup_reload_and_shoot_flow_advances_one_turn_each() {
        let mut game = Game::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let id = game.player_id;
        let p = game.position(id).unwrap();
        let item = game.spawn("ammo.pistol", Some(p), None, 2).unwrap();
        assert!(
            matches!(game.key_action('g'), Action::Pickup { item: selected } if selected == item)
        );
        game.pending_action = Some(game.key_action('g'));
        game.update();
        assert_eq!(game.turn, 1);
        let gun = game.spawn("item.glock", None, Some(id), 1).unwrap();
        game.equip_auto(id, gun).unwrap();
        assert!(matches!(game.key_action('f'), Action::Message { .. }));
        game.pending_action = Some(game.key_action('r'));
        game.update();
        assert_eq!(game.turn, 2);
        let target = game
            .spawn("monster.ghoul_worker", Some(p.offset(3, 0)), None, 1)
            .unwrap();
        assert!(matches!(game.key_action('f'), Action::OpenTargeting));
        game.pending_action = Some(game.key_action('f'));
        game.update();
        assert_eq!(game.turn, 2);
        assert!(
            matches!(game.panel, Panel::Targeting { active_entity } if active_entity == target)
        );
        game.pending_action = Some(Action::FireSelected);
        game.update();
        assert_eq!(game.turn, 3);
        assert!(matches!(game.entities[&gun], Entity::Item(ref i) if i.loaded_ammo == 1));
        game.damage(id, id, u32::MAX).unwrap();
        game.events();
        game.update();
        assert!(game.running && game.entities[&id].alive());
        assert!(matches!(game.entities[&gun], Entity::Item(ref i) if i.loaded_ammo == 1));
    }
}
