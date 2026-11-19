//! Deterministic chunk boundaries, shared by the editor and game.
use crate::{
    graph::{Graph, Image, Result},
    terrain::{SeamMode, TerrainBand, TerrainPalette},
};
use std::{collections::BTreeMap, sync::Arc};
pub type Coordinate = [i32; 2];

fn hash(seed: u64, x: i64, y: i64) -> u64 {
    crate::recipe::derive_seed(seed, &format!("{x}:{y}"))
}
fn random(seed: u64, x: i64, y: i64) -> f64 {
    (hash(seed, x, y) >> 11) as f64 / (1u64 << 53) as f64
}
fn smooth(seed: u64, x: i64, y: i64, scale: i64) -> f64 {
    let (cx, cy) = (x.div_euclid(scale), y.div_euclid(scale));
    let ease = |v: i64| {
        let t = v.rem_euclid(scale) as f64 / scale as f64;
        t * t * (3.0 - 2.0 * t)
    };
    let (u, v) = (ease(x), ease(y));
    let a = random(seed, cx, cy) * (1.0 - u) + random(seed, cx + 1, cy) * u;
    let b = random(seed, cx, cy + 1) * (1.0 - u) + random(seed, cx + 1, cy + 1) * u;
    a * (1.0 - v) + b * v
}
fn offset(c: Coordinate, x: i32, y: i32) -> Result<Coordinate> {
    Ok([
        c[0].checked_add(x).ok_or("tile x overflow")?,
        c[1].checked_add(y).ok_or("tile y overflow")?,
    ])
}
fn raw(
    graph: &Graph,
    seed: u64,
    inputs: &BTreeMap<String, f64>,
    coord: Coordinate,
    cancel: &dyn Fn() -> bool,
) -> Result<Arc<Image>> {
    if cancel() {
        return Err("cancelled".into());
    }
    let seed = if coord == [0, 0] {
        seed
    } else {
        hash(seed, coord[0] as i64, coord[1] as i64)
    };
    let executable = crate::recipe::Recipe::from_graph(graph)?.instantiate(seed, inputs)?;
    executable
        .render(cancel)?
        .remove(&executable.output)
        .ok_or("最終出力がありません")?
}

/// Generate one chunk using its eight raw neighbors. The order of requests is irrelevant.
pub fn generate_tile(
    graph: &Graph,
    seed: u64,
    inputs: &BTreeMap<String, f64>,
    coord: Coordinate,
) -> Result<Arc<Image>> {
    let mut tiles = BTreeMap::new();
    for y in -1..=1 {
        for x in -1..=1 {
            let p = offset(coord, x, y)?;
            tiles.insert(p, raw(graph, seed, inputs, p, &|| false)?);
        }
    }
    stitch_tile(&tiles, coord, seed, &|| false).map(Arc::new)
}

/// A 3×3 atlas, with an extra one-chunk halo so outer tiles have real neighbors too.
pub fn generate_neighborhood(
    graph: &Graph,
    seed: u64,
    inputs: &BTreeMap<String, f64>,
    center: Coordinate,
    cancel: &dyn Fn() -> bool,
) -> Result<Arc<Image>> {
    let mut tiles = BTreeMap::new();
    for y in -2..=2 {
        for x in -2..=2 {
            let p = offset(center, x, y)?;
            tiles.insert(p, raw(graph, seed, inputs, p, cancel)?);
        }
    }
    let mut chunks = Vec::new();
    for y in -1..=1 {
        for x in -1..=1 {
            chunks.push(stitch_tile(&tiles, offset(center, x, y)?, seed, cancel)?);
        }
    }
    let n = graph.resolution as usize;
    let (palette, lookup) = palette(chunks.iter())?;
    let mut image = Image {
        size: n * 3,
        pixels: vec![0.0; n * n * 9],
        alpha: Some(vec![0; n * n * 9]),
        terrain: palette.map(Arc::new),
    };
    for (chunk, source) in chunks.iter().enumerate() {
        for y in 0..n {
            for x in 0..n {
                let src = y * n + x;
                let dst = (chunk / 3 * n + y) * n * 3 + chunk % 3 * n + x;
                image.pixels[dst] = encoded(source, src, &lookup);
                image.alpha.as_mut().unwrap()[dst] = alpha(source, src);
            }
        }
    }
    Ok(Arc::new(image))
}
fn alpha(image: &Image, i: usize) -> u8 {
    image.alpha.as_ref().map_or(255, |a| a[i])
}
type Lookup = BTreeMap<String, usize>;
fn palette<'a>(
    images: impl Iterator<Item = &'a Image>,
) -> Result<(Option<TerrainPalette>, Lookup)> {
    let mut bands: Vec<TerrainBand> = Vec::new();
    let mut lookup = BTreeMap::new();
    let mut categorical = None;
    for image in images {
        if categorical.is_some_and(|b| b != image.terrain.is_some()) {
            return Err("隣接タイルは全て地形ID付き、または全て濃淡にしてください".into());
        }
        categorical = Some(image.terrain.is_some());
        if let Some(p) = &image.terrain {
            for b in &p.bands {
                if let Some(&i) = lookup.get(&b.id) {
                    let existing: &TerrainBand = &bands[i];
                    if existing.color != b.color || existing.seam != b.seam {
                        return Err(format!(
                            "地形ID {} の色・境界設定が隣接タイルで異なります",
                            b.id
                        ));
                    }
                } else {
                    lookup.insert(b.id.clone(), bands.len());
                    bands.push(b.clone());
                }
            }
        }
    }
    if bands.is_empty() {
        return Ok((None, lookup));
    }
    if bands.len() > 256 {
        return Err("隣接タイルの地形IDは最大256種類です".into());
    }
    let count = bands.len();
    for (i, b) in bands.iter_mut().enumerate() {
        b.max = (i + 1) as f32 / count as f32;
    }
    Ok((Some(TerrainPalette { bands }), lookup))
}
fn encoded(image: &Image, i: usize, lookup: &Lookup) -> f32 {
    image.terrain.as_ref().map_or(image.pixels[i], |p| {
        (lookup[&p.band(image.pixels[i]).id] as f32 + 0.5) / lookup.len() as f32
    })
}

/// Stitch a chunk from supplied raw images; callers may supply different JSON maps per coordinate.
/// All nine neighbors must exist, share resolution and agree on settings for shared IDs.
pub fn stitch_tile(
    tiles: &BTreeMap<Coordinate, Arc<Image>>,
    coord: Coordinate,
    seed: u64,
    cancel: &dyn Fn() -> bool,
) -> Result<Image> {
    let own = tiles.get(&coord).ok_or("中央タイルがありません")?;
    let n = own.size;
    if n < 2 {
        return Err("タイルは2マス以上必要です".into());
    }
    let mut neighbors = Vec::new();
    for y in -1..=1 {
        for x in -1..=1 {
            let c = offset(coord, x, y)?;
            let tile = tiles.get(&c).ok_or("周囲8タイルが必要です")?;
            if tile.size != n
                || tile.pixels.len() != n * n
                || tile.alpha.as_ref().is_some_and(|a| a.len() != n * n)
            {
                return Err("タイル寸法が一致しません".into());
            }
            neighbors.push((c, tile));
        }
    }
    let (p, lookup) = palette(neighbors.iter().map(|(_, t)| t.as_ref()))?;
    let Some(p) = p else {
        return Ok(own.as_ref().clone());
    };
    let width = p
        .bands
        .iter()
        .filter(|b| matches!(b.seam.mode, SeamMode::Mosaic | SeamMode::Fill))
        .map(|b| b.seam.width as usize)
        .max()
        .unwrap_or(1)
        .min(n / 2)
        .max(1);
    let mut result = Image {
        size: n,
        pixels: (0..n * n).map(|i| encoded(own, i, &lookup)).collect(),
        alpha: Some((0..n * n).map(|i| alpha(own, i)).collect()),
        terrain: Some(Arc::new(p)),
    };
    for y in 0..n {
        if cancel() {
            return Err("cancelled".into());
        }
        for x in 0..n {
            let i = y * n + x;
            let band = own.terrain.as_ref().unwrap().band(own.pixels[i]);
            if alpha(own, i) == 0 || !matches!(band.seam.mode, SeamMode::Mosaic | SeamMode::Fill) {
                continue;
            }
            // Clamp source coordinates to each tile's boundary. At the seam, both sides
            // use equal weights and the same noise sample (including four-way corners).
            let axis = |v: usize| -> Vec<(i32, f64)> {
                let w = width.min(band.seam.width as usize).max(1);
                let (direction, d) = if v < n / 2 { (-1, v) } else { (1, n - 1 - v) };
                if d >= w {
                    vec![(0, 1.0)]
                } else {
                    let t = if w == 1 {
                        0.5
                    } else {
                        0.5 * (1.0 - d as f64 / (w - 1) as f64)
                    };
                    vec![(0, 1.0 - t), (direction, t)]
                }
            };
            let mut choices = Vec::new();
            for (dy, wy) in axis(y) {
                for (dx, wx) in axis(x) {
                    let c = offset(coord, dx, dy)?;
                    let t = &tiles[&c];
                    let sx = (x as i64 - dx as i64 * n as i64).clamp(0, n as i64 - 1) as usize;
                    let sy = (y as i64 - dy as i64 * n as i64).clamp(0, n as i64 - 1) as usize;
                    let j = sy * n + sx;
                    let b = t.terrain.as_ref().unwrap().band(t.pixels[j]);
                    if alpha(t, j) > 0 && matches!(b.seam.mode, SeamMode::Mosaic | SeamMode::Fill) {
                        choices.push((c, t, j, wx * wy));
                    }
                }
            }
            choices.sort_by_key(|(c, _, _, _)| *c);
            let snap = |v: usize, c: i32| {
                c as i64 * n as i64 + if v == n - 1 { n as i64 } else { v as i64 }
            };
            let (gx, gy) = (snap(x, coord[0]), snap(y, coord[1]));
            let natural = choices.iter().any(|(_, t, j, _)| {
                t.terrain.as_ref().unwrap().band(t.pixels[*j]).seam.mode == SeamMode::Fill
            });
            let noise = if natural {
                smooth(seed, gx, gy, (width as i64 / 2).max(2))
            } else {
                random(seed, gx, gy)
            };
            let mut threshold = noise * choices.iter().map(|(_, _, _, w)| w).sum::<f64>();
            for (_, t, j, w) in choices {
                threshold -= w;
                if threshold <= 0.0 {
                    result.pixels[i] = encoded(t, j, &lookup);
                    result.alpha.as_mut().unwrap()[i] = alpha(t, j);
                    break;
                }
            }
        }
    }
    // Each edge is processed in a canonical order. Find the nearest same-ID pair
    // within its search strip, then route both sides to one shared crossing.
    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let other_coord = offset(coord, dx, dy)?;
        let other = &tiles[&other_coord];
        let horizontal = dx != 0;
        let positive = dx + dy > 0;
        let a = if positive { own } else { other };
        let b = if positive { other } else { own };
        for band in &result.terrain.as_ref().unwrap().clone().bands {
            if band.seam.mode != SeamMode::Road {
                continue;
            }
            let w = (band.seam.width as usize).min(n / 2).max(1);
            let points = |t: &Image, far: bool| {
                let mut points = Vec::new();
                for along in 0..n {
                    for depth in 0..w {
                        let across = if far { n - 1 - depth } else { depth };
                        let (x, y) = if horizontal {
                            (across, along)
                        } else {
                            (along, across)
                        };
                        let j = y * n + x;
                        if alpha(t, j) > 0
                            && t.terrain.as_ref().unwrap().band(t.pixels[j]).id == band.id
                        {
                            points.push((along, depth));
                        }
                    }
                }
                points
            };
            let (ap, bp) = (points(a, true), points(b, false));
            // Retain the shallowest road cell in each row, then join every
            // separate run on either side to its closest run on the other side.
            let rows = |points: Vec<(usize, usize)>| {
                let mut rows = BTreeMap::new();
                for (r, d) in points {
                    rows.entry(r).or_insert(d);
                }
                rows
            };
            let (ap, bp) = (rows(ap), rows(bp));
            let nearest = |source: &BTreeMap<usize, usize>,
                           target: &BTreeMap<usize, usize>,
                           reverse: bool| {
                // Manhattan distance transform in two sweeps, including target depth.
                let mut field = vec![None; n];
                let mut best: Option<(usize, usize)> = None;
                for (row, entry) in field.iter_mut().enumerate() {
                    if let Some(&depth) = target.get(&row)
                        && best.is_none_or(|(r, d)| depth <= row.abs_diff(r) + d)
                    {
                        best = Some((row, depth));
                    }
                    *entry = best;
                }
                best = None;
                for (row, entry) in field.iter_mut().enumerate().rev() {
                    if let Some(&depth) = target.get(&row)
                        && best.is_none_or(|(r, d)| depth <= row.abs_diff(r) + d)
                    {
                        best = Some((row, depth));
                    }
                    if let Some(candidate) = best
                        && entry.is_none_or(|(r, d)| {
                            row.abs_diff(candidate.0) + candidate.1 < row.abs_diff(r) + d
                        })
                    {
                        *entry = Some(candidate);
                    }
                }
                let mut result = Vec::new();
                let mut previous = None;
                let mut run = None;
                for (&r, &d) in source {
                    if previous.is_some_and(|p| r > p + 1)
                        && let Some(pair) = run.take()
                    {
                        result.push(pair);
                    }
                    if let Some((tr, td)) = field[r] {
                        let pair = if reverse {
                            (r.abs_diff(tr) + d + td, tr, td, r, d)
                        } else {
                            (r.abs_diff(tr) + d + td, r, d, tr, td)
                        };
                        if run.is_none_or(|v| pair < v) {
                            run = Some(pair);
                        }
                    }
                    previous = Some(r);
                }
                if let Some(pair) = run {
                    result.push(pair);
                }
                result
            };
            let mut pairs = nearest(&ap, &bp, false);
            pairs.extend(nearest(&bp, &ap, true));
            pairs.sort();
            pairs.dedup();
            let mut crossings: Vec<Vec<usize>> = pairs
                .iter()
                .map(|(_, ar, _, br, _)| vec![(ar + br) / 2])
                .collect();
            // Preserve existing road mouths by giving both sides the same openings.
            for row in 0..n {
                let ai = if horizontal {
                    row * n + n - 1
                } else {
                    (n - 1) * n + row
                };
                let bi = if horizontal { row * n } else { row };
                let road_at = |t: &Image, i| {
                    alpha(t, i) > 0 && t.terrain.as_ref().unwrap().band(t.pixels[i]).id == band.id
                };
                if (road_at(a, ai) || road_at(b, bi)) && !pairs.is_empty() {
                    let closest = pairs
                        .iter()
                        .enumerate()
                        .min_by_key(|(i, (_, ar, _, br, _))| (row.abs_diff((ar + br) / 2), *i))
                        .unwrap()
                        .0;
                    crossings[closest].push(row);
                }
            }
            for ((_, ar, ad, br, bd), openings) in pairs.into_iter().zip(crossings) {
                for crossing in openings {
                    let (along, depth) = if positive { (ar, ad) } else { (br, bd) };
                    let across = if positive {
                        (n - 1 - depth).min(n - 2)
                    } else {
                        depth.max(1)
                    };
                    let edge = if positive { n - 1 } else { 0 };
                    let (start, end) = if horizontal {
                        ([across, along], [edge, crossing])
                    } else {
                        ([along, across], [crossing, edge])
                    };
                    let mut point = start;
                    // Manhattan routing guarantees four-neighbor connectivity.
                    loop {
                        let j = point[1] * n + point[0];
                        result.pixels[j] = (lookup[&band.id] as f32 + 0.5) / lookup.len() as f32;
                        result.alpha.as_mut().unwrap()[j] = 255;
                        if point == end {
                            break;
                        }
                        let axis = if horizontal {
                            if point[1] != end[1] { 1 } else { 0 }
                        } else if point[0] != end[0] {
                            0
                        } else {
                            1
                        };
                        if point[axis] < end[axis] {
                            point[axis] += 1;
                        } else {
                            point[axis] -= 1;
                        }
                    }
                }
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        graph::NodeKind,
        terrain::{Seam, TerrainPalette},
    };
    fn image(mode: SeamMode, value: f32) -> Arc<Image> {
        let bands = ["a", "b", "wall", "road"]
            .iter()
            .enumerate()
            .map(|(i, id)| TerrainBand {
                id: (*id).into(),
                max: (i + 1) as f32 / 4.0,
                color: [i as u8 * 40, 0, 0],
                seam: Seam {
                    mode: if i == 2 {
                        SeamMode::Cut
                    } else if i == 3 {
                        SeamMode::Road
                    } else {
                        mode
                    },
                    width: 4,
                },
            })
            .collect();
        Arc::new(Image {
            size: 16,
            pixels: vec![value; 256],
            alpha: None,
            terrain: Some(Arc::new(TerrainPalette { bands })),
        })
    }
    fn id(image: &Image, x: usize, y: usize) -> &str {
        &image
            .terrain
            .as_ref()
            .unwrap()
            .band(image.pixels[y * image.size + x])
            .id
    }
    fn tiles(mode: SeamMode) -> BTreeMap<Coordinate, Arc<Image>> {
        (-2..=2)
            .flat_map(|y| {
                (-2..=2).map(move |x| {
                    (
                        [x, y],
                        image(mode, if (x + y) % 2 == 0 { 0.1 } else { 0.4 }),
                    )
                })
            })
            .collect()
    }
    #[test]
    fn categorical_seams_match_in_both_axes_and_four_way_corners() {
        for mode in [SeamMode::Mosaic, SeamMode::Fill] {
            let tiles = tiles(mode);
            for coord in [[-1, -1], [0, 0]] {
                let a = stitch_tile(&tiles, coord, 42, &|| false).unwrap();
                let b = stitch_tile(&tiles, offset(coord, 1, 0).unwrap(), 42, &|| false).unwrap();
                let c = stitch_tile(&tiles, offset(coord, 0, 1).unwrap(), 42, &|| false).unwrap();
                let d = stitch_tile(&tiles, offset(coord, 1, 1).unwrap(), 42, &|| false).unwrap();
                for i in 0..16 {
                    assert_eq!(id(&a, 15, i), id(&b, 0, i));
                    assert_eq!(id(&a, i, 15), id(&c, i, 0));
                }
                assert_eq!(id(&a, 15, 15), id(&d, 0, 0));
                assert_eq!(id(&a, 8, 8), id(&tiles[&coord], 8, 8));
                assert_eq!(
                    a.pixels,
                    stitch_tile(&tiles, coord, 42, &|| false).unwrap().pixels
                );
            }
        }
    }
    #[test]
    fn cut_and_transparency_are_preserved_and_invalid_neighbors_rejected() {
        let mut tiles = tiles(SeamMode::Fill);
        tiles.insert([0, 0], image(SeamMode::Fill, 0.7));
        let a = stitch_tile(&tiles, [0, 0], 1, &|| false).unwrap();
        assert!((0..256).all(|i| id(&a, i % 16, i / 16) == "wall"));
        let own = Arc::make_mut(tiles.get_mut(&[0, 0]).unwrap());
        own.alpha = Some(vec![0; 256]);
        let a = stitch_tile(&tiles, [0, 0], 1, &|| false).unwrap();
        assert!(a.alpha.unwrap().iter().all(|a| *a == 0));
        assert!(stitch_tile(&tiles, [0, 0], 1, &|| true).is_err());
        let wrong = Arc::make_mut(tiles.get_mut(&[1, 0]).unwrap());
        Arc::make_mut(wrong.terrain.as_mut().unwrap()).bands[0]
            .seam
            .width = 5;
        assert!(stitch_tile(&tiles, [0, 0], 1, &|| false).is_err());
        tiles.remove(&[1, 0]);
        assert!(stitch_tile(&tiles, [0, 0], 1, &|| false).is_err());
    }
    #[test]
    fn roads_route_to_one_shared_crossing_in_both_axes() {
        for horizontal in [true, false] {
            let mut tiles = tiles(SeamMode::Mosaic);
            let neighbor = if horizontal { [1, 0] } else { [0, 1] };
            for (c, across, along) in [([0, 0], 13, 3), (neighbor, 2, 10)] {
                let t = Arc::make_mut(tiles.get_mut(&c).unwrap());
                let j = if horizontal {
                    along * 16 + across
                } else {
                    across * 16 + along
                };
                t.pixels[j] = 0.9;
            }
            let a = stitch_tile(&tiles, [0, 0], 42, &|| false).unwrap();
            let b = stitch_tile(&tiles, neighbor, 42, &|| false).unwrap();
            let mut crossings = 0;
            for row in 0..16 {
                let (ax, ay, bx, by) = if horizontal {
                    (15, row, 0, row)
                } else {
                    (row, 15, row, 0)
                };
                assert_eq!(id(&a, ax, ay) == "road", id(&b, bx, by) == "road");
                if id(&a, ax, ay) == "road" {
                    crossings += 1;
                }
            }
            assert_eq!(crossings, 1);
            // Flood fill from the original endpoint across the stitched boundary.
            let mut seen = std::collections::BTreeSet::new();
            let start = if horizontal { (13, 3) } else { (3, 13) };
            let goal = if horizontal { (18, 10) } else { (10, 18) };
            let mut queue = vec![start];
            while let Some((x, y)) = queue.pop() {
                if !seen.insert((x, y)) {
                    continue;
                }
                for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                    if nx < 0
                        || ny < 0
                        || nx >= if horizontal { 32 } else { 16 }
                        || ny >= if horizontal { 16 } else { 32 }
                    {
                        continue;
                    }
                    let (t, lx, ly) = if horizontal && nx >= 16 {
                        (&b, nx - 16, ny)
                    } else if !horizontal && ny >= 16 {
                        (&b, nx, ny - 16)
                    } else {
                        (&a, nx, ny)
                    };
                    if id(t, lx as usize, ly as usize) == "road" && !seen.contains(&(nx, ny)) {
                        queue.push((nx, ny));
                    }
                }
            }
            assert!(seen.contains(&goal));
        }
    }
    #[test]
    fn multiple_road_mouths_remain_matched() {
        let mut tiles = tiles(SeamMode::Mosaic);
        for (c, points) in [
            ([0, 0], vec![(15, 1), (14, 12)]),
            ([1, 0], vec![(1, 9), (0, 13)]),
        ] {
            let t = Arc::make_mut(tiles.get_mut(&c).unwrap());
            for (x, y) in points {
                t.pixels[y * 16 + x] = 0.9;
            }
        }
        let a = stitch_tile(&tiles, [0, 0], 42, &|| false).unwrap();
        let b = stitch_tile(&tiles, [1, 0], 42, &|| false).unwrap();
        for row in 0..16 {
            assert_eq!(id(&a, 15, row) == "road", id(&b, 0, row) == "road");
        }
        assert_eq!(id(&a, 15, 1), "road");
        assert_eq!(id(&b, 0, 13), "road");
    }

    #[test]
    fn neighborhood_matches_individual_generation_and_json_roundtrip() {
        let mut graph = Graph {
            resolution: 16,
            ..Default::default()
        };
        let mut palette = TerrainPalette::default();
        for b in &mut palette.bands {
            b.seam = Seam {
                mode: SeamMode::Mosaic,
                width: 4,
            };
        }
        let source = graph.output;
        let output = graph.add(NodeKind::Terrain(palette), [0.0, 0.0]).unwrap();
        graph.connect(source, output, 0).unwrap();
        graph.output = output;
        let restored: Graph =
            serde_json::from_str(&serde_json::to_string(&graph).unwrap()).unwrap();
        assert_eq!(graph, restored);
        let atlas = generate_neighborhood(&graph, 72, &BTreeMap::new(), [0, 0], &|| false).unwrap();
        for (coord, cx, cy) in [([-1, -1], 0, 0), ([0, 0], 1, 1), ([1, 0], 2, 1)] {
            let single = generate_tile(&restored, 72, &BTreeMap::new(), coord).unwrap();
            for y in 0..16 {
                for x in 0..16 {
                    assert_eq!(id(&single, x, y), id(&atlas, cx * 16 + x, cy * 16 + y));
                }
            }
        }
        assert!(generate_neighborhood(&graph, 72, &BTreeMap::new(), [0, 0], &|| true).is_err());
        assert!(generate_tile(&graph, 72, &BTreeMap::new(), [i32::MAX, 0]).is_err());
        let legacy: TerrainBand =
            serde_json::from_str(r#"{"max":1.0,"id":"floor","color":[0,0,0]}"#).unwrap();
        assert_eq!(legacy.seam.mode, SeamMode::Cut);
    }
}
