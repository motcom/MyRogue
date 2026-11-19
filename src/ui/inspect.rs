//! ターンを進めず、マップ上のEntityの種類と説明を調べる。
use crate::{
    game::{Game, Panel},
    model::config::*,
    model::entity::*,
    scripting::Action,
    ui::japanese::JapanesePrint,
};
use bracket_lib::prelude as bl;
use std::cmp::Reverse;

impl Game {
    pub(crate) fn inspect_key_action(
        &self,
        key: bl::VirtualKeyCode,
        shift: bool,
    ) -> Option<Action> {
        use bl::VirtualKeyCode as K;
        let (dx, dy) = match key {
            K::H | K::Left => (-1, 0),
            K::J | K::Down => (0, 1),
            K::K | K::Up => (0, -1),
            K::L | K::Right => (1, 0),
            K::Y => (-1, -1),
            K::U => (1, -1),
            K::B => (-1, 1),
            K::N => (1, 1),
            K::Semicolon | K::Escape => return Some(Action::ClosePanel),
            K::Tab => {
                return Some(Action::SelectInspected {
                    delta: if shift { -1 } else { 1 },
                });
            }
            _ => return None,
        };
        Some(Action::MoveCursor { dx, dy })
    }

    pub(crate) fn inspected_entities(&self, position: Pos) -> Vec<Id> {
        if !self.visible_positions().contains(&position) {
            return Vec::new();
        }
        let mut ids: Vec<_> = self
            .at(position)
            .into_iter()
            .filter(|id| {
                self.entities
                    .get(id)
                    .is_some_and(|e| e.map_position() == Some(position))
            })
            .collect();
        // 見えている一番上のEntityから選ぶ。地面もTabで確認できる。
        ids.sort_by_key(|id| Reverse((self.definition(*id).common.appearance.layer, *id)));
        ids
    }

    pub(crate) fn move_cursor(&mut self, dx: i32, dy: i32) -> Result<()> {
        ensure(
            (-1..=1).contains(&dx) && (-1..=1).contains(&dy),
            "invalid cursor movement",
        )?;
        if let Panel::Inspect { position, selected } = &mut self.panel {
            let next = Pos {
                x: (position.x + dx).clamp(0, self.map.width - 1),
                y: (position.y + dy).clamp(0, self.map.height - 1),
                z: position.z,
            };
            if next != *position {
                *position = next;
                *selected = 0;
            }
        }
        Ok(())
    }

    pub(crate) fn select_inspected(&mut self, delta: i32) -> Result<()> {
        ensure((-1..=1).contains(&delta), "invalid inspect selection")?;
        if let Panel::Inspect { position, selected } = self.panel {
            let len = self.inspected_entities(position).len();
            self.panel = Panel::Inspect {
                position,
                selected: if len == 0 {
                    0
                } else {
                    (selected.min(len - 1) as i64 + i64::from(delta)).rem_euclid(len as i64)
                        as usize
                },
            };
        }
        Ok(())
    }

    pub(crate) fn observed_health(&self, id: Id) -> Option<&'static str> {
        let entity = self.entities.get(&id)?;
        let actor = self.actor(id)?;
        if !self.visible_positions().contains(&actor.position) {
            return None;
        }
        if !entity.alive() {
            return Some("倒れている");
        }
        let mut severity = injury_grade(&actor.hp);
        for stage in actor.body.values() {
            severity = severity.max(match stage {
                InjuryStage::Healthy => 0,
                InjuryStage::Bleeding => 1,
                InjuryStage::HeavyBleeding | InjuryStage::Severed => 2,
            });
        }
        if actor
            .status
            .iter()
            .any(|s| s.enable && s.value > 0 && s.name == "bleeding")
        {
            severity = severity.max(1);
        }
        Some(["元気", "軽症", "重症", "瀕死"][severity])
    }

    pub(crate) fn observed_condition_lines(&self, id: Id) -> Option<Vec<String>> {
        let health = self.observed_health(id)?;
        let actor = self.actor(id)?;
        let bleeding = actor.bleeding_damage() > 0
            || actor
                .status
                .iter()
                .any(|s| s.enable && s.value > 0 && s.name == "bleeding");
        let mut lines = vec![format!(
            "状態: {health}    出血: {}",
            if bleeding { "あり" } else { "なし" }
        )];
        if actor.respiratory_status().is_some() {
            lines.push(format!("病状: {}", actor.illness_label()));
        }
        let order = [
            "head",
            "body",
            "left_arm",
            "right_arm",
            "left_leg",
            "right_leg",
        ];
        let summaries: Vec<_> = order
            .iter()
            .filter_map(|name| {
                actor
                    .body
                    .get(*name)
                    .map(|_| format!("{}: {}", body_part_name(name), actor.wound_label(name)))
            })
            .collect();
        for pair in summaries.chunks(2) {
            lines.push(format!(
                "{:<36} {}",
                pair[0],
                pair.get(1).map_or("", String::as_str)
            ));
        }
        Some(lines)
    }

    pub(crate) fn render_inspection(&self, ctx: &mut bl::BTerm, position: Pos, selected: usize) {
        self.map_highlight(ctx, position, bl::RGB::named(crate::ui::theme::ACCENT));
        let top = self.map.height.min(crate::ui::theme::MAP_ROWS) + 4;
        crate::ui::theme::draw_box(
            ctx,
            0,
            top,
            79,
            49 - top,
            crate::ui::theme::ACCENT,
            crate::ui::theme::BACKGROUND,
        );
        ctx.print_ja_color(
            2,
            top + 1,
            bl::RGB::named(crate::ui::theme::ACCENT),
            format!(
                "カーソルモード ({}, {})  時間は進みません",
                position.x, position.y
            ),
        );
        ctx.print_ja(
            2,
            top + 2,
            "hjklyubn/矢印:移動  Tab/Shift+Tab:対象切替  ;/Esc:戻る",
        );
        let ids = self.inspected_entities(position);
        let Some(id) = ids.get(selected.min(ids.len().saturating_sub(1))) else {
            ctx.print_ja(
                2,
                top + 4,
                if self.visible_positions().contains(&position) {
                    "このマスにEntityはありません"
                } else {
                    "視界外のため調べられません"
                },
            );
            return;
        };
        let common = &self.definition(*id).common;
        let kind = match &self.entities[id] {
            Entity::Movable(actor) => match actor.kind {
                CreatureKind::Player => "可動体・プレイヤー",
                CreatureKind::Monster => "可動体・モンスター",
                CreatureKind::Animal => "可動体・動物",
                CreatureKind::Npc => "可動体・NPC",
            },
            Entity::Static(entity) => match entity.role {
                StaticRole::Terrain => "固定物・地形",
                StaticRole::Obstacle => "固定物・障害物",
                StaticRole::Facility => "固定物・設備",
            },
            Entity::Item(_) => "アイテム",
        };
        let title = format!(
            "{}/{}  {} {}  種類: {kind}",
            selected.min(ids.len() - 1) + 1,
            ids.len(),
            self.display_name(*id),
            self.combat_summary(*id)
        );
        ctx.print_ja(2, top + 4, title.chars().take(76).collect::<String>());
        let mut description_top = top + 5;
        if let Some(lines) = self.observed_condition_lines(*id) {
            for (row, line) in lines
                .iter()
                .take((49 - description_top).max(0) as usize)
                .enumerate()
            {
                ctx.print_ja_color(
                    2,
                    description_top + row as i32,
                    bl::RGB::named(if row == 0 {
                        crate::ui::theme::ACCENT
                    } else {
                        crate::ui::theme::TEXT
                    }),
                    line,
                );
            }
            description_top += lines.len() as i32;
        }
        let description = if common.description.trim().is_empty() {
            "説明はまだ登録されていません"
        } else {
            &common.description
        };
        for (row, line) in wrap_description(description, 76)
            .iter()
            .take((49 - description_top).max(0) as usize)
            .enumerate()
        {
            ctx.print_ja(2, description_top + row as i32, line);
        }
    }
}

fn injury_grade(part: &HP) -> usize {
    let hp = u64::from(part.hp);
    let max = u64::from(part.max_hp);
    if hp * 4 >= max * 3 {
        0
    } else if hp * 2 >= max {
        1
    } else if hp * 4 >= max {
        2
    } else {
        3
    }
}

pub(crate) fn wrap_description(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut count = 0;
    for c in text.chars() {
        if c == '\n' || count == width {
            lines.push(std::mem::take(&mut line));
            count = 0;
        }
        if !c.is_control() {
            line.push(c);
            count += 1;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Game {
        Game::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn action(game: &mut Game, action: Action) {
        game.pending_action = Some(action);
        game.update();
    }

    #[test]
    fn observed_body_summaries_show_bleeding_and_limbs_without_numbers() {
        let mut game = game();
        let p = game.position(game.player_id).unwrap();
        let enemy = game
            .spawn("monster.ghoul_worker", Some(p.offset(1, 0)), None, 1)
            .unwrap();
        let lines = game.observed_condition_lines(enemy).unwrap();
        assert!(lines[0].contains("出血: なし"));
        assert_eq!(lines.len(), 4);
        assert!(lines[1].contains("頭: 異常なし"));
        let actor = game.actor_mut(enemy).unwrap();
        actor
            .body
            .insert("left_arm".into(), InjuryStage::HeavyBleeding);
        actor.body.insert("left_leg".into(), InjuryStage::Severed);
        actor.body.insert("right_leg".into(), InjuryStage::Bleeding);
        let before = game.entity_snapshot(enemy).unwrap();
        let lines = game.observed_condition_lines(enemy).unwrap();
        assert!(lines[0].contains("出血: あり"));
        assert!(lines[2].contains("左腕: 大出血"));
        assert!(lines[2].contains("右腕: 異常なし"));
        assert!(lines[3].contains("左脚: 切断"));
        assert!(lines[3].contains("右脚: 出血"));
        assert!(
            lines
                .iter()
                .all(|line| !line.chars().any(|c| c.is_ascii_digit()) && !line.contains('/'))
        );
        assert_eq!(game.entity_snapshot(enemy).unwrap(), before);
        game.actor_mut(enemy)
            .unwrap()
            .body
            .values_mut()
            .for_each(|s| *s = InjuryStage::Healthy);
        assert!(game.observed_condition_lines(enemy).unwrap()[0].contains("出血: なし"));
        assert_eq!(game.turn, 0);
    }

    #[test]
    fn observed_health_uses_rough_labels_and_handles_limb_injuries() {
        let mut game = game();
        let p = game.position(game.player_id).unwrap();
        let enemy = game
            .spawn("monster.ghoul_worker", Some(p.offset(1, 0)), None, 1)
            .unwrap();
        game.actor_mut(enemy).unwrap().hp = HP {
            hp: 100,
            max_hp: 100,
        };
        for (hp, expected) in [
            (100, "元気"),
            (99, "元気"),
            (75, "元気"),
            (74, "軽症"),
            (50, "軽症"),
            (49, "重症"),
            (25, "重症"),
            (24, "瀕死"),
            (1, "瀕死"),
        ] {
            game.actor_mut(enemy).unwrap().hp.hp = hp;
            assert_eq!(game.observed_health(enemy), Some(expected));
        }
        game.actor_mut(enemy).unwrap().hp.hp = 0;
        assert_eq!(game.observed_health(enemy), Some("倒れている"));
        game.actor_mut(enemy).unwrap().hp.hp = 100;
        for (stage, expected) in [
            (InjuryStage::Bleeding, "軽症"),
            (InjuryStage::HeavyBleeding, "重症"),
            (InjuryStage::Severed, "重症"),
        ] {
            game.actor_mut(enemy)
                .unwrap()
                .body
                .insert("left_arm".into(), stage);
            assert_eq!(game.observed_health(enemy), Some(expected));
            assert!(game.observed_condition_lines(enemy).unwrap()[0].contains("出血: あり"));
        }
        game.actor_mut(enemy)
            .unwrap()
            .body
            .insert("head".into(), InjuryStage::Severed);
        assert_eq!(game.observed_health(enemy), Some("倒れている"));
    }

    #[test]
    fn observing_health_is_read_only_and_does_not_reveal_hidden_enemies() {
        let mut game = game();
        let player = game.player_id;
        let p = game.position(player).unwrap();
        let enemy = game
            .spawn("monster.ghoul_worker", Some(p.offset(1, 0)), None, 1)
            .unwrap();
        let before = game.entity_snapshot(enemy).unwrap();
        let mut expected_rng = game.rng.clone();
        action(&mut game, Action::OpenInspect);
        action(&mut game, Action::MoveCursor { dx: 1, dy: 0 });
        assert_eq!(game.inspected_entities(p.offset(1, 0))[0], enemy);
        assert_eq!(game.observed_health(enemy), Some("元気"));
        assert_eq!(game.turn, 0);
        assert_eq!(game.entity_snapshot(enemy).unwrap(), before);
        assert_eq!(game.rng.roll(1000), expected_rng.roll(1000));
        let wall = game
            .spawn("obstacle.stone_wall", Some(p.offset(0, 1)), None, 1)
            .unwrap();
        game.move_to(enemy, p.offset(1, 1)).unwrap();
        game.move_to(enemy, p.offset(0, 2)).unwrap();
        assert_eq!(game.observed_health(enemy), None);
        assert!(game.observed_condition_lines(enemy).is_none());
        game.remove(wall);
        assert_eq!(game.observed_health(enemy), Some("元気"));
        game.remove(enemy);
        assert_eq!(game.observed_health(enemy), None);
        assert!(game.observed_condition_lines(enemy).is_none());
    }

    #[test]
    fn inspecting_moves_only_cursor_and_does_not_advance_world() {
        let mut game = game();
        let player = game.position(game.player_id).unwrap();
        let positions: Vec<_> = game
            .entities
            .iter()
            .map(|(id, e)| (*id, e.map_position()))
            .collect();
        action(&mut game, Action::OpenInspect);
        assert!(
            matches!(game.panel, Panel::Inspect { position, selected: 0 } if position == player)
        );
        let movement = game
            .inspect_key_action(bl::VirtualKeyCode::L, false)
            .unwrap();
        action(&mut game, movement);
        assert!(
            matches!(game.panel, Panel::Inspect { position, .. } if position == player.offset(1, 0))
        );
        assert_eq!(game.position(game.player_id), Some(player));
        action(&mut game, Action::SelectInspected { delta: 1 });
        action(&mut game, Action::ClosePanel);
        assert!(matches!(game.panel, Panel::World));
        assert_eq!(game.turn, 0);
        assert!(game.running);
        assert_eq!(
            positions,
            game.entities
                .iter()
                .map(|(id, e)| (*id, e.map_position()))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn cursor_crosses_walls_and_stays_inside_map() {
        let mut game = game();
        game.panel = Panel::Inspect {
            position: Pos { x: 1, y: 1, z: 0 },
            selected: 0,
        };
        action(&mut game, Action::MoveCursor { dx: -1, dy: -1 });
        action(&mut game, Action::MoveCursor { dx: -1, dy: -1 });
        assert!(matches!(
            game.panel,
            Panel::Inspect {
                position: Pos { x: 0, y: 0, .. },
                ..
            }
        ));
        assert!(game.inspected_entities(Pos { x: 0, y: 0, z: 0 }).is_empty());
        game.panel = Panel::Inspect {
            position: Pos {
                x: game.map.width - 1,
                y: game.map.height - 1,
                z: 0,
            },
            selected: 0,
        };
        action(&mut game, Action::MoveCursor { dx: 1, dy: 1 });
        assert!(
            matches!(game.panel, Panel::Inspect { position, .. } if position.x == game.map.width - 1 && position.y == game.map.height - 1)
        );
        assert_eq!(game.turn, 0);
    }

    #[test]
    fn stacked_entities_cycle_in_both_directions_and_exclude_inventory() {
        let mut game = game();
        let p = game.position(game.player_id).unwrap().offset(1, 0);
        action(&mut game, Action::OpenInspect);
        action(&mut game, Action::MoveCursor { dx: 1, dy: 0 });
        let ids = game.inspected_entities(p);
        assert!(ids.len() >= 2);
        assert_eq!(game.definition(ids[0]).id, "item.glock");
        assert!(
            ids.iter()
                .all(|id| !game.definition(*id).common.description.is_empty())
        );
        action(&mut game, Action::SelectInspected { delta: -1 });
        assert!(matches!(game.panel, Panel::Inspect { selected, .. } if selected == ids.len() - 1));
        action(&mut game, Action::SelectInspected { delta: 1 });
        assert!(matches!(game.panel, Panel::Inspect { selected: 0, .. }));
        for id in ids {
            game.remove(id);
        }
        action(&mut game, Action::SelectInspected { delta: 1 });
        assert!(game.inspected_entities(p).is_empty());
        assert!(matches!(game.panel, Panel::Inspect { selected: 0, .. }));
        action(&mut game, Action::MoveCursor { dx: -1, dy: 0 });
        let ids = game.inspected_entities(game.position(game.player_id).unwrap());
        assert!(!ids.iter().any(|id| game.entities[id].carrier().is_some()));
        assert_eq!(game.turn, 0);
        assert!(game.running);
    }

    #[test]
    fn inspection_keys_and_description_wrapping() {
        let game = game();
        assert!(matches!(
            game.inspect_key_action(bl::VirtualKeyCode::Tab, true),
            Some(Action::SelectInspected { delta: -1 })
        ));
        assert!(matches!(
            game.inspect_key_action(bl::VirtualKeyCode::Semicolon, false),
            Some(Action::ClosePanel)
        ));
        assert!(
            game.inspect_key_action(bl::VirtualKeyCode::F, false)
                .is_none()
        );
        assert_eq!(
            wrap_description("あいうえ\nおかき", 3),
            vec!["あいう", "え", "おかき"]
        );
    }
}
