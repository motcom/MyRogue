use crate::{
    game::{Game, Panel},
    model::config::*,
    model::entity::*,
};
use bracket_lib::prelude as bl;
pub fn number_slot(key: bl::VirtualKeyCode) -> Option<usize> {
    use bl::VirtualKeyCode as K;
    Some(match key {
        K::Key1 | K::Numpad1 => 0,
        K::Key2 | K::Numpad2 => 1,
        K::Key3 | K::Numpad3 => 2,
        K::Key4 | K::Numpad4 => 3,
        K::Key5 | K::Numpad5 => 4,
        K::Key6 | K::Numpad6 => 5,
        K::Key7 | K::Numpad7 => 6,
        K::Key8 | K::Numpad8 => 7,
        K::Key9 | K::Numpad9 => 8,
        K::Key0 | K::Numpad0 => 9,
        _ => return None,
    })
}
pub fn slot_label(slot: usize) -> String {
    ((slot + 1) % 10).to_string()
}
impl Game {
    pub fn quick_bindable(&self, item: Id) -> bool {
        self.entities.get(&item).is_some_and(|e| {
            e.alive()
                && crate::game::equipment::body_armor(self.definition(item)).is_none()
                && (crate::game::equipment::equipment_hands(self.definition(item)).is_some()
                    || self
                        .definition(item)
                        .item
                        .as_ref()
                        .is_some_and(|i| i.use_effect.is_some() || i.throwable.is_some()))
        })
    }
    pub(crate) fn default_quick_grip(&self, item: Id) -> Option<Grip> {
        let a = self.actor(self.player_id)?;
        let choices = self.equipment_choices(a.id, item);
        let current = a.inventory.grip(item);
        if let Some(g) = current.filter(|g| choices.contains(&Some(*g))) {
            return Some(g);
        }
        if choices.contains(&Some(Grip::Both)) {
            return Some(Grip::Both);
        }
        let main = if a.dominant_hand == Hand::Right {
            Grip::Right
        } else {
            Grip::Left
        };
        if choices.contains(&Some(main)) {
            Some(main)
        } else {
            choices.into_iter().flatten().next()
        }
    }
    pub fn bind_quick_slot(&mut self, item: Id, slot: usize, grip: Option<Grip>) -> Result<()> {
        ensure(slot < 10, "invalid quick slot")?;
        let actor = self.player_id;
        ensure(
            self.entities.get(&actor).is_some_and(Entity::alive),
            "dead actor",
        )?;
        ensure(
            self.entities
                .get(&item)
                .is_some_and(|e| e.carrier() == Some(actor))
                && self.quick_bindable(item),
            "この品は数字キーに登録できません",
        )?;
        let (binding, equipped) =
            if crate::game::equipment::equipment_hands(self.definition(item)).is_some() {
                let grip = grip.ok_or("登録する持ち方を選んでください")?;
                self.equip_in_hand(actor, item, grip)?;
                (QuickSlot::Equipment { item, grip }, true)
            } else {
                ensure(grip.is_none(), "消耗品に持ち方は指定しません")?;
                (
                    QuickSlot::Consumable {
                        definition_id: self.entities[&item].script().definition_id.clone(),
                    },
                    false,
                )
            };
        self.actor_mut(actor).unwrap().inventory.quick_slots[slot] = Some(binding);
        self.log(format!(
            "{}を数字{}に登録しました",
            self.definition(item).common.name,
            slot_label(slot)
        ));
        if matches!(self.panel, Panel::Equipment { .. }) {
            self.close_panel();
        }
        if equipped {
            self.advance_turn();
        }
        Ok(())
    }
    pub fn activate_quick_slot(&mut self, slot: usize) -> Result<()> {
        ensure(slot < 10, "invalid quick slot")?;
        let p = self.player_id;
        ensure(
            self.entities.get(&p).is_some_and(Entity::alive),
            "dead actor",
        )?;
        let binding = self.actor(p).ok_or("not actor")?.inventory.quick_slots[slot]
            .clone()
            .ok_or("この数字には登録されていません")?;
        match binding {
            QuickSlot::Equipment { item, grip } => {
                let a = self.actor(p).unwrap();
                if self
                    .entities
                    .get(&item)
                    .is_some_and(|e| e.alive() && e.carrier() == Some(p))
                    && a.inventory.grip(item) == Some(grip)
                    && a.inventory.equipped == Some(item)
                    && self.view().item_hand_usable(p, item)
                {
                    return Ok(());
                }
                self.equip_in_hand(p, item, grip)?;
                self.log(format!(
                    "{}を{}に装備しました",
                    self.definition(item).common.name,
                    grip.label()
                ));
                self.advance_turn();
            }
            QuickSlot::Consumable { definition_id } => {
                let item = self
                    .actor(p)
                    .unwrap()
                    .inventory
                    .items
                    .iter()
                    .copied()
                    .find(|id| {
                        self.entities
                            .get(id)
                            .is_some_and(|e| e.alive() && e.script().definition_id == definition_id)
                    })
                    .ok_or("登録した消耗品を所持していません")?;
                self.use_item(item)?;
            }
        }
        Ok(())
    }
    pub fn quick_slot_summary(&self, slot: usize) -> String {
        let Some(binding) = self
            .actor(self.player_id)
            .and_then(|a| a.inventory.quick_slots.get(slot))
            .and_then(|s| s.as_ref())
        else {
            return "未登録".into();
        };
        match binding {
            QuickSlot::Equipment { item, grip } => self
                .entities
                .get(item)
                .map(|_| format!("{}（{}）", self.definition(*item).common.name, grip.label()))
                .unwrap_or_else(|| "未所持".into()),
            QuickSlot::Consumable { definition_id } => {
                let name = self
                    .entity_definitions
                    .get(definition_id)
                    .map_or("不明", |d| d.common.name.as_str());
                let count: u64 = self
                    .actor(self.player_id)
                    .unwrap()
                    .inventory
                    .items
                    .iter()
                    .filter_map(|id| match self.entities.get(id) {
                        Some(Entity::Item(i)) if i.script.definition_id == *definition_id => {
                            Some(u64::from(i.quantity))
                        }
                        _ => None,
                    })
                    .sum();
                format!("{name} ×{count}")
            }
        }
    }
    pub(crate) fn render_quick_slots(&self, ctx: &mut bl::BTerm, x: i32, y: i32) {
        use crate::ui::japanese::JapanesePrint;
        for slot in 0..10 {
            ctx.print_ja(
                x + (slot / 5) as i32 * 35,
                y + (slot % 5) as i32,
                format!(
                    "{}: {}",
                    slot_label(slot),
                    self.quick_slot_summary(slot)
                        .chars()
                        .take(29)
                        .collect::<String>()
                ),
            );
        }
    }
}

impl Game {
    pub(crate) fn item_quick_keys(&self, item: Id) -> String {
        let Some(a) = self.actor(self.player_id) else {
            return String::new();
        };
        let definition_id = &self.entities[&item].script().definition_id;
        let keys: Vec<_> = a
            .inventory
            .quick_slots
            .iter()
            .enumerate()
            .filter_map(|(slot, binding)| {
                let same = match binding {
                    Some(QuickSlot::Equipment { item: id, .. }) => *id == item,
                    Some(QuickSlot::Consumable {
                        definition_id: kind,
                    }) => kind == definition_id,
                    None => false,
                };
                same.then(|| slot_label(slot))
            })
            .collect();
        if keys.is_empty() {
            String::new()
        } else {
            format!(" [{}]", keys.join(","))
        }
    }
}
