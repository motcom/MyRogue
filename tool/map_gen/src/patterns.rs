//! Procedural masks and periodic, seeded image stamping (independent of GUI).
use crate::graph::{Image, Result, bilinear, finite};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    Circle,
    Square,
    Triangle,
    Diamond,
    Hexagon,
}
impl ShapeKind {
    pub const ALL: [Self; 5] = [
        Self::Circle,
        Self::Square,
        Self::Triangle,
        Self::Diamond,
        Self::Hexagon,
    ];
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shape {
    pub kind: ShapeKind,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation: f32,
    pub softness: f32,
}
impl Default for Shape {
    fn default() -> Self {
        Self {
            kind: ShapeKind::Circle,
            scale_x: 0.8,
            scale_y: 0.8,
            rotation: 0.0,
            softness: 0.0,
        }
    }
}
impl Shape {
    pub fn validate(&self) -> bool {
        finite(self.scale_x, 0.01, 1.0)
            && finite(self.scale_y, 0.01, 1.0)
            && finite(self.rotation, -180.0, 180.0)
            && finite(self.softness, 0.0, 0.5)
    }
    pub fn sample(&self, u: f32, v: f32) -> f32 {
        let (sin, cos) = self.rotation.to_radians().sin_cos();
        let x = ((u - 0.5) * cos + (v - 0.5) * sin) * 2.0 / self.scale_x;
        let y = (-(u - 0.5) * sin + (v - 0.5) * cos) * 2.0 / self.scale_y;
        let distance = match self.kind {
            ShapeKind::Circle => x.hypot(y),
            ShapeKind::Square => x.abs().max(y.abs()),
            ShapeKind::Diamond => x.abs() + y.abs(),
            // Regular polygons inscribed in the unit circle, with a vertex upward.
            ShapeKind::Triangle | ShapeKind::Hexagon => {
                let n = if self.kind == ShapeKind::Triangle {
                    3
                } else {
                    6
                };
                (0..n)
                    .map(|i| {
                        let angle = -std::f32::consts::FRAC_PI_2
                            + (2 * i + 1) as f32 * std::f32::consts::PI / n as f32;
                        (x * angle.cos() + y * angle.sin())
                            / (std::f32::consts::PI / n as f32).cos()
                    })
                    .fold(f32::NEG_INFINITY, f32::max)
            }
        };
        if self.softness == 0.0 {
            return if distance <= 1.0 { 1.0 } else { 0.0 };
        }
        let t = ((1.0 - distance) / self.softness).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TileSampler {
    pub count_x: u32,
    pub count_y: u32,
    pub scale: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub scale_random: f32,
    pub position_random_x: f32,
    pub position_random_y: f32,
    pub rotation: f32,
    pub rotation_random: f32,
    pub seed: u64,
}
impl Default for TileSampler {
    fn default() -> Self {
        Self {
            count_x: 4,
            count_y: 4,
            scale: 0.8,
            scale_x: 1.0,
            scale_y: 1.0,
            scale_random: 0.0,
            position_random_x: 0.0,
            position_random_y: 0.0,
            rotation: 0.0,
            rotation_random: 0.0,
            seed: 42,
        }
    }
}
// Stateless cell/channel hash: stable when resolution changes, no RNG dependencies.
fn random(seed: u64, cell: u64, channel: u64) -> f32 {
    let mut v = seed
        .wrapping_add(cell.wrapping_mul(0x9e3779b97f4a7c15))
        .wrapping_add(channel.wrapping_mul(0xd1b54a32d192ed03));
    v = (v ^ (v >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    v = (v ^ (v >> 27)).wrapping_mul(0x94d049bb133111eb);
    ((v ^ (v >> 31)) >> 40) as f32 / 16777216.0
}
impl TileSampler {
    pub fn validate(&self) -> bool {
        (1..=64).contains(&self.count_x)
            && (1..=64).contains(&self.count_y)
            && [self.scale, self.scale_x, self.scale_y]
                .iter()
                .all(|&v| finite(v, 0.01, 1.0))
            && [
                self.scale_random,
                self.position_random_x,
                self.position_random_y,
            ]
            .iter()
            .all(|&v| finite(v, 0.0, 1.0))
            && finite(self.rotation, -180.0, 180.0)
            && finite(self.rotation_random, 0.0, 180.0)
    }
    pub fn render(&self, source: &Image, cancelled: &dyn Fn() -> bool) -> Result<Image> {
        let size = source.size;
        let mut pixels = vec![0.0_f32; size * size];
        let cell_w = size as f32 / self.count_x as f32;
        let cell_h = size as f32 / self.count_y as f32;
        for cy in 0..self.count_y {
            for cx in 0..self.count_x {
                if cancelled() {
                    return Err("cancelled".into());
                }
                let id = (cy * self.count_x + cx) as u64;
                let rand = |channel| random(self.seed, id, channel);
                let center_x =
                    (cx as f32 + 0.5 + (rand(0) - 0.5) * self.position_random_x) * cell_w;
                let center_y =
                    (cy as f32 + 0.5 + (rand(1) - 0.5) * self.position_random_y) * cell_h;
                let scale = self.scale * (1.0 - self.scale_random * rand(2)).max(0.001);
                let width = cell_w * scale * self.scale_x;
                let height = cell_h * scale * self.scale_y;
                let (sin, cos) = (self.rotation + (rand(3) * 2.0 - 1.0) * self.rotation_random)
                    .to_radians()
                    .sin_cos();
                let rx = (width * cos.abs() + height * sin.abs()) * 0.5;
                let ry = (width * sin.abs() + height * cos.abs()) * 0.5;
                for y in (center_y - ry).floor() as isize..(center_y + ry).ceil() as isize {
                    if cancelled() {
                        return Err("cancelled".into());
                    }
                    for x in (center_x - rx).floor() as isize..(center_x + rx).ceil() as isize {
                        let dx = x as f32 + 0.5 - center_x;
                        let dy = y as f32 + 0.5 - center_y;
                        let u = (dx * cos + dy * sin) / width + 0.5;
                        let v = (-dx * sin + dy * cos) / height + 0.5;
                        if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                            continue;
                        }
                        let value = bilinear(source, u * size as f32 - 0.5, v * size as f32 - 0.5);
                        let index = y.rem_euclid(size as isize) as usize * size
                            + x.rem_euclid(size as isize) as usize;
                        pixels[index] = pixels[index].max(value);
                    }
                }
            }
        }
        Ok(Image {
            alpha: None,
            terrain: None,
            size,
            pixels,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Graph, ImageNode, NodeKind, RenderContext};
    use std::sync::Arc;
    #[test]
    fn shapes_have_expected_silhouettes_and_transforms() {
        for kind in ShapeKind::ALL {
            let s = Shape {
                kind,
                ..Default::default()
            };
            assert_eq!(s.sample(0.5, 0.5), 1.0);
            assert_eq!(s.sample(0.0, 0.0), 0.0);
        }
        let square = Shape {
            kind: ShapeKind::Square,
            ..Default::default()
        };
        let circle = Shape::default();
        let diamond = Shape {
            kind: ShapeKind::Diamond,
            ..Default::default()
        };
        assert_eq!(square.sample(0.8, 0.8), 1.0);
        assert_eq!(circle.sample(0.8, 0.8), 0.0);
        assert_eq!(circle.sample(0.78, 0.65), 1.0);
        assert_eq!(diamond.sample(0.78, 0.65), 0.0);
        let wide = Shape {
            scale_x: 0.8,
            scale_y: 0.2,
            ..square.clone()
        };
        let rotated = Shape {
            rotation: 90.0,
            ..wide.clone()
        };
        assert_eq!(wide.sample(0.8, 0.5), 1.0);
        assert_eq!(rotated.sample(0.8, 0.5), 0.0);
        assert_eq!(rotated.sample(0.5, 0.8), 1.0);
        let soft = Shape {
            softness: 0.5,
            ..square
        };
        assert!(soft.sample(0.85, 0.5) > 0.0 && soft.sample(0.85, 0.5) < 1.0);
    }
    #[test]
    fn tile_counts_scale_identity_and_periodic_wrap() {
        let source = Image {
            alpha: None,
            terrain: None,
            size: 64,
            pixels: vec![1.0; 4096],
        };
        let t = TileSampler {
            count_x: 4,
            count_y: 2,
            scale: 0.5,
            ..Default::default()
        };
        let image = t.render(&source, &|| false).unwrap();
        for y in 0..64 {
            for x in 0..64 {
                let inside = (4..12).contains(&(x % 16)) && (8..24).contains(&(y % 32));
                assert_eq!(image.pixels[y * 64 + x], if inside { 1.0 } else { 0.0 });
            }
        }
        let t = TileSampler {
            count_x: 1,
            count_y: 1,
            scale: 1.0,
            ..Default::default()
        };
        let ramp = Image {
            alpha: None,
            terrain: None,
            size: 64,
            pixels: (0..4096).map(|i| i as f32 / 4096.0).collect(),
        };
        assert_eq!(t.render(&ramp, &|| false).unwrap().pixels, ramp.pixels);
        // A translated full-size square must wrap, leaving no black border.
        let shifted = TileSampler {
            position_random_x: 1.0,
            position_random_y: 1.0,
            ..t
        };
        assert!(
            shifted
                .render(&source, &|| false)
                .unwrap()
                .pixels
                .iter()
                .all(|&v| v == 1.0)
        );
    }
    #[test]
    fn connected_patterns_roundtrip_seed_validation_and_cancellation() {
        let mut graph = Graph {
            resolution: 64,
            ..Default::default()
        };
        let shape = graph
            .add(NodeKind::Shape(Shape::default()), [0.0, 0.0])
            .unwrap();
        let t = TileSampler {
            position_random_x: 1.0,
            position_random_y: 0.8,
            scale_random: 0.6,
            rotation_random: 180.0,
            ..Default::default()
        };
        let tile = graph
            .add(NodeKind::TileSampler(t.clone()), [220.0, 0.0])
            .unwrap();
        graph.connect(shape, tile, 0).unwrap();
        graph.output = tile;
        graph.validate().unwrap();
        let copy: Graph = serde_json::from_str(&serde_json::to_string(&graph).unwrap()).unwrap();
        assert_eq!(graph, copy);
        let a = graph.render(&|| false).unwrap()[&tile]
            .as_ref()
            .unwrap()
            .pixels
            .clone();
        assert_eq!(
            a,
            copy.render(&|| false).unwrap()[&tile]
                .as_ref()
                .unwrap()
                .pixels
        );
        if let NodeKind::TileSampler(t) = &mut graph.node_mut(tile).unwrap().kind {
            t.seed += 1;
        }
        let b = graph.render(&|| false).unwrap()[&tile]
            .as_ref()
            .unwrap()
            .pixels
            .clone();
        assert_ne!(a, b);
        assert!(a.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
        let context = RenderContext {
            size: 64,
            extent: 64.0,
        };
        let source = Arc::new(Image {
            alpha: None,
            terrain: None,
            size: 64,
            pixels: vec![1.0; 4096],
        });
        assert!(
            NodeKind::TileSampler(t.clone())
                .evaluate(&[], context, &|| false)
                .is_err()
        );
        assert!(
            NodeKind::TileSampler(t.clone())
                .evaluate(&[source], context, &|| true)
                .is_err()
        );
        for invalid in [
            TileSampler {
                count_x: 0,
                ..t.clone()
            },
            TileSampler {
                count_y: 65,
                ..t.clone()
            },
            TileSampler {
                scale: f32::NAN,
                ..t
            },
        ] {
            graph.node_mut(tile).unwrap().kind = NodeKind::TileSampler(invalid);
            assert!(graph.validate().is_err());
        }
    }
}
