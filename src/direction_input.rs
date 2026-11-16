//! Direction presses must survive a release between frames; held movement uses
//! game timing rather than the operating system's keyboard repeat delay.
use bracket_lib::prelude::{BEvent, VirtualKeyCode as Key};

const FIRST_REPEAT_MS: f32 = 350.0;
const REPEAT_MS: f32 = 180.0;

pub(crate) fn is_direction(key: Key) -> bool {
    matches!(
        key,
        Key::H
            | Key::J
            | Key::K
            | Key::L
            | Key::Y
            | Key::U
            | Key::B
            | Key::N
            | Key::Left
            | Key::Down
            | Key::Up
            | Key::Right
    )
}

#[derive(Clone, Default)]
pub(crate) struct DirectionInput {
    held: Vec<Key>,
    active: Option<Key>,
    remaining_ms: f32,
}

impl DirectionInput {
    pub(crate) fn key(
        &mut self,
        events: &[BEvent],
        fallback: Option<Key>,
        elapsed_ms: f32,
        enabled: bool,
    ) -> Option<Key> {
        let mut pressed = None;
        let mut direction_event = false;
        let mut focus_lost = false;
        for event in events {
            match *event {
                BEvent::Focused { focused: false } => {
                    self.held.clear();
                    self.active = None;
                    pressed = None;
                    focus_lost = true;
                }
                BEvent::KeyboardInput {
                    key, pressed: down, ..
                } if is_direction(key) => {
                    direction_event = true;
                    if down {
                        if !self.held.contains(&key) {
                            self.held.push(key);
                            if enabled && !focus_lost {
                                pressed = Some(key);
                                self.active = Some(key);
                                self.remaining_ms = FIRST_REPEAT_MS;
                            }
                        }
                    } else {
                        self.held.retain(|held| *held != key);
                        if self.active == Some(key) {
                            // Releasing a key must never produce another move.
                            self.active = None;
                        }
                    }
                }
                _ => {}
            }
        }
        if !enabled {
            self.active = None;
            return fallback;
        }
        if focus_lost {
            return fallback.filter(|key| !is_direction(*key));
        }
        if pressed.is_some() {
            return pressed;
        }
        // Menus, shortcuts and quit take precedence over held movement.
        if fallback.is_some_and(|key| !is_direction(key)) {
            self.active = None;
            return fallback;
        }
        if let Some(key) = self.active {
            let elapsed = if elapsed_ms.is_finite() {
                elapsed_ms.clamp(0.0, 100.0)
            } else {
                0.0
            };
            self.remaining_ms -= elapsed;
            if self.remaining_ms <= 0.0 {
                // At most one action per frame; never catch up a backlog of moves.
                self.remaining_ms = REPEAT_MS;
                return Some(key);
            }
            return None;
        }
        if direction_event || !self.held.is_empty() {
            None
        } else {
            // Supports callers that provide only BTerm::key (headless input tests).
            fallback
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(key: Key, pressed: bool) -> BEvent {
        BEvent::KeyboardInput {
            key,
            scan_code: 0,
            pressed,
        }
    }
    #[test]
    fn tap_is_preserved_when_released_before_the_frame() {
        let mut input = DirectionInput::default();
        assert_eq!(
            input.key(
                &[event(Key::Right, true), event(Key::Right, false)],
                None,
                16.0,
                true
            ),
            Some(Key::Right)
        );
        assert_eq!(input.key(&[], None, 200.0, true), None);
    }
    #[test]
    fn hold_repeats_without_os_events_and_ignores_os_repeat() {
        let mut input = DirectionInput::default();
        assert_eq!(
            input.key(&[event(Key::H, true)], Some(Key::H), 16.0, true),
            Some(Key::H)
        );
        assert_eq!(
            input.key(&[event(Key::H, true)], Some(Key::H), 100.0, true),
            None
        );
        assert_eq!(input.key(&[], None, 100.0, true), None);
        assert_eq!(input.key(&[], None, 100.0, true), None);
        assert_eq!(input.key(&[], None, 49.0, true), None);
        assert_eq!(input.key(&[], None, 1.0, true), Some(Key::H));
        assert_eq!(input.key(&[], None, 100.0, true), None);
        assert_eq!(input.key(&[], None, 79.0, true), None);
        assert_eq!(input.key(&[], None, 1.0, true), Some(Key::H));
        assert_eq!(input.key(&[event(Key::H, false)], None, 100.0, true), None);
    }
    #[test]
    fn new_direction_is_immediate_and_focus_loss_stops_movement() {
        let mut input = DirectionInput::default();
        input.key(&[event(Key::H, true)], None, 0.0, true);
        assert_eq!(
            input.key(&[event(Key::J, true)], None, 0.0, true),
            Some(Key::J)
        );
        assert_eq!(
            input.key(
                &[BEvent::Focused { focused: false }],
                Some(Key::J),
                100.0,
                true
            ),
            None
        );
        assert_eq!(input.key(&[], None, 100.0, true), None);
    }
    #[test]
    fn menus_and_animations_do_not_start_or_resume_held_movement() {
        let mut input = DirectionInput::default();
        input.key(&[event(Key::H, true)], None, 0.0, true);
        assert_eq!(input.key(&[], Some(Key::I), 100.0, true), Some(Key::I));
        assert_eq!(
            input.key(&[event(Key::H, true)], Some(Key::H), 100.0, false),
            Some(Key::H)
        );
        assert_eq!(input.key(&[], None, 100.0, true), None);
        input.key(&[event(Key::H, false)], None, 0.0, true);
        assert_eq!(
            input.key(&[event(Key::H, true)], None, 0.0, true),
            Some(Key::H)
        );
    }
}
