//! Connect disconnected white mask regions with seeded, adjustable corridors.
use crate::graph::{Image, Result, finite};
use crate::recipe::derive_seed;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectRegions {
    pub cutoff: f32,
    pub width: u32,
    pub distortion: f32,
    pub frequency: f32,
    pub seed: u64,
    pub include_source: bool,
}
impl Default for ConnectRegions {
    fn default() -> Self {
        Self {
            cutoff: 0.5,
            width: 3,
            distortion: 0.03,
            frequency: 1.5,
            seed: 42,
            include_source: true,
        }
    }
}
/// Four-neighbor components, represented by an actual region pixel nearest its centroid.
pub(crate) fn representatives(
    image: &Image,
    cutoff: f32,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<(usize, usize)>> {
    let size = image.size;
    let mut visited = vec![false; size * size];
    let mut result = Vec::new();
    let mut queue = Vec::new();
    for start in 0..size * size {
        if start % size == 0 && cancelled() {
            return Err("cancelled".into());
        }
        if visited[start] || image.pixels[start] < cutoff {
            continue;
        }
        if result.len() == 4096 {
            return Err(
                "白い領域が4096個を超えています。Threshold/Blurで小さな領域を整理してください"
                    .into(),
            );
        }
        visited[start] = true;
        queue.clear();
        queue.push(start);
        let (mut head, mut sx, mut sy) = (0, 0u64, 0u64);
        while head < queue.len() {
            if head % 4096 == 0 && cancelled() {
                return Err("cancelled".into());
            }
            let index = queue[head];
            head += 1;
            let x = index % size;
            let y = index / size;
            sx += x as u64;
            sy += y as u64;
            for (nx, ny) in [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ] {
                if nx < size && ny < size {
                    let next = ny * size + nx;
                    if !visited[next] && image.pixels[next] >= cutoff {
                        visited[next] = true;
                        queue.push(next);
                    }
                }
            }
        }
        let cx = sx as f64 / queue.len() as f64;
        let cy = sy as f64 / queue.len() as f64;
        let &pixel = queue
            .iter()
            .min_by(|&&a, &&b| {
                let distance = |i: usize| (i % size) as f64 - cx;
                let dy = |i: usize| (i / size) as f64 - cy;
                (distance(a).powi(2) + dy(a).powi(2))
                    .total_cmp(&(distance(b).powi(2) + dy(b).powi(2)))
            })
            .unwrap();
        result.push((pixel % size, pixel / size));
    }
    Ok(result)
}
/// Supercover staircase: diagonal raster segments remain connected by four neighbors.
fn line(mask: &mut [bool], size: usize, from: (i32, i32), to: (i32, i32)) {
    let (mut x, mut y) = from;
    let (dx, dy) = ((to.0 - x).abs(), -(to.1 - y).abs());
    let sx = if x < to.0 { 1 } else { -1 };
    let sy = if y < to.1 { 1 } else { -1 };
    let mut error = dx + dy;
    loop {
        mask[y as usize * size + x as usize] = true;
        if (x, y) == to {
            break;
        }
        let twice = 2 * error;
        if twice >= dy {
            error += dy;
            x += sx;
            mask[y as usize * size + x as usize] = true;
        }
        if twice <= dx {
            error += dx;
            y += sy;
        }
    }
}
pub(crate) fn tree_edges(
    points: &[(usize, usize)],
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<(usize, usize)>> {
    let count = points.len();
    if count < 2 {
        return Ok(Vec::new());
    }
    let mut edges = Vec::new();
    // Prim's minimum spanning tree of region representatives: N-1 edges, all regions connected.
    let mut used = vec![false; count];
    let mut distance = vec![f64::INFINITY; count];
    let mut parent = vec![0; count];
    distance[0] = 0.0;
    for _ in 0..count {
        if cancelled() {
            return Err("cancelled".into());
        }
        let next = (0..count)
            .filter(|&i| !used[i])
            .min_by(|&a, &b| distance[a].total_cmp(&distance[b]))
            .unwrap();
        used[next] = true;
        if next != 0 {
            edges.push((parent[next], next));
        }
        for i in 0..count {
            if !used[i] {
                let dx = points[i].0 as f64 - points[next].0 as f64;
                let dy = points[i].1 as f64 - points[next].1 as f64;
                let d = dx * dx + dy * dy;
                if d < distance[i] {
                    distance[i] = d;
                    parent[i] = next;
                }
            }
        }
    }
    Ok(edges)
}
impl ConnectRegions {
    pub fn validate(&self) -> bool {
        finite(self.cutoff, 0.01, 1.0)
            && (1..=64).contains(&self.width)
            && finite(self.distortion, 0.0, 0.25)
            && finite(self.frequency, 0.5, 8.0)
    }
    pub fn render(&self, source: &Image, cancelled: &dyn Fn() -> bool) -> Result<Image> {
        let points = representatives(source, self.cutoff, cancelled)?;
        let edges = tree_edges(&points, cancelled)?;
        self.render_links(source, &points, &edges, cancelled)
    }
    pub(crate) fn render_links(
        &self,
        source: &Image,
        points: &[(usize, usize)],
        edges: &[(usize, usize)],
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Image> {
        let size = source.size;
        let mut pixels = if self.include_source {
            source.pixels.clone()
        } else {
            vec![0.0; size * size]
        };
        if edges.is_empty() {
            return Ok(Image {
                alpha: None,
                terrain: None,
                size,
                pixels,
            });
        }
        let mut skeleton = vec![false; size * size];
        for &(from, to) in edges {
            if cancelled() {
                return Err("cancelled".into());
            }
            let a = points[from];
            let b = points[to];
            let (dx, dy) = (b.0 as f32 - a.0 as f32, b.1 as f32 - a.1 as f32);
            let length = dx.hypot(dy);
            let phase = (derive_seed(self.seed, &format!("{from}_{to}")) >> 40) as f32 / 16777216.0
                * std::f32::consts::TAU;
            let steps = ((length * 2.0).ceil() as usize)
                .max((self.frequency * 32.0) as usize)
                .max(1);
            let mut previous = (a.0 as i32, a.1 as i32);
            for step in 1..=steps {
                if step % 128 == 0 && cancelled() {
                    return Err("cancelled".into());
                }
                let t = step as f32 / steps as f32;
                let offset = (std::f32::consts::PI * t).sin()
                    * (std::f32::consts::TAU * self.frequency * t + phase).sin()
                    * self.distortion
                    * size as f32;
                let x = a.0 as f32 + dx * t - dy / length * offset;
                let y = a.1 as f32 + dy * t + dx / length * offset;
                let current = (
                    x.round().clamp(0.0, (size - 1) as f32) as i32,
                    y.round().clamp(0.0, (size - 1) as f32) as i32,
                );
                line(&mut skeleton, size, previous, current);
                previous = current;
            }
        }
        // Separable square-brush dilation: O(size²), independent of corridor width.
        let before = (self.width as usize - 1) / 2;
        let after = self.width as usize / 2;
        let mut horizontal = vec![false; size * size];
        let mut prefix = vec![0usize; size + 1];
        for y in 0..size {
            if cancelled() {
                return Err("cancelled".into());
            }
            prefix[0] = 0;
            for x in 0..size {
                prefix[x + 1] = prefix[x] + usize::from(skeleton[y * size + x]);
            }
            for x in 0..size {
                horizontal[y * size + x] =
                    prefix[(x + after + 1).min(size)] > prefix[x.saturating_sub(before)];
            }
        }
        for x in 0..size {
            if cancelled() {
                return Err("cancelled".into());
            }
            prefix[0] = 0;
            for y in 0..size {
                prefix[y + 1] = prefix[y] + usize::from(horizontal[y * size + x]);
            }
            for y in 0..size {
                if prefix[(y + after + 1).min(size)] > prefix[y.saturating_sub(before)] {
                    pixels[y * size + x] = 1.0;
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
    use crate::graph::{
        ConnectRegions as Settings, Graph, ImageNode, InputMode, NodeKind, NumberInput,
        RenderContext,
    };
    use crate::recipe::Recipe;
    use std::{collections::BTreeMap, sync::Arc};
    fn rooms() -> Image {
        let mut image = Image {
            alpha: None,
            terrain: None,
            size: 64,
            pixels: vec![0.0; 4096],
        };
        for (cx, cy) in [(8, 8), (48, 10), (12, 50), (52, 48)] {
            for y in cy - 2..=cy + 2 {
                for x in cx - 2..=cx + 2 {
                    image.pixels[y * 64 + x] = 1.0;
                }
            }
        }
        image
    }
    #[test]
    fn connects_all_rooms_with_four_neighbor_paths_even_at_width_one() {
        let source = rooms();
        assert_eq!(representatives(&source, 0.5, &|| false).unwrap().len(), 4);
        for (width, distortion, frequency) in
            [(1, 0.0, 1.5), (1, 0.25, 8.0), (2, 0.03, 1.5), (9, 0.1, 3.0)]
        {
            let node = ConnectRegions {
                width,
                distortion,
                frequency,
                ..Default::default()
            };
            let result = node.render(&source, &|| false).unwrap();
            assert_eq!(representatives(&result, 0.5, &|| false).unwrap().len(), 1);
            for (&old, &new) in source.pixels.iter().zip(&result.pixels) {
                assert!(new >= old);
            }
        }
    }
    #[test]
    fn width_distortion_and_seed_change_paths_without_changing_source() {
        let source = rooms();
        let thin = ConnectRegions {
            width: 1,
            distortion: 0.0,
            include_source: false,
            ..Default::default()
        };
        let a = thin.render(&source, &|| false).unwrap();
        let wide = ConnectRegions {
            width: 7,
            ..thin.clone()
        }
        .render(&source, &|| false)
        .unwrap();
        assert!(wide.pixels.iter().sum::<f32>() > a.pixels.iter().sum::<f32>());
        assert!(a.pixels.iter().zip(&wide.pixels).all(|(a, b)| b >= a));
        let curved = ConnectRegions {
            distortion: 0.1,
            ..thin
        };
        let b = curved.render(&source, &|| false).unwrap();
        assert_ne!(a.pixels, b.pixels);
        assert_eq!(b.pixels, curved.render(&source, &|| false).unwrap().pixels);
        assert_ne!(
            b.pixels,
            ConnectRegions {
                seed: 123,
                ..curved
            }
            .render(&source, &|| false)
            .unwrap()
            .pixels
        );
        assert_eq!(representatives(&b, 0.5, &|| false).unwrap().len(), 1);
    }
    #[test]
    fn empty_single_concave_regions_limits_and_cancellation() {
        let mut source = Image {
            alpha: None,
            terrain: None,
            size: 32,
            pixels: vec![0.0; 1024],
        };
        let node = ConnectRegions::default();
        assert_eq!(
            node.render(&source, &|| false).unwrap().pixels,
            source.pixels
        );
        // A U-shaped component has its centroid in the black hole: its representative must be white.
        for y in 4..26 {
            source.pixels[y * 32 + 4] = 1.0;
            source.pixels[y * 32 + 25] = 1.0;
        }
        for x in 4..26 {
            source.pixels[25 * 32 + x] = 1.0;
        }
        let points = representatives(&source, 0.5, &|| false).unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(source.pixels[points[0].1 * 32 + points[0].0], 1.0);
        assert_eq!(
            node.render(&source, &|| false).unwrap().pixels,
            source.pixels
        );
        assert!(
            ConnectRegions {
                include_source: false,
                ..node.clone()
            }
            .render(&source, &|| false)
            .unwrap()
            .pixels
            .iter()
            .all(|v| *v == 0.0)
        );
        assert!(node.render(&source, &|| true).is_err());
        let checker = Image {
            alpha: None,
            terrain: None,
            size: 128,
            pixels: (0..16384)
                .map(|i| {
                    if (i % 128 + i / 128) % 2 == 0 {
                        1.0
                    } else {
                        0.0
                    }
                })
                .collect(),
        };
        assert!(
            node.render(&checker, &|| false)
                .unwrap_err()
                .contains("4096")
        );
        assert!(
            !ConnectRegions {
                width: 0,
                ..node.clone()
            }
            .validate()
        );
        assert!(
            !ConnectRegions {
                distortion: f32::NAN,
                ..node
            }
            .validate()
        );
    }
    #[test]
    fn connection_node_roundtrips_in_recipes_and_accepts_numeric_input() {
        let mut graph = Graph {
            resolution: 64,
            ..Default::default()
        };
        let tile = graph
            .add(NodeKind::TileSampler(Default::default()), [0.0, 0.0])
            .unwrap();
        let shape = graph
            .add(NodeKind::Shape(Default::default()), [0.0, 0.0])
            .unwrap();
        graph.connect(shape, tile, 0).unwrap();
        let connect = graph
            .add(NodeKind::ConnectRegions(Settings::default()), [260.0, 0.0])
            .unwrap();
        graph.connect(tile, connect, 0).unwrap();
        let input = graph
            .add(NodeKind::Input(NumberInput::default()), [0.0, 340.0])
            .unwrap();
        graph.node_mut(input).unwrap().kind = NodeKind::Input(NumberInput {
            key: "corridor_width".into(),
            mode: InputMode::Range,
            min: 1.0,
            max: 8.0,
            preview: 3.0,
            integer: true,
            ..Default::default()
        });
        graph.bind_parameter(connect, "width", Some(input)).unwrap();
        graph.output = connect;
        let recipe = Recipe::from_graph(&graph).unwrap();
        let recipe: Recipe =
            serde_json::from_str(&serde_json::to_string(&recipe).unwrap()).unwrap();
        let evaluated = recipe
            .instantiate(42, &BTreeMap::from([("corridor_width".into(), 5.0)]))
            .unwrap();
        assert!(matches!(
            evaluated.node(connect).unwrap().kind,
            NodeKind::ConnectRegions(ConnectRegions { width: 5, .. })
        ));
        let images = evaluated.render(&|| false).unwrap();
        assert_eq!(
            representatives(images[&connect].as_ref().unwrap(), 0.5, &|| false)
                .unwrap()
                .len(),
            1
        );
        assert!(
            NodeKind::ConnectRegions(Default::default())
                .evaluate(
                    &[],
                    RenderContext {
                        size: 64,
                        extent: 64.0
                    },
                    &|| false
                )
                .is_err()
        );
        let source = Arc::new(rooms());
        assert!(
            NodeKind::ConnectRegions(Default::default())
                .evaluate(
                    &[source],
                    RenderContext {
                        size: 64,
                        extent: 64.0
                    },
                    &|| true
                )
                .is_err()
        );
    }
}
