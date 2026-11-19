//! Mask-based map operators. All coordinates and distances are output pixels.
use crate::{
    connections::{representatives, tree_edges},
    graph::{ConnectRegions, Image, Result, finite},
    recipe::derive_seed,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MorphMode {
    Dilate,
    Erode,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum MapNode {
    Morphology {
        mode: MorphMode,
        radius: u32,
        cutoff: f32,
    },
    Outline {
        width: u32,
        cutoff: f32,
    },
    Distance {
        range: f32,
        inside: bool,
        cutoff: f32,
    },
    Scatter {
        count: u32,
        min_distance: f32,
        radius: u32,
        seed: u64,
        cutoff: f32,
    },
    Entrances {
        #[serde(default)]
        rectangle: bool,
        #[serde(default = "entrance_depth")]
        depth: u32,
        #[serde(default)]
        offset: f32,
        #[serde(default = "entrance_merge")]
        merge: bool,
        width: u32,
        spacing: f32,
        cutoff: f32,
    },
    ExtraConnections {
        count: u32,
        width: u32,
        distortion: f32,
        frequency: f32,
        seed: u64,
        cutoff: f32,
        include_source: bool,
    },
    SelectRegions {
        min_area: u32,
        max_area: u32,
        largest_only: bool,
        cutoff: f32,
    },
    Cellular {
        iterations: u32,
        birth_limit: u32,
        survival_limit: u32,
        cutoff: f32,
    },
    Partition {
        min_size: u32,
        depth: u32,
        wall_width: u32,
        seed: u64,
        cutoff: f32,
    },
    Reachable {
        start_x: u32,
        start_y: u32,
        cutoff: f32,
    },
}
impl MapNode {
    pub fn catalog() -> Vec<Self> {
        vec![
            Self::Morphology {
                mode: MorphMode::Dilate,
                radius: 2,
                cutoff: 0.5,
            },
            Self::Outline {
                width: 2,
                cutoff: 0.5,
            },
            Self::Distance {
                range: 16.0,
                inside: true,
                cutoff: 0.5,
            },
            Self::Scatter {
                count: 32,
                min_distance: 8.0,
                radius: 0,
                seed: 42,
                cutoff: 0.5,
            },
            Self::Entrances {
                rectangle: false,
                depth: 1,
                offset: 0.0,
                merge: true,
                width: 3,
                spacing: 8.0,
                cutoff: 0.5,
            },
            Self::ExtraConnections {
                count: 2,
                width: 3,
                distortion: 0.02,
                frequency: 1.5,
                seed: 42,
                cutoff: 0.5,
                include_source: true,
            },
            Self::SelectRegions {
                min_area: 16,
                max_area: 1048576,
                largest_only: false,
                cutoff: 0.5,
            },
            Self::Cellular {
                iterations: 4,
                birth_limit: 5,
                survival_limit: 4,
                cutoff: 0.5,
            },
            Self::Partition {
                min_size: 12,
                depth: 3,
                wall_width: 2,
                seed: 42,
                cutoff: 0.5,
            },
            Self::Reachable {
                start_x: 0,
                start_y: 0,
                cutoff: 0.5,
            },
        ]
    }
    pub fn title(&self) -> &'static str {
        match self {
            Self::Morphology { .. } => "Morphology",
            Self::Outline { .. } => "Wall Outline",
            Self::Distance { .. } => "Distance Map",
            Self::Scatter { .. } => "Mask Scatter",
            Self::Entrances { .. } => "Entrance Candidates",
            Self::ExtraConnections { .. } => "Extra Connections",
            Self::SelectRegions { .. } => "Select Regions",
            Self::Cellular { .. } => "Cellular Automata",
            Self::Partition { .. } => "Partition",
            Self::Reachable { .. } => "Reachable Area",
        }
    }
    pub fn inputs(&self) -> &'static [&'static str] {
        match self {
            Self::Entrances { .. } => &["Rooms", "Corridors"],
            Self::ExtraConnections { .. } => &["Rooms", "Existing Paths"],
            _ => &["Source"],
        }
    }
    pub fn seed_mut(&mut self) -> Option<&mut u64> {
        match self {
            Self::Scatter { seed, .. }
            | Self::ExtraConnections { seed, .. }
            | Self::Partition { seed, .. } => Some(seed),
            _ => None,
        }
    }
    pub fn validate(&self) -> bool {
        let cutoff = match self {
            Self::Morphology { cutoff, .. }
            | Self::Outline { cutoff, .. }
            | Self::Distance { cutoff, .. }
            | Self::Scatter { cutoff, .. }
            | Self::Entrances { cutoff, .. }
            | Self::ExtraConnections { cutoff, .. }
            | Self::SelectRegions { cutoff, .. }
            | Self::Cellular { cutoff, .. }
            | Self::Partition { cutoff, .. }
            | Self::Reachable { cutoff, .. } => *cutoff,
        };
        finite(cutoff, 0.01, 1.0)
            && match self {
                Self::Morphology { radius, .. } => *radius <= 64,
                Self::Outline { width, .. } => (1..=64).contains(width),
                Self::Distance { range, .. } => finite(*range, 1.0, 1024.0),
                Self::Scatter {
                    count,
                    min_distance,
                    radius,
                    ..
                } => *count <= 4096 && finite(*min_distance, 1.0, 1024.0) && *radius <= 32,
                Self::Entrances {
                    width,
                    depth,
                    offset,
                    spacing,
                    ..
                } => {
                    (1..=64).contains(width)
                        && (1..=64).contains(depth)
                        && finite(*offset, 0.0, 64.0)
                        && finite(*spacing, 0.0, 1024.0)
                }
                Self::ExtraConnections {
                    count,
                    width,
                    distortion,
                    frequency,
                    ..
                } => {
                    *count <= 64
                        && (1..=64).contains(width)
                        && finite(*distortion, 0.0, 0.25)
                        && finite(*frequency, 0.5, 8.0)
                }
                Self::SelectRegions {
                    min_area, max_area, ..
                } => *min_area >= 1 && *min_area <= *max_area && *max_area <= 1048576,
                Self::Cellular {
                    iterations,
                    birth_limit,
                    survival_limit,
                    ..
                } => *iterations <= 64 && *birth_limit <= 8 && *survival_limit <= 8,
                Self::Partition {
                    min_size,
                    depth,
                    wall_width,
                    ..
                } => (2..=512).contains(min_size) && *depth <= 8 && (1..=32).contains(wall_width),
                Self::Reachable {
                    start_x, start_y, ..
                } => *start_x <= 1023 && *start_y <= 1023,
            }
    }
    pub fn render(&self, inputs: &[Arc<Image>], cancelled: &dyn Fn() -> bool) -> Result<Image> {
        check(cancelled)?;
        let source = &inputs[0];
        let n = source.size;
        let mut output = vec![0.0; n * n];
        match *self {
            Self::Morphology {
                mode,
                radius,
                cutoff,
            } => {
                let mask = binary(source, cutoff);
                let distance = distances(&mask, n, mode == MorphMode::Dilate, cancelled)?;
                for i in 0..n * n {
                    output[i] = f32::from(match mode {
                        MorphMode::Dilate => distance[i] <= (radius * radius) as f32,
                        MorphMode::Erode => {
                            mask[i]
                                && distance[i].min(edge_distance(i, n).powi(2))
                                    > (radius * radius) as f32
                        }
                    });
                }
            }
            Self::Outline { width, cutoff } => {
                let mask = binary(source, cutoff);
                let distance = distances(&mask, n, true, cancelled)?;
                for i in 0..n * n {
                    output[i] = f32::from(!mask[i] && distance[i] <= (width * width) as f32);
                }
            }
            Self::Distance {
                range,
                inside,
                cutoff,
            } => {
                let mask = binary(source, cutoff);
                let distance = distances(&mask, n, !inside, cancelled)?;
                for i in 0..n * n {
                    output[i] = if inside {
                        if mask[i] {
                            distance[i].sqrt().min(edge_distance(i, n)) / range
                        } else {
                            0.0
                        }
                    } else {
                        distance[i].sqrt() / range
                    }
                    .clamp(0.0, 1.0);
                }
            }
            Self::Scatter {
                count,
                min_distance,
                radius,
                seed,
                cutoff,
            } => {
                if count == 0 {
                    check(cancelled)?;
                    return Ok(Image {
                        alpha: None,
                        terrain: None,
                        size: n,
                        pixels: output,
                    });
                }
                let mask = binary(source, cutoff);
                let mut candidates: Vec<_> = (0..n * n).filter(|&i| mask[i]).collect();
                for i in (1..candidates.len()).rev() {
                    if i % 4096 == 0 {
                        check(cancelled)?;
                    }
                    let j = ((derive_seed(seed, &i.to_string()) as u128 * (i + 1) as u128) >> 64)
                        as usize;
                    candidates.swap(i, j);
                }
                let cell = min_distance.ceil() as usize;
                let columns = n.div_ceil(cell);
                let mut buckets: Vec<Vec<(usize, usize)>> = vec![vec![]; columns * columns];
                let mut placed = 0;
                for (iteration, i) in candidates.into_iter().enumerate() {
                    if placed == count {
                        break;
                    }
                    if iteration % 1024 == 0 {
                        check(cancelled)?;
                    }
                    let (x, y) = (i % n, i / n);
                    let (bx, by) = (x / cell, y / cell);
                    let mut valid = true;
                    for ny in by.saturating_sub(1)..=(by + 1).min(columns - 1) {
                        for nx in bx.saturating_sub(1)..=(bx + 1).min(columns - 1) {
                            if buckets[ny * columns + nx].iter().any(|&(px, py)| {
                                squared((x, y), (px, py)) < min_distance * min_distance
                            }) {
                                valid = false;
                            }
                        }
                    }
                    if valid {
                        buckets[by * columns + bx].push((x, y));
                        placed += 1;
                        for py in
                            y.saturating_sub(radius as usize)..=(y + radius as usize).min(n - 1)
                        {
                            for px in
                                x.saturating_sub(radius as usize)..=(x + radius as usize).min(n - 1)
                            {
                                if mask[py * n + px]
                                    && squared((x, y), (px, py)) <= (radius * radius) as f32
                                {
                                    output[py * n + px] = 1.0;
                                }
                            }
                        }
                    }
                }
            }
            Self::Entrances {
                rectangle,
                depth,
                offset,
                merge,
                width,
                spacing,
                cutoff,
            } => {
                output = entrance_markers(
                    &binary(source, cutoff),
                    &binary(&inputs[1], cutoff),
                    n,
                    EntranceMarker {
                        rectangle,
                        depth,
                        offset,
                        merge,
                        width,
                        spacing,
                    },
                    cancelled,
                )?;
            }
            Self::ExtraConnections {
                count,
                width,
                distortion,
                frequency,
                seed,
                cutoff,
                include_source,
            } => {
                if count == 0 {
                    check(cancelled)?;
                    if include_source {
                        for (i, p) in output.iter_mut().enumerate() {
                            *p = source.pixels[i].max(inputs[1].pixels[i]);
                        }
                    }
                    return Ok(Image {
                        alpha: None,
                        terrain: None,
                        size: n,
                        pixels: output,
                    });
                }
                let points = representatives(source, cutoff, cancelled)?;
                // Candidate pairs are bounded explicitly; the base connector still supports 4096 regions.
                if points.len() > 512 {
                    return Err(
                        "Extra Connectionsは最大512領域です。Select Regionsで対象を絞ってください"
                            .into(),
                    );
                }
                let tree: BTreeSet<_> = tree_edges(&points, cancelled)?
                    .into_iter()
                    .map(|(a, b)| (a.min(b), a.max(b)))
                    .collect();
                let mut pairs = Vec::new();
                for a in 0..points.len() {
                    check(cancelled)?;
                    for b in a + 1..points.len() {
                        if !tree.contains(&(a, b)) {
                            pairs.push((squared(points[a], points[b]), a, b));
                        }
                    }
                }
                pairs.sort_by(|a, b| {
                    a.0.total_cmp(&b.0)
                        .then_with(|| (a.1, a.2).cmp(&(b.1, b.2)))
                });
                let base = Image {
                    alpha: None,
                    terrain: None,
                    size: n,
                    pixels: source
                        .pixels
                        .iter()
                        .zip(&inputs[1].pixels)
                        .map(|(a, b)| a.max(*b))
                        .collect(),
                };
                let settings = ConnectRegions {
                    cutoff,
                    width,
                    distortion,
                    frequency,
                    seed,
                    include_source: false,
                };
                let mut selected = 0;
                for (_, a, b) in pairs.into_iter().take((count as usize * 4).clamp(16, 256)) {
                    if selected == count {
                        break;
                    }
                    check(cancelled)?;
                    let candidate = settings.render_links(source, &points, &[(a, b)], cancelled)?;
                    if candidate
                        .pixels
                        .iter()
                        .zip(&base.pixels)
                        .zip(&output)
                        .any(|((&p, &old), &added)| p >= cutoff && old < cutoff && added < cutoff)
                    {
                        for (dst, p) in output.iter_mut().zip(candidate.pixels) {
                            *dst = dst.max(p);
                        }
                        selected += 1;
                    }
                }
                if include_source {
                    for (dst, old) in output.iter_mut().zip(base.pixels) {
                        *dst = dst.max(old);
                    }
                }
            }
            Self::SelectRegions {
                min_area,
                max_area,
                largest_only,
                cutoff,
            } => {
                let mask = binary(source, cutoff);
                let groups = components(&mask, n, cancelled)?;
                let eligible = groups
                    .iter()
                    .filter(|g| (min_area as usize..=max_area as usize).contains(&g.len()));
                if largest_only {
                    if let Some(group) = eligible.max_by_key(|g| g.len()) {
                        for &i in group {
                            output[i] = source.pixels[i];
                        }
                    }
                } else {
                    for group in eligible {
                        for &i in group {
                            output[i] = source.pixels[i];
                        }
                    }
                }
            }
            Self::Cellular {
                iterations,
                birth_limit,
                survival_limit,
                cutoff,
            } => {
                let mut mask = binary(source, cutoff);
                let mut next = vec![false; n * n];
                for _ in 0..iterations {
                    for y in 0..n {
                        check(cancelled)?;
                        for x in 0..n {
                            let mut count = 0;
                            for dy in -1isize..=1 {
                                for dx in -1isize..=1 {
                                    let (nx, ny) = (x as isize + dx, y as isize + dy);
                                    if (dx != 0 || dy != 0)
                                        && nx >= 0
                                        && ny >= 0
                                        && nx < n as isize
                                        && ny < n as isize
                                        && mask[ny as usize * n + nx as usize]
                                    {
                                        count += 1;
                                    }
                                }
                            }
                            let i = y * n + x;
                            next[i] = count >= if mask[i] { survival_limit } else { birth_limit };
                        }
                    }
                    std::mem::swap(&mut mask, &mut next);
                }
                return Ok(image(n, &mask));
            }
            Self::Partition {
                min_size,
                depth,
                wall_width,
                seed,
                cutoff,
            } => {
                let mask = binary(source, cutoff);
                output = mask.iter().map(|b| f32::from(*b)).collect();
                for group in components(&mask, n, cancelled)? {
                    let minx = group.iter().map(|i| i % n).min().unwrap();
                    let maxx = group.iter().map(|i| i % n).max().unwrap() + 1;
                    let miny = group.iter().map(|i| i / n).min().unwrap();
                    let maxy = group.iter().map(|i| i / n).max().unwrap() + 1;
                    let mut stack = vec![(
                        minx,
                        miny,
                        maxx,
                        maxy,
                        depth,
                        derive_seed(seed, &group[0].to_string()),
                    )];
                    while let Some((x0, y0, x1, y1, remaining, key)) = stack.pop() {
                        check(cancelled)?;
                        if remaining == 0 {
                            continue;
                        }
                        let (w, h) = (x1 - x0, y1 - y0);
                        let required = 2 * min_size as usize + wall_width as usize;
                        let horizontal = if w < required {
                            true
                        } else if h < required {
                            false
                        } else if w == h {
                            key & 1 == 0
                        } else {
                            h > w
                        };
                        let length = if horizontal { h } else { w };
                        if length < required {
                            continue;
                        }
                        let offset = min_size as usize
                            + ((derive_seed(key, "cut") as u128 * (length - required + 1) as u128)
                                >> 64) as usize;
                        let width = wall_width as usize;
                        if horizontal {
                            let cut = y0 + offset;
                            for y in cut..cut + width {
                                for x in x0..x1 {
                                    output[y * n + x] = 0.0;
                                }
                            }
                            stack.push((x0, y0, x1, cut, remaining - 1, derive_seed(key, "a")));
                            stack.push((
                                x0,
                                cut + width,
                                x1,
                                y1,
                                remaining - 1,
                                derive_seed(key, "b"),
                            ));
                        } else {
                            let cut = x0 + offset;
                            for y in y0..y1 {
                                for x in cut..cut + width {
                                    output[y * n + x] = 0.0;
                                }
                            }
                            stack.push((x0, y0, cut, y1, remaining - 1, derive_seed(key, "a")));
                            stack.push((
                                cut + width,
                                y0,
                                x1,
                                y1,
                                remaining - 1,
                                derive_seed(key, "b"),
                            ));
                        }
                    }
                }
            }
            Self::Reachable {
                start_x,
                start_y,
                cutoff,
            } => {
                if start_x as usize >= n || start_y as usize >= n {
                    return Err("開始座標が画像の範囲外です".into());
                }
                let mask = binary(source, cutoff);
                let start = start_y as usize * n + start_x as usize;
                if mask[start] {
                    let mut visited = vec![false; n * n];
                    for i in flood(&mask, n, start, &mut visited, cancelled)? {
                        output[i] = source.pixels[i];
                    }
                }
            }
        }
        check(cancelled)?;
        Ok(Image {
            alpha: None,
            terrain: None,
            size: n,
            pixels: output,
        })
    }
}
fn entrance_depth() -> u32 {
    1
}
fn entrance_merge() -> bool {
    true
}

#[derive(Clone, Copy)]
struct EntranceMarker {
    rectangle: bool,
    depth: u32,
    offset: f32,
    merge: bool,
    width: u32,
    spacing: f32,
}
#[derive(Clone, Copy)]
struct DoorMarker {
    x: f32,
    y: f32,
    dx: i32,
    dy: i32,
    w: usize,
    h: usize,
}
fn entrance_markers(
    rooms: &[bool],
    paths: &[bool],
    n: usize,
    settings: EntranceMarker,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<f32>> {
    let EntranceMarker {
        rectangle,
        depth,
        offset,
        merge,
        width,
        spacing,
    } = settings;
    let directions = [(-1, 0), (1, 0), (0, -1), (0, 1)];
    let mut contacts = vec![vec![false; n * n]; if rectangle || offset > 0.0 { 4 } else { 1 }];
    for y in 0..n {
        check(cancelled)?;
        for x in 0..n {
            let i = y * n + x;
            if !rooms[i] || !paths[i] {
                continue;
            }
            let mut best = None;
            for (d, &(dx, dy)) in directions.iter().enumerate() {
                let (px, py) = (x as i32 + dx, y as i32 + dy);
                let inside = px >= 0 && py >= 0 && px < n as i32 && py < n as i32;
                let j = if inside {
                    py as usize * n + px as usize
                } else {
                    0
                };
                if inside && !rooms[j] {
                    let score = if inside && paths[j] { 2 } else { 1 };
                    if best.is_none_or(|(_, previous)| score > previous) {
                        best = Some((d, score));
                    }
                }
            }
            if let Some((d, _)) = best {
                let group = if contacts.len() == 1 { 0 } else { d };
                contacts[group][i] = true;
            }
        }
    }
    let mut markers = Vec::<DoorMarker>::new();
    for (direction, mask) in contacts.iter().enumerate() {
        for (x, y) in representatives(&image(n, mask), 0.5, cancelled)? {
            let (dx, dy) = directions[direction];
            let (w, h) = if rectangle {
                if dx != 0 {
                    (depth as usize, width as usize)
                } else {
                    (width as usize, depth as usize)
                }
            } else {
                (width as usize, width as usize)
            };
            markers.push(DoorMarker {
                x: x as f32 + dx as f32 * offset,
                y: y as f32 + dy as f32 * offset,
                dx,
                dy,
                w,
                h,
            });
        }
    }
    if markers.len() > 4096 {
        return Err("入口候補は最大4096個です".into());
    }
    let mut consumed = vec![false; markers.len()];
    let mut chosen = Vec::<DoorMarker>::new();
    for i in 0..markers.len() {
        check(cancelled)?;
        if consumed[i] {
            continue;
        }
        let mut marker = markers[i];
        if rectangle && merge {
            // Pair opposite room boundaries only when their shifted door footprints overlap.
            let partner = (i + 1..markers.len())
                .filter(|&j| !consumed[j])
                .filter(|&j| {
                    let b = markers[j];
                    marker.dx == -b.dx
                        && marker.dy == -b.dy
                        && (marker.x - b.x).abs() < (marker.w + b.w) as f32 * 0.5
                        && (marker.y - b.y).abs() < (marker.h + b.h) as f32 * 0.5
                })
                .min_by(|&a, &b| {
                    let distance =
                        |m: DoorMarker| (m.x - marker.x).powi(2) + (m.y - marker.y).powi(2);
                    distance(markers[a])
                        .total_cmp(&distance(markers[b]))
                        .then(a.cmp(&b))
                });
            if let Some(j) = partner {
                marker.x = (marker.x + markers[j].x) * 0.5;
                marker.y = (marker.y + markers[j].y) * 0.5;
                consumed[j] = true;
            }
        }
        if chosen
            .iter()
            .all(|p| (marker.x - p.x).powi(2) + (marker.y - p.y).powi(2) >= spacing * spacing)
        {
            chosen.push(marker);
        }
    }
    let mut output = vec![0.0; n * n];
    for marker in chosen {
        let x0 = (marker.x - (marker.w as f32 - 1.0) * 0.5).round() as i32;
        let y0 = (marker.y - (marker.h as f32 - 1.0) * 0.5).round() as i32;
        for py in y0.max(0)..(y0 + marker.h as i32).min(n as i32) {
            for px in x0.max(0)..(x0 + marker.w as i32).min(n as i32) {
                output[py as usize * n + px as usize] = 1.0;
            }
        }
    }
    Ok(output)
}

fn check(cancelled: &dyn Fn() -> bool) -> Result<()> {
    if cancelled() {
        Err("cancelled".into())
    } else {
        Ok(())
    }
}
fn binary(image: &Image, cutoff: f32) -> Vec<bool> {
    image.pixels.iter().map(|p| *p >= cutoff).collect()
}
fn image(size: usize, mask: &[bool]) -> Image {
    Image {
        alpha: None,
        terrain: None,
        size,
        pixels: mask.iter().map(|b| f32::from(*b)).collect(),
    }
}
fn squared(a: (usize, usize), b: (usize, usize)) -> f32 {
    (a.0 as f32 - b.0 as f32).powi(2) + (a.1 as f32 - b.1 as f32).powi(2)
}
fn edge_distance(i: usize, n: usize) -> f32 {
    (i % n + 1).min(n - i % n).min(i / n + 1).min(n - i / n) as f32
}
fn neighbors(i: usize, n: usize) -> [Option<usize>; 4] {
    let (x, y) = (i % n, i / n);
    [
        (x > 0).then(|| i - 1),
        (x + 1 < n).then_some(i + 1),
        (y > 0).then(|| i - n),
        (y + 1 < n).then_some(i + n),
    ]
}
fn flood(
    mask: &[bool],
    n: usize,
    start: usize,
    visited: &mut [bool],
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<usize>> {
    let mut queue = vec![start];
    visited[start] = true;
    let mut head = 0;
    while head < queue.len() {
        if head % 4096 == 0 {
            check(cancelled)?;
        }
        let i = queue[head];
        head += 1;
        for j in neighbors(i, n).into_iter().flatten() {
            if mask[j] && !visited[j] {
                visited[j] = true;
                queue.push(j);
            }
        }
    }
    Ok(queue)
}
fn components(mask: &[bool], n: usize, cancelled: &dyn Fn() -> bool) -> Result<Vec<Vec<usize>>> {
    let mut visited = vec![false; n * n];
    let mut result = Vec::new();
    for i in 0..n * n {
        if i % n == 0 {
            check(cancelled)?;
        }
        if mask[i] && !visited[i] {
            result.push(flood(mask, n, i, &mut visited, cancelled)?);
        }
    }
    Ok(result)
}
// Exact squared Euclidean distance transform using envelopes of parabolas, O(size²).
fn distances(
    mask: &[bool],
    n: usize,
    target: bool,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<f32>> {
    if !mask.contains(&target) {
        check(cancelled)?;
        return Ok(vec![f32::INFINITY; n * n]);
    }
    let far = (n * n * 4) as f32;
    let mut values: Vec<_> = mask
        .iter()
        .map(|&b| if b == target { 0.0 } else { far })
        .collect();
    let mut row = vec![0.0; n];
    let mut transformed = vec![0.0; n];
    for y in 0..n {
        check(cancelled)?;
        edt_line(&values[y * n..(y + 1) * n], &mut transformed);
        values[y * n..(y + 1) * n].copy_from_slice(&transformed);
    }
    for x in 0..n {
        check(cancelled)?;
        for y in 0..n {
            row[y] = values[y * n + x];
        }
        edt_line(&row, &mut transformed);
        for y in 0..n {
            values[y * n + x] = transformed[y];
        }
    }
    Ok(values)
}
fn edt_line(f: &[f32], d: &mut [f32]) {
    let n = f.len();
    let mut v = vec![0usize; n];
    let mut z = vec![0.0; n + 1];
    let mut k = 0;
    z[0] = f32::NEG_INFINITY;
    z[1] = f32::INFINITY;
    for q in 1..n {
        let mut s;
        loop {
            let p = v[k];
            s = ((f[q] + (q * q) as f32) - (f[p] + (p * p) as f32)) / (2.0 * (q - p) as f32);
            if s > z[k] {
                break;
            }
            k -= 1;
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f32::INFINITY;
    }
    k = 0;
    for (q, value) in d.iter_mut().enumerate() {
        while z[k + 1] < (q as f32) {
            k += 1;
        }
        *value = (q as f32 - v[k] as f32).powi(2) + f[v[k]];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        graph::{Graph, ImageNode, NodeKind, NumberInput, RenderContext},
        recipe::{Recipe, numeric_parameters},
    };
    use std::collections::BTreeMap;
    fn blank(n: usize) -> Image {
        Image {
            alpha: None,
            terrain: None,
            size: n,
            pixels: vec![0.0; n * n],
        }
    }
    fn rectangle(image: &mut Image, x0: usize, y0: usize, x1: usize, y1: usize) {
        for y in y0..y1 {
            for x in x0..x1 {
                image.pixels[y * image.size + x] = 1.0;
            }
        }
    }
    fn run(node: MapNode, sources: &[Image]) -> Image {
        let n = sources[0].size;
        NodeKind::Map(node)
            .evaluate(
                &sources.iter().cloned().map(Arc::new).collect::<Vec<_>>(),
                RenderContext {
                    size: n,
                    extent: 64.0,
                },
                &|| false,
            )
            .unwrap()
    }
    #[test]
    fn exact_distances_match_brute_force_and_morphology_handles_empty_edges() {
        let n = 16;
        let mut mask = vec![false; n * n];
        for (i, value) in mask.iter_mut().enumerate() {
            *value = derive_seed(7, &i.to_string()).is_multiple_of(13);
        }
        for target in [false, true] {
            let d = distances(&mask, n, target, &|| false).unwrap();
            for (i, actual) in d.iter().enumerate() {
                let expected = (0..n * n)
                    .filter(|&j| mask[j] == target)
                    .map(|j| squared((i % n, i / n), (j % n, j / n)))
                    .fold(f32::INFINITY, f32::min);
                assert!((actual - expected).abs() < 0.001);
            }
        }
        let empty = blank(n);
        let full = Image {
            alpha: None,
            terrain: None,
            size: n,
            pixels: vec![1.0; n * n],
        };
        assert!(
            run(
                MapNode::Morphology {
                    mode: MorphMode::Dilate,
                    radius: 64,
                    cutoff: 0.5
                },
                std::slice::from_ref(&empty)
            )
            .pixels
            .iter()
            .all(|v| *v == 0.0)
        );
        let eroded = run(
            MapNode::Morphology {
                mode: MorphMode::Erode,
                radius: 2,
                cutoff: 0.5,
            },
            std::slice::from_ref(&full),
        );
        assert_eq!(eroded.pixels.iter().sum::<f32>(), 144.0);
        let mut source = blank(n);
        rectangle(&mut source, 6, 6, 10, 10);
        let expanded = run(
            MapNode::Morphology {
                mode: MorphMode::Dilate,
                radius: 2,
                cutoff: 0.5,
            },
            std::slice::from_ref(&source),
        );
        let outline = run(
            MapNode::Outline {
                width: 2,
                cutoff: 0.5,
            },
            std::slice::from_ref(&source),
        );
        for i in 0..n * n {
            assert_eq!(expanded.pixels[i], source.pixels[i].max(outline.pixels[i]));
            assert_eq!(source.pixels[i] * outline.pixels[i], 0.0);
        }
        let distance = run(
            MapNode::Distance {
                range: 4.0,
                inside: true,
                cutoff: 0.5,
            },
            std::slice::from_ref(&source),
        );
        assert_eq!(distance.pixels[7 * n + 7], 0.5);
        assert_eq!(distance.pixels[0], 0.0);
        assert!(
            run(
                MapNode::Distance {
                    range: 4.0,
                    inside: false,
                    cutoff: 0.5
                },
                &[empty]
            )
            .pixels
            .iter()
            .all(|v| *v == 1.0)
        );
    }
    #[test]
    fn scatter_is_seeded_stays_in_mask_and_respects_center_spacing() {
        let mut source = blank(32);
        rectangle(&mut source, 2, 2, 30, 30);
        let node = MapNode::Scatter {
            count: 12,
            min_distance: 5.0,
            radius: 0,
            seed: 42,
            cutoff: 0.5,
        };
        let a = run(node.clone(), std::slice::from_ref(&source));
        let b = run(node.clone(), std::slice::from_ref(&source));
        assert_eq!(a.pixels, b.pixels);
        let points: Vec<_> = a
            .pixels
            .iter()
            .enumerate()
            .filter(|(_, p)| **p > 0.5)
            .map(|(i, _)| (i % 32, i / 32))
            .collect();
        assert_eq!(points.len(), 12);
        for (i, &point) in points.iter().enumerate() {
            assert_eq!(source.pixels[point.1 * 32 + point.0], 1.0);
            for &other in &points[i + 1..] {
                assert!(squared(point, other) >= 25.0);
            }
        }
        assert_ne!(
            a.pixels,
            run(
                MapNode::Scatter {
                    count: 12,
                    min_distance: 5.0,
                    radius: 0,
                    seed: 43,
                    cutoff: 0.5
                },
                &[source]
            )
            .pixels
        );
        assert!(run(node, &[blank(32)]).pixels.iter().all(|v| *v == 0.0));
    }
    #[test]
    fn rectangular_entrances_rotate_offset_merge_and_clip() {
        for transpose in [false, true] {
            let n = 32;
            let mut rooms = blank(n);
            let mut path = blank(n);
            rectangle(&mut rooms, 2, 4, 15, 28);
            rectangle(&mut rooms, 17, 4, 30, 28);
            rectangle(&mut path, 14, 16, 18, 17);
            if transpose {
                for image in [&mut rooms, &mut path] {
                    let old = image.pixels.clone();
                    for y in 0..n {
                        for x in 0..n {
                            image.pixels[y * n + x] = old[x * n + y];
                        }
                    }
                }
            }
            let marker = MapNode::Entrances {
                rectangle: true,
                depth: 2,
                offset: 1.5,
                merge: true,
                width: 3,
                spacing: 0.0,
                cutoff: 0.5,
            };
            let result = run(marker.clone(), &[rooms.clone(), path.clone()]);
            // One merged 2x3 footprint in the two-pixel divider.
            for y in 0..n {
                for x in 0..n {
                    let (normal, tangent) = if transpose { (y, x) } else { (x, y) };
                    let expected = (15..17).contains(&normal) && (15..18).contains(&tangent);
                    assert_eq!(
                        result.pixels[y * n + x],
                        f32::from(expected),
                        "transpose={transpose}, x={x}, y={y}"
                    );
                }
            }
            let merged = run(
                MapNode::Entrances {
                    rectangle: true,
                    depth: 4,
                    offset: 0.0,
                    merge: true,
                    width: 3,
                    spacing: 0.0,
                    cutoff: 0.5,
                },
                &[rooms.clone(), path.clone()],
            );
            let separate = run(
                MapNode::Entrances {
                    rectangle: true,
                    depth: 4,
                    offset: 0.0,
                    merge: false,
                    width: 3,
                    spacing: 0.0,
                    cutoff: 0.5,
                },
                &[rooms.clone(), path.clone()],
            );
            assert_eq!(merged.pixels.iter().sum::<f32>(), 12.0);
            assert_eq!(separate.pixels.iter().sum::<f32>(), 21.0);
            let output = marker.render(&[Arc::new(rooms), Arc::new(path)], &|| true);
            assert!(output.is_err());
        }
        let mut rooms = blank(16);
        let mut path = blank(16);
        rectangle(&mut rooms, 1, 1, 8, 15);
        rectangle(&mut path, 0, 8, 16, 9);
        let result = run(
            MapNode::Entrances {
                rectangle: true,
                depth: 3,
                offset: 64.0,
                merge: true,
                width: 5,
                spacing: 0.0,
                cutoff: 0.5,
            },
            &[rooms, path],
        );
        assert!(result.pixels.iter().all(|v| *v == 0.0));
        let legacy: MapNode = serde_json::from_str(
            r#"{"operation":"entrances","width":1,"spacing":0.0,"cutoff":0.5}"#,
        )
        .unwrap();
        assert!(legacy.validate());
    }

    #[test]
    fn entrances_find_both_sides_and_extra_paths_close_a_loop() {
        let mut room = blank(16);
        rectangle(&mut room, 4, 4, 12, 12);
        let mut path = blank(16);
        rectangle(&mut path, 0, 7, 16, 8);
        let doors = run(
            MapNode::Entrances {
                rectangle: false,
                depth: 1,
                offset: 0.0,
                merge: true,
                width: 1,
                spacing: 0.0,
                cutoff: 0.5,
            },
            &[room, path],
        );
        assert_eq!(doors.pixels.iter().sum::<f32>(), 2.0);
        assert_eq!(doors.pixels[7 * 16 + 4], 1.0);
        assert_eq!(doors.pixels[7 * 16 + 11], 1.0);
        let mut rooms = blank(32);
        for (x, y) in [(4, 4), (24, 4), (4, 24), (24, 24)] {
            rectangle(&mut rooms, x, y, x + 3, y + 3);
        }
        let existing = ConnectRegions {
            width: 1,
            distortion: 0.0,
            ..Default::default()
        }
        .render(&rooms, &|| false)
        .unwrap();
        let extra = run(
            MapNode::ExtraConnections {
                count: 1,
                width: 1,
                distortion: 0.0,
                frequency: 1.5,
                seed: 42,
                cutoff: 0.5,
                include_source: true,
            },
            &[rooms, existing.clone()],
        );
        assert!(extra.pixels.iter().sum::<f32>() > existing.pixels.iter().sum::<f32>());
        let white = binary(&extra, 0.5);
        assert_eq!(components(&white, 32, &|| false).unwrap().len(), 1);
        let black: Vec<_> = white.iter().map(|v| !*v).collect();
        let black_groups = components(&black, 32, &|| false).unwrap();
        assert!(
            black_groups.iter().any(|g| g
                .iter()
                .all(|i| i % 32 > 0 && i % 32 < 31 && i / 32 > 0 && i / 32 < 31)),
            "extra edge must create an enclosed loop in this fixture"
        );
    }
    #[test]
    fn selection_cellular_partition_and_reachability_have_expected_topology() {
        let mut source = blank(32);
        rectangle(&mut source, 2, 2, 3, 3);
        rectangle(&mut source, 8, 8, 10, 10);
        rectangle(&mut source, 20, 20, 23, 23);
        let selected = run(
            MapNode::SelectRegions {
                min_area: 2,
                max_area: 8,
                largest_only: false,
                cutoff: 0.5,
            },
            std::slice::from_ref(&source),
        );
        assert_eq!(selected.pixels.iter().sum::<f32>(), 4.0);
        let largest = run(
            MapNode::SelectRegions {
                min_area: 1,
                max_area: 1024,
                largest_only: true,
                cutoff: 0.5,
            },
            std::slice::from_ref(&source),
        );
        assert_eq!(largest.pixels.iter().sum::<f32>(), 9.0);
        let reachable = run(
            MapNode::Reachable {
                start_x: 20,
                start_y: 20,
                cutoff: 0.5,
            },
            std::slice::from_ref(&source),
        );
        assert_eq!(reachable.pixels, largest.pixels);
        assert!(
            run(
                MapNode::Reachable {
                    start_x: 0,
                    start_y: 0,
                    cutoff: 0.5
                },
                std::slice::from_ref(&source)
            )
            .pixels
            .iter()
            .all(|v| *v == 0.0)
        );
        let mut isolated = blank(16);
        isolated.pixels[8 * 16 + 8] = 1.0;
        assert!(
            run(
                MapNode::Cellular {
                    iterations: 1,
                    birth_limit: 5,
                    survival_limit: 1,
                    cutoff: 0.5
                },
                &[isolated]
            )
            .pixels
            .iter()
            .all(|v| *v == 0.0)
        );
        let full = Image {
            alpha: None,
            terrain: None,
            size: 32,
            pixels: vec![1.0; 1024],
        };
        let partition = MapNode::Partition {
            min_size: 6,
            depth: 2,
            wall_width: 2,
            seed: 42,
            cutoff: 0.5,
        };
        let a = run(partition.clone(), std::slice::from_ref(&full));
        let groups = components(&binary(&a, 0.5), 32, &|| false).unwrap();
        assert_eq!(groups.len(), 4);
        for group in groups {
            let width = group.iter().map(|i| i % 32).max().unwrap()
                - group.iter().map(|i| i % 32).min().unwrap()
                + 1;
            let height = group.iter().map(|i| i / 32).max().unwrap()
                - group.iter().map(|i| i / 32).min().unwrap()
                + 1;
            assert!(width >= 6 && height >= 6);
        }
        assert_eq!(a.pixels, run(partition, std::slice::from_ref(&full)).pixels);
        assert_ne!(
            a.pixels,
            run(
                MapNode::Partition {
                    min_size: 6,
                    depth: 2,
                    wall_width: 2,
                    seed: 43,
                    cutoff: 0.5
                },
                &[full]
            )
            .pixels
        );
    }
    #[test]
    fn every_map_node_supports_recipe_parameters_validation_and_cancellation() {
        assert_eq!(MapNode::catalog().len(), 10);
        for settings in MapNode::catalog() {
            let mut graph = Graph {
                resolution: 16,
                ..Default::default()
            };
            let source = graph
                .add(NodeKind::Constant { value: 0.75 }, [0.0, 0.0])
                .unwrap();
            let node = graph
                .add(NodeKind::Map(settings.clone()), [260.0, 0.0])
                .unwrap();
            for index in 0..settings.inputs().len() {
                graph.connect(source, node, index).unwrap();
            }
            let input = graph
                .add(NodeKind::Input(NumberInput::default()), [0.0, 340.0])
                .unwrap();
            let key = numeric_parameters(&graph.node(node).unwrap().kind)[0].clone();
            let json = serde_json::to_value(&graph.node(node).unwrap().kind).unwrap();
            let value = json[&key].as_f64().unwrap();
            graph.node_mut(input).unwrap().kind = NodeKind::Input(NumberInput {
                value,
                integer: !json[&key].is_f64(),
                ..Default::default()
            });
            graph.bind_parameter(node, &key, Some(input)).unwrap();
            graph.output = node;
            let recipe = Recipe::from_graph(&graph).unwrap();
            let recipe: Recipe =
                serde_json::from_str(&serde_json::to_string(&recipe).unwrap()).unwrap();
            let resolved = recipe.instantiate(7, &BTreeMap::new()).unwrap();
            let result = resolved.render(&|| false).unwrap();
            let image = result[&node].as_ref().unwrap();
            assert!(
                image
                    .pixels
                    .iter()
                    .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
                "{}",
                settings.title()
            );
            let data = Arc::new(Image {
                alpha: None,
                terrain: None,
                size: 16,
                pixels: vec![1.0; 256],
            });
            let sources = vec![data; settings.inputs().len()];
            assert!(
                NodeKind::Map(settings.clone())
                    .evaluate(
                        &sources,
                        RenderContext {
                            size: 16,
                            extent: 16.0
                        },
                        &|| true
                    )
                    .is_err(),
                "{}",
                settings.title()
            );
        }
        assert!(
            !MapNode::Morphology {
                mode: MorphMode::Dilate,
                radius: 65,
                cutoff: 0.5
            }
            .validate()
        );
        assert!(
            !MapNode::SelectRegions {
                min_area: 5,
                max_area: 2,
                largest_only: false,
                cutoff: 0.5
            }
            .validate()
        );
        assert!(
            !MapNode::Scatter {
                count: 1,
                min_distance: f32::NAN,
                radius: 0,
                seed: 1,
                cutoff: 0.5
            }
            .validate()
        );
    }
}
