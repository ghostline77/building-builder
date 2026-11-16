//! Arbitrary partitions, window apertures and measured rooms inside a surveyed shell.
use crate::model::{Compiled, Kit, Module, Placement, Project, Solid, identifier};
use anyhow::{Context, Result, ensure};
use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Aperture {
    pub id: String,
    pub start: f32,
    pub width: f32,
    pub sill: f32,
    pub height: f32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Wall {
    pub id: String,
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub height: f32,
    pub thickness: f32,
    #[serde(default)]
    pub openings: Vec<Aperture>,
    #[serde(default)]
    pub level: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ShellRoom {
    pub id: String,
    pub position: [f32; 3],
    pub size: [f32; 3],
    pub yaw: f32,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub retained_sides: BTreeSet<String>,
    #[serde(default)]
    pub openings: BTreeMap<String, Vec<Aperture>>,
}

pub fn walls(project: &Project) -> Vec<Wall> {
    let mut walls = project.partitions.clone();
    for room in &project.shell_rooms {
        let [w, d, h] = room.size;
        let rotation = Quat::from_rotation_z(room.yaw.to_radians());
        let origin = Vec3::from_array(room.position);
        for (side, a, b) in [
            ("south", [0., 0., 0.], [w, 0., 0.]),
            ("east", [w, 0., 0.], [w, d, 0.]),
            ("north", [w, d, 0.], [0., d, 0.]),
            ("west", [0., d, 0.], [0., 0., 0.]),
        ] {
            if room.retained_sides.contains(side) {
                continue;
            }
            walls.push(Wall {
                id: format!("{}_{}", room.id, side),
                from: (origin + rotation * Vec3::from_array(a)).to_array(),
                to: (origin + rotation * Vec3::from_array(b)).to_array(),
                height: h,
                thickness: 0.12,
                openings: room.openings.get(side).cloned().unwrap_or_default(),
                level: room.level,
            });
        }
    }
    walls
}

pub fn validate(project: &Project) -> Result<()> {
    ensure!(
        project.partitions.len() <= 128 && project.shell_rooms.len() <= 32,
        "Too many partitions or shell rooms"
    );
    let mut ids: BTreeSet<_> = project
        .rooms
        .iter()
        .map(|r| r.id.clone())
        .chain(project.objects.iter().map(|o| o.id.clone()))
        .collect();
    for room in &project.shell_rooms {
        identifier(&room.id)?;
        ensure!(ids.insert(room.id.clone()), "Duplicate shell room identity");
        ensure!(
            room.position
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 1000.)
                && room
                    .size
                    .iter()
                    .all(|v| v.is_finite() && (0.3..=100.).contains(v))
                && room.size[2] >= 1.8
                && room.yaw.is_finite()
                && room.level < 4,
            "Invalid shell room dimensions"
        );
        ensure!(
            room.retained_sides
                .iter()
                .all(|s| ["south", "east", "north", "west", "floor", "ceiling"]
                    .contains(&s.as_str()))
                && room
                    .openings
                    .keys()
                    .all(|s| ["south", "east", "north", "west"].contains(&s.as_str())),
            "Invalid shell room side"
        );
        ensure!(
            room.retained_sides
                .iter()
                .all(|s| !room.openings.contains_key(s)),
            "Retained shell sides cannot acquire authored apertures; patch the surveyed wall first"
        );
        let corners = |position: [f32; 3], size: [f32; 2], yaw: f32| {
            let rotation = Quat::from_rotation_z(yaw.to_radians());
            [[0., 0.], [size[0], 0.], size, [0., size[1]]].map(|[x, y]| {
                let p = Vec3::from_array(position) + rotation * Vec3::new(x, y, 0.);
                [p.x, p.y]
            })
        };
        let footprint = corners(room.position, [room.size[0], room.size[1]], room.yaw);
        for other in project.shell_rooms.iter().take_while(|r| r.id != room.id) {
            if other.level == room.level {
                ensure!(
                    !overlaps(
                        &footprint,
                        &corners(other.position, [other.size[0], other.size[1]], other.yaw)
                    ),
                    "Measured shell rooms overlap"
                );
            }
        }
        for other in &project.rooms {
            if other.level == room.level {
                ensure!(
                    !overlaps(
                        &footprint,
                        &corners(
                            [other.x as f32 * 3., other.y as f32 * 3., 0.],
                            [other.width as f32 * 3., other.depth as f32 * 3.],
                            0.
                        )
                    ),
                    "Measured shell room overlaps a grid room"
                );
            }
        }
    }
    for wall in walls(project) {
        identifier(&wall.id)?;
        ensure!(ids.insert(wall.id.clone()), "Duplicate partition identity");
        let length = (Vec3::from_array(wall.to) - Vec3::from_array(wall.from)).length();
        ensure!(
            wall.from
                .iter()
                .chain(&wall.to)
                .all(|v| v.is_finite() && v.abs() <= 1000.)
                && (wall.to[2] - wall.from[2]).abs() < 0.001
                && (0.1..=100.).contains(&length)
                && wall.height.is_finite()
                && (0.1..=20.).contains(&wall.height)
                && wall.thickness.is_finite()
                && (0.02..=2.).contains(&wall.thickness)
                && wall.level < 4,
            "Invalid partition transform or dimensions"
        );
        let mut openings = Vec::new();
        for opening in &wall.openings {
            identifier(&opening.id)?;
            ensure!(
                [opening.start, opening.width, opening.sill, opening.height]
                    .iter()
                    .all(|v| v.is_finite())
                    && opening.start >= 0.
                    && opening.width > 0.1
                    && opening.start + opening.width <= length + 0.001
                    && opening.sill >= 0.
                    && opening.height > 0.1
                    && opening.sill + opening.height <= wall.height + 0.001,
                "Aperture exceeds partition bounds"
            );
            ensure!(
                !openings
                    .iter()
                    .any(|(a, b): &(f32, f32)| opening.start < *b
                        && opening.start + opening.width > *a),
                "Partition apertures overlap"
            );
            openings.push((opening.start, opening.start + opening.width));
        }
    }
    Ok(())
}

fn overlaps(a: &[[f32; 2]; 4], b: &[[f32; 2]; 4]) -> bool {
    for polygon in [a, b] {
        for edge in 0..4 {
            let p = polygon[edge];
            let q = polygon[(edge + 1) % 4];
            let normal = [p[1] - q[1], q[0] - p[0]];
            let projection = |r: &[[f32; 2]; 4]| {
                r.iter()
                    .map(|p| p[0] * normal[0] + p[1] * normal[1])
                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), p| {
                        (lo.min(p), hi.max(p))
                    })
            };
            let (al, ah) = projection(a);
            let (bl, bh) = projection(b);
            if ah.min(bh) - al.max(bl) < 0.0001 {
                return false;
            }
        }
    }
    true
}

#[expect(
    clippy::too_many_arguments,
    reason = "Explicit panel transform and dimensions"
)]
fn fitted(
    c: &mut Compiled,
    k: &Kit,
    modules: &BTreeMap<&str, &Module>,
    id: String,
    role: &str,
    center: Vec3,
    size: Vec3,
    yaw: f32,
    level: u32,
) -> Result<()> {
    let module = modules
        .get(k.pieces[role].mesh.as_str())
        .context("Kit geometry unavailable")?;
    let bounds = &module.bounds;
    let scale = size / Vec3::from_array(bounds.dimensions);
    let local_center = (Vec3::from_array(bounds.min) + Vec3::from_array(bounds.max)) * 0.5;
    let position = center - Quat::from_rotation_z(yaw.to_radians()) * (scale * local_center);
    c.placements.push(Placement {
        id: id.clone(),
        role: role.into(),
        mesh: module.path.clone(),
        appearance: if role == "floor" {
            c.project.floor_appearance.clone()
        } else if role == "wall" {
            c.project.wall_appearance.clone()
        } else {
            k.pieces[role].preview_appearance.clone()
        },
        position: position.to_array(),
        yaw,
        scale: scale.to_array(),
        bounds: bounds.clone(),
        cutaway: role == "ceiling",
        rotation: None,
        reference_only: false,
        level,
        entity: false,
        assembly_options: Default::default(),
    });
    c.solids.push(Solid {
        id,
        center: center.to_array(),
        size: size.to_array(),
        yaw,
    });
    Ok(())
}

pub fn compile(c: &mut Compiled, k: &Kit, modules: &BTreeMap<&str, &Module>) -> Result<()> {
    for room in c.project.shell_rooms.clone() {
        let origin = Vec3::from_array(room.position);
        let rotation = Quat::from_rotation_z(room.yaw.to_radians());
        let [w, d, h] = room.size;
        for (role, z, t) in [("floor", -0.1, 0.2), ("ceiling", h + 0.05, 0.1)] {
            if room.retained_sides.contains(role) {
                continue;
            }
            fitted(
                c,
                k,
                modules,
                format!("shell:{}:{role}", room.id),
                role,
                origin + rotation * Vec3::new(w / 2., d / 2., z),
                Vec3::new(w, d, t),
                room.yaw,
                room.level,
            )?;
        }
    }
    for wall in walls(&c.project) {
        compile_wall(c, k, modules, &wall)?;
    }
    Ok(())
}

pub fn compile_wall(
    c: &mut Compiled,
    k: &Kit,
    modules: &BTreeMap<&str, &Module>,
    wall: &Wall,
) -> Result<()> {
    let delta = Vec3::from_array(wall.to) - Vec3::from_array(wall.from);
    let length = delta.length();
    let yaw = delta.y.atan2(delta.x).to_degrees();
    let rotation = Quat::from_rotation_z(yaw.to_radians());
    let mut panels = vec![[0., 0., length, wall.height]];
    for opening in &wall.openings {
        panels = crate::stairs::subtract(
            panels,
            [
                opening.start,
                opening.sill,
                opening.start + opening.width,
                opening.sill + opening.height,
            ],
        );
    }
    for (index, [x0, z0, x1, z1]) in panels.into_iter().enumerate() {
        fitted(
            c,
            k,
            modules,
            format!("partition:{}:{index}", wall.id),
            "wall",
            Vec3::from_array(wall.from) + rotation * Vec3::new((x0 + x1) / 2., 0., (z0 + z1) / 2.),
            Vec3::new(x1 - x0, wall.thickness, z1 - z0),
            yaw,
            wall.level,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window_removes_the_same_volume_from_visual_panels_and_collision() {
        let mut p: Project =
            serde_json::from_str(include_str!("../examples/workshop.building.json")).unwrap();
        p.rooms.clear();
        p.openings.clear();
        p.partitions = vec![Wall {
            id: "window_wall".into(),
            from: [0.; 3],
            to: [6., 0., 0.],
            height: 3.,
            thickness: 0.12,
            level: 0,
            openings: vec![Aperture {
                id: "window".into(),
                start: 2.,
                width: 1.5,
                sill: 1.,
                height: 1.2,
            }],
        }];
        let c = crate::model::compile(&p, &crate::model::test_library().unwrap()).unwrap();
        let volume: f32 = c
            .solids
            .iter()
            .map(|s| s.size.iter().product::<f32>())
            .sum();
        assert!((volume - (18. - 1.8) * 0.12).abs() < 0.0001);
        assert_eq!(c.solids.len(), c.placements.len());
        assert!(
            !c.solids
                .iter()
                .any(|s| (s.center[0] - 2.75).abs() < s.size[0] / 2.
                    && (s.center[2] - 1.6).abs() < s.size[2] / 2.)
        );
    }
    #[test]
    fn rotated_shell_partition_blocks_routes_until_its_doorway_is_cut() {
        let mut p: Project =
            serde_json::from_str(include_str!("../examples/workshop.building.json")).unwrap();
        p.rooms.clear();
        p.openings.clear();
        let rotation = Quat::from_rotation_z(30f32.to_radians());
        p.shell_rooms = vec![ShellRoom {
            id: "shell".into(),
            position: [0.; 3],
            size: [6., 6., 3.],
            yaw: 30.,
            level: 0,
            retained_sides: BTreeSet::new(),
            openings: BTreeMap::new(),
        }];
        p.partitions = vec![Wall {
            id: "partition".into(),
            from: (rotation * Vec3::new(3., 0., 0.)).to_array(),
            to: (rotation * Vec3::new(3., 6., 0.)).to_array(),
            height: 3.,
            thickness: 0.12,
            level: 0,
            openings: Vec::new(),
        }];
        let start = (rotation * Vec3::new(1.5, 3., 0.)).to_array();
        let end = (rotation * Vec3::new(4.5, 3., 0.)).to_array();
        let library = crate::model::test_library().unwrap();
        let c = crate::model::compile(&p, &library).unwrap();
        let nav = crate::navigation::compile(&c).unwrap();
        assert_eq!(
            crate::navigation::route(&nav, start, end).unwrap()["connected"],
            false
        );
        p.partitions[0].openings.push(Aperture {
            id: "entry".into(),
            start: 2.,
            width: 2.,
            sill: 0.,
            height: 2.4,
        });
        let c = crate::model::compile(&p, &library).unwrap();
        let nav = crate::navigation::compile(&c).unwrap();
        assert_eq!(
            crate::navigation::route(&nav, start, end).unwrap()["connected"],
            true
        );
    }
}
