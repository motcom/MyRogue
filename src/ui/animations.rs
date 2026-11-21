//! 射撃・投擲の表示用アニメーション。戦闘の結果と乱数は変更しない。
use super::melee::MeleeMotion;
use crate::{
    game::Game,
    model::config::{BlastKind, Pos},
    model::entity::Id,
};
use bracket_lib::prelude as bl;
use std::{cmp::Reverse, collections::BTreeSet};

const IMPACT_MS: f32 = 90.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Style {
    Bullet,
    Bolt,
}

#[derive(Clone)]
pub(crate) struct Animation {
    sound_started: bool,
    pub(crate) agility: u32,
    batch: u64,
    pub(super) melee: Option<MeleeMotion>,
    blast: Option<Blast>,
    origin: Pos,
    path: Vec<Pos>,
    visible_at_firing: BTreeSet<Pos>,
    style: Style,
    hit: bool,
    elapsed_ms: f32,
}

#[derive(Clone)]
struct Blast {
    kind: BlastKind,
    cells: Vec<Pos>,
    sound_started: bool,
}

impl Animation {
    fn travel_ms(&self) -> f32 {
        if self.melee.is_some() {
            return 160.0;
        }
        if self.blast.is_some() {
            return (self.path.len() as f32 * 65.0).clamp(180.0, 500.0);
        }
        match self.style {
            Style::Bullet => 120.0,
            Style::Bolt => (self.path.len() as f32 * 45.0).clamp(90.0, 450.0),
        }
    }

    fn duration_ms(&self) -> f32 {
        if self.melee.is_some() {
            return self.travel_ms();
        }
        self.travel_ms()
            + if self.blast.is_some() {
                500.0
            } else {
                IMPACT_MS
            }
    }

    fn effect_frame(&self, position: Pos) -> Option<(&'static str, usize)> {
        if self.melee.is_some() {
            return None;
        }
        if self.blast.is_none() {
            if self.elapsed_ms >= self.travel_ms() {
                let frame = ((self.elapsed_ms - self.travel_ms()) / (IMPACT_MS / 4.0)) as usize;
                return Some((
                    if self.hit {
                        "impact.hit"
                    } else {
                        "impact.miss"
                    },
                    frame.min(3),
                ));
            }
            let end = *self.path.last()?;
            let direction = match (
                (end.x - self.origin.x).signum(),
                (end.y - self.origin.y).signum(),
            ) {
                (1, 0) => 0,
                (0, 1) => 1,
                (-1, 0) => 2,
                (0, -1) => 3,
                (1, 1) => 4,
                (-1, 1) => 5,
                (-1, -1) => 6,
                (1, -1) => 7,
                _ => 0,
            };
            let animation = match self.style {
                Style::Bullet => "shot.bullet",
                Style::Bolt => "shot.bolt",
            };
            return Some((animation, direction));
        }
        let blast = self.blast.as_ref()?;
        let age = self.elapsed_ms - self.travel_ms();
        let (animation, frame) = if age < 0.0 {
            (
                match blast.kind {
                    BlastKind::Explosive => "throw.explosive",
                    BlastKind::Flash => "throw.flash",
                },
                (self.elapsed_ms / 65.0) as usize % 4,
            )
        } else {
            let center = *self.path.last()?;
            let local_age = (age - center.distance(position) as f32 * 65.0).max(0.0);
            (
                match blast.kind {
                    BlastKind::Explosive => "burst.explosive",
                    BlastKind::Flash => "burst.flash",
                },
                (local_age / 90.0) as usize,
            )
        };
        Some((animation, frame.min(3)))
    }

    fn cells(&self) -> Vec<(Pos, char, bl::RGB)> {
        if self.melee.is_some() {
            return Vec::new();
        }
        let end = *self.path.last().unwrap();
        if let Some(blast) = &self.blast {
            let cells = if self.elapsed_ms < self.travel_ms() {
                let head = ((self.elapsed_ms / self.travel_ms()) * self.path.len() as f32) as usize;
                vec![(
                    self.path[head.min(self.path.len() - 1)],
                    'o',
                    bl::RGB::named(bl::GREEN),
                )]
            } else {
                let age = self.elapsed_ms - self.travel_ms();
                let ring = (age / 65.0) as i32;
                let fade = (1.0 - age / 700.0).max(0.25);
                blast
                    .cells
                    .iter()
                    .filter(|p| end.distance(**p) <= ring)
                    .map(|p| {
                        let color = match blast.kind {
                            BlastKind::Explosive if end.distance(*p) == ring => bl::YELLOW,
                            BlastKind::Explosive => bl::ORANGE,
                            BlastKind::Flash if end.distance(*p) == ring => bl::WHITE,
                            BlastKind::Flash => bl::CYAN,
                        };
                        (
                            *p,
                            if blast.kind == BlastKind::Explosive {
                                '*'
                            } else {
                                '+'
                            },
                            bl::RGB::named(color) * fade,
                        )
                    })
                    .collect()
            };
            return cells
                .into_iter()
                .filter(|(p, _, _)| self.visible_at_firing.contains(p))
                .collect();
        }
        let cells = if self.elapsed_ms >= self.travel_ms() {
            vec![(
                end,
                if self.hit { '*' } else { 'x' },
                bl::RGB::named(if self.hit { bl::ORANGE } else { bl::GRAY }),
            )]
        } else {
            let head = ((self.elapsed_ms / self.travel_ms()) * self.path.len() as f32) as usize;
            let head = head.min(self.path.len() - 1);
            let start = self.origin;
            let dx = end.x - start.x;
            let dy = end.y - start.y;
            let glyph = match self.style {
                Style::Bolt if dx.abs() >= dy.abs() => {
                    if dx >= 0 {
                        '>'
                    } else {
                        '<'
                    }
                }
                Style::Bolt => {
                    if dy >= 0 {
                        'v'
                    } else {
                        '^'
                    }
                }
                Style::Bullet if dy == 0 => '-',
                Style::Bullet if dx == 0 => '|',
                Style::Bullet if dx.signum() == dy.signum() => '\\',
                Style::Bullet => '/',
            };
            let color = bl::RGB::named(match self.style {
                Style::Bullet => bl::YELLOW,
                Style::Bolt => bl::CYAN,
            });
            // 銃は短い光の尾を引き、矢は一マスずつ進む。
            let tail = if self.style == Style::Bullet {
                head.saturating_sub(2)
            } else {
                head
            };
            self.path[tail..=head]
                .iter()
                .map(|p| (*p, glyph, color))
                .collect()
        };
        cells
            .into_iter()
            .filter(|(p, _, _)| self.visible_at_firing.contains(p))
            .collect()
    }
}

impl Game {
    pub(crate) fn melee_animation(&self, actor: Id, target: Id) -> Option<Animation> {
        let origin = self.position(actor)?;
        let destination = self.position(target)?;
        let visible = self.visible_positions();
        if !visible.contains(&origin) {
            return None;
        }
        Some(Animation {
            sound_started: false,
            agility: self.actor(actor)?.subayasa,
            batch: self.animation_batch,
            melee: Some(MeleeMotion::new(
                self.entities.get(&actor)?.clone(),
                destination,
            )),
            blast: None,
            origin,
            path: vec![destination],
            visible_at_firing: visible,
            style: Style::Bullet,
            hit: true,
            elapsed_ms: 0.0,
        })
    }

    pub(crate) fn moving_actor(&self) -> Option<(&MeleeMotion, Pos, f32)> {
        let animation = self.animations.front()?;
        Some((
            animation.melee.as_ref()?,
            animation.origin,
            animation.elapsed_ms,
        ))
    }

    pub(crate) fn projectile(
        &self,
        actor: Id,
        weapon: Id,
        path: Vec<Pos>,
        hit: bool,
    ) -> Option<Animation> {
        let w = self.definition(weapon).item.as_ref()?.weapon.as_ref()?;
        if w.mode != "ranged" {
            return None;
        }
        let origin = self.position(actor)?;
        let visible_at_firing = self.visible_positions();
        if path.is_empty() || !path.iter().any(|p| visible_at_firing.contains(p)) {
            return None;
        }
        Some(Animation {
            sound_started: false,
            agility: self.actor(actor).map_or(0, |a| a.subayasa),
            batch: self.animation_batch,
            melee: None,
            blast: None,
            origin,
            path,
            visible_at_firing,
            style: if w.ammo_definition_id.as_deref() == Some("ammo.bolt") {
                Style::Bolt
            } else {
                Style::Bullet
            },
            hit,
            elapsed_ms: 0.0,
        })
    }

    /// Snapshot coverage before damage destroys walls or actors.
    pub(crate) fn thrown_projectile(
        &self,
        origin: Pos,
        center: Pos,
        mut path: Vec<Pos>,
        kind: BlastKind,
        radius: u32,
    ) -> Option<Animation> {
        let visible_at_firing = self.visible_positions();
        // Throwing at one's own feet still has a launch and an impact phase.
        if path.is_empty() {
            path.push(center);
        }
        let radius = radius as i32;
        let cells: Vec<_> = (-radius..=radius)
            .flat_map(|dy| (-radius..=radius).map(move |dx| center.offset(dx, dy)))
            .filter(|p| self.contains(*p) && self.sight(center, *p))
            .collect();
        if !path
            .iter()
            .chain(cells.iter())
            .any(|p| visible_at_firing.contains(p))
        {
            return None;
        }
        Some(Animation {
            sound_started: false,
            agility: 0,
            batch: self.animation_batch,
            melee: None,
            blast: Some(Blast {
                kind,
                cells,
                sound_started: false,
            }),
            origin,
            path,
            visible_at_firing,
            style: Style::Bullet,
            hit: true,
            elapsed_ms: 0.0,
        })
    }

    /// input → update → renderを保ち、演出だけを実時間で進める。
    pub fn update_frame(&mut self, elapsed_ms: f32) {
        self.advance_animations(elapsed_ms);
        self.update();
        self.order_animations();
        let mut sounds = Vec::new();
        if let Some(projectile) = self.animations.front_mut() {
            if !projectile.sound_started {
                projectile.sound_started = true;
                sounds.push(if projectile.melee.is_some() {
                    crate::media::sound::Sound::Melee
                } else if projectile.blast.is_some() {
                    crate::media::sound::Sound::Throw
                } else {
                    match projectile.style {
                        Style::Bullet => crate::media::sound::Sound::Gunshot,
                        Style::Bolt => crate::media::sound::Sound::Crossbow,
                    }
                });
            }
            let landed = projectile.elapsed_ms >= projectile.travel_ms();
            if let Some(blast) = &mut projectile.blast
                && landed
                && !blast.sound_started
            {
                blast.sound_started = true;
                sounds.push(match blast.kind {
                    BlastKind::Explosive => crate::media::sound::Sound::Explosion,
                    BlastKind::Flash => crate::media::sound::Sound::FlashBang,
                });
            }
        }
        for sound in sounds {
            self.queue_sound(sound);
        }
    }

    fn order_animations(&mut self) {
        // A running motion stays in place. Stable sorting preserves ties.
        let frozen = usize::from(
            self.animations
                .front()
                .is_some_and(|a| a.sound_started || a.elapsed_ms > 0.0),
        );
        self.animations.make_contiguous()[frozen..].sort_by_key(|a| (a.batch, Reverse(a.agility)));
    }

    pub(crate) fn advance_animations(&mut self, elapsed_ms: f32) {
        self.order_animations();
        if !elapsed_ms.is_finite() || elapsed_ms <= 0.0 {
            return;
        }
        if let Some(projectile) = self.animations.front_mut() {
            // 重いフレームでも飛翔・着弾の表示を丸ごと飛ばさない。
            projectile.elapsed_ms += elapsed_ms.min(50.0);
            if projectile.elapsed_ms >= projectile.duration_ms() {
                self.animations.pop_front();
            }
        }
        // 次の射撃は次フレームから開始し、一フレームの遅延で飛ばさない。
    }

    pub(crate) fn render_animations(&self, ctx: &mut bl::BTerm, visible: &BTreeSet<Pos>) {
        if let Some((motion, origin, elapsed)) = self.moving_actor() {
            self.render_melee(ctx, visible, motion, origin, elapsed);
        }
        if let Some(projectile) = self.animations.front() {
            let cells = projectile.cells();
            let head = cells.last().map(|c| c.0);
            for (p, glyph, color) in cells {
                if self.texture_renderer.is_some()
                    && projectile.blast.is_none()
                    && projectile.style == Style::Bullet
                    && projectile.elapsed_ms < projectile.travel_ms()
                    && Some(p) != head
                {
                    continue;
                }
                if visible.contains(&p) {
                    let background = if projectile.blast.is_some()
                        && projectile.elapsed_ms >= projectile.travel_ms()
                    {
                        color * 0.22
                    } else {
                        bl::RGB::named(bl::BLACK)
                    };
                    if let Some(screen) = self.map_draw_position(p) {
                        if let Some(textures) = &self.texture_renderer
                            && let Some((animation, frame)) = projectile.effect_frame(p)
                            && textures.render_effect(ctx, screen, animation, frame)
                        {
                            continue;
                        }
                        ctx.set(screen.x, screen.y, color, background, bl::to_cp437(glyph));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::combat::Rng, model::entity::Entity, model::map::line_cells, scripting::Action,
    };

    fn test_shot(game: &Game, target: Id, weapon: Id) -> Animation {
        let actor = game.player_id;
        let origin = game.position(actor).unwrap();
        let end = game.position(target).unwrap();
        game.projectile(actor, weapon, line_cells(origin, end).collect(), true)
            .unwrap()
    }

    fn throw_setup(definition: &str) -> (Game, Id, Pos) {
        let mut game = Game::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let player = game.player_id;
        let ids: Vec<_> = game
            .entities
            .keys()
            .copied()
            .filter(|id| *id != player)
            .collect();
        for id in ids {
            game.remove(id);
        }
        let origin = Pos { x: 10, y: 15, z: 0 };
        while game.position(player).unwrap() != origin {
            let p = game.position(player).unwrap();
            game.move_to(
                player,
                p.offset((origin.x - p.x).signum(), (origin.y - p.y).signum()),
            )
            .unwrap();
        }
        let item = game.spawn(definition, None, Some(player), 2).unwrap();
        (game, item, origin)
    }

    #[test]
    fn agility_orders_player_and_enemy_motions_and_audio_without_replaying_combat() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        let (mut game, gun, target) = setup("item.glock", "ammo.pistol");
        let player = game.player_id;
        let p = game.position(player).unwrap();
        let fast = game
            .spawn("monster.ghoul_worker", Some(p.offset(1, 0)), None, 1)
            .unwrap();
        let tied = game
            .spawn("monster.ghoul_worker", Some(p.offset(0, 1)), None, 1)
            .unwrap();
        game.actor_mut(player).unwrap().subayasa = 5;
        game.actor_mut(fast).unwrap().subayasa = 30;
        game.actor_mut(tied).unwrap().subayasa = 30;
        game.attack(player, target, Some(gun)).unwrap();
        game.attack(fast, player, None).unwrap();
        game.attack(tied, player, None).unwrap();
        assert_eq!(game.animations.len(), 3);
        assert_eq!(game.take_sounds().count(), 0);
        let snapshots: Vec<_> = game
            .entities
            .keys()
            .map(|id| (*id, game.entity_snapshot(*id).unwrap()))
            .collect();
        let mut rng = game.rng.clone();
        game.update_frame(0.0);
        let mut sounds: Vec<_> = game.take_sounds().collect();
        for expected in [fast, tied] {
            assert_eq!(game.moving_actor().unwrap().0.actor.id(), expected);
            let length = game.animations.len();
            while game.animations.len() == length {
                game.update_frame(25.0);
                sounds.extend(game.take_sounds());
            }
        }
        assert!(game.animations[0].melee.is_none());
        assert_eq!(game.animations[0].origin, p);
        while !game.animations.is_empty() {
            game.update_frame(25.0);
            sounds.extend(game.take_sounds());
        }
        assert_eq!(
            sounds,
            vec![
                crate::media::sound::Sound::Melee,
                crate::media::sound::Sound::Melee,
                crate::media::sound::Sound::Gunshot
            ]
        );
        assert_eq!(game.turn, 0);
        assert_eq!(game.rng.roll(100000), rng.roll(100000));
        for (id, snapshot) in snapshots {
            assert_eq!(game.entity_snapshot(id).unwrap(), snapshot);
        }
    }

    #[test]
    fn earlier_turns_and_started_motions_stay_ahead_of_faster_later_attacks() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        let (mut game, _, target) = setup("item.glock", "ammo.pistol");
        let mut old = game.melee_animation(game.player_id, target).unwrap();
        old.agility = 1;
        let mut later = old.clone();
        later.agility = 100;
        later.batch += 1;
        game.animations.push_back(later);
        game.animations.push_back(old);
        game.update_frame(0.0);
        assert_eq!(game.animations[0].agility, 1);
        let mut newly_added = game.animations[0].clone();
        newly_added.sound_started = false;
        newly_added.agility = 200;
        game.animations.push_back(newly_added);
        game.update_frame(25.0);
        assert_eq!(game.animations[0].agility, 1);
        assert_eq!(game.animations[1].agility, 200);
        let mut ctx = context(bl::VirtualKeyCode::Period);
        game.input(&ctx);
        assert!(game.pending_action.is_none());
        ctx.key = Some(bl::VirtualKeyCode::Q);
        ctx.control = true;
        game.input(&ctx);
        game.update_frame(0.0);
        assert!(!game.running);
    }

    #[test]
    fn melee_lunge_uses_a_fractional_layer_and_restores_the_piece_at_all_zooms() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        use crate::media::textures::TextureCatalog;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("data/entity_tex/manifest.json")).unwrap(),
        )
        .unwrap();
        let (mut game, _, origin) = throw_setup("item.frag_grenade");
        let player = game.player_id;
        let target = game
            .spawn("monster.ghoul_worker", Some(origin.offset(1, 0)), None, 1)
            .unwrap();
        let actor_glyph = manifest["entries"]
            .as_array()
            .unwrap()
            .iter()
            .position(|e| e["id"].as_str() == Some(&game.entities[&player].script().definition_id))
            .unwrap() as u16;
        let mut ctx = context(bl::VirtualKeyCode::F);
        let world = ctx.register_console(bl::SparseConsole::init(80, 50), 0);
        let motion = ctx.register_console(bl::SparseConsole::init(80, 50), 0);
        let overlay = ctx.register_console(bl::SimpleConsole::init(80, 50), 0);
        let ui = ctx.register_console(bl::SimpleConsole::init(80, 50), 0);
        ctx.active_console = ui;
        game.set_map_consoles(world, overlay);
        game.set_motion_console(motion);
        TextureCatalog::load(root)
            .unwrap()
            .install(&mut game, world);
        game.animations
            .push_back(game.melee_animation(player, target).unwrap());
        for zoom in [1, 2, 4] {
            game.map_zoom = zoom;
            game.animations[0].elapsed_ms = 80.0;
            game.render(&mut ctx);
            let screen = game.map_draw_position(origin).unwrap();
            let index = ((49 - screen.y) * 80 + screen.x) as usize;
            {
                let backend = bl::BACKEND_INTERNAL.lock();
                let world = backend.consoles[world]
                    .console
                    .as_any()
                    .downcast_ref::<bl::SparseConsole>()
                    .unwrap();
                let layer = backend.consoles[motion]
                    .console
                    .as_any()
                    .downcast_ref::<bl::SparseConsole>()
                    .unwrap();
                assert!(
                    world
                        .tiles
                        .iter()
                        .filter(|t| t.idx == index)
                        .all(|t| t.glyph != actor_glyph)
                );
                assert!(
                    layer
                        .tiles
                        .iter()
                        .any(|t| t.idx == index && t.glyph == actor_glyph)
                );
                let dx = game.moving_actor().unwrap().0.offset(origin, 80.0).0;
                assert!(
                    (layer.offset_x - (1.0 + dx * f32::from(zoom)) * 2.0 / 80.0).abs() < 0.00001
                );
                assert_eq!(layer.scale, f32::from(zoom));
                assert!(
                    layer
                        .tiles
                        .iter()
                        .any(|t| t.fg.a == 0.0 && (t.fg.r - 0.5).abs() < 0.00001)
                );
            }
            assert_eq!(game.position(player), Some(origin));
        }
        let snapshot = game.animations[0].clone();
        game.animations.clear();
        game.render(&mut ctx);
        let screen = game.map_draw_position(origin).unwrap();
        let index = ((49 - screen.y) * 80 + screen.x) as usize;
        {
            let backend = bl::BACKEND_INTERNAL.lock();
            let world = backend.consoles[world]
                .console
                .as_any()
                .downcast_ref::<bl::SparseConsole>()
                .unwrap();
            assert!(
                world
                    .tiles
                    .iter()
                    .any(|t| t.idx == index && t.glyph == actor_glyph)
            );
        }
        // Historical attack snapshots remain renderable even if the attacker died.
        game.animations.push_back(snapshot);
        game.remove(player);
        assert!(game.moving_actor().unwrap().0.actor.alive());
        game.render_animations(&mut ctx, &BTreeSet::from([origin]));
    }

    #[test]
    fn bullet_and_bolt_sprites_follow_eight_directions_and_switch_to_impact_frames() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        let (game, gun, target) = setup("item.glock", "ammo.pistol");
        let origin = game.position(game.player_id).unwrap();
        for (direction, (dx, dy)) in [
            (1, 0),
            (0, 1),
            (-1, 0),
            (0, -1),
            (1, 1),
            (-1, 1),
            (-1, -1),
            (1, -1),
        ]
        .into_iter()
        .enumerate()
        {
            let mut shot = test_shot(&game, target, gun);
            shot.path = vec![origin.offset(dx, dy)];
            assert_eq!(
                shot.effect_frame(origin.offset(dx, dy)),
                Some(("shot.bullet", direction))
            );
            shot.style = Style::Bolt;
            assert_eq!(
                shot.effect_frame(origin.offset(dx, dy)),
                Some(("shot.bolt", direction))
            );
            shot.elapsed_ms = shot.travel_ms() + 55.0;
            assert_eq!(shot.effect_frame(origin), Some(("impact.hit", 2)));
            shot.hit = false;
            assert_eq!(shot.effect_frame(origin), Some(("impact.miss", 2)));
        }
    }

    #[test]
    fn ranged_attack_stops_at_the_first_successful_interception_and_animates_the_same_path() {
        for (weapon, ammo) in [("item.m16", "ammo.rifle"), ("item.crossbow", "ammo.bolt")] {
            let (mut game, _, origin) = throw_setup("item.frag_grenade");
            let player = game.player_id;
            let weapon = game.spawn(weapon, None, Some(player), 1).unwrap();
            game.spawn(ammo, None, Some(player), 1).unwrap();
            game.equip_auto(player, weapon).unwrap();
            game.reload(player).unwrap();
            let first_pos = origin.offset(2, 1);
            let second_pos = origin.offset(4, 2);
            let end = origin.offset(5, 2);
            let first = game
                .spawn("player.survivor", Some(first_pos), None, 1)
                .unwrap();
            let second = game
                .spawn("monster.ghoul_worker", Some(second_pos), None, 1)
                .unwrap();
            let target = game
                .spawn("monster.ghoul_worker", Some(end), None, 1)
                .unwrap();
            game.spawn("monster.ghoul_worker", Some(origin.offset(1, 1)), None, 1)
                .unwrap();
            let target_before = game.entity_snapshot(target).unwrap();
            let second_before = game.entity_snapshot(second).unwrap();
            let first_before = game.entity_snapshot(first).unwrap();
            for (blocks_first, blocks_second) in [(true, true), (false, true), (false, false)] {
                let mut probe = game.clone();
                let seed = (0..1000)
                    .find(|seed| {
                        let mut rng = Rng::new(*seed);
                        (rng.roll(100) <= 20) == blocks_first
                            && (rng.roll(100) <= 20) == blocks_second
                    })
                    .unwrap();
                probe.rng = Rng::new(seed);
                probe.attack(player, target, Some(weapon)).unwrap();
                assert!(
                    matches!(&probe.entities[&weapon], Entity::Item(item) if item.loaded_ammo == 0)
                );
                let effect = probe.animations.back().unwrap();
                let path = [
                    origin.offset(1, 0),
                    first_pos,
                    origin.offset(3, 1),
                    second_pos,
                    end,
                ];
                let n = if blocks_first {
                    2
                } else if blocks_second {
                    4
                } else {
                    5
                };
                assert_eq!(effect.path, path[..n]);
                if blocks_first || blocks_second {
                    assert!(effect.hit);
                    assert_eq!(probe.entity_snapshot(target).unwrap(), target_before);
                }
                if blocks_first {
                    assert!(probe.actor(first).unwrap().hp.hp < game.actor(first).unwrap().hp.hp);
                    assert_eq!(probe.entity_snapshot(second).unwrap(), second_before);
                } else {
                    assert_eq!(probe.entity_snapshot(first).unwrap(), first_before);
                    if blocks_second {
                        assert!(
                            probe.actor(second).unwrap().hp.hp < game.actor(second).unwrap().hp.hp
                        );
                    }
                }
            }
            game.spawn("obstacle.stone_wall", Some(origin.offset(1, 0)), None, 1)
                .unwrap();
            let mut rng = game.rng.clone();
            assert!(game.attack(player, target, Some(weapon)).is_err());
            assert_eq!(game.rng.roll(100), rng.roll(100));
            assert!(matches!(&game.entities[&weapon], Entity::Item(item) if item.loaded_ammo == 1));
        }
    }

    #[test]
    fn intercepted_throw_centers_blast_and_animation_at_the_blocking_actor() {
        let (mut game, item, origin) = throw_setup("item.frag_grenade");
        let center = origin.offset(2, 0);
        let destination = origin.offset(5, 0);
        let blocker = game
            .spawn("monster.ghoul_brute", Some(center), None, 1)
            .unwrap();
        let far = game
            .spawn("monster.ghoul_brute", Some(destination), None, 1)
            .unwrap();
        let blocker_hp = game.actor(blocker).unwrap().hp.hp;
        let far_hp = game.actor(far).unwrap().hp.hp;
        let seed = (0..100)
            .find(|seed| Rng::new(*seed).roll(100) <= 20)
            .unwrap();
        game.rng = Rng::new(seed);
        game.throw_item(game.player_id, item, destination).unwrap();
        assert_eq!(game.actor(blocker).unwrap().hp.hp, blocker_hp - 36);
        assert_eq!(game.actor(far).unwrap().hp.hp, far_hp);
        assert!(matches!(&game.entities[&item], Entity::Item(item) if item.quantity == 1));
        let effect = game.animations.back().unwrap();
        assert_eq!(effect.path, [origin.offset(1, 0), center]);
        assert!(!effect.blast.as_ref().unwrap().cells.contains(&destination));
    }

    #[test]
    fn throws_fly_then_expand_and_sound_once_without_repeating_gameplay() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        use crate::media::sound::Sound;
        for (definition, kind, impact, glyph) in [
            (
                "item.frag_grenade",
                BlastKind::Explosive,
                Sound::Explosion,
                '*',
            ),
            (
                "item.flash_grenade",
                BlastKind::Flash,
                Sound::FlashBang,
                '+',
            ),
        ] {
            let (mut game, item, origin) = throw_setup(definition);
            let center = origin.offset(4, 0);
            game.use_item(item).unwrap();
            for _ in 0..4 {
                game.move_item_target(1, 0).unwrap();
            }
            game.pending_action = Some(Action::ConfirmItemTarget);
            game.update_frame(0.0);
            assert_eq!(game.turn, 1);
            assert!(matches!(&game.entities[&item], Entity::Item(i) if i.quantity == 1));
            assert_eq!(game.take_sounds().collect::<Vec<_>>(), vec![Sound::Throw]);
            let projectile = &game.animations[0];
            assert_eq!(
                projectile.path,
                (1..=4).map(|x| origin.offset(x, 0)).collect::<Vec<_>>()
            );
            assert_eq!(projectile.blast.as_ref().unwrap().kind, kind);
            assert_eq!(projectile.cells()[0].1, 'o');
            let before: Vec<_> = game
                .entities
                .keys()
                .map(|id| (*id, game.entity_snapshot(*id).unwrap()))
                .collect();
            let mut rng = game.rng.clone();
            let mut seen = BTreeSet::new();
            let mut flight_positions = BTreeSet::new();
            while !game.animations.is_empty() {
                game.input(&context(bl::VirtualKeyCode::Period));
                assert!(game.pending_action.is_none());
                game.update_frame(25.0);
                if let Some(projectile) = game.animations.front() {
                    for (pos, ch, _) in projectile.cells() {
                        if ch == glyph {
                            seen.insert(pos);
                        }
                        if ch == 'o' {
                            flight_positions.insert(pos);
                        }
                    }
                }
                assert_eq!(game.turn, 1);
            }
            assert!(flight_positions.len() > 1);
            assert_eq!(seen.len(), 25);
            assert!(seen.contains(&center.offset(-2, -2)));
            assert!(seen.contains(&center.offset(2, 2)));
            assert!(!seen.contains(&center.offset(3, 0)));
            assert_eq!(game.take_sounds().collect::<Vec<_>>(), vec![impact]);
            assert_eq!(game.rng.roll(100), rng.roll(100));
            for (id, snapshot) in before {
                assert_eq!(game.entity_snapshot(id).unwrap(), snapshot);
            }
        }
    }

    #[test]
    fn blast_coverage_is_snapshotted_before_wall_removal_and_never_reveals_hidden_cells() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        let (mut game, item, origin) = throw_setup("item.frag_grenade");
        let center = origin.offset(3, 0);
        let wall = game
            .spawn("obstacle.wooden_wall", Some(center.offset(1, 0)), None, 1)
            .unwrap();
        game.throw_item(game.player_id, item, center).unwrap();
        game.remove(wall);
        let projectile = game.animations.front_mut().unwrap();
        let blast = projectile.blast.as_ref().unwrap();
        assert!(blast.cells.contains(&center.offset(1, 0)));
        assert!(!blast.cells.contains(&center.offset(2, 0)));
        projectile.elapsed_ms = projectile.travel_ms() + 200.0;
        let shown = center.offset(-1, 0);
        projectile.visible_at_firing = BTreeSet::from([shown]);
        assert_eq!(
            projectile.cells().iter().map(|c| c.0).collect::<Vec<_>>(),
            vec![shown]
        );
        projectile.visible_at_firing.clear();
        assert!(projectile.cells().is_empty());
        let hidden = Pos { x: 40, y: 25, z: 0 };
        assert!(
            game.thrown_projectile(hidden, hidden, Vec::new(), BlastKind::Explosive, 2)
                .is_none()
        );
    }

    #[test]
    fn failed_throw_stays_silent_and_throwing_at_feet_animates_then_allows_quitting() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        use crate::media::sound::Sound;
        let (mut game, item, origin) = throw_setup("item.flash_grenade");
        assert!(
            game.throw_item(game.player_id, item, origin.offset(7, 0))
                .is_err()
        );
        assert!(game.animations.is_empty());
        assert_eq!(game.take_sounds().count(), 0);
        assert!(matches!(&game.entities[&item], Entity::Item(i) if i.quantity == 2));
        game.throw_item(game.player_id, item, origin).unwrap();
        assert_eq!(game.animations[0].path, vec![origin]);
        game.update_frame(0.0);
        assert_eq!(game.take_sounds().collect::<Vec<_>>(), vec![Sound::Throw]);
        let mut quit = context(bl::VirtualKeyCode::Q);
        quit.control = true;
        game.input(&quit);
        game.update_frame(16.0);
        assert!(!game.running);
    }

    #[test]
    fn textured_throws_and_blasts_animate_on_the_zoomed_world_layer() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        use crate::media::textures::TextureCatalog;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("data/entity_tex/manifest.json")).unwrap(),
        )
        .unwrap();
        let frames = manifest["effects"].as_array().unwrap();
        let base = manifest["entries"].as_array().unwrap().len();
        for definition in ["item.frag_grenade", "item.flash_grenade"] {
            let (mut game, item, origin) = throw_setup(definition);
            let center = origin.offset(4, 0);
            game.throw_item(game.player_id, item, center).unwrap();
            let mut ctx = context(bl::VirtualKeyCode::F);
            let world = ctx.register_console(bl::SparseConsole::init(80, 50), 0);
            let overlay = ctx.register_console(bl::SimpleConsole::init(80, 50), 0);
            ctx.active_console = overlay;
            game.set_map_consoles(world, overlay);
            TextureCatalog::load(root)
                .unwrap()
                .install(&mut game, world);
            let travel = game.animations[0].travel_ms();
            let mut seen = BTreeSet::new();
            for zoom in [1, 2, 4] {
                game.map_zoom = zoom;
                for elapsed in [
                    0.0,
                    70.0,
                    140.0,
                    210.0,
                    travel,
                    travel + 100.0,
                    travel + 220.0,
                    travel + 350.0,
                    travel + 480.0,
                ] {
                    game.animations[0].elapsed_ms = elapsed;
                    game.render(&mut ctx);
                    assert_eq!(ctx.active_console, overlay);
                    let projectile = &game.animations[0];
                    let visible = game.visible_positions();
                    let layer = bl::BACKEND_INTERNAL.lock().consoles[world]
                        .console
                        .to_xp_layer();
                    for (position, _, _) in projectile
                        .cells()
                        .into_iter()
                        .filter(|(p, _, _)| visible.contains(p))
                    {
                        let Some(screen) = game.map_draw_position(position) else {
                            continue;
                        };
                        let (animation, frame) = projectile.effect_frame(position).unwrap();
                        let key = format!("{animation}.{frame}");
                        let index = frames
                            .iter()
                            .position(|e| e["id"].as_str() == Some(&key))
                            .unwrap()
                            + base;
                        assert_eq!(
                            layer
                                .get(screen.x as usize, (49 - screen.y) as usize)
                                .unwrap()
                                .ch,
                            index as u32
                        );
                        seen.insert(key);
                    }
                    assert_eq!(game.turn, 0);
                }
            }
            assert_eq!(seen.len(), 8);
            ctx.set_active_console(world);
            ctx.cls();
            ctx.set_active_console(overlay);
            game.render_animations(&mut ctx, &BTreeSet::new());
            let layer = bl::BACKEND_INTERNAL.lock().consoles[world]
                .console
                .to_xp_layer();
            assert!(layer.cells.iter().all(|c| c.ch == 0));
        }
    }

    #[test]
    fn console_shows_whole_blast_with_colored_background_and_respects_current_view() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        let (mut game, item, origin) = throw_setup("item.frag_grenade");
        let center = origin.offset(4, 0);
        game.throw_item(game.player_id, item, center).unwrap();
        let projectile = game.animations.front_mut().unwrap();
        projectile.elapsed_ms = projectile.travel_ms() + 200.0;
        let mut ctx = context(bl::VirtualKeyCode::F);
        ctx.active_console = ctx.register_console(bl::SimpleConsole::init(80, 50), 0);
        game.render_animations(&mut ctx, &game.visible_positions());
        {
            let layer = bl::BACKEND_INTERNAL.lock().consoles[ctx.active_console]
                .console
                .to_xp_layer();
            for dy in -2..=2 {
                for dx in -2..=2 {
                    let p = center.offset(dx, dy);
                    let cell = layer.get((p.x + 1) as usize, (p.y + 4) as usize).unwrap();
                    assert_eq!(cell.ch, bl::to_cp437('*') as u32);
                    assert!(cell.bg.r > 0);
                }
            }
        }
        ctx.cls();
        game.render_animations(&mut ctx, &BTreeSet::new());
        let layer = bl::BACKEND_INTERNAL.lock().consoles[ctx.active_console]
            .console
            .to_xp_layer();
        assert!(layer.cells.iter().all(|cell| cell.ch == 32));
    }

    fn setup(weapon: &str, ammo: &str) -> (Game, Id, Id) {
        let mut game = Game::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let enemies: Vec<_> = game
            .entities
            .iter()
            .filter(|(id, entity)| **id != game.player_id && matches!(entity, Entity::Movable(_)))
            .map(|(id, _)| *id)
            .collect();
        for id in enemies {
            game.remove(id);
        }
        let player = game.player_id;
        let gun = game.spawn(weapon, None, Some(player), 1).unwrap();
        game.spawn(ammo, None, Some(player), 10).unwrap();
        game.equip_auto(player, gun).unwrap();
        game.reload(player).unwrap();
        game.take_sounds().for_each(drop); // Discard setup reload before testing projectile sounds.
        let p = game.position(player).unwrap();
        let target = game
            .spawn("monster.ghoul_worker", Some(p.offset(4, 0)), None, 1)
            .unwrap();
        (game, gun, target)
    }

    fn context(key: bl::VirtualKeyCode) -> bl::BTerm {
        bl::BTerm {
            width_pixels: 1280,
            height_pixels: 800,
            original_height_pixels: 800,
            original_width_pixels: 1280,
            fps: 60.0,
            frame_time_ms: 16.0,
            active_console: 0,
            key: Some(key),
            mouse_pos: (0, 0),
            left_click: false,
            shift: false,
            control: false,
            alt: false,
            web_button: None,
            quitting: false,
            post_scanlines: false,
            post_screenburn: false,
            screen_burn_color: bl::RGB::named(bl::BLACK),
            mouse_visible: true,
        }
    }

    #[test]
    fn guns_and_crossbow_animate_in_order_without_more_turns_ammo_or_random_rolls() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        for (weapon, ammo, style) in [
            ("item.glock", "ammo.pistol", Style::Bullet),
            ("item.sks", "ammo.rifle", Style::Bullet),
            ("item.crossbow", "ammo.bolt", Style::Bolt),
        ] {
            let (mut game, gun, target) = setup(weapon, ammo);
            let origin = game.position(game.player_id).unwrap();
            game.pending_action = Some(Action::Shoot { target });
            game.update_frame(0.0);
            assert_eq!(game.turn, 1);
            let projectile = game.animations.front().unwrap();
            assert_eq!(projectile.style, style);
            assert_eq!(projectile.path.first(), Some(&origin.offset(1, 0)));
            assert_eq!(projectile.path.last(), Some(&origin.offset(4, 0)));
            assert_eq!(projectile.cells()[0].0, origin.offset(1, 0));
            assert_eq!(
                game.take_sounds().collect::<Vec<_>>(),
                vec![match style {
                    Style::Bullet => crate::media::sound::Sound::Gunshot,
                    Style::Bolt => crate::media::sound::Sound::Crossbow,
                }]
            );
            let before = game.entity_snapshot(gun).unwrap();
            let positions: Vec<_> = game
                .entities
                .values()
                .map(|e| (e.id(), e.map_position()))
                .collect();
            let mut rng = game.rng.clone();
            let mut saw_impact = false;
            while !game.animations.is_empty() {
                game.input(&context(bl::VirtualKeyCode::Period));
                assert!(game.pending_action.is_none());
                game.update_frame(30.0);
                if let Some(projectile) = game.animations.front() {
                    saw_impact |= projectile
                        .cells()
                        .iter()
                        .any(|(_, c, _)| matches!(c, '*' | 'x'));
                }
                assert_eq!(game.turn, 1);
            }
            assert!(saw_impact);
            assert_eq!(game.take_sounds().count(), 0);
            assert_eq!(game.entity_snapshot(gun).unwrap(), before);
            assert_eq!(game.rng.roll(100), rng.roll(100));
            assert_eq!(
                game.entities
                    .values()
                    .map(|e| (e.id(), e.map_position()))
                    .collect::<Vec<_>>(),
                positions
            );
            game.input(&context(bl::VirtualKeyCode::Period));
            assert!(matches!(game.pending_action, Some(Action::WaitTurn)));
        }
    }

    #[test]
    fn misses_animate_but_invalid_and_melee_attacks_do_not() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        let (mut game, gun, target) = setup("item.glock", "ammo.pistol");
        assert!(game.shoot(game.player_id, Id::MAX).is_err());
        assert!(game.animations.is_empty());
        assert_eq!(game.take_sounds().count(), 0);
        let seed = (1..1000).find(|s| Rng::new(*s).roll(20) == 1).unwrap();
        game.rng = Rng::new(seed);
        game.shoot(game.player_id, target).unwrap();
        assert!(!game.animations[0].hit);
        game.update_frame(0.0);
        assert_eq!(
            game.take_sounds().collect::<Vec<_>>(),
            vec![crate::media::sound::Sound::Gunshot]
        );
        let p = game.animations.front_mut().unwrap();
        p.elapsed_ms = p.travel_ms();
        assert_eq!(p.cells()[0].1, 'x');
        game.animations.clear();
        assert!(game.shoot(game.player_id, target).is_ok());
        game.animations.clear();
        let player = game.player_id;
        let knife = game.spawn("item.knife", None, Some(player), 1).unwrap();
        game.equip_auto(player, knife).unwrap();
        game.remove(target);
        let target = game
            .spawn(
                "monster.ghoul_worker",
                Some(game.position(player).unwrap().offset(0, -1)),
                None,
                1,
            )
            .unwrap();
        game.attack(player, target, Some(knife)).unwrap();
        assert!(game.animations.front().unwrap().melee.is_some());
        assert_eq!(game.take_sounds().count(), 0);
        game.update_frame(0.0);
        assert_eq!(
            game.take_sounds().collect::<Vec<_>>(),
            vec![crate::media::sound::Sound::Melee]
        );
        assert!(matches!(&game.entities[&gun], Entity::Item(item) if item.loaded_ammo == 8));
    }

    #[test]
    fn hidden_paths_stay_hidden_and_impact_survives_target_removal() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        let (mut game, gun, target) = setup("item.glock", "ammo.pistol");
        let end = game.position(target).unwrap();
        let mut projectile = test_shot(&game, target, gun);
        projectile.visible_at_firing = BTreeSet::from([end]);
        assert!(projectile.cells().is_empty());
        projectile.elapsed_ms = projectile.travel_ms();
        game.remove(target);
        assert_eq!(projectile.cells()[0].0, end);
        assert_eq!(projectile.cells()[0].1, '*');
        game.animations.push_back(projectile.clone());
        projectile.elapsed_ms = 0.0;
        game.animations.push_back(projectile);
        game.advance_animations(f32::NAN);
        assert_eq!(game.animations.len(), 2);
        for _ in 0..2 {
            game.advance_animations(1000.0);
        }
        assert_eq!(game.animations.len(), 1);
        // 次の射撃へ余った時間を持ち越さない。
        assert_eq!(game.animations[0].elapsed_ms, 0.0);
    }

    #[test]
    fn enemy_bolts_use_same_animation_and_control_q_still_quits() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        let (mut game, _, _) = setup("item.glock", "ammo.pistol");
        let p = game.position(game.player_id).unwrap();
        let enemy = game
            .spawn("monster.ghoul_archer", Some(p.offset(0, -3)), None, 1)
            .unwrap();
        game.reload(enemy).unwrap();
        game.shoot(enemy, game.player_id).unwrap();
        assert_eq!(game.animations[0].style, Style::Bolt);
        assert_eq!(game.animations[0].cells()[0].1, 'v');
        game.input(&context(bl::VirtualKeyCode::Escape));
        game.update_frame(16.0);
        assert!(game.running);
        let mut quit = context(bl::VirtualKeyCode::Q);
        quit.control = true;
        game.input(&quit);
        game.update_frame(16.0);
        assert!(!game.running);
    }

    #[test]
    fn queued_shots_start_one_sound_each_and_empty_gun_stays_silent() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        use crate::media::sound::Sound;
        let (mut game, gun, target) = setup("item.glock", "ammo.pistol");
        let player = game.player_id;
        // Miss deterministically so the target survives both shots.
        let seed = (1..1000).find(|s| Rng::new(*s).roll(20) == 1).unwrap();
        for _ in 0..2 {
            game.rng = Rng::new(seed);
            game.shoot(player, target).unwrap();
        }
        assert_eq!(game.take_sounds().count(), 0);
        game.update_frame(0.0);
        assert_eq!(game.take_sounds().collect::<Vec<_>>(), vec![Sound::Gunshot]);
        while game.animations.len() == 2 {
            game.update_frame(50.0);
        }
        assert_eq!(game.take_sounds().collect::<Vec<_>>(), vec![Sound::Gunshot]);
        while !game.animations.is_empty() {
            game.update_frame(50.0);
        }
        assert_eq!(game.take_sounds().count(), 0);
        if let Some(Entity::Item(item)) = game.entities.get_mut(&gun) {
            item.loaded_ammo = 0;
        }
        assert!(game.shoot(player, target).is_err());
        game.update_frame(0.0);
        assert_eq!(game.take_sounds().count(), 0);
    }

    #[test]
    fn console_renders_projectile_above_world_and_hides_it_outside_view() {
        let _console_guard = crate::ui::CONSOLE_TEST_LOCK.lock().unwrap();
        let (mut game, gun, target) = setup("item.crossbow", "ammo.bolt");
        let player = game.player_id;
        let mut projectile = test_shot(&game, target, gun);
        projectile.elapsed_ms = projectile.travel_ms() / 2.0;
        let (p, glyph, _) = projectile.cells()[0];
        game.animations.push_back(projectile);
        let mut ctx = context(bl::VirtualKeyCode::F);
        ctx.active_console = ctx.register_console(bl::SimpleConsole::init(80, 50), 0);
        game.render(&mut ctx);
        let layer = bl::BACKEND_INTERNAL.lock().consoles[ctx.active_console]
            .console
            .to_xp_layer();
        assert_eq!(
            layer
                .get((p.x + 1) as usize, (p.y + 4) as usize)
                .unwrap()
                .ch,
            bl::to_cp437(glyph) as u32
        );
        ctx.cls();
        game.render_animations(&mut ctx, &BTreeSet::new());
        let layer = bl::BACKEND_INTERNAL.lock().consoles[ctx.active_console]
            .console
            .to_xp_layer();
        assert!(layer.cells.iter().all(|cell| cell.ch == 32));
        // Weapon details expose the Lua critical threshold on a separate row.
        ctx.cls();
        game.render_item_details(&mut ctx, gun);
        {
            let layer = bl::BACKEND_INTERNAL.lock().consoles[ctx.active_console]
                .console
                .to_xp_layer();
            for (offset, ch) in "3D6".chars().enumerate() {
                assert_eq!(
                    layer.get(12 + offset, 13).unwrap().ch,
                    bl::to_cp437(ch) as u32
                );
            }
            assert_eq!(layer.get(16, 13).unwrap().ch, bl::to_cp437('1') as u32);
            assert_eq!(layer.get(17, 13).unwrap().ch, bl::to_cp437('5') as u32);
        }
        game.panel = crate::game::Panel::Equipment { item: gun };
        game.render(&mut ctx);
        {
            let layer = bl::BACKEND_INTERNAL.lock().consoles[ctx.active_console]
                .console
                .to_xp_layer();
            let chars: Vec<_> = include_str!("../assets/japanese_chars.txt")
                .chars()
                .collect();
            let row = |y| -> String {
                (5..75)
                    .map(|x| {
                        let code = layer.get(x, y).unwrap().ch;
                        if code < 256 {
                            char::from_u32(code).unwrap_or(' ')
                        } else {
                            chars[(code - 256) as usize]
                        }
                    })
                    .collect()
            };
            assert!(row(7).contains("装備"));
            assert!(row(18).contains("> 両手に装備"));
            assert!(row(19).contains("装備を外す"));
            assert!(!row(20).contains("に装備"));
        }
        let bandage = *game
            .actor(player)
            .unwrap()
            .inventory
            .items
            .iter()
            .find(|id| game.entities[id].script().definition_id == "item.bandage")
            .unwrap();
        game.actor_mut(player).unwrap().body.insert(
            "left_arm".into(),
            crate::model::entity::InjuryStage::Bleeding,
        );
        game.actor_mut(player).unwrap().body.insert(
            "right_leg".into(),
            crate::model::entity::InjuryStage::HeavyBleeding,
        );
        game.panel = crate::game::Panel::Bandage { item: bandage };
        game.render(&mut ctx);
        {
            let layer = bl::BACKEND_INTERNAL.lock().consoles[ctx.active_console]
                .console
                .to_xp_layer();
            let chars: Vec<_> = include_str!("../assets/japanese_chars.txt")
                .chars()
                .collect();
            let row = |y| -> String {
                (5..75)
                    .map(|x| {
                        let code = layer.get(x, y).unwrap().ch;
                        if code < 256 {
                            char::from_u32(code).unwrap_or(' ')
                        } else {
                            chars[(code - 256) as usize]
                        }
                    })
                    .collect()
            };
            assert!(row(11).contains("左腕: 出血"));
            assert!(row(12).contains("右脚: 大出血"));
            assert!(row(13).trim().is_empty());
            assert!(row(21).contains("100ターン"));
        }
        game.actor_mut(player).unwrap().body.insert(
            "left_arm".into(),
            crate::model::entity::InjuryStage::Healthy,
        );
        game.actor_mut(player).unwrap().body.insert(
            "right_leg".into(),
            crate::model::entity::InjuryStage::Healthy,
        );
        game.panel = crate::game::Panel::World;
        let world_console = ctx.active_console;
        let popup_console = ctx.register_console(bl::SimpleConsole::init(80, 50), 0);
        game.set_popup_console(popup_console);
        for panel in [
            crate::game::Panel::InteractionDirection,
            crate::game::Panel::InteractionMenu {
                position: game.position(player).unwrap(),
                carried: None,
            },
        ] {
            game.panel = panel;
            game.render(&mut ctx);
            assert_eq!(ctx.active_console, world_console);
            let backend = bl::BACKEND_INTERNAL.lock();
            let base = backend.consoles[world_console].console.to_xp_layer();
            assert_eq!(
                base.get((p.x + 1) as usize, (p.y + 4) as usize).unwrap().ch,
                bl::to_cp437(glyph) as u32
            );
            let popup = backend.consoles[popup_console]
                .console
                .as_any()
                .downcast_ref::<bl::SimpleConsole>()
                .unwrap();
            let background = &popup.tiles[bl::Console::at(popup, 20, 20)];
            assert_eq!(background.glyph, bl::to_cp437(' '));
            assert_eq!(background.bg.a, 0.6);
            assert_eq!(popup.tiles[bl::Console::at(popup, 5, 6)].fg.a, 1.0);
            assert_eq!(popup.tiles[bl::Console::at(popup, 5, 6)].bg.a, 0.6);
            assert_eq!(popup.tiles[0].fg.a, 0.0);
            assert_eq!(popup.tiles[0].bg.a, 0.0);
        }
        game.panel = crate::game::Panel::World;
        game.render(&mut ctx);
        {
            let backend = bl::BACKEND_INTERNAL.lock();
            let popup = backend.consoles[popup_console]
                .console
                .as_any()
                .downcast_ref::<bl::SimpleConsole>()
                .unwrap();
            assert!(
                popup
                    .tiles
                    .iter()
                    .all(|tile| tile.fg.a == 0.0 && tile.bg.a == 0.0)
            );
        }
        assert_eq!(ctx.active_console, world_console);
        bl::BACKEND_INTERNAL.lock().consoles.remove(popup_console);
        bl::BACKEND_INTERNAL
            .lock()
            .consoles
            .remove(ctx.active_console);
    }
}
