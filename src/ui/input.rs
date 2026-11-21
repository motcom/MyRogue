//! Translate keyboard events into pending actions.
use crate::{
    game::{Game, Panel},
    model::entity::*,
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
                && self.animations.is_empty()
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
            let delta = crate::ui::camera::zoom_delta(&input_events, ctx.key, ctx.shift);
            if let Some(delta) = delta {
                self.pending_action = Some(Action::ZoomMap { delta });
                return;
            }
        }
        // 演出中もCtrl+Qで終了できる。
        if !self.animations.is_empty() {
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
                | Panel::PickupMenu
                | Panel::SmashDirection
                | Panel::SmashMenu { .. }
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
                && let Some(slot) = crate::ui::quick_slots::number_slot(key)
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
                    if crate::game::equipment::body_armor(self.definition(item)).is_some() {
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
            && let Some(slot) = crate::ui::quick_slots::number_slot(key)
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
                K::S => 's',
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
                "smash" => Action::BeginSmash,
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
            's' => Action::BeginSmash,
            '.' => Action::WaitTurn,
            'g' => self.pickup_action(),
            'f' => self.shoot_action(),
            'r' => Action::Reload,
            'v' => Action::ToggleRunning,
            _ => Action::Message {
                message: "Unknown key".into(),
            },
        }
    }
    fn pickup_action(&self) -> Action {
        match self.items_at_feet().as_slice() {
            [] => Action::Message {
                message: "足元に拾える物はありません".into(),
            },
            [item] => Action::Pickup { item: *item },
            _ => Action::OpenPickupMenu,
        }
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
}
