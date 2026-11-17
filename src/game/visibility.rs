//! 配置索引とEntityの遮蔽設定をbracket-libの視野計算へ渡す。
use crate::{game::Game, model::config::Pos};
use bracket_lib::prelude as bl;
use std::collections::BTreeSet;

struct VisibilityMap<'a>(&'a Game);

impl bl::BaseMap for VisibilityMap<'_> {
    fn is_opaque(&self, idx: usize) -> bool {
        self.0.map.cells.get(idx).is_none_or(|cell| {
            cell.entity_ids.iter().any(|id| {
                self.0
                    .entities
                    .get(id)
                    .is_some_and(|entity| entity.blocks_sight())
            })
        })
    }
}

impl bl::Algorithm2D for VisibilityMap<'_> {
    fn dimensions(&self) -> bl::Point {
        bl::Point::new(self.0.map.width, self.0.map.height)
    }
}

impl Game {
    /// 毎回現在の配置から計算し、移動・伐採・リスポーン後も古い視野を残さない。
    pub(crate) fn visible_positions(&self) -> BTreeSet<Pos> {
        let Some(origin) = self.position(self.player_id) else {
            return BTreeSet::new();
        };
        let range = self
            .definition(self.player_id)
            .movable
            .as_ref()
            .unwrap()
            .senses
            .sight_range;
        bl::field_of_view(
            bl::Point::new(origin.x, origin.y),
            range as i32,
            &VisibilityMap(self),
        )
        .into_iter()
        .map(|p| Pos {
            x: p.x,
            y: p.y,
            z: origin.z,
        })
        .filter(|p| self.contains(*p))
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obstacles_are_visible_but_hide_entities_behind_them_and_removal_reveals() {
        let mut game = Game::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let player = game.player_id;
        let origin = game.position(player).unwrap();
        // 他の配置に影響されず、壁・木の両方と視界外を確認する。
        let others: Vec<_> = game
            .entities
            .keys()
            .copied()
            .filter(|id| *id != player)
            .collect();
        for id in others {
            game.remove(id);
        }
        for definition in ["obstacle.stone_wall", "obstacle.tree"] {
            let wall = game
                .spawn(definition, Some(origin.offset(1, 0)), None, 1)
                .unwrap();
            let item = game
                .spawn("item.glock", Some(origin.offset(2, 0)), None, 1)
                .unwrap();
            let visible = game.visible_positions();
            assert!(visible.contains(&origin));
            assert!(visible.contains(&origin.offset(1, 0)));
            assert!(!visible.contains(&origin.offset(2, 0)));
            assert!(game.inspected_entities(origin.offset(2, 0)).is_empty());
            game.remove(wall);
            assert!(game.visible_positions().contains(&origin.offset(2, 0)));
            assert!(game.inspected_entities(origin.offset(2, 0)).contains(&item));
            game.remove(item);
        }
        let range = game
            .definition(player)
            .movable
            .as_ref()
            .unwrap()
            .senses
            .sight_range as i32;
        assert!(
            !game
                .visible_positions()
                .contains(&origin.offset(range + 1, 0))
        );
        game.move_to(player, origin.offset(1, 0)).unwrap();
        assert!(
            game.visible_positions()
                .contains(&origin.offset(range + 1, 0))
        );
    }
}
