pub fn equipment_hands(d: &crate::config::Definition) -> Option<crate::config::WeaponHands> {
    let item = d.item.as_ref()?;
    item.weapon.as_ref().map(|w| w.hands).or_else(|| {
        d.common
            .tags
            .iter()
            .any(|tag| tag == "shield")
            .then_some(crate::config::WeaponHands::One)
    })
}
use crate::{config::*, entity::*, game::Game, world::EntityView};
impl EntityView<'_> {
    pub fn item_hand_usable(&self, actor: Id, item: Id) -> bool {
        let Some(a) = self.actor(actor) else {
            return false;
        };
        match a.inventory.grip(item) {
            Some(Grip::Right) => a.hand_usable(Hand::Right),
            Some(Grip::Left) => a.hand_usable(Hand::Left),
            Some(Grip::Both) => a.hand_usable(Hand::Right) && a.hand_usable(Hand::Left),
            None => true,
        }
    }
    pub fn weapon_usable(&self, actor: Id, item: Id) -> bool {
        let Some(a) = self.actor(actor) else {
            return false;
        };
        let Some(grip) = a.inventory.grip(item) else {
            return false;
        };
        let Some(e) = self.entities.get(&item) else {
            return false;
        };
        if !e.alive() || e.carrier() != Some(actor) || !self.item_hand_usable(actor, item) {
            return false;
        }
        let Some(w) = self
            .definition(item)
            .item
            .as_ref()
            .and_then(|i| i.weapon.as_ref())
        else {
            return false;
        };
        matches!(
            (w.hands, grip),
            (WeaponHands::Either, _)
                | (WeaponHands::Two, Grip::Both)
                | (WeaponHands::One, Grip::Right | Grip::Left)
        )
    }
    pub fn active_weapon(&self, actor: Id, ranged: bool) -> Option<Id> {
        let a = self.actor(actor)?;
        let (main, other) = match a.dominant_hand {
            Hand::Right => (a.inventory.right_hand, a.inventory.left_hand),
            Hand::Left => (a.inventory.left_hand, a.inventory.right_hand),
        };
        [a.inventory.equipped, main, other]
            .into_iter()
            .flatten()
            .find(|id| {
                self.weapon_usable(actor, *id)
                    && (!ranged
                        || self
                            .definition(*id)
                            .item
                            .as_ref()
                            .unwrap()
                            .weapon
                            .as_ref()
                            .unwrap()
                            .mode
                            == "ranged")
            })
    }
    pub fn off_hand(&self, actor: Id, item: Id) -> bool {
        let Some(a) = self.actor(actor) else {
            return false;
        };
        matches!(
            (a.dominant_hand, a.inventory.grip(item)),
            (Hand::Right, Some(Grip::Left)) | (Hand::Left, Some(Grip::Right))
        )
    }
    pub fn effective_range(&self, actor: Id, item: Id) -> u32 {
        let w = self
            .definition(item)
            .item
            .as_ref()
            .unwrap()
            .weapon
            .as_ref()
            .unwrap();
        let single = self
            .actor(actor)
            .is_some_and(|a| a.inventory.grip(item) != Some(Grip::Both));
        w.range
            .saturating_sub(if single { w.one_hand_range_penalty } else { 0 })
            .max(1)
    }
    pub fn one_hand_accuracy_penalty(&self, actor: Id, item: Id) -> u32 {
        let w = self
            .definition(item)
            .item
            .as_ref()
            .unwrap()
            .weapon
            .as_ref()
            .unwrap();
        if self
            .actor(actor)
            .is_some_and(|a| a.inventory.grip(item) == Some(Grip::Both))
        {
            0
        } else {
            w.one_hand_accuracy_penalty
        }
    }
    pub fn effective_weapon_pv(&self, actor: Id, item: Id) -> i32 {
        let a = self.actor(actor).unwrap();
        let w = self
            .definition(item)
            .item
            .as_ref()
            .unwrap()
            .weapon
            .as_ref()
            .unwrap();
        if w.mode == "melee" {
            (w.base_pv as i32
                + crate::combat::modifier(a.kinryoku).min(w.strength_cap as i32)
                + if a.inventory.grip(item) == Some(Grip::Both) {
                    w.two_hand_pv_bonus as i32
                } else {
                    0
                })
            .max(0)
        } else {
            w.base_pv as i32
        }
    }
}
impl Game {
    pub fn equip_auto(&mut self, actor: Id, item: Id) -> Result<()> {
        ensure(
            self.entities
                .get(&item)
                .is_some_and(|e| e.carrier() == Some(actor) && e.alive()),
            "item not held/usable",
        )?;
        let a = self.actor(actor).ok_or("not an actor")?;
        let hands = equipment_hands(self.definition(item)).ok_or("装備できるのは武器と盾です")?;
        let grip = if hands == WeaponHands::Two
            || (hands == WeaponHands::Either
                && a.hand_usable(Hand::Right)
                && a.hand_usable(Hand::Left))
        {
            Grip::Both
        } else if a.hand_usable(a.dominant_hand) {
            match a.dominant_hand {
                Hand::Right => Grip::Right,
                Hand::Left => Grip::Left,
            }
        } else {
            match a.dominant_hand {
                Hand::Right => Grip::Left,
                Hand::Left => Grip::Right,
            }
        };
        self.equip_in_hand(actor, item, grip)
    }
    pub fn equip_in_hand(&mut self, actor: Id, item: Id, grip: Grip) -> Result<()> {
        ensure(
            self.entities.get(&actor).is_some_and(Entity::alive),
            "dead actor",
        )?;
        ensure(
            self.entities
                .get(&item)
                .is_some_and(|e| e.carrier() == Some(actor) && e.alive()),
            "item not held/usable",
        )?;
        let hands = equipment_hands(self.definition(item)).ok_or("装備できるのは武器と盾です")?;
        ensure(
            matches!(
                (hands, grip),
                (WeaponHands::Either, _)
                    | (WeaponHands::Two, Grip::Both)
                    | (WeaponHands::One, Grip::Right | Grip::Left)
            ),
            "この持ち方では装備できません",
        )?;
        let a = self.actor_mut(actor).ok_or("not an actor")?;
        ensure(
            (grip == Grip::Left || a.hand_usable(Hand::Right))
                && (grip == Grip::Right || a.hand_usable(Hand::Left)),
            "切断された手には装備できません",
        )?;
        // 置き換える装備が両手持ちなら、両方のスロットから外す。
        let conflicts = [
            if grip != Grip::Left {
                a.inventory.right_hand
            } else {
                None
            },
            if grip != Grip::Right {
                a.inventory.left_hand
            } else {
                None
            },
        ];
        for old in conflicts.into_iter().flatten() {
            a.inventory.unequip(old);
        }
        a.inventory.unequip(item);
        if grip != Grip::Left {
            a.inventory.right_hand = Some(item);
        }
        if grip != Grip::Right {
            a.inventory.left_hand = Some(item);
        }
        a.inventory.equipped = Some(item);
        Ok(())
    }
}

impl Game {
    pub(crate) fn render_equipment(&self, ctx: &mut bracket_lib::prelude::BTerm, item: Id) {
        use crate::japanese::JapanesePrint;
        use bracket_lib::prelude as bl;
        crate::theme::draw_box(
            ctx,
            3,
            5,
            72,
            37,
            crate::theme::TEXT,
            crate::theme::BACKGROUND,
        );
        ctx.print_ja(5, 7, format!("装備: {}", self.definition(item).common.name));
        if equipment_hands(self.definition(item)).is_none() {
            ctx.print_ja(5, 9, "1〜0:登録する数字キーを押す  Esc:戻る");
            ctx.print_ja(5, 12, "登録後は通常画面で数字キーを押すと使用できます");
            ctx.print_ja(5, 14, "包帯は数字キーで出血部位の選択を開きます");
            self.render_quick_slots(ctx, 5, 26);
            ctx.print_ja(5, 39, "消耗品の登録では時間が進みません");
            return;
        }
        let a = self.actor(self.player_id).unwrap();
        ctx.print_ja(
            5,
            9,
            format!(
                "利き手: {}  j/k:持ち方  1〜0:登録・装備  Esc:戻る",
                a.dominant_hand.label()
            ),
        );
        for (row, hand) in [Hand::Right, Hand::Left].iter().enumerate() {
            let item = if *hand == Hand::Right {
                a.inventory.right_hand
            } else {
                a.inventory.left_hand
            };
            let name = item
                .map(|id| self.definition(id).common.name.as_str())
                .unwrap_or("なし");
            ctx.print_ja(
                5,
                11 + row as i32,
                format!(
                    "{}: {}{}",
                    hand.label(),
                    name,
                    if a.hand_usable(*hand) {
                        ""
                    } else {
                        "（切断・使用不可）"
                    }
                ),
            );
        }
        if let Some(w) = self
            .definition(item)
            .item
            .as_ref()
            .and_then(|i| i.weapon.as_ref())
        {
            let label = match w.hands {
                WeaponHands::One => "片手",
                WeaponHands::Two => "両手",
                WeaponHands::Either => "片手・両手",
            };
            ctx.print_ja(
                5,
                14,
                format!("対応: {label}    両手PV加算: {}", w.two_hand_pv_bonus),
            );
            if w.hands == WeaponHands::Either && w.mode == "ranged" {
                ctx.print_ja(
                    5,
                    15,
                    format!(
                        "片手時: 射程 -{} 命中 -{}",
                        w.one_hand_range_penalty, w.one_hand_accuracy_penalty
                    ),
                );
            }
        }
        for (index, grip) in self
            .equipment_choices(self.player_id, item)
            .iter()
            .enumerate()
        {
            let label = grip
                .map(|g| format!("{}に装備", g.label()))
                .unwrap_or_else(|| "装備を外す".into());
            ctx.print_ja_color(
                7,
                18 + index as i32,
                bl::RGB::named(if index == self.selection {
                    crate::theme::ACCENT
                } else {
                    crate::theme::TEXT
                }),
                format!(
                    "{} {label}",
                    if index == self.selection { ">" } else { " " }
                ),
            );
        }
        ctx.print_ja(5, 24, "数字キーの登録内容（同じ数字への登録は上書き）");
        self.render_quick_slots(ctx, 5, 26);
        ctx.print_ja(5, 37, "逆手の武器はダメージ3/4。切断した手の装備は使用不可");
        ctx.print_ja(5, 39, "選択中は時間が進みません。装備確定で一ターン");
    }
}

impl Game {
    pub fn held_weapon_summary(&self, actor: Id, item: Id) -> String {
        let Some(a) = self.actor(actor) else {
            return String::new();
        };
        let Some(grip) = a.inventory.grip(item) else {
            return String::new();
        };
        let Some(w) = self
            .definition(item)
            .item
            .as_ref()
            .and_then(|i| i.weapon.as_ref())
        else {
            return String::new();
        };
        format!(
            "{} {} PV{} 射程{}{}{}",
            grip.label(),
            w.damage,
            self.view().effective_weapon_pv(actor, item),
            self.view().effective_range(actor, item),
            if self.view().off_hand(actor, item) {
                " 被害3/4"
            } else {
                ""
            },
            if self.view().weapon_usable(actor, item) {
                ""
            } else {
                " 使用不可"
            }
        )
    }
}

impl Game {
    pub fn equipment_choices(&self, actor: Id, item: Id) -> Vec<Option<Grip>> {
        let Some(a) = self.actor(actor) else {
            return vec![];
        };
        let Some(e) = self.entities.get(&item) else {
            return vec![];
        };
        if e.carrier() != Some(actor) {
            return vec![];
        }
        let Some(hands) = equipment_hands(self.definition(item)) else {
            return vec![];
        };
        let mut choices: Vec<_> = [Grip::Right, Grip::Left, Grip::Both]
            .into_iter()
            .filter(|g| {
                (hands == WeaponHands::Either
                    || (hands == WeaponHands::Two && *g == Grip::Both)
                    || (hands == WeaponHands::One && *g != Grip::Both))
                    && (*g == Grip::Left || a.hand_usable(Hand::Right))
                    && (*g == Grip::Right || a.hand_usable(Hand::Left))
            })
            .map(Some)
            .collect();
        if a.inventory.grip(item).is_some() {
            choices.push(None);
        }
        choices
    }
}

/// 盾は手、防具はLuaで指定した防具枠へ装備する。
pub fn body_armor(d: &Definition) -> Option<&Armor> {
    if d.item.as_ref()?.weapon.is_some() || d.common.tags.iter().any(|t| t == "shield") {
        return None;
    }
    d.item.as_ref()?.armor.as_ref()
}
impl Game {
    pub fn toggle_armor(&mut self, actor: Id, item: Id) -> Result<()> {
        ensure(
            self.entities.get(&actor).is_some_and(Entity::alive),
            "dead actor",
        )?;
        ensure(
            self.entities
                .get(&item)
                .is_some_and(|e| e.alive() && e.carrier() == Some(actor)),
            "item not held/usable",
        )?;
        let slot = body_armor(self.definition(item))
            .ok_or("not body armor")?
            .slot
            .clone();
        let a = self.actor_mut(actor).ok_or("not actor")?;
        if a.inventory.armor.get(&slot) == Some(&item) {
            a.inventory.armor.remove(&slot);
        } else {
            a.inventory.armor.insert(slot, item);
        }
        Ok(())
    }
}

pub fn armor_slot_name(slot: &str) -> &str {
    match slot {
        "head" => "頭",
        "body" => "胴体",
        "arms" => "腕",
        "hands" => "手",
        "legs" => "脚",
        "feet" => "足",
        slot => slot,
    }
}
