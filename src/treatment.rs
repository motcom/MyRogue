//! 包帯は部位ごとに処置を保持し、全体HPと切断状態から分離する。
use crate::{
    config::*,
    entity::*,
    game::{Game, Panel},
    japanese::JapanesePrint,
};
impl Game {
    pub fn bandage_choices(&self, actor: Id) -> Vec<String> {
        let Some(a) = self.actor(actor) else {
            return vec![];
        };
        [
            "head",
            "body",
            "left_arm",
            "right_arm",
            "left_leg",
            "right_leg",
        ]
        .into_iter()
        .filter(|part| a.part_bleeding(part) > 0 && !a.treatment.bandages.contains_key(*part))
        .map(String::from)
        .collect()
    }
    pub(crate) fn open_bandage_selection(&mut self, item: Id) -> Result<()> {
        ensure(
            !self.bandage_choices(self.player_id).is_empty(),
            "包帯を使える出血部位がありません",
        )?;
        self.bandage_return_to_world = matches!(self.panel, Panel::World);
        self.panel = Panel::Bandage { item };
        self.selection = 0;
        Ok(())
    }
    pub(crate) fn apply_bandage(&mut self, source: Id, target: Id, part: &str) -> Result<()> {
        ensure(
            self.entities
                .get(&source)
                .is_some_and(|e| e.carrier() == Some(target) && e.alive()),
            "bandage must be held by patient",
        )?;
        ensure(
            self.entities.get(&target).is_some_and(Entity::alive),
            "patient is not alive",
        )?;
        let use_effect = self
            .definition(source)
            .item
            .as_ref()
            .and_then(|i| i.use_effect.as_ref())
            .filter(|u| u.kind == "bandage")
            .ok_or("source is not a bandage")?;
        let duration = use_effect.amount.ok_or("bandage duration missing")?;
        ensure(
            self.bandage_choices(target).iter().any(|p| p == part),
            "この部位には包帯を使えません",
        )?;
        let expires = self.turn.saturating_add(u64::from(duration));
        self.actor_mut(target)
            .unwrap()
            .treatment
            .bandages
            .insert(part.into(), expires);
        Ok(())
    }
    pub fn use_bandage_on(&mut self, item: Id, part: &str) -> Result<bool> {
        let player = self.player_id;
        ensure(
            self.entities.get(&player).is_some_and(Entity::alive),
            "patient is not alive",
        )?;
        ensure(
            self.entities
                .get(&item)
                .is_some_and(|e| e.carrier() == Some(player) && e.alive()),
            "item not held/usable",
        )?;
        ensure(
            self.definition(item)
                .item
                .as_ref()
                .and_then(|i| i.use_effect.as_ref())
                .is_some_and(|u| u.kind == "bandage"),
            "not a bandage",
        )?;
        ensure(
            self.bandage_choices(player).iter().any(|p| p == part),
            "この部位には包帯を使えません",
        )?;
        let changed = self.hook(
            item,
            "on_use",
            serde_json::json!({"user":player,"body_part":part}),
        )?;
        if changed {
            if matches!(self.panel, Panel::Bandage { .. }) {
                self.close_panel();
            }
            self.log(format!("{}に包帯を巻きました", body_part_name(part)));
            self.events();
            self.advance_turn();
        }
        Ok(changed)
    }
    pub(crate) fn finish_bandages(&mut self) {
        let mut finished = Vec::new();
        let turn = self.turn;
        for (id, e) in &mut self.entities {
            if !e.alive() {
                continue;
            }
            if let Entity::Movable(a) = e {
                let parts: Vec<_> = a
                    .treatment
                    .bandages
                    .iter()
                    .filter(|(_, expires)| **expires <= turn)
                    .map(|(part, _)| part.clone())
                    .collect();
                for part in parts {
                    a.treatment.bandages.remove(&part);
                    a.treatment.stopped.insert(part.clone());
                    finished.push((*id, part));
                }
            }
        }
        for (id, part) in finished {
            if id == self.player_id {
                self.log(format!(
                    "{}の止血が完了し、包帯が外れました",
                    body_part_name(&part)
                ));
            }
        }
    }
    pub(crate) fn render_bandage_selection(&self, ctx: &mut bracket_lib::prelude::BTerm, item: Id) {
        use bracket_lib::prelude as bl;
        crate::theme::draw_box(
            ctx,
            3,
            5,
            72,
            24,
            crate::theme::TEXT,
            crate::theme::BACKGROUND,
        );
        ctx.print_ja(5, 7, "包帯を使う部位");
        ctx.print_ja(5, 9, "j/k:選択  Enter:巻く  Esc:戻る");
        let choices = self.bandage_choices(self.player_id);
        if choices.is_empty() {
            ctx.print_ja(5, 11, "包帯を使える出血部位がありません");
        }
        for (index, part) in choices.iter().enumerate() {
            let a = self.actor(self.player_id).unwrap();
            ctx.print_ja_color(
                7,
                11 + index as i32,
                bl::RGB::named(if index == self.selection {
                    crate::theme::ACCENT
                } else {
                    crate::theme::TEXT
                }),
                format!(
                    "{} {}: {}",
                    if index == self.selection { ">" } else { " " },
                    body_part_name(part),
                    a.wound_label(part)
                ),
            );
        }
        let duration = self
            .entities
            .get(&item)
            .and_then(|_| self.definition(item).item.as_ref())
            .and_then(|i| i.use_effect.as_ref())
            .and_then(|u| u.amount)
            .unwrap_or(100);
        ctx.print_ja(
            5,
            21,
            format!("一部位に一個使用。{duration}ターン後に止血し包帯が外れます"),
        );
        ctx.print_ja(5, 23, "処置中も出血は続きます。選択中は時間が進みません");
    }
}
