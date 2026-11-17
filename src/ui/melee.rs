//! Short presentation-only lunges. Snapshots survive combat deaths; map positions never change.
use crate::{
    game::Game,
    model::{
        config::Pos,
        entity::{Entity, Id},
    },
};
use bracket_lib::prelude as bl;
use std::collections::BTreeSet;

#[derive(Clone)]
pub(crate) struct MeleeMotion {
    pub actor: Box<Entity>,
    target: Pos,
}
impl MeleeMotion {
    pub(super) fn new(actor: Entity, target: Pos) -> Self {
        Self {
            actor: Box::new(actor),
            target,
        }
    }
    pub(crate) fn offset(&self, origin: Pos, elapsed_ms: f32) -> (f32, f32) {
        let phase = (elapsed_ms / 160.0).clamp(0.0, 1.0);
        let amount = if phase < 0.45 {
            phase / 0.45
        } else {
            (1.0 - phase) / 0.55
        } * 0.32;
        (
            ((self.target.x - origin.x).signum() as f32) * amount,
            ((self.target.y - origin.y).signum() as f32) * amount,
        )
    }
}
impl Game {
    pub fn set_motion_console(&mut self, console: usize) {
        self.motion_console = Some(console);
    }

    pub(crate) fn motion_actor_id(&self, visible: &BTreeSet<Pos>) -> Option<Id> {
        self.motion_console?;
        let (motion, origin, _) = self.moving_actor()?;
        (visible.contains(&origin) && self.map_draw_position(origin).is_some())
            .then_some(motion.actor.id())
    }

    pub(crate) fn render_melee(
        &self,
        ctx: &mut bl::BTerm,
        visible: &BTreeSet<Pos>,
        motion: &MeleeMotion,
        origin: Pos,
        elapsed: f32,
    ) {
        if !visible.contains(&origin) {
            return;
        }
        let Some(screen) = self.map_draw_position(origin) else {
            return;
        };
        let previous = ctx.active_console;
        let (dx, dy) = motion.offset(origin, elapsed);
        if let Some(console) = self.motion_console {
            ctx.set_active_console(console);
            let zoom = f32::from(self.map_zoom);
            ctx.set_offset(1.0 + dx * zoom, -4.0 - dy * zoom);
            // SparseConsole caches tiles rather than offset/scale. Make progress part of that key.
            let invisible = bl::RGBA::from_f32(elapsed / 160.0, 0.0, 0.0, 0.0);
            ctx.set(78, 49, invisible, invisible, 0);
        }
        if self.motion_console.is_some()
            && let Some(textures) = &self.texture_renderer
        {
            textures.render_actor(self, ctx, screen, &motion.actor);
        } else {
            let appearance = &self.entity_definitions[&motion.actor.script().definition_id]
                .common
                .appearance;
            let glyph = motion
                .actor
                .script()
                .glyph
                .as_deref()
                .unwrap_or(&appearance.glyph)
                .chars()
                .next()
                .unwrap_or('@');
            ctx.set(
                screen.x,
                screen.y,
                bl::RGB::from_u8(appearance.fg[0], appearance.fg[1], appearance.fg[2]),
                bl::RGBA::from_f32(0.0, 0.0, 0.0, 0.0),
                bl::to_cp437(glyph),
            );
        }
        ctx.set_active_console(previous);
    }
}
