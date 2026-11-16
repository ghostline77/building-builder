use crate::model::{Compiled, Kit, Module, Project, Solid, add_piece, identifier};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Stair {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub run: f32,
    pub level: u32,
    pub landing: f32,
    pub reverse: bool,
    pub railings: bool,
    pub axis: char,
    #[serde(rename = "type")]
    pub kind: String,
    pub flight_width: f32,
}
impl Default for Stair {
    fn default() -> Self {
        Self {
            id: "stairs".into(),
            x: 0.,
            y: 3.,
            width: 3.,
            run: 9.,
            level: 0,
            landing: 3.,
            reverse: false,
            railings: true,
            axis: 'y',
            kind: "straight".into(),
            flight_width: 1.2,
        }
    }
}

pub(crate) fn rectangle(s: &Stair, r: [f32; 4]) -> [f32; 4] {
    if s.axis == 'x' {
        [s.x + r[1], s.y + r[0], s.x + r[3], s.y + r[2]]
    } else {
        [s.x + r[0], s.y + r[1], s.x + r[2], s.y + r[3]]
    }
}

pub fn elevations(p: &Project, k: &Kit, s: &Stair) -> [f32; 2] {
    [s.level, s.level + 1].map(|level| {
        p.shell_rooms
            .iter()
            .find(|r| r.level == level)
            .map_or(level as f32 * k.storey_height, |r| r.position[2])
    })
}

pub fn validate(p: &Project, k: &Kit) -> Result<()> {
    ensure!(
        p.stairs.len() <= 16,
        "At most 16 stair flights are supported"
    );
    let mut ids = BTreeSet::new();
    let mut shafts = Vec::new();
    for s in &p.stairs {
        identifier(&s.id)?;
        ensure!(ids.insert(&s.id), "Duplicate stair identity");
        ensure!(
            ['x', 'y'].contains(&s.axis) && ["straight", "return"].contains(&s.kind.as_str()),
            "Invalid stair axis or type"
        );
        for (value, low, high) in [
            (s.x, -120., 120.),
            (s.y, -120., 120.),
            (s.width, 1.2, 6.),
            (s.run, 3., 15.),
            (s.landing, 1.2, 3.),
        ] {
            ensure!(
                value.is_finite()
                    && (low..=high).contains(&value)
                    && (value * 20. - (value * 20.).round()).abs() < 0.0001,
                "Stair dimensions must be on the 5 cm grid and within supported limits"
            );
        }
        ensure!(s.level < 3, "Stair upper floor exceeds supported storeys");
        let returning = s.kind == "return";
        ensure!(
            !returning
                || (s.axis == 'y'
                    && !s.reverse
                    && s.flight_width.is_finite()
                    && (1.2..=2.5).contains(&s.flight_width)
                    && (0.15..=0.4).contains(&(s.width - 2. * s.flight_width))),
            "Return stair requires two flights, a 15–40 cm divider and Y axis"
        );
        ensure!(
            ((elevations(p, k, s)[1] - elevations(p, k, s)[0])
                / if returning { 2. } else { 1. }
                / s.run)
                .atan()
                .to_degrees()
                <= 35.,
            "Stair slope exceeds 35 degrees"
        );
        let r = rectangle(s, [0., -s.landing, s.width, s.run + s.landing]);
        for level in [s.level, s.level + 1] {
            let mut uncovered = vec![r];
            for room in p.rooms.iter().filter(|room| room.level == level) {
                uncovered = subtract(
                    uncovered,
                    [
                        room.x as f32 * 3.,
                        room.y as f32 * 3.,
                        (room.x as f32 + room.width as f32) * 3.,
                        (room.y as f32 + room.depth as f32) * 3.,
                    ],
                );
            }
            for room in p.shell_rooms.iter().filter(|room| room.level == level) {
                // Subtract the exact rotated footprint, rather than its bounding box.
                let q = glam::Quat::from_rotation_z(-room.yaw.to_radians());
                let origin = glam::Vec3::from_array(room.position);
                let covered = [(r[0], r[1]), (r[0], r[3]), (r[2], r[1]), (r[2], r[3])]
                    .iter()
                    .all(|&(x, y)| {
                        let point = q * (glam::Vec3::new(x, y, origin.z) - origin);
                        point.x >= -0.001
                            && point.y >= -0.001
                            && point.x <= room.size[0] + 0.001
                            && point.y <= room.size[1] + 0.001
                    });
                if covered {
                    uncovered.clear();
                    break;
                }
            }
            ensure!(
                uncovered.is_empty(),
                "Stair shaft and landings must be covered by adjoining rooms on both floors"
            );
            let shaft = rectangle(s, [0., 0., s.width, s.run]);
            ensure!(
                !shafts.iter().any(|(l, other): &(u32, [f32; 4])| *l == level
                    && shaft[0] < other[2]
                    && shaft[2] > other[0]
                    && shaft[1] < other[3]
                    && shaft[3] > other[1]),
                "Stair shafts overlap"
            );
            shafts.push((level, shaft));
        }
    }
    Ok(())
}

pub fn slabs(p: &Project, level: u32, x: i32, y: i32, ceiling: bool) -> Vec<[f32; 4]> {
    let mut result = vec![[
        x as f32 * 3.,
        y as f32 * 3.,
        (x + 1) as f32 * 3.,
        (y + 1) as f32 * 3.,
    ]];
    for s in &p.stairs {
        if level != s.level + u32::from(!ceiling) {
            continue;
        }
        let pad = if s.railings { 0.12 } else { 0. };
        let cut = rectangle(
            s,
            [
                -pad,
                if s.kind == "return" { -s.landing } else { 0. },
                s.width + pad,
                s.run,
            ],
        );
        result = subtract(result, cut);
    }
    result
}

pub(crate) fn subtract(rects: Vec<[f32; 4]>, cut: [f32; 4]) -> Vec<[f32; 4]> {
    let mut output = Vec::new();
    for [l, b, r, t] in rects {
        let [a, c, d, e] = [l.max(cut[0]), b.max(cut[1]), r.min(cut[2]), t.min(cut[3])];
        if a >= d || c >= e {
            output.push([l, b, r, t]);
            continue;
        }
        for rect in [[l, b, a, t], [d, b, r, t], [a, b, d, c], [a, e, d, t]] {
            if rect[2] - rect[0] > 0.000001 && rect[3] - rect[1] > 0.000001 {
                output.push(rect);
            }
        }
    }
    output
}

pub fn geometry(c: &mut Compiled, k: &Kit, modules: &BTreeMap<&str, &Module>) -> Result<()> {
    for s in c.project.stairs.clone() {
        let [base, upper] = elevations(&c.project, k, &s);
        let rise = upper - base;
        ensure!(
            rise > 0.2,
            "Stair upper elevation must be above the lower floor"
        );
        let count = (rise / if s.kind == "return" { 2. } else { 1. } / 0.2).ceil() as usize;
        let tread = s.run / count as f32;
        let riser = (rise / if s.kind == "return" { 2. } else { 1. }) / count as f32;
        let mut boxes = Vec::new();
        let flights = if s.kind == "return" { 2 } else { 1 };
        for side in 0..flights {
            let fw = if flights == 2 {
                s.flight_width
            } else {
                s.width
            };
            let sx = if side == 1 { s.width - fw } else { 0. };
            for i in 0..count {
                let height = if flights == 2 {
                    if side == 0 {
                        (count - i) as f32 * riser
                    } else {
                        rise / 2. + (i + 1) as f32 * riser
                    }
                } else if s.reverse {
                    (count - i) as f32 * riser
                } else {
                    (i + 1) as f32 * riser
                };
                boxes.push((
                    format!("stair:{}:{side}:{i}", s.id),
                    [sx + fw / 2., (i as f32 + 0.5) * tread, base + height / 2.],
                    [fw, tread, height],
                ));
                if s.railings {
                    for edge in if flights == 2 {
                        vec![if side == 0 { -0.06 } else { s.width + 0.06 }]
                    } else {
                        vec![-0.06, s.width + 0.06]
                    } {
                        let top = (base + height + 1.05).min(upper - 0.2);
                        boxes.push((
                            format!("guard:{}:{side}:{i}:{edge}", s.id),
                            [edge, (i as f32 + 0.5) * tread, (base + top) / 2.],
                            [0.12, tread, top - base],
                        ));
                    }
                }
            }
        }
        if flights == 2 {
            boxes.push((
                format!("stair:{}:turn", s.id),
                [s.width / 2., -s.landing / 2., base + rise / 4.],
                [s.width, s.landing, rise / 2.],
            ));
            boxes.push((
                format!("guard:{}:centre", s.id),
                [s.width / 2., s.run / 2., (base + upper + 1.05) / 2.],
                [s.width - 2. * s.flight_width, s.run, upper + 1.05 - base],
            ));
        }
        if s.railings {
            for (i, x) in [-0.06, s.width + 0.06].into_iter().enumerate() {
                boxes.push((
                    format!("guard:{}:upper:{i}", s.id),
                    [x, s.run / 2., upper + 0.45],
                    [0.12, s.run, 1.3],
                ));
            }
            let y = if flights == 2 || s.reverse {
                s.run + 0.06
            } else {
                -0.06
            };
            boxes.push((
                format!("guard:{}:end", s.id),
                [
                    if flights == 2 {
                        (s.width - s.flight_width) / 2.
                    } else {
                        s.width / 2.
                    },
                    y,
                    upper + 0.45,
                ],
                [
                    if flights == 2 {
                        s.width - s.flight_width
                    } else {
                        s.width
                    },
                    0.12,
                    1.3,
                ],
            ));
        }
        for (id, mut center, mut size) in boxes {
            if s.axis == 'x' {
                center = [s.x + center[1], s.y + center[0], center[2]];
                size.swap(0, 1);
            } else {
                center[0] += s.x;
                center[1] += s.y;
            }
            if id.starts_with("guard:")
                && c.solids.iter().any(|wall| {
                    if !wall.id.starts_with("wall:") {
                        return false;
                    }
                    let delta = glam::Quat::from_rotation_z(-wall.yaw.to_radians())
                        * (glam::Vec3::from_array(center) - glam::Vec3::from_array(wall.center));
                    delta.x.abs() <= wall.size[0] / 2. + 0.08
                        && delta.y.abs() <= wall.size[1] / 2. + 0.08
                        && center[2] - size[2] / 2. >= wall.center[2] - wall.size[2] / 2. - 0.01
                        && center[2] + size[2] / 2. <= wall.center[2] + wall.size[2] / 2. + 0.01
                })
            {
                continue;
            }
            structural_box(c, k, modules, &id, center, size)?;
            c.solids.push(Solid {
                id,
                center,
                size,
                yaw: 0.,
            });
        }
    }
    Ok(())
}

pub fn structural_box(
    c: &mut Compiled,
    k: &Kit,
    modules: &BTreeMap<&str, &Module>,
    id: &str,
    center: [f32; 3],
    size: [f32; 3],
) -> Result<()> {
    let cap = 0.02_f32.min(size[2] / 2.);
    let upper = [
        center[0] + size[0] / 2.,
        center[1] + size[1] / 2.,
        center[2] + size[2] / 2.,
    ];
    add_piece(
        c,
        k,
        modules,
        format!("{id}:body"),
        "ceiling",
        [upper[0], upper[1], center[2] - size[2] / 2.],
        0.,
        [size[0] / 3., size[1] / 3., (size[2] - cap) / 0.1],
        false,
    )?;
    add_piece(
        c,
        k,
        modules,
        format!("{id}:cap"),
        "floor",
        [
            upper[0] - 0.0025 * size[0] / 3.005,
            upper[1] - 0.0025 * size[1] / 3.005,
            upper[2],
        ],
        0.,
        [size[0] / 3.005, size[1] / 3.005, cap / 0.2],
        false,
    )?;
    let floor = (center[2] / k.storey_height).max(0.).floor() as u32;
    let count = c.placements.len();
    for p in &mut c.placements[count.saturating_sub(2)..] {
        p.level = floor;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adjoining_rooms_remove_the_wall_across_the_stair_shaft() {
        let mut p: Project =
            serde_json::from_str(include_str!("../examples/stairs-and-devices.building.json"))
                .unwrap();
        p.openings.clear();
        p.door_states.clear();
        p.rooms.clear();
        for level in 0..2 {
            for (id, y, depth) in [("front", 0, 2), ("back", 2, 3)] {
                p.rooms.push(crate::model::Room {
                    id: format!("{id}_{level}"),
                    x: 0,
                    y,
                    width: 3,
                    depth,
                    level,
                    ceiling_height: None,
                });
            }
        }
        let c = crate::model::compile(&p, &crate::model::test_library().unwrap()).unwrap();
        let middle = [4.5, 6., 1.];
        assert!(
            !c.solids
                .iter()
                .filter(|s| s.id.starts_with("wall:") || s.id.contains("stair_join"))
                .any(|s| {
                    let local = glam::Quat::from_rotation_z(-s.yaw.to_radians())
                        * (glam::Vec3::from_array(middle) - glam::Vec3::from_array(s.center));
                    local
                        .abs()
                        .cmplt(glam::Vec3::from_array(s.size) * 0.5 + glam::Vec3::splat(0.0001))
                        .all()
                })
        );
    }
}
