//! プレイヤーの現在値を表示する。閲覧とスクロールは時間を消費しない。
use crate::model::entity::body_part_name;
use crate::{game::Game, ui::inspect::wrap_description, ui::japanese::JapanesePrint};
use bracket_lib::prelude as bl;

impl Game {
    pub(crate) fn property_lines(&self) -> Vec<String> {
        let Some(a) = self.actor(self.player_id) else {
            return vec![];
        };
        let [protein, carbohydrate, vitamin] = super::displayed_nutrition(&a.nutrition);
        let mut lines = vec![
            format!("{:<8} {}", "名前", self.definition(a.id).common.name),
            self.level_progress_label(),
            format!(
                "位置 ({}, {}, {})  正気度:{}/{}",
                a.position.x, a.position.y, a.position.z, a.sanity, a.max_sanity
            ),
            format!("筋力 {:>3}    敏捷 {:>3}", a.kinryoku, a.subayasa),
            format!("器用さ {:>3}    精神力 {:>3}", a.kiyousa, a.seishinryoku),
            format!("{:<8} {}", "防御", self.combat_summary(a.id)),
            format!("水分 {}/{}", a.hydration.current, a.hydration.max),
            format!(
                "移動モード: {} / スタミナ {}/{}",
                if a.running_mode { "走行" } else { "歩行" },
                a.stamina,
                a.max_stamina
            ),
            format!(
                "{:<8} タンパク質 {:>3}/100    炭水化物 {:>3}/100    ビタミン {:>3}/100",
                "栄養", protein, carbohydrate, vitamin
            ),
            format!(
                "{:<8} {} マス",
                "視野",
                self.definition(a.id)
                    .movable
                    .as_ref()
                    .unwrap()
                    .senses
                    .sight_range
            ),
            String::new(),
            format!("HP {}/{}", a.hp.hp, a.hp.max_hp),
            format!(
                "閃光による行動不能: 残り{}ターン",
                a.stunned_until.saturating_sub(self.turn)
            ),
            "【部位の状態】".into(),
        ];
        lines.push(format!(
            "自然回復: {} / 病状: {}",
            if a.respiratory_status().is_some() {
                "病気で停止"
            } else if a.can_regenerate() {
                "HP1/ターン（タンパク質を消費）"
            } else {
                "タンパク質不足で停止"
            },
            a.illness_label()
        ));
        if a.respiratory_status() == Some("pneumonia") {
            lines.push(format!(
                "移動時HP減少: 10移動ターンで1（残り{}、下限HP1）",
                10 - a.pneumonia_move_turns
            ));
        }
        for name in a.body.keys() {
            lines.push(format!(
                "{:<8} {}",
                body_part_name(name),
                a.wound_label(name)
            ));
        }
        lines.extend([String::new(), "【装備・所持品】".into()]);
        lines.push(format!(
            "利き手: {}    移動: 1マス/{}ターン    出血: {}/ターン",
            a.dominant_hand.label(),
            a.movement_turns(),
            a.bleeding_damage()
        ));
        for (label, item) in [
            ("右手", a.inventory.right_hand),
            ("左手", a.inventory.left_hand),
        ] {
            lines.push(format!(
                "{label:<8} {}",
                item.map(|id| format!(
                    "{}{}",
                    self.display_name(id),
                    if self.view().item_hand_usable(a.id, id) {
                        ""
                    } else {
                        "（使用不可）"
                    }
                ))
                .unwrap_or_else(|| "なし".into())
            ));
        }
        lines.push(if let Some(id) = self.view().active_weapon(a.id, false) {
            format!(
                "{:<8} {}  {}",
                "装備",
                self.display_name(id),
                self.held_weapon_summary(a.id, id)
            )
        } else {
            format!("{:<8} なし", "装備")
        });
        for (slot, item) in &a.inventory.armor {
            lines.push(format!(
                "防具（{}）: {} {}",
                crate::game::equipment::armor_slot_name(slot),
                self.display_name(*item),
                self.combat_summary(*item)
            ));
        }
        lines.extend([String::new(), "【数字キー】".into()]);
        for slot in 0..10 {
            lines.push(format!(
                "{}: {}",
                crate::ui::quick_slots::slot_label(slot),
                self.quick_slot_summary(slot)
            ));
        }
        lines.push(format!(
            "{:<8} {} 組  詳細は i キー",
            "所持品",
            a.inventory.items.len()
        ));
        lines.push(format!(
            "{:<8} 長さ {} mm / 容量 {} ml / 重量 {} g",
            "所持上限", a.inventory.max_longsize, a.inventory.max_youryou, a.inventory.max_jyuuryou
        ));
        lines.extend([String::new(), "【技能】".into()]);
        for skill in crate::game::progression::SkillKind::ALL {
            let value = skill.value(&a.skill);
            let progress = a.progression.practice.get(&skill).copied().unwrap_or(0);
            let detail = if value >= self.growth_rules.max_skill {
                "上限".into()
            } else {
                let needed = self.growth_rules.practice_needed(value);
                format!(
                    "使用:{progress}/{needed}  次まで:{}回",
                    needed.saturating_sub(progress)
                )
            };
            lines.push(format!("{}:{value}  {detail}", skill.label()));
        }
        lines.extend([String::new(), "【状態】".into()]);
        if a.status.iter().all(|s| !s.enable) {
            lines.push("なし".into());
        }
        for status in a.status.iter().filter(|s| s.enable) {
            lines.push(format!(
                "{:<8} {:>3}",
                if status.name == "cold" {
                    "風邪"
                } else if status.name == "pneumonia" {
                    "肺炎"
                } else {
                    &status.name
                },
                status.value
            ));
        }
        lines.extend([String::new(), "【超能力】".into()]);
        if a.psykick.is_empty() {
            lines.push("なし".into());
        }
        for power in &a.psykick {
            lines.push(format!(
                "{:<8} 残り使用回数 {:>3}",
                power.name, power.shiyou_kaisu
            ));
            lines.extend(wrap_description(&power.disc, 72));
        }
        lines.extend([String::new(), "【製作】".into()]);
        lines.push(
            a.crafting
                .as_ref()
                .map(|job| format!("{}: {} ターン進行", job.recipe, job.completed_turns))
                .unwrap_or_else(|| "進行中の作業なし".into()),
        );
        lines
            .into_iter()
            .flat_map(|line| {
                if line.is_empty() {
                    vec![line]
                } else {
                    wrap_description(&line, 72)
                }
            })
            .collect()
    }

    pub(crate) fn render_properties(&self, ctx: &mut bl::BTerm) {
        crate::ui::theme::draw_box(
            ctx,
            1,
            2,
            77,
            46,
            crate::ui::theme::TEXT,
            crate::ui::theme::BACKGROUND,
        );
        ctx.print_ja_color(
            3,
            3,
            bl::RGB::named(crate::ui::theme::ACCENT),
            "キャラクターのプロパティ",
        );
        ctx.print_ja(3, 4, "j/k・矢印・PageUp/Down:スクロール  @/Esc:戻る");
        let lines = self.property_lines();
        for (row, line) in lines.iter().skip(self.selection).take(40).enumerate() {
            ctx.print_ja_color(
                3,
                5 + row as i32,
                bl::RGB::named(if line.starts_with('【') {
                    crate::ui::theme::ACCENT
                } else {
                    crate::ui::theme::TEXT
                }),
                line,
            );
        }
        ctx.print_ja(
            3,
            46,
            format!(
                "表示 {}-{} / {} 行  閲覧中は時間が進みません",
                self.selection + 1,
                (self.selection + 40).min(lines.len()),
                lines.len()
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{game::Panel, scripting::Action};

    #[test]
    fn properties_show_current_values_and_scroll_without_advancing_turns() {
        let mut game = Game::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let before = game.entity_snapshot(game.player_id).unwrap();
        game.pending_action = Some(Action::OpenProperties);
        game.update();
        assert!(matches!(game.panel, Panel::Properties));
        game.actor_mut(game.player_id).unwrap().kinryoku = 19;
        assert!(
            game.property_lines()
                .iter()
                .any(|s| { s.starts_with("筋力") && s.split_whitespace().nth(1) == Some("19") })
        );
        game.actor_mut(game.player_id).unwrap().kinryoku = 8;
        game.pending_action = Some(Action::ScrollProperties { delta: i32::MAX });
        game.update();
        assert_eq!(
            game.selection,
            game.property_lines().len().saturating_sub(40)
        );
        assert!(game.selection > 0);
        game.pending_action = Some(Action::ScrollProperties { delta: i32::MIN });
        game.update();
        assert_eq!(game.selection, 0);
        game.pending_action = Some(Action::ClosePanel);
        game.update();
        assert!(matches!(game.panel, Panel::World));
        assert_eq!(game.turn, 0);
        assert_eq!(game.entity_snapshot(game.player_id).unwrap(), before);
    }
}
