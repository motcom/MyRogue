//! 投擲の位置選択と、射程を持つ近接武器の敵選択への接続。
use crate::{
    config::*,
    entity::*,
    game::{Game, Panel},
    japanese::JapanesePrint,
};
use bracket_lib::prelude as bl;
impl Game {
    pub(crate) fn reach_weapon(&self) -> Option<Id> {
        self.view()
            .active_weapon(self.player_id, false)
            .filter(|id| {
                self.definition(*id)
                    .item
                    .as_ref()
                    .and_then(|i| i.weapon.as_ref())
                    .is_some_and(|w| w.mode == "melee" && w.range > 1)
            })
    }
    pub(crate) fn open_item_target(&mut self, item: Id) -> Result<()> {
        let actor = self.player_id;
        ensure(
            self.entities
                .get(&item)
                .is_some_and(|e| e.alive() && e.carrier() == Some(actor)),
            "item not held",
        )?;
        let d = self.definition(item).item.as_ref().ok_or("not an item")?;
        if d.weapon
            .as_ref()
            .is_some_and(|w| w.mode == "melee" && w.range > 1)
        {
            ensure(
                self.reach_weapon() == Some(item),
                "使用する武器を装備してください",
            )?;
            return self.open_targeting();
        }
        ensure(d.throwable.is_some(), "投擲できるアイテムが必要です")?;
        ensure(
            self.actor(actor)
                .is_some_and(|a| a.hand_usable(Hand::Left) || a.hand_usable(Hand::Right)),
            "使用できる手がありません",
        )?;
        self.panel = Panel::ItemTarget {
            item,
            from_world: matches!(self.panel, Panel::World),
            position: self.position(actor).ok_or("player missing")?,
        };
        Ok(())
    }
    pub(crate) fn move_item_target(&mut self, dx: i32, dy: i32) -> Result<()> {
        ensure(
            (-1..=1).contains(&dx) && (-1..=1).contains(&dy),
            "invalid direction",
        )?;
        let Panel::ItemTarget {
            item,
            position,
            from_world,
        } = self.panel
        else {
            return Err("not targeting an item".into());
        };
        let next = position.offset(dx, dy);
        if self.contains(next) && self.visible_positions().contains(&next) {
            self.panel = Panel::ItemTarget {
                item,
                from_world,
                position: next,
            };
        }
        Ok(())
    }
    pub(crate) fn confirm_item_target(&mut self) -> Result<()> {
        let Panel::ItemTarget { item, position, .. } = self.panel else {
            return Err("not targeting an item".into());
        };
        ensure(self.entities.contains_key(&item), "item missing")?;
        self.throw_item(self.player_id, item, position)?;
        self.panel = Panel::World;
        self.events();
        self.advance_turn();
        Ok(())
    }
    pub fn throw_item(&mut self, actor: Id, item: Id, center: Pos) -> Result<()> {
        ensure(
            self.entities.get(&actor).is_some_and(Entity::alive),
            "dead thrower",
        )?;
        ensure(
            self.entities
                .get(&item)
                .is_some_and(|e| e.alive() && e.carrier() == Some(actor)),
            "item not held",
        )?;
        let a = self.actor(actor).ok_or("thrower missing")?;
        ensure(
            a.hand_usable(Hand::Left) || a.hand_usable(Hand::Right),
            "使用できる手がありません",
        )?;
        let origin = a.position;
        let t = self
            .definition(item)
            .item
            .as_ref()
            .and_then(|i| i.throwable.clone())
            .ok_or("not throwable")?;
        ensure(
            self.contains(center)
                && origin.distance(center) <= t.range as i32
                && self.sight(origin, center),
            "投擲先が射程外か遮られています",
        )?;
        if actor == self.player_id {
            ensure(
                self.visible_positions().contains(&center),
                "投擲先が見えません",
            )?;
        }
        ensure(
            !self
                .at(center)
                .iter()
                .any(|id| self.entities[id].blocks_sight()),
            "投擲先が壁で塞がれています",
        )?;
        let victims: Vec<_> = self
            .entities
            .iter()
            .filter_map(|(id, e)| {
                let p = e.map_position()?;
                let distance = center.distance(p);
                (e.alive()
                    && distance <= t.radius as i32
                    && self.sight(center, p)
                    && (self.actor(*id).is_some()
                        || matches!(e, Entity::Static(s) if s.destructible)))
                .then(|| {
                    (
                        *id,
                        t.power.saturating_mul(t.radius + 1 - distance as u32) / (t.radius + 1),
                    )
                })
            })
            .collect();
        let name = self.definition(item).common.name.clone();
        let animation = self.thrown_projectile(origin, center, t.kind, t.radius);
        let before = self.clone();
        let result = (|| {
            self.consume(item, 1)?;
            self.practice_skill(actor, crate::progression::SkillKind::Touteki);
            self.log(format!("{name}を投擲しました ({}, {})", center.x, center.y));
            for (id, amount) in victims {
                match t.kind {
                    BlastKind::Explosive => self.damage(actor, id, amount)?,
                    BlastKind::Flash => {
                        let until = if amount == 0 {
                            self.turn
                        } else {
                            self.turn
                                .saturating_add(u64::from(amount))
                                .saturating_add(1)
                        };
                        if let Some(a) = self.actor_mut(id) {
                            a.stunned_until = a.stunned_until.max(until);
                        }
                    }
                }
            }
            if let Some(animation) = animation
                && self.projectiles.len() < 64
            {
                self.projectiles.push_back(animation);
            }
            self.record_attack_exertion(actor);
            Ok(())
        })();
        if result.is_err() {
            *self = before;
        }
        result
    }
    pub(crate) fn render_item_target(&self, ctx: &mut bl::BTerm, item: Id, position: Pos) {
        ctx.print_ja_color(
            2,
            2,
            bl::RGB::named(crate::theme::ACCENT),
            format!(
                "{}: 方向で照準  Enter/f:使用  Esc:中止",
                self.definition(item).common.name
            ),
        );
        // The world is drawn at the same coordinates as the inspection cursor.
        self.render_inspection(ctx, position, 0);
        ctx.print_ja(
            2,
            self.map.height + 6,
            "hjklyubn/矢印:照準移動  Enter/f:使用  Esc:中止       ",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::Action;
    fn game() -> Game {
        Game::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap()
    }
    #[test]
    fn f_opens_spear_enemy_selection_and_g_only_picks_up() {
        let mut g = game();
        let p = g.player_id;
        let spear = g.spawn("item.spear", None, Some(p), 1).unwrap();
        g.equip_in_hand(p, spear, Grip::Both).unwrap();
        let target = g
            .spawn(
                "monster.ghoul_brute",
                Some(g.position(p).unwrap().offset(2, 0)),
                None,
                1,
            )
            .unwrap();
        assert!(matches!(g.key_action('f'), Action::OpenTargeting));
        assert!(matches!(g.key_action('g'), Action::Message { .. }));
        let loot = g.spawn("item.apple", g.position(p), None, 1).unwrap();
        assert!(matches!(g.key_action('g'),Action::Pickup{item} if item==loot));
        g.pending_action = Some(g.key_action('f'));
        g.update();
        assert!(matches!(g.panel,Panel::Targeting{active_entity} if active_entity==target));
        g.pending_action = Some(Action::ClosePanel);
        g.update();
        assert!(matches!(g.panel, Panel::World));
        assert_eq!(g.turn, 0);
        g.pending_action = Some(g.key_action('f'));
        g.update();
        g.pending_action = Some(Action::FireSelected);
        g.update();
        assert_eq!(g.turn, 1);
        assert!(matches!(g.panel, Panel::World));
        assert!(g.entities.contains_key(&spear));
    }
    #[test]
    fn grenade_aim_cancel_and_stale_confirmation_do_not_consume() {
        let mut g = game();
        let p = g.player_id;
        let item = g.spawn("item.frag_grenade", None, Some(p), 2).unwrap();
        g.panel = Panel::Inventory;
        g.use_item(item).unwrap();
        g.move_item_target(1, 0).unwrap();
        g.pending_action = Some(Action::ClosePanel);
        g.update();
        assert_eq!(g.turn, 0);
        assert!(matches!(&g.entities[&item],Entity::Item(i) if i.quantity==2));
        g.use_item(item).unwrap();
        g.drop_item(p, item).unwrap();
        assert!(g.confirm_item_target().is_err());
        assert_eq!(g.turn, 0);
        g.remove(item);
        assert!(g.confirm_item_target().is_err());
    }
    #[test]
    fn confirmation_uses_one_item_and_one_turn() {
        let mut g = game();
        let p = g.player_id;
        let ids: Vec<_> = g
            .entities
            .iter()
            .filter_map(|(id, e)| (matches!(e, Entity::Movable(_)) && *id != p).then_some(*id))
            .collect();
        for id in ids {
            g.remove(id);
        }
        let item = g.spawn("item.frag_grenade", None, Some(p), 2).unwrap();
        g.use_item(item).unwrap();
        g.move_item_target(1, 0).unwrap();
        g.move_item_target(1, 0).unwrap();
        g.pending_action = Some(Action::ConfirmItemTarget);
        g.update();
        assert_eq!(g.turn, 1);
        assert!(matches!(g.panel, Panel::World));
        assert!(matches!(&g.entities[&item],Entity::Item(i) if i.quantity==1));
    }
    #[test]
    fn throwable_shortcut_targets_cancels_and_uses_replenishment() {
        let mut g = game();
        let p = g.player_id;
        let enemies: Vec<_> = g
            .entities
            .iter()
            .filter_map(|(id, e)| (matches!(e, Entity::Movable(_)) && *id != p).then_some(*id))
            .collect();
        for id in enemies {
            g.remove(id);
        }
        let item = g.spawn("item.flash_grenade", None, Some(p), 1).unwrap();
        g.bind_quick_slot(item, 0, None).unwrap();
        g.activate_quick_slot(0).unwrap();
        assert_eq!(g.turn, 0);
        assert!(matches!(
            g.panel,
            Panel::ItemTarget {
                from_world: true,
                ..
            }
        ));
        g.pending_action = Some(Action::ClosePanel);
        g.update();
        assert!(matches!(g.panel, Panel::World));
        assert!(g.entities.contains_key(&item));
        g.activate_quick_slot(0).unwrap();
        for _ in 0..3 {
            g.move_item_target(0, 1).unwrap();
        }
        g.confirm_item_target().unwrap();
        assert_eq!(g.turn, 1);
        assert!(!g.entities.contains_key(&item));
        let second = g.spawn("item.flash_grenade", None, Some(p), 1).unwrap();
        g.activate_quick_slot(0).unwrap();
        assert!(matches!(g.panel,Panel::ItemTarget {item,..} if item==second));
        assert_eq!(g.turn, 1);
    }
    #[test]
    fn stunned_player_can_browse_but_actions_spend_time_without_effect() {
        let mut g = game();
        let p = g.player_id;
        g.actor_mut(p).unwrap().stunned_until = 3;
        let old = g.position(p).unwrap();
        for action in [
            Action::OpenInventory,
            Action::ClosePanel,
            Action::OpenCrafting,
            Action::SelectRecipe { delta: 1 },
            Action::ClosePanel,
        ] {
            g.pending_action = Some(action);
            g.update();
            assert_eq!(g.turn, 0);
        }
        g.pending_action = Some(Action::MoveTo {
            position: old.offset(0, 1),
        });
        g.update();
        assert_eq!(g.turn, 1);
        assert_eq!(g.position(p), Some(old));
    }
}
