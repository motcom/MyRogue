//! 射撃・投擲の表示用アニメーション。戦闘の結果と乱数は変更しない。
use crate::{
    config::{BlastKind, Pos},
    entity::Id,
    game::Game,
};
use bracket_lib::prelude as bl;
use std::collections::BTreeSet;

const IMPACT_MS: f32 = 90.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Style {
    Bullet,
    Bolt,
}

#[derive(Clone)]
pub(crate) struct Projectile {
    sound_started: bool,
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

impl Projectile {
    fn travel_ms(&self) -> f32 {
        if self.blast.is_some() {
            return (self.path.len() as f32 * 65.0).clamp(180.0, 500.0);
        }
        match self.style {
            Style::Bullet => 120.0,
            Style::Bolt => (self.path.len() as f32 * 45.0).clamp(90.0, 450.0),
        }
    }

    fn duration_ms(&self) -> f32 {
        self.travel_ms()
            + if self.blast.is_some() {
                500.0
            } else {
                IMPACT_MS
            }
    }

    fn cells(&self) -> Vec<(Pos, char, bl::RGB)> {
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
            // 銃は短い光の尾、矢は一マスの矢印を進める。
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
    pub(crate) fn projectile(
        &self,
        actor: Id,
        target: Id,
        weapon: Id,
        hit: bool,
    ) -> Option<Projectile> {
        let w = self.definition(weapon).item.as_ref()?.weapon.as_ref()?;
        if w.mode != "ranged" {
            return None;
        }
        let (origin, end) = (self.position(actor)?, self.position(target)?);
        let visible_at_firing = self.visible_positions();
        let path: Vec<_> = bl::line2d(
            bl::LineAlg::Bresenham,
            bl::Point::new(origin.x, origin.y),
            bl::Point::new(end.x, end.y),
        )
        .into_iter()
        .skip(1)
        .map(|p| Pos {
            x: p.x,
            y: p.y,
            z: origin.z,
        })
        .collect();
        if path.is_empty() || !path.iter().any(|p| visible_at_firing.contains(p)) {
            return None;
        }
        Some(Projectile {
            sound_started: false,
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
        kind: BlastKind,
        radius: u32,
    ) -> Option<Projectile> {
        let visible_at_firing = self.visible_positions();
        let mut path: Vec<_> = bl::line2d(
            bl::LineAlg::Bresenham,
            bl::Point::new(origin.x, origin.y),
            bl::Point::new(center.x, center.y),
        )
        .into_iter()
        .skip(1)
        .map(|p| Pos {
            x: p.x,
            y: p.y,
            z: origin.z,
        })
        .collect();
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
        Some(Projectile {
            sound_started: false,
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
    pub(crate) fn update_frame(&mut self, elapsed_ms: f32) {
        self.advance_projectiles(elapsed_ms);
        self.update();
        let mut sounds = Vec::new();
        if let Some(projectile) = self.projectiles.front_mut() {
            if !projectile.sound_started {
                projectile.sound_started = true;
                sounds.push(if projectile.blast.is_some() {
                    crate::sound::Sound::Throw
                } else {
                    match projectile.style {
                        Style::Bullet => crate::sound::Sound::Gunshot,
                        Style::Bolt => crate::sound::Sound::Crossbow,
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
                    BlastKind::Explosive => crate::sound::Sound::Explosion,
                    BlastKind::Flash => crate::sound::Sound::FlashBang,
                });
            }
        }
        for sound in sounds {
            self.queue_sound(sound);
        }
    }

    pub(crate) fn advance_projectiles(&mut self, elapsed_ms: f32) {
        if !elapsed_ms.is_finite() || elapsed_ms <= 0.0 {
            return;
        }
        if let Some(projectile) = self.projectiles.front_mut() {
            // 重いフレームでも飛翔・着弾の表示を丸ごと飛ばさない。
            projectile.elapsed_ms += elapsed_ms.min(50.0);
            if projectile.elapsed_ms >= projectile.duration_ms() {
                self.projectiles.pop_front();
            }
        }
        // 次の射撃は次フレームから開始し、一フレームの遅延で飛ばさない。
    }

    pub(crate) fn render_projectiles(&self, ctx: &mut bl::BTerm, visible: &BTreeSet<Pos>) {
        if let Some(projectile) = self.projectiles.front() {
            for (p, glyph, color) in projectile.cells() {
                if visible.contains(&p) {
                    let background = if projectile.blast.is_some()
                        && projectile.elapsed_ms >= projectile.travel_ms()
                    {
                        color * 0.22
                    } else {
                        bl::RGB::named(bl::BLACK)
                    };
                    if let Some(p) = self.map_draw_position(p) {
                        ctx.set(p.x, p.y, color, background, bl::to_cp437(glyph));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{combat::Rng, entity::Entity, scripting::Action};

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
    fn throws_fly_then_expand_and_sound_once_without_repeating_gameplay() {
        use crate::sound::Sound;
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
            let projectile = &game.projectiles[0];
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
            while !game.projectiles.is_empty() {
                game.input(&context(bl::VirtualKeyCode::Period));
                assert!(game.pending_action.is_none());
                game.update_frame(25.0);
                if let Some(projectile) = game.projectiles.front() {
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
        let (mut game, item, origin) = throw_setup("item.frag_grenade");
        let center = origin.offset(3, 0);
        let wall = game
            .spawn("obstacle.wooden_wall", Some(center.offset(1, 0)), None, 1)
            .unwrap();
        game.throw_item(game.player_id, item, center).unwrap();
        game.remove(wall);
        let projectile = game.projectiles.front_mut().unwrap();
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
            game.thrown_projectile(hidden, hidden, BlastKind::Explosive, 2)
                .is_none()
        );
    }

    #[test]
    fn failed_throw_stays_silent_and_throwing_at_feet_animates_then_allows_quitting() {
        use crate::sound::Sound;
        let (mut game, item, origin) = throw_setup("item.flash_grenade");
        assert!(
            game.throw_item(game.player_id, item, origin.offset(7, 0))
                .is_err()
        );
        assert!(game.projectiles.is_empty());
        assert_eq!(game.take_sounds().count(), 0);
        assert!(matches!(&game.entities[&item], Entity::Item(i) if i.quantity == 2));
        game.throw_item(game.player_id, item, origin).unwrap();
        assert_eq!(game.projectiles[0].path, vec![origin]);
        game.update_frame(0.0);
        assert_eq!(game.take_sounds().collect::<Vec<_>>(), vec![Sound::Throw]);
        let mut quit = context(bl::VirtualKeyCode::Q);
        quit.control = true;
        game.input(&quit);
        game.update_frame(16.0);
        assert!(!game.running);
    }

    #[test]
    fn console_shows_whole_blast_with_colored_background_and_respects_current_view() {
        let (mut game, item, origin) = throw_setup("item.frag_grenade");
        let center = origin.offset(4, 0);
        game.throw_item(game.player_id, item, center).unwrap();
        let projectile = game.projectiles.front_mut().unwrap();
        projectile.elapsed_ms = projectile.travel_ms() + 200.0;
        let mut ctx = context(bl::VirtualKeyCode::F);
        ctx.active_console = ctx.register_console(bl::SimpleConsole::init(80, 50), 0);
        game.render_projectiles(&mut ctx, &game.visible_positions());
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
        game.render_projectiles(&mut ctx, &BTreeSet::new());
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
            let projectile = game.projectiles.front().unwrap();
            assert_eq!(projectile.style, style);
            assert_eq!(projectile.path.first(), Some(&origin.offset(1, 0)));
            assert_eq!(projectile.path.last(), Some(&origin.offset(4, 0)));
            assert_eq!(projectile.cells()[0].0, origin.offset(1, 0));
            assert_eq!(
                game.take_sounds().collect::<Vec<_>>(),
                vec![match style {
                    Style::Bullet => crate::sound::Sound::Gunshot,
                    Style::Bolt => crate::sound::Sound::Crossbow,
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
            while !game.projectiles.is_empty() {
                game.input(&context(bl::VirtualKeyCode::Period));
                assert!(game.pending_action.is_none());
                game.update_frame(30.0);
                if let Some(projectile) = game.projectiles.front() {
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
        let (mut game, gun, target) = setup("item.glock", "ammo.pistol");
        assert!(game.shoot(game.player_id, Id::MAX).is_err());
        assert!(game.projectiles.is_empty());
        assert_eq!(game.take_sounds().count(), 0);
        let seed = (1..1000).find(|s| Rng::new(*s).roll(20) == 1).unwrap();
        game.rng = Rng::new(seed);
        game.shoot(game.player_id, target).unwrap();
        assert!(!game.projectiles[0].hit);
        game.update_frame(0.0);
        assert_eq!(
            game.take_sounds().collect::<Vec<_>>(),
            vec![crate::sound::Sound::Gunshot]
        );
        let p = game.projectiles.front_mut().unwrap();
        p.elapsed_ms = p.travel_ms();
        assert_eq!(p.cells()[0].1, 'x');
        game.projectiles.clear();
        assert!(game.shoot(game.player_id, target).is_ok());
        game.projectiles.clear();
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
        assert!(game.projectiles.is_empty());
        assert_eq!(
            game.take_sounds().collect::<Vec<_>>(),
            vec![crate::sound::Sound::Melee]
        );
        assert!(matches!(&game.entities[&gun], Entity::Item(item) if item.loaded_ammo == 8));
    }

    #[test]
    fn hidden_paths_stay_hidden_and_impact_survives_target_removal() {
        let (mut game, gun, target) = setup("item.glock", "ammo.pistol");
        let player = game.player_id;
        let end = game.position(target).unwrap();
        let mut projectile = game.projectile(player, target, gun, true).unwrap();
        projectile.visible_at_firing = BTreeSet::from([end]);
        assert!(projectile.cells().is_empty());
        projectile.elapsed_ms = projectile.travel_ms();
        game.remove(target);
        assert_eq!(projectile.cells()[0].0, end);
        assert_eq!(projectile.cells()[0].1, '*');
        game.projectiles.push_back(projectile.clone());
        projectile.elapsed_ms = 0.0;
        game.projectiles.push_back(projectile);
        game.advance_projectiles(f32::NAN);
        assert_eq!(game.projectiles.len(), 2);
        for _ in 0..2 {
            game.advance_projectiles(1000.0);
        }
        assert_eq!(game.projectiles.len(), 1);
        // 次の射撃へ余った時間を持ち越さない。
        assert_eq!(game.projectiles[0].elapsed_ms, 0.0);
    }

    #[test]
    fn enemy_bolts_use_same_animation_and_control_q_still_quits() {
        let (mut game, _, _) = setup("item.glock", "ammo.pistol");
        let p = game.position(game.player_id).unwrap();
        let enemy = game
            .spawn("monster.ghoul_archer", Some(p.offset(0, -3)), None, 1)
            .unwrap();
        game.reload(enemy).unwrap();
        game.shoot(enemy, game.player_id).unwrap();
        assert_eq!(game.projectiles[0].style, Style::Bolt);
        assert_eq!(game.projectiles[0].cells()[0].1, 'v');
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
        use crate::sound::Sound;
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
        while game.projectiles.len() == 2 {
            game.update_frame(50.0);
        }
        assert_eq!(game.take_sounds().collect::<Vec<_>>(), vec![Sound::Gunshot]);
        while !game.projectiles.is_empty() {
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
        let (mut game, gun, target) = setup("item.crossbow", "ammo.bolt");
        let player = game.player_id;
        let mut projectile = game.projectile(player, target, gun, true).unwrap();
        projectile.elapsed_ms = projectile.travel_ms() / 2.0;
        let (p, glyph, _) = projectile.cells()[0];
        game.projectiles.push_back(projectile);
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
        game.render_projectiles(&mut ctx, &BTreeSet::new());
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
            for (offset, ch) in "2D6".chars().enumerate() {
                assert_eq!(
                    layer.get(12 + offset, 13).unwrap().ch,
                    bl::to_cp437(ch) as u32
                );
            }
            assert_eq!(layer.get(16, 13).unwrap().ch, bl::to_cp437('1') as u32);
            assert_eq!(layer.get(17, 13).unwrap().ch, bl::to_cp437('0') as u32);
        }
        game.panel = crate::game::Panel::Equipment { item: gun };
        game.render(&mut ctx);
        {
            let layer = bl::BACKEND_INTERNAL.lock().consoles[ctx.active_console]
                .console
                .to_xp_layer();
            let chars: Vec<_> = include_str!("assets/japanese_chars.txt").chars().collect();
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
        game.actor_mut(player)
            .unwrap()
            .body
            .insert("left_arm".into(), crate::entity::InjuryStage::Bleeding);
        game.actor_mut(player).unwrap().body.insert(
            "right_leg".into(),
            crate::entity::InjuryStage::HeavyBleeding,
        );
        game.panel = crate::game::Panel::Bandage { item: bandage };
        game.render(&mut ctx);
        {
            let layer = bl::BACKEND_INTERNAL.lock().consoles[ctx.active_console]
                .console
                .to_xp_layer();
            let chars: Vec<_> = include_str!("assets/japanese_chars.txt").chars().collect();
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
        game.actor_mut(player)
            .unwrap()
            .body
            .insert("left_arm".into(), crate::entity::InjuryStage::Healthy);
        game.actor_mut(player)
            .unwrap()
            .body
            .insert("right_leg".into(), crate::entity::InjuryStage::Healthy);
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
