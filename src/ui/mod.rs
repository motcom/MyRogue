//! Input, display and the GameState frame adapter.

pub mod animations;
pub(crate) mod camera;
pub(crate) mod direction_input;
pub(crate) mod footing;
pub mod inspect;
pub mod japanese;
pub use animations as projectiles;
mod melee;
pub mod properties;
pub mod quick_slots;
pub mod targeting;
pub mod theme;

mod game_over;
mod input;
mod render;

use crate::game::Game;
use bracket_lib::prelude as bl;

impl bl::GameState for Game {
    fn tick(&mut self, ctx: &mut bl::BTerm) {
        if self.game_over {
            self.game_over_tick(ctx);
            return;
        }
        if !self.running {
            ctx.quitting = true;
            return;
        }
        self.input(ctx);
        self.update_frame(ctx.frame_time_ms);
        self.render(ctx);
        if !self.running && !self.game_over {
            ctx.quitting = true;
        }
    }
}

#[cfg(test)]
pub(crate) static CONSOLE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{game::Panel, model::entity::Entity, scripting::Action};

    #[test]
    fn player_document_keys_generate_expected_actions() {
        let game = Game::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let p = game.position(game.player_id).unwrap();
        assert!(matches!(game.key_action('@'), Action::OpenProperties));
        for (key, dx, dy) in [
            ('h', -1, 0),
            ('j', 0, 1),
            ('k', 0, -1),
            ('l', 1, 0),
            ('y', -1, -1),
            ('u', 1, -1),
            ('b', -1, 1),
            ('n', 1, 1),
        ] {
            assert!(
                matches!(game.key_action(key), Action::MoveTo { position } if position == p.offset(dx, dy))
            );
        }
        assert!(matches!(game.key_action('r'), Action::Reload));
        assert!(matches!(game.key_action('v'), Action::ToggleRunning));
        assert!(matches!(game.key_action('i'), Action::OpenInventory));
        assert!(matches!(game.key_action('m'), Action::OpenMap));
        assert!(matches!(game.key_action('p'), Action::OpenCrafting));
        // 初期装備はナイフ。fで近接攻撃は行わない。
        assert!(matches!(game.key_action('f'), Action::Message { .. }));
    }

    #[test]
    fn pickup_reload_and_shoot_flow_advances_one_turn_each() {
        let mut game = Game::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let id = game.player_id;
        let p = game.position(id).unwrap();
        let item = game.spawn("ammo.pistol", Some(p), None, 2).unwrap();
        assert!(
            matches!(game.key_action('g'), Action::Pickup { item: selected } if selected == item)
        );
        game.pending_action = Some(game.key_action('g'));
        game.update();
        assert_eq!(game.turn, 1);
        let gun = game.spawn("item.glock", None, Some(id), 1).unwrap();
        game.equip_auto(id, gun).unwrap();
        assert!(matches!(game.key_action('f'), Action::Message { .. }));
        game.pending_action = Some(game.key_action('r'));
        game.update();
        assert_eq!(game.turn, 2);
        let target = game
            .spawn("monster.ghoul_worker", Some(p.offset(3, 0)), None, 1)
            .unwrap();
        assert!(matches!(game.key_action('f'), Action::OpenTargeting));
        game.pending_action = Some(game.key_action('f'));
        game.update();
        assert_eq!(game.turn, 2);
        assert!(
            matches!(game.panel, Panel::Targeting { active_entity } if active_entity == target)
        );
        game.pending_action = Some(Action::FireSelected);
        game.update();
        assert_eq!(game.turn, 3);
        assert!(matches!(game.entities[&gun], Entity::Item(ref i) if i.loaded_ammo == 1));
        game.damage(id, id, u32::MAX).unwrap();
        game.events();
        game.update();
        assert!(game.running && game.entities[&id].alive());
        assert!(matches!(game.entities[&gun], Entity::Item(ref i) if i.loaded_ammo == 1));
    }
}
