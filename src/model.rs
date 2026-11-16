use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub revision: u64,
    pub kit: String,
    pub origin: [f32; 3],
    pub yaw: f32,
    pub wall_appearance: String,
    pub floor_appearance: String,
    pub rooms: Vec<Room>,
    pub openings: BTreeMap<String, Opening>,
    pub objects: Vec<Object>,
    #[serde(default)]
    pub stairs: Vec<crate::stairs::Stair>,
    #[serde(default)]
    pub lights: Vec<crate::technical::Light>,
    #[serde(default)]
    pub audio_areas: Vec<crate::technical::AudioArea>,
    #[serde(default)]
    pub door_states: BTreeMap<String, crate::technical::DoorState>,
    #[serde(default)]
    pub interior: bool,
    #[serde(default)]
    pub navigation: crate::navigation::Settings,
    #[serde(default)]
    pub partitions: Vec<crate::interiors::Wall>,
    #[serde(default)]
    pub shell_rooms: Vec<crate::interiors::ShellRoom>,
    #[serde(default)]
    pub door_devices: BTreeMap<String, crate::technical::DoorDevice>,
    #[serde(default)]
    pub retrofit: Vec<crate::retrofit::SectorPatch>,
    #[serde(default)]
    pub workstations: Vec<crate::encounters::Workstation>,
    #[serde(default)]
    pub shells: Vec<crate::shell::ImportedShell>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Room {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub depth: u32,
    pub level: u32,
    #[serde(default)]
    pub ceiling_height: Option<f32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Opening {
    Doorway,
    Passage,
    DoorAuto,
    DoorManual,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Object {
    pub id: String,
    pub asset: String,
    pub appearance: String,
    pub position: [f32; 3],
    pub yaw: f32,
    #[serde(default)]
    pub anchor: Anchor,
    #[serde(default = "unit_scale")]
    pub scale: [f32; 3],
    #[serde(default)]
    pub rotation: Option<[f32; 4]>,
    #[serde(default)]
    pub reference_only: bool,
    #[serde(default)]
    pub collision: bool,
    #[serde(default)]
    pub source_content_sha256: Option<String>,
    #[serde(default)]
    pub level: Option<u32>,
}
fn unit_scale() -> [f32; 3] {
    [1.; 3]
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Anchor {
    #[default]
    Pivot,
    BaseCenter,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Library {
    pub source: String,
    pub assets: Vec<Module>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Module {
    pub id: String,
    pub path: String,
    pub name: String,
    pub family: String,
    pub role: String,
    pub status: String,
    pub bounds: Bounds,
    pub appearances: Vec<String>,
    pub source_content_sha256: Option<String>,
    #[serde(default)]
    pub entity: bool,
    #[serde(default)]
    pub catalogue_bounds: BTreeMap<String, Bounds>,
    #[serde(default)]
    pub assembly_options: BTreeMap<String, ghostline_renderer::AssemblyOptions>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Bounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub dimensions: [f32; 3],
}

#[derive(Deserialize, Serialize)]
pub struct Kit {
    pub id: String,
    pub name: String,
    pub wall_height: f32,
    pub storey_height: f32,
    pub wall_thickness: f32,
    #[serde(default)]
    pub wall_facing_depth: Option<f32>,
    pub door_clearance: [f32; 2],
    pub wall_appearances: Vec<String>,
    pub floor_appearances: Vec<String>,
    pub pieces: BTreeMap<String, Piece>,
}

#[derive(Deserialize, Serialize)]
pub struct Piece {
    pub mesh: String,
    pub preview_appearance: String,
    #[serde(default)]
    pub offset: [f32; 3],
}

#[derive(Clone, Serialize)]
pub struct Placement {
    pub id: String,
    pub role: String,
    pub mesh: String,
    pub appearance: String,
    pub position: [f32; 3],
    pub yaw: f32,
    pub scale: [f32; 3],
    pub bounds: Bounds,
    pub cutaway: bool,
    pub rotation: Option<[f32; 4]>,
    pub reference_only: bool,
    pub level: u32,
    pub entity: bool,
    pub assembly_options: ghostline_renderer::AssemblyOptions,
}

#[derive(Clone, Serialize)]
pub struct Solid {
    pub id: String,
    pub center: [f32; 3],
    pub size: [f32; 3],
    pub yaw: f32,
}

#[derive(Serialize)]
pub struct Compiled {
    pub project: Project,
    pub placements: Vec<Placement>,
    pub solids: Vec<Solid>,
    pub edges: Vec<Edge>,
    pub diagnostics: Vec<String>,
    #[serde(skip)]
    pub edited_meshes: BTreeMap<String, crate::shell::EditedMesh>,
    #[serde(skip)]
    pub generated_collision: Vec<ghostline_red::world::CollisionBox>,
    pub reference_solids: Vec<Solid>,
    pub cooked_collision: Vec<crate::shell::CookedCollision>,
    #[serde(skip)]
    pub reference_collision: Vec<ghostline_red::physics::CollisionGeometry>,
}

#[derive(Serialize)]
pub struct Edge {
    pub id: String,
    pub axis: char,
    pub x: i32,
    pub y: i32,
    pub level: u32,
    pub shared: bool,
}

pub fn library() -> Result<Library> {
    Ok(Library {
        source: "Published items catalogue".into(),
        assets: Vec::new(),
    })
}

#[cfg(test)]
pub fn test_library() -> Result<Library> {
    // Analytic fixture for topology tests; never used as an asset inventory.
    let kit = kit("common-3m")?;
    let mut library = library()?;
    for (role, piece) in &kit.pieces {
        let bounds = match role.as_str() {
            "floor" => Bounds {
                min: [-3., -3., -0.2],
                max: [0.005, 0.005, 0.],
                dimensions: [3.005, 3.005, 0.2],
            },
            "ceiling" => Bounds {
                min: [-3., -3., 0.],
                max: [0., 0., 0.1],
                dimensions: [3., 3., 0.1],
            },
            _ => Bounds {
                min: [-3., 0., 0.],
                max: [0., 0.23, 3.8],
                dimensions: [3., 0.23, 3.8],
            },
        };
        library.assets.push(Module {
            id: format!("fixture_{role}"),
            path: piece.mesh.clone(),
            name: role.clone(),
            family: "test fixture".into(),
            role: role.clone(),
            status: "prepared".into(),
            bounds,
            appearances: if role == "floor" {
                kit.floor_appearances.clone()
            } else if role == "ceiling" {
                vec![piece.preview_appearance.clone()]
            } else {
                kit.wall_appearances.clone()
            },
            source_content_sha256: None,
            entity: false,
            catalogue_bounds: BTreeMap::new(),
            assembly_options: BTreeMap::new(),
        });
    }
    Ok(library)
}

pub fn kits() -> Result<Vec<Kit>> {
    Ok(serde_json::from_str(
        include_str!("../data/kits.json").trim_start_matches('\u{feff}'),
    )?)
}

pub fn kit(id: &str) -> Result<Kit> {
    kits()?
        .into_iter()
        .find(|kit| kit.id == id)
        .ok_or_else(|| anyhow::anyhow!("Unknown measured kit {id}"))
}

pub fn identifier(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 64
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
        "Identity must contain 1–64 letters, digits, underscores or hyphens"
    );
    Ok(())
}

/// Native project coordinates are Z-up metres; assets supplied by renderer are Y-up.
pub fn transform(position: [f32; 3], yaw: f32, scale: [f32; 3]) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::new(scale[0], scale[2], scale[1]),
        Quat::from_rotation_y(yaw.to_radians()),
        Vec3::new(position[0], position[2], -position[1]),
    )
}

pub fn placement_transform(p: &Placement) -> Mat4 {
    if let Some(rotation) = p.rotation {
        let basis = Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        basis
            * Mat4::from_scale_rotation_translation(
                Vec3::from_array(p.scale),
                Quat::from_array(rotation),
                Vec3::from_array(p.position),
            )
            * basis.transpose()
    } else {
        transform(p.position, p.yaw, p.scale)
    }
}

pub fn compile(project: &Project, library: &Library) -> Result<Compiled> {
    let kit = kit(&project.kit)?;
    ensure!(project.schema_version == 1, "Unsupported project schema");
    identifier(&project.id)?;
    ensure!(project.name.len() <= 200, "Building name is too long");
    ensure!(project.kit == kit.id, "Unsupported measured room kit");
    ensure!(
        project
            .origin
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 15000.)
            && project.yaw.is_finite(),
        "Invalid world anchor"
    );
    ensure!(
        kit.wall_appearances.contains(&project.wall_appearance)
            && kit.floor_appearances.contains(&project.floor_appearance),
        "Finish is absent from the measured kit"
    );
    ensure!(
        project.rooms.len() <= 32 && project.objects.len() <= 1000,
        "Project exceeds room/object limits"
    );
    let mut cells = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for room in &project.rooms {
        identifier(&room.id)?;
        ensure!(ids.insert(&room.id), "Duplicate room/object identity");
        ensure!(
            room.width > 0
                && room.depth > 0
                && room.width <= 20
                && room.depth <= 20
                && room.level < 4
                && room.x.abs() <= 40
                && room.y.abs() <= 40,
            "Room dimensions/level exceed supported limits"
        );
        ensure!(
            room.ceiling_height.is_none_or(
                |height| height.is_finite() && (1.8..=kit.wall_height).contains(&height)
            ),
            "Ceiling height must be 1.8 m to the native kit height"
        );
        for x in room.x..room.x + room.width as i32 {
            for y in room.y..room.y + room.depth as i32 {
                ensure!(
                    cells.insert((room.level, x, y), &room.id).is_none(),
                    "Rooms overlap on floor {}",
                    room.level
                );
            }
        }
    }
    ensure!(cells.len() <= 600, "Project exceeds 600 floor tiles");
    crate::stairs::validate(project, &kit)?;
    crate::technical::validate(project)?;
    crate::navigation::validate(project)?;
    crate::interiors::validate(project)?;
    crate::encounters::validate(project)?;
    let mut compiled = Compiled {
        edited_meshes: BTreeMap::new(),
        generated_collision: Vec::new(),
        reference_solids: Vec::new(),
        cooked_collision: Vec::new(),
        reference_collision: Vec::new(),
        project: project.clone(),
        placements: Vec::new(),
        solids: Vec::new(),
        edges: Vec::new(),
        diagnostics: vec![
            "Runtime walking, streaming, door behaviour and save/load require an in-game check."
                .into(),
        ],
    };
    let modules: BTreeMap<_, _> = library
        .assets
        .iter()
        .filter(|m| !m.id.starts_with("world:"))
        .map(|m| (m.path.as_str(), m))
        .collect();
    for &(level, x, y) in cells.keys() {
        let z = level as f32 * kit.storey_height;
        let room_height = project
            .rooms
            .iter()
            .find(|r| &r.id == cells[&(level, x, y)])
            .and_then(|r| r.ceiling_height)
            .unwrap_or(kit.wall_height);
        for (role, height) in [("floor", z), ("ceiling", z + room_height)] {
            for (part, [xmin, ymin, xmax, ymax]) in
                crate::stairs::slabs(project, level, x, y, role == "ceiling")
                    .into_iter()
                    .enumerate()
            {
                let width = xmax - xmin;
                let depth = ymax - ymin;
                let thickness = if role == "floor" {
                    0.2
                } else if cells.contains_key(&(level + 1, x, y)) {
                    0.1_f32.max(kit.storey_height - room_height - 0.2)
                } else {
                    0.1
                };
                let scale = if role == "floor" {
                    [width / 3.005, depth / 3.005, 1.]
                } else {
                    [width / 3., depth / 3., thickness / 0.1]
                };
                let lip = if role == "floor" {
                    0.0025 * scale[0]
                } else {
                    0.
                };
                add_piece(
                    &mut compiled,
                    &kit,
                    &modules,
                    format!("tile:{level}:{x}:{y}:{role}:{part}"),
                    role,
                    [
                        xmax - lip,
                        ymax - if role == "floor" {
                            0.0025 * scale[1]
                        } else {
                            0.
                        },
                        height,
                    ],
                    0.,
                    scale,
                    role == "ceiling",
                )?;
                if let Some(placement) = compiled.placements.last_mut() {
                    placement.level = level;
                }
                compiled.solids.push(Solid {
                    id: format!("tile:{level}:{x}:{y}:{role}:{part}"),
                    center: [
                        (xmin + xmax) / 2.,
                        (ymin + ymax) / 2.,
                        if role == "floor" {
                            z - 0.1
                        } else {
                            height + thickness / 2.
                        },
                    ],
                    size: [width, depth, thickness],
                    yaw: 0.,
                });
            }
        }
    }
    let mut walls: BTreeMap<(u32, char, i32, i32), Vec<u8>> = BTreeMap::new();
    for (&(level, x, y), owner) in &cells {
        for (side, axis, ex, ey, neighbor) in [
            (0, 'x', x, y, (level, x, y - 1)),
            (1, 'y', x + 1, y, (level, x + 1, y)),
            (2, 'x', x, y + 1, (level, x, y + 1)),
            (3, 'y', x, y, (level, x - 1, y)),
        ] {
            if cells.get(&neighbor) != Some(owner) {
                walls.entry((level, axis, ex, ey)).or_default().push(side);
            }
        }
    }
    for ((level, axis, x, y), sides) in walls {
        let key = format!("{level}:{axis}:{x}:{y}");
        let side = sides[0];
        let shared = sides.len() == 2;
        compiled.edges.push(Edge {
            id: key.clone(),
            axis,
            x,
            y,
            level,
            shared,
        });
        if matches!(project.openings.get(&key), Some(Opening::Passage)) {
            continue;
        }
        let opening = project.openings.contains_key(&key);
        let yaw: f32 = [0., 90., 180., -90.][side as usize];
        let p = [
            x as f32 * 3. + if side == 0 { 3. } else { 0. },
            y as f32 * 3. + if side == 1 { 3. } else { 0. },
            level as f32 * kit.storey_height,
        ];
        let role = if opening { "doorway" } else { "wall" };
        if shared && !opening {
            let mut holes = Vec::new();
            let start = [
                p[0] - 3. * yaw.to_radians().cos(),
                p[1] - 3. * yaw.to_radians().sin(),
                p[2],
            ];
            for stair in &project.stairs {
                if level != stair.level && level != stair.level + 1 {
                    continue;
                }
                let r = crate::stairs::rectangle(
                    stair,
                    [0., -stair.landing, stair.width, stair.run + stair.landing],
                );
                let (coordinate, lo, hi) = if axis == 'x' {
                    (p[1], r[1], r[3])
                } else {
                    (p[0], r[0], r[2])
                };
                if coordinate <= lo + 0.01 || coordinate >= hi - 0.01 {
                    continue;
                }
                let mut spans = if axis == 'x' {
                    [r[0], r[2]].map(|v| (v - start[0]) * yaw.to_radians().cos())
                } else {
                    [r[1], r[3]].map(|v| (v - start[1]) * yaw.to_radians().sin())
                };
                spans.sort_by(f32::total_cmp);
                let from = spans[0].max(0.);
                let to = spans[1].min(3.);
                if to - from > 0.001 {
                    holes.push(crate::interiors::Aperture {
                        id: format!("stair_{}", stair.id),
                        start: from,
                        width: to - from,
                        sill: 0.,
                        height: kit.wall_height,
                    });
                }
            }
            if !holes.is_empty() {
                crate::interiors::compile_wall(
                    &mut compiled,
                    &kit,
                    &modules,
                    &crate::interiors::Wall {
                        id: format!("stair_join_{}", key.replace(':', "_")),
                        from: start,
                        to: p,
                        height: kit.wall_height,
                        thickness: kit.wall_thickness,
                        openings: holes,
                        level,
                    },
                )?;
                continue;
            }
        }
        add_piece(
            &mut compiled,
            &kit,
            &modules,
            format!("wall:{key}"),
            role,
            p,
            yaw,
            [1.; 3],
            side == 0 || side == 1,
        )?;
        let a = yaw.to_radians();
        if shared {
            let reverse = [
                p[0] - 3. * a.cos() - kit.wall_facing_depth.unwrap_or(kit.wall_thickness) * a.sin(),
                p[1] - 3. * a.sin() + kit.wall_facing_depth.unwrap_or(kit.wall_thickness) * a.cos(),
                p[2],
            ];
            add_piece(
                &mut compiled,
                &kit,
                &modules,
                format!("wall:{key}:reverse"),
                role,
                reverse,
                yaw + 180.,
                [1.; 3],
                side == 0 || side == 1,
            )?;
        }
        let h = kit.wall_height;
        let [width, clear] = kit.door_clearance;
        let jamb = (3. - width) / 2.;
        let parts = if opening {
            vec![
                (-3. + jamb / 2., h / 2., jamb, h),
                (-jamb / 2., h / 2., jamb, h),
                (-1.5, (clear + h) / 2., width, h - clear),
            ]
        } else {
            vec![(-1.5, h / 2., 3., h)]
        };
        for (i, (cx, cz, w, height)) in parts.into_iter().enumerate() {
            compiled.solids.push(Solid {
                id: format!("wall:{key}:{i}"),
                center: [
                    p[0] + cx * a.cos() - kit.wall_thickness / 2. * a.sin(),
                    p[1] + cx * a.sin() + kit.wall_thickness / 2. * a.cos(),
                    p[2] + cz,
                ],
                size: [w, kit.wall_thickness, height],
                yaw,
            });
        }
    }
    ensure!(
        project
            .openings
            .keys()
            .all(|key| compiled.edges.iter().any(|e| &e.id == key)),
        "Opening no longer belongs to a wall; remove or reposition it"
    );
    for object in &project.objects {
        identifier(&object.id)?;
        ensure!(ids.insert(&object.id), "Duplicate room/object identity");
        ensure!(
            object
                .position
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 1000.)
                && object.yaw.is_finite(),
            "Invalid object transform"
        );
        let module = library
            .assets
            .iter()
            .find(|m| m.id == object.asset)
            .ok_or_else(|| anyhow::anyhow!("Unknown module {}", object.asset))?;
        ensure!(
            module.status == "prepared" && module.appearances.contains(&object.appearance),
            "Module/appearance is unavailable"
        );
        ensure!(
            object
                .source_content_sha256
                .as_ref()
                .is_none_or(|hash| Some(hash) == module.source_content_sha256.as_ref()),
            "Module source changed; review and reselect {}",
            object.id
        );
        ensure!(
            object.scale.iter().all(|v| v.is_finite()
                && if object.reference_only {
                    v.abs() > 0.00001
                } else {
                    (0.1..=10.).contains(v)
                }),
            "Module scale must be 0.1–10 on each axis"
        );
        ensure!(
            !object.reference_only || !object.collision,
            "Existing shell references cannot add collision"
        );
        if let Some(rotation) = object.rotation {
            ensure!(
                matches!(object.anchor, Anchor::Pivot)
                    && !object.collision
                    && rotation.iter().all(|v| v.is_finite())
                    && (Quat::from_array(rotation).length_squared() - 1.).abs() < 0.0001,
                "Full rotation requires a normalized quaternion, native pivot and no box collision"
            );
        }
        let bounds = module
            .catalogue_bounds
            .get(&object.appearance)
            .unwrap_or(&module.bounds);
        let low = Vec3::from_array(bounds.min) * Vec3::from_array(object.scale);
        let high = Vec3::from_array(bounds.max) * Vec3::from_array(object.scale);
        let mut position = object.position;
        if matches!(object.anchor, Anchor::BaseCenter) {
            let center = low.midpoint(high);
            let angle = object.yaw.to_radians();
            position[0] -= center.x * angle.cos() - center.y * angle.sin();
            position[1] -= center.x * angle.sin() + center.y * angle.cos();
            position[2] -= low.z;
        }
        compiled.placements.push(Placement {
            id: object.id.clone(),
            role: "object".into(),
            mesh: module.path.clone(),
            appearance: object.appearance.clone(),
            position,
            yaw: object.yaw,
            scale: object.scale,
            bounds: bounds.clone(),
            cutaway: false,
            rotation: object.rotation,
            reference_only: object.reference_only,
            level: object
                .level
                .unwrap_or_else(|| (object.position[2] / kit.storey_height).max(0.).floor() as u32),
            entity: module.entity,
            assembly_options: module
                .assembly_options
                .get(&object.appearance)
                .cloned()
                .unwrap_or_default(),
        });
        if object.collision {
            ensure!(
                ["wall", "floor", "ceiling", "structure"].contains(&module.role.as_str())
                    && (high - low).min_element() > 0.00001,
                "Solid boxes require a nonzero reviewed structural module; they fill openings"
            );
            let center = Quat::from_rotation_z(object.yaw.to_radians()) * low.midpoint(high)
                + Vec3::from_array(position);
            compiled.solids.push(Solid {
                id: format!("object:{}", object.id),
                center: center.to_array(),
                size: (high - low).to_array(),
                yaw: object.yaw,
            });
        }
    }
    if !project.objects.is_empty() {
        compiled.diagnostics.push("Separate meshes are visual only unless authored with structural box collision. Complete entities retain native components; interaction and device behaviour require in-game validation.".into());
    }
    crate::stairs::geometry(&mut compiled, &kit, &modules)?;
    crate::interiors::compile(&mut compiled, &kit, &modules)?;
    Ok(compiled)
}

#[expect(
    clippy::too_many_arguments,
    reason = "explicit measured placement fields"
)]
pub(crate) fn add_piece(
    compiled: &mut Compiled,
    kit: &Kit,
    modules: &BTreeMap<&str, &Module>,
    id: String,
    role: &str,
    mut position: [f32; 3],
    yaw: f32,
    scale: [f32; 3],
    cutaway: bool,
) -> Result<()> {
    let piece = &kit.pieces[role];
    let angle = yaw.to_radians();
    position[0] += piece.offset[0] * angle.cos() - piece.offset[1] * angle.sin();
    position[1] += piece.offset[0] * angle.sin() + piece.offset[1] * angle.cos();
    position[2] += piece.offset[2];
    let module = modules
        .get(piece.mesh.as_str())
        .ok_or_else(|| anyhow::anyhow!("Measured kit mesh missing from library"))?;
    let appearance = match role {
        "floor" => &compiled.project.floor_appearance,
        "wall" | "doorway" => &compiled.project.wall_appearance,
        _ => &piece.preview_appearance,
    };
    ensure!(
        module.appearances.contains(appearance),
        "Appearance {appearance} is absent on {}",
        piece.mesh
    );
    compiled.placements.push(Placement {
        id,
        role: role.into(),
        mesh: piece.mesh.clone(),
        appearance: appearance.clone(),
        position,
        yaw,
        scale,
        bounds: module.bounds.clone(),
        cutaway,
        rotation: None,
        reference_only: false,
        entity: false,
        assembly_options: Default::default(),
        level: ((position[2]
            - if role == "ceiling" {
                kit.wall_height
            } else {
                0.
            })
            / kit.storey_height)
            .round()
            .max(0.) as u32,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn example() -> Project {
        serde_json::from_str(include_str!("../examples/workshop.building.json")).unwrap()
    }
    #[test]
    fn adjoining_rooms_share_one_collision_wall_and_two_facings() {
        let mut p = example();
        p.rooms = vec![
            Room {
                id: "a".into(),
                x: 0,
                y: 0,
                width: 1,
                depth: 1,
                level: 0,
                ceiling_height: None,
            },
            Room {
                id: "b".into(),
                x: 1,
                y: 0,
                width: 1,
                depth: 1,
                level: 0,
                ceiling_height: None,
            },
        ];
        p.openings.clear();
        let c = compile(&p, &test_library().unwrap()).unwrap();
        assert_eq!(
            (c.edges.len(), c.placements.len(), c.solids.len()),
            (7, 12, 11)
        );
    }
    #[test]
    fn overlapping_rooms_and_stale_openings_fail() {
        let mut p = example();
        p.rooms.push(p.rooms[0].clone());
        p.rooms[1].id = "overlap".into();
        assert!(compile(&p, &test_library().unwrap()).is_err());
        p.rooms.pop();
        p.openings.insert("0:x:100:100".into(), Opening::Doorway);
        assert!(compile(&p, &test_library().unwrap()).is_err());
    }
    #[test]
    fn coordinate_conversion_preserves_native_yaw_and_vertical_axis() {
        let result = transform([2., 3., 4.], 90., [1.; 3]).transform_point3(Vec3::X);
        assert!((result - Vec3::new(2., 4., -4.)).length() < 0.00001);
    }
}
