use crate::model::config::Dice;
use bracket_lib::prelude::RandomNumberGenerator;

#[derive(Clone)]
pub struct Rng(RandomNumberGenerator);
impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(RandomNumberGenerator::seeded(seed))
    }
    pub fn roll(&mut self, sides: u32) -> u32 {
        self.0.range(0, sides) + 1
    }
}
pub fn modifier(attribute: u32) -> i32 {
    (i64::from(attribute) - 8).div_euclid(2) as i32
}
#[derive(Debug, PartialEq)]
pub struct Outcome {
    pub hit: bool,
    pub penetrations: u32,
    pub damage: u32,
}
pub fn resolve(
    rng: &mut Rng,
    accuracy: i32,
    dv: Option<i32>,
    pv: i32,
    av: u32,
    damage: &Dice,
) -> Outcome {
    if let Some(dv) = dv {
        let roll = rng.roll(20);
        if roll == 1 || (roll != 20 && roll as i32 + accuracy <= dv) {
            return Outcome {
                hit: false,
                penetrations: 0,
                damage: 0,
            };
        }
    }
    let mut penetrations = 0;
    let mut effective = pv;
    for _ in 0..10 {
        let successes = (0..3)
            .filter(|_| rng.roll(10) as i32 + effective > av as i32 + 5)
            .count();
        if successes == 0 {
            break;
        }
        penetrations += 1;
        if successes < 3 {
            break;
        }
        effective -= 2;
    }
    let mut total = 0u32;
    for _ in 0..penetrations {
        let roll: i32 = (0..damage.dice)
            .map(|_| rng.roll(damage.sides) as i32)
            .sum();
        total = total.saturating_add((roll + damage.bonus).max(0) as u32);
    }
    Outcome {
        hit: true,
        penetrations,
        damage: total,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn impossible_armor_and_bounded_penetrations() {
        let die = Dice {
            dice: 1,
            sides: 6,
            bonus: 0,
        };
        let mut rng = Rng::new(42);
        assert_eq!(resolve(&mut rng, 0, None, 0, 100, &die).damage, 0);
        let out = resolve(&mut rng, 0, None, 100, 0, &die);
        assert_eq!(out.penetrations, 10);
        assert!((10..=60).contains(&out.damage));
    }
    #[test]
    fn random_ranges_and_clones_preserve_sequence_including_zero_seed() {
        for seed in [0, 1, 42, u64::MAX] {
            let mut rng = Rng::new(seed);
            for sides in [1, 6, 10, 20, u32::MAX] {
                for _ in 0..100 {
                    assert!((1..=sides).contains(&rng.roll(sides)));
                }
            }
            let mut cloned = rng.clone();
            for _ in 0..100 {
                assert_eq!(rng.roll(20), cloned.roll(20));
            }
        }
    }
    #[test]
    fn modifiers_use_floor_and_seed_repeats() {
        assert_eq!(modifier(7), -1);
        assert_eq!(modifier(8), 0);
        let (mut a, mut b) = (Rng::new(7), Rng::new(7));
        for _ in 0..100 {
            assert_eq!(a.roll(20), b.roll(20));
        }
    }
}
