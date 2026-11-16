use crate::{combat, config::*, entity::*, game::Game};
impl Game {
    pub(crate) fn has_usable_shield(&self, actor: EntityId) -> bool {
        self.actor(actor).is_some_and(|a| {
            [a.inventory.right_hand, a.inventory.left_hand]
                .into_iter()
                .flatten()
                .any(|id| {
                    self.entities
                        .get(&id)
                        .is_some_and(|e| e.alive() && e.carrier() == Some(actor))
                        && self
                            .definition(id)
                            .common
                            .tags
                            .iter()
                            .any(|tag| tag == "shield")
                        && self.view().item_hand_usable(actor, id)
                })
        })
    }

    pub fn weapon_damage(&self, weapon: EntityId) -> Result<Dice> {
        let w = self
            .definition(weapon)
            .item
            .as_ref()
            .and_then(|i| i.weapon.as_ref())
            .ok_or("not a weapon")?;
        let mut damage = w.damage.clone();
        if let Some(Entity::Item(item)) = self.entities.get(&weapon) {
            damage.bonus = damage.bonus.saturating_add(item.quality);
        }
        Ok(damage)
    }
    pub fn defense_values(&self, id: EntityId) -> Option<(u32, i32)> {
        if let Some(actor) = self.actor(id) {
            let c = &self.definition(id).movable.as_ref()?.combat;
            let mut av = c.base_av;
            let mut dv = 6 + combat::modifier(actor.subayasa) + c.dv_bonus;
            let mut items: std::collections::BTreeSet<_> =
                actor.inventory.armor.values().copied().collect();
            for item in [actor.inventory.right_hand, actor.inventory.left_hand]
                .into_iter()
                .flatten()
            {
                if self.entities.get(&item).is_some_and(|_| {
                    self.definition(item)
                        .common
                        .tags
                        .iter()
                        .any(|t| t == "shield")
                }) && self.view().item_hand_usable(id, item)
                {
                    items.insert(item);
                }
            }
            for item in items {
                if self
                    .entities
                    .get(&item)
                    .is_some_and(|e| e.alive() && e.carrier() == Some(id))
                    && let Some(armor) = self
                        .definition(item)
                        .item
                        .as_ref()
                        .and_then(|i| i.armor.as_ref())
                {
                    av = av.saturating_add(armor.av);
                    dv = dv.saturating_add(armor.dv);
                }
            }
            if self.has_usable_shield(id) {
                dv += (actor.skill.shield / self.growth_rules.combat_skill_step) as i32;
            }
            Some((av, dv))
        } else {
            self.definition(id)
                .r#static
                .as_ref()
                .map(|s| (s.combat.base_av, 0))
        }
    }
    pub fn combat_summary(&self, id: EntityId) -> String {
        if let Some((av, dv)) = self.defense_values(id) {
            return format!("AV{av} DV{dv}");
        }
        let Some(item) = &self.definition(id).item else {
            return String::new();
        };
        let mut parts = Vec::new();
        if let Some(w) = &item.weapon
            && let Ok(damage) = self.weapon_damage(id)
        {
            parts.push(format!("{damage} PV{}", w.base_pv));
        }
        if let Some(armor) = &item.armor {
            parts.push(format!("AV{} DV{:+}", armor.av, armor.dv));
        }
        parts.join(" ")
    }
}
