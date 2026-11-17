//! Apply actions and advance game state.
use crate::{
    game::{Game, Panel},
    model::{config::*, entity::*},
    scripting::Action,
};
impl Game {
    pub fn update(&mut self) {
        self.check_sanity_game_over();
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
                    let max = crate::ui::inspect::wrap_description(
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
                let consumable =
                    crate::game::equipment::equipment_hands(self.definition(item)).is_none();
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
            Action::CancelCraft => crate::game::crafting::CraftingSystem::cancel(
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
}
