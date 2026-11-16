//! Analytic Human surfaces, route queries and native VAND v8 tile resources.
use crate::{
    model::{Compiled, Opening, Project, kit},
    technical,
};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use geo::{BooleanOps, TriangulateEarcut};
use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const TILE: f32 = 32.4;
type Triangle = [[f32; 3]; 3];
type PointKey = [i64; 3];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub export: bool,
    pub radius: f32,
    pub height: f32,
    pub climb: f32,
    pub layer: u8,
    pub connections: Vec<Connection>,
    pub terrain_mask: Option<crate::terrain::Mask>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            export: false,
            radius: 0.3,
            height: 1.8,
            climb: 0.6,
            layer: 200,
            connections: Vec::new(),
            terrain_mask: None,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub id: String,
    pub from: [f32; 3],
    pub to: [f32; 3],
}
#[derive(Clone, Serialize)]
pub struct Link {
    pub id: u64,
    pub reference: String,
    pub endpoints: [[f32; 3]; 2],
    pub enabled: bool,
    pub area: u8,
    pub radius: f32,
}
#[derive(Serialize)]
pub struct Navigation {
    pub triangles: Vec<Triangle>,
    pub links: Vec<Link>,
}

pub fn validate(p: &Project) -> Result<()> {
    let s = &p.navigation;
    ensure!(
        s.radius.is_finite()
            && (0.2..=0.65).contains(&s.radius)
            && s.height.is_finite()
            && (1.2..=2.4).contains(&s.height)
            && s.climb.is_finite()
            && (0.1..=0.6).contains(&s.climb),
        "Invalid Human navigation dimensions"
    );
    ensure!(
        !s.export || s.layer != 0,
        "Additive navigation requires a nonzero layer"
    );
    ensure!(s.connections.len() <= 64, "Too many entrance connections");
    let mut ids = BTreeSet::new();
    for link in &s.connections {
        crate::model::identifier(&link.id)?;
        ensure!(ids.insert(&link.id), "Duplicate entrance connection");
        let a = Vec3::from_array(link.from);
        let b = Vec3::from_array(link.to);
        ensure!(
            a.is_finite() && b.is_finite() && a.distance(b) <= 4. && (a.z - b.z).abs() <= 0.6,
            "Entrance connection must be within 4 m and 0.6 m height"
        );
    }
    Ok(())
}

fn key(p: [f32; 3]) -> PointKey {
    p.map(|v| (f64::from(v) * 10000.).round() as i64)
}
fn rect_contains(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] - 0.00001 && x <= r[2] + 0.00001 && y >= r[1] - 0.00001 && y <= r[3] + 0.00001
}
fn coordinates(values: &mut Vec<f32>) {
    values.sort_by(f32::total_cmp);
    values.dedup_by(|a, b| (*a - *b).abs() < 0.0001);
}
fn quad(out: &mut Vec<Triangle>, a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3]) {
    out.push([a, b, c]);
    out.push([a, c, d]);
}

pub fn compile(c: &Compiled) -> Result<Navigation> {
    let s = &c.project.navigation;
    let k = kit(&c.project.kit)?;
    let mut triangles = Vec::new();
    for level in 0..4 {
        let z = level as f32 * k.storey_height;
        let floors: Vec<_> = c
            .solids
            .iter()
            .filter(|solid| {
                solid.id.starts_with(&format!("tile:{level}:")) && solid.id.contains(":floor:")
            })
            .map(|solid| {
                [
                    solid.center[0] - solid.size[0] / 2.,
                    solid.center[1] - solid.size[1] / 2.,
                    solid.center[0] + solid.size[0] / 2.,
                    solid.center[1] + solid.size[1] / 2.,
                ]
            })
            .collect();
        if floors.is_empty() {
            continue;
        }
        let mut blocks = Vec::new();
        for solid in c.solids.iter().chain(&c.reference_solids) {
            if solid.center[2] + solid.size[2] / 2. <= z + s.climb
                || solid.center[2] - solid.size[2] / 2. >= z + s.height
                || solid.id.starts_with("stair:")
            {
                continue;
            }
            let a = solid.yaw.to_radians();
            let sx =
                (solid.size[0] * a.cos().abs() + solid.size[1] * a.sin().abs()) / 2. + s.radius;
            let sy =
                (solid.size[0] * a.sin().abs() + solid.size[1] * a.cos().abs()) / 2. + s.radius;
            blocks.push([
                solid.center[0] - sx,
                solid.center[1] - sy,
                solid.center[0] + sx,
                solid.center[1] + sy,
            ]);
        }
        for stair in &c.project.stairs {
            if level == stair.level || level == stair.level + 1 {
                let back = if stair.kind == "return" {
                    stair.landing
                } else {
                    0.
                };
                let r = if stair.axis == 'y' {
                    [
                        stair.x,
                        stair.y - back,
                        stair.x + stair.width,
                        stair.y + stair.run,
                    ]
                } else {
                    [
                        stair.x - back,
                        stair.y,
                        stair.x + stair.run,
                        stair.y + stair.width,
                    ]
                };
                blocks.push(r);
            }
        }
        // Device surfaces terminate at their plane; the corresponding Door
        // off-mesh polygon owns traversal and locked/unlocked state.
        for (edge, opening) in &c.project.openings {
            if !matches!(opening, Opening::DoorAuto | Opening::DoorManual) {
                continue;
            }
            if let Some(e) = c.edges.iter().find(|e| e.id == *edge && e.level == level) {
                blocks.push(if e.axis == 'x' {
                    [
                        e.x as f32 * 3.,
                        e.y as f32 * 3. - 0.03,
                        (e.x + 1) as f32 * 3.,
                        e.y as f32 * 3. + 0.03,
                    ]
                } else {
                    [
                        e.x as f32 * 3. - 0.03,
                        e.y as f32 * 3.,
                        e.x as f32 * 3. + 0.03,
                        (e.y + 1) as f32 * 3.,
                    ]
                });
            }
        }
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        for r in &floors {
            xs.extend([r[0], r[2], r[0] + s.radius, r[2] - s.radius]);
            ys.extend([r[1], r[3], r[1] + s.radius, r[3] - s.radius]);
        }
        for r in &blocks {
            xs.extend([r[0], r[2]]);
            ys.extend([r[1], r[3]]);
        }
        for stair in &c.project.stairs {
            if stair.axis == 'y' {
                xs.extend([stair.x + s.radius, stair.x + stair.width - s.radius]);
                ys.extend([stair.y, stair.y + stair.run]);
            } else {
                ys.extend([stair.y + s.radius, stair.y + stair.width - s.radius]);
                xs.extend([stair.x, stair.x + stair.run]);
            }
        }
        coordinates(&mut xs);
        coordinates(&mut ys);
        ensure!(
            xs.len() * ys.len() <= 200000,
            "Navigation surface complexity exceeds supported limits"
        );
        for x in xs.windows(2) {
            for y in ys.windows(2) {
                let cx = (x[0] + x[1]) / 2.;
                let cy = (y[0] + y[1]) / 2.;
                let on_floor = [
                    [-s.radius, -s.radius],
                    [-s.radius, s.radius],
                    [s.radius, -s.radius],
                    [s.radius, s.radius],
                ]
                .iter()
                .all(|d| {
                    floors
                        .iter()
                        .any(|r| rect_contains(*r, cx + d[0], cy + d[1]))
                        || c.project
                            .stairs
                            .iter()
                            .filter(|stair| level == stair.level || level == stair.level + 1)
                            .any(|stair| {
                                rect_contains(
                                    if stair.axis == 'y' {
                                        [
                                            stair.x,
                                            stair.y,
                                            stair.x + stair.width,
                                            stair.y + stair.run,
                                        ]
                                    } else {
                                        [
                                            stair.x,
                                            stair.y,
                                            stair.x + stair.run,
                                            stair.y + stair.width,
                                        ]
                                    },
                                    cx + d[0],
                                    cy + d[1],
                                )
                            })
                });
                if on_floor && !blocks.iter().any(|r| rect_contains(*r, cx, cy)) {
                    quad(
                        &mut triangles,
                        [x[0], y[0], z],
                        [x[0], y[1], z],
                        [x[1], y[1], z],
                        [x[1], y[0], z],
                    );
                }
            }
        }
    }
    // Flights are continuous sloped walking surfaces over the physical treads.
    for stair in &c.project.stairs {
        let [base, upper] = crate::stairs::elevations(&c.project, &k, stair);
        let rise = upper - base;
        let flights = if stair.kind == "return" { 2 } else { 1 };
        for side in 0..flights {
            let fw = if flights == 2 {
                stair.flight_width
            } else {
                stair.width
            };
            let x = if side == 1 { stair.width - fw } else { 0. };
            let levels = if flights == 2 {
                if side == 0 {
                    [base + rise / 2., base]
                } else {
                    [base + rise / 2., upper]
                }
            } else if stair.reverse {
                [upper, base]
            } else {
                [base, upper]
            };
            let point = |x: f32, y: f32, z: f32| {
                if stair.axis == 'y' {
                    [stair.x + x, stair.y + y, z]
                } else {
                    [stair.x + y, stair.y + x, z]
                }
            };
            let left = x + s.radius;
            let right = x + fw - s.radius;
            quad(
                &mut triangles,
                point(left, 0., levels[0]),
                point(left, stair.run, levels[1]),
                point(right, stair.run, levels[1]),
                point(right, 0., levels[0]),
            );
        }
        if flights == 2 {
            let h = base + rise / 2.;
            let l = stair.x + s.radius;
            let r = stair.x + stair.width - s.radius;
            let b = stair.y - stair.landing + s.radius;
            let t = stair.y;
            quad(&mut triangles, [l, b, h], [l, t, h], [r, t, h], [r, b, h]);
        }
    }
    shell_surfaces(c, &mut triangles)?;
    // Split edges at every collinear surface vertex to preserve reciprocal
    // adjacency where ramp landings meet the horizontal decomposition.
    triangles = conform(triangles)?;
    let mut links = Vec::new();
    for (edge, opening) in &c.project.openings {
        if !matches!(opening, Opening::DoorAuto | Opening::DoorManual) {
            continue;
        }
        let visual = c
            .placements
            .iter()
            .find(|p| p.id == format!("wall:{edge}"))
            .context("Door placement missing")?;
        let a = visual.yaw.to_radians();
        let middle = [
            visual.position[0] - 1.5 * a.cos(),
            visual.position[1] - 1.5 * a.sin(),
            visual.position[2],
        ];
        let reference = format!(
            "$/03_night_city/#building_builder_{}/door_{}_connection",
            c.project.id,
            edge.replace(':', "_")
        );
        links.push(Link {
            id: technical::node_hash(&reference),
            reference,
            endpoints: [-1., 1.].map(|d| {
                [
                    middle[0] - d * 0.8 * a.sin(),
                    middle[1] + d * 0.8 * a.cos(),
                    middle[2],
                ]
            }),
            enabled: !matches!(
                c.project.door_states.get(edge),
                Some(technical::DoorState::Locked)
            ),
            area: 10,
            radius: 0.75,
        });
    }
    for connection in &s.connections {
        let reference = format!(
            "$/03_night_city/#building_builder_{}/entrance_{}",
            c.project.id, connection.id
        );
        links.push(Link {
            id: technical::node_hash(&reference),
            reference,
            endpoints: [connection.from, connection.to],
            enabled: true,
            area: 0,
            radius: 0.75,
        });
    }
    for (id, device) in &c.project.door_devices {
        if device.position.is_none() {
            continue;
        }
        let reference = format!(
            "$/03_night_city/#building_builder_{}/door_{}_connection",
            c.project.id, id
        );
        links.push(Link {
            id: technical::node_hash(&reference),
            reference,
            endpoints: device.endpoints.context("Shell door endpoints absent")?,
            enabled: !matches!(
                c.project.door_states.get(id),
                Some(technical::DoorState::Locked)
            ),
            area: 10,
            radius: 0.75,
        });
    }
    Ok(Navigation { triangles, links })
}

fn convex_outline(mut points: Vec<glam::Vec2>) -> Vec<[f32; 2]> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup_by(|a, b| a.distance_squared(*b) < 1e-12);
    let mut hull = Vec::<glam::Vec2>::new();
    for p in &points {
        while hull.len() >= 2
            && (hull[hull.len() - 1] - hull[hull.len() - 2]).perp_dot(*p - hull[hull.len() - 1])
                <= 0.
        {
            hull.pop();
        }
        hull.push(*p);
    }
    let lower = hull.len();
    for p in points.iter().rev().skip(1) {
        while hull.len() > lower
            && (hull[hull.len() - 1] - hull[hull.len() - 2]).perp_dot(*p - hull[hull.len() - 1])
                <= 0.
        {
            hull.pop();
        }
        hull.push(*p);
    }
    hull.pop();
    hull.into_iter().map(|p| p.to_array()).collect()
}

fn shell_surfaces(c: &Compiled, triangles: &mut Vec<Triangle>) -> Result<()> {
    let s = &c.project.navigation;
    for room in &c.project.shell_rooms {
        let [w, d, h] = room.size;
        ensure!(h >= s.height, "Shell room is below agent head clearance");
        let inset = s.radius;
        ensure!(
            w > 2. * inset && d > 2. * inset,
            "Shell room has no agent clearance"
        );
        let rotation = Quat::from_rotation_z(room.yaw.to_radians());
        let inverse = rotation.inverse();
        let origin = Vec3::from_array(room.position);
        let boundary = vec![
            [inset, inset],
            [w - inset, inset],
            [w - inset, d - inset],
            [inset, d - inset],
        ];
        let mut cuts = Vec::new();
        for stair in c
            .project
            .stairs
            .iter()
            .filter(|s| room.level == s.level || room.level == s.level + 1)
        {
            let r = crate::stairs::rectangle(
                stair,
                [
                    0.,
                    if stair.kind == "return" {
                        -stair.landing
                    } else {
                        0.
                    },
                    stair.width,
                    stair.run,
                ],
            );
            let cut = [[r[0], r[1]], [r[2], r[1]], [r[2], r[3]], [r[0], r[3]]].map(|[x, y]| {
                let point = inverse * (Vec3::new(x, y, origin.z) - origin);
                [point.x, point.y]
            });
            cuts.push(cut.to_vec());
        }
        for solid in &c.solids {
            if solid.center[2] + solid.size[2] / 2. <= origin.z + s.climb
                || solid.center[2] - solid.size[2] / 2. >= origin.z + s.height
            {
                continue;
            }
            let local = inverse * (Vec3::from_array(solid.center) - origin);
            let facing = Quat::from_rotation_z((solid.yaw - room.yaw).to_radians());
            let sx = solid.size[0] / 2. + s.radius;
            let sy = solid.size[1] / 2. + s.radius;
            let cut = [[-sx, -sy], [sx, -sy], [sx, sy], [-sx, sy]].map(|[x, y]| {
                let p = local + facing * Vec3::new(x, y, 0.);
                [p.x, p.y]
            });
            cuts.push(cut.to_vec());
        }
        for geometry in c
            .reference_collision
            .iter()
            .chain(c.cooked_collision.iter().map(|c| &c.geometry))
        {
            for indices in &geometry.triangles {
                let mut face: Vec<Vec3> = indices
                    .iter()
                    .map(|&i| inverse * (Vec3::from_array(geometry.vertices[i as usize]) - origin))
                    .collect();
                for (height, above) in [(s.climb, true), (s.height, false)] {
                    let mut clipped = Vec::new();
                    for i in 0..face.len() {
                        let a = face[i];
                        let b = face[(i + 1) % face.len()];
                        let da = if above { height - a.z } else { a.z - height };
                        let db = if above { height - b.z } else { b.z - height };
                        if da <= 0. {
                            clipped.push(a);
                        }
                        if (da <= 0.) != (db <= 0.) {
                            clipped.push(a.lerp(b, da / (da - db)));
                        }
                    }
                    face = clipped;
                }
                if face.is_empty()
                    || face.iter().all(|p| p.x < -s.radius)
                    || face.iter().all(|p| p.x > w + s.radius)
                    || face.iter().all(|p| p.y < -s.radius)
                    || face.iter().all(|p| p.y > d + s.radius)
                {
                    continue;
                }
                let mut points = Vec::<glam::Vec2>::new();
                // Circumscribed radius keeps the full circular agent clear between samples.
                let radius = s.radius / (std::f32::consts::PI / 12.).cos();
                for p in face {
                    for i in 0..12 {
                        let angle = i as f32 * std::f32::consts::TAU / 12.;
                        points.push(
                            p.truncate() + glam::Vec2::new(angle.cos(), angle.sin()) * radius,
                        );
                    }
                }
                let cut = convex_outline(points);
                cuts.push(cut);
            }
        }
        for (id, device) in &c.project.door_devices {
            let Some(position) = device.position else {
                continue;
            };
            let point = inverse * (Vec3::from_array(position) - origin);
            if point.z.abs() > s.climb {
                continue;
            }
            let (_, _, hinge, _) = technical::door_preset(&device.preset)?;
            let angle = (device.yaw - room.yaw).to_radians();
            let center = point - Vec3::new(hinge * angle.cos(), hinge * angle.sin(), 0.);
            let facing = Quat::from_rotation_z(angle);
            let half = if device.preset == "sliding_kitsch_double_f"
                || device.preset == "garage_w300_h250"
            {
                1.5 * device.scale[0]
            } else {
                0.75 * device.scale[0]
            };
            let cut = [[-half, -0.02], [half, -0.02], [half, 0.02], [-half, 0.02]].map(|[x, y]| {
                let p = center + facing * Vec3::new(x, y, 0.);
                [p.x, p.y]
            });
            cuts.push(cut.to_vec());
            let _ = id;
        }
        let polygon = |points: Vec<[f32; 2]>| {
            geo::Polygon::new(
                geo::LineString::from(
                    points
                        .into_iter()
                        .map(|p| (f64::from(p[0]), f64::from(p[1])))
                        .collect::<Vec<_>>(),
                ),
                vec![],
            )
        };
        let obstacles = geo::unary_union(cuts.into_iter().map(polygon).collect::<Vec<_>>().iter());
        let walkable = polygon(boundary).difference(&obstacles);
        for polygon in walkable {
            for face in polygon.earcut_triangles() {
                let t = face.to_array().map(|p| {
                    (origin + rotation * Vec3::new(p.x as f32, p.y as f32, 0.)).to_array()
                });
                let a = Vec3::from_array(t[1]) - Vec3::from_array(t[0]);
                let b = Vec3::from_array(t[2]) - Vec3::from_array(t[0]);
                if a.cross(b).length() > 0.000001 {
                    triangles.push(t);
                }
            }
        }
    }
    Ok(())
}

fn conform(triangles: Vec<Triangle>) -> Result<Vec<Triangle>> {
    let mut unique = BTreeSet::new();
    let triangles: Vec<Triangle> = triangles
        .into_iter()
        .map(|t| t.map(|p| key(p).map(|n| n as f32 / 10000.)))
        .filter(|t| {
            let [a, b, c] = t.map(Vec3::from_array);
            let mut identity = t.map(key);
            identity.sort();
            (b - a).cross(c - a).length_squared() > 1e-12 && unique.insert(identity)
        })
        .collect();
    let vertices: BTreeMap<_, _> = triangles.iter().flatten().map(|p| (key(*p), *p)).collect();
    ensure!(vertices.len() <= 100000, "Navigation vertex limit exceeded");
    // Only boundary edges need subdivision; internal grid edges already match.
    let mut owners = BTreeMap::new();
    for t in &triangles {
        for i in 0..3 {
            let mut edge = [key(t[i]), key(t[(i + 1) % 3])];
            edge.sort();
            *owners.entry(edge).or_insert(0_u32) += 1;
        }
    }
    let mut result = Vec::new();
    for triangle in triangles {
        let mut polygon = Vec::new();
        for i in 0..3 {
            let a = Vec3::from_array(triangle[i]);
            let b = Vec3::from_array(triangle[(i + 1) % 3]);
            let d = b - a;
            polygon.push(triangle[i]);
            let mut edge = [key(a.to_array()), key(b.to_array())];
            edge.sort();
            if owners[&edge] != 1 {
                continue;
            }
            let mut points: Vec<_> = vertices
                .values()
                .filter_map(|p| {
                    let p = Vec3::from_array(*p);
                    let f = (p - a).dot(d) / d.length_squared();
                    (f > 0.00001 && f < 0.99999 && (a + d * f).distance(p) < 0.0001)
                        .then_some((f, p.to_array()))
                })
                .collect();
            points.sort_by(|a, b| a.0.total_cmp(&b.0));
            polygon.extend(points.into_iter().map(|p| p.1));
        }
        if polygon.len() == 3 {
            result.push(triangle);
        } else {
            let center = (Vec3::from_array(triangle[0])
                + Vec3::from_array(triangle[1])
                + Vec3::from_array(triangle[2]))
                / 3.;
            for i in 0..polygon.len() {
                result.push([
                    center.to_array(),
                    polygon[i],
                    polygon[(i + 1) % polygon.len()],
                ]);
            }
        }
    }
    Ok(result)
}

pub fn route(nav: &Navigation, start: [f32; 3], end: [f32; 3]) -> Result<Value> {
    let start = locate(&nav.triangles, start)
        .context("Start point is outside generated Human navigation")?;
    let end =
        locate(&nav.triangles, end).context("End point is outside generated Human navigation")?;
    let mut edges: BTreeMap<_, Vec<usize>> = BTreeMap::new();
    let mut neighbors = vec![Vec::new(); nav.triangles.len()];
    for (face, t) in nav.triangles.iter().enumerate() {
        for i in 0..3 {
            let mut e = [key(t[i]), key(t[(i + 1) % 3])];
            e.sort();
            edges.entry(e).or_default().push(face);
        }
    }
    for owners in edges.values() {
        if let [a, b] = owners[..] {
            neighbors[a].push(b);
            neighbors[b].push(a);
        }
    }
    for link in nav.links.iter().filter(|l| l.enabled) {
        if let Some((a, b)) =
            locate(&nav.triangles, link.endpoints[0]).zip(locate(&nav.triangles, link.endpoints[1]))
        {
            neighbors[a].push(b);
            neighbors[b].push(a);
        }
    }
    let mut previous = vec![None; nav.triangles.len()];
    let mut pending = VecDeque::from([start]);
    previous[start] = Some(start);
    while let Some(face) = pending.pop_front() {
        if face == end {
            break;
        }
        for &next in &neighbors[face] {
            if previous[next].is_none() {
                previous[next] = Some(face);
                pending.push_back(next);
            }
        }
    }
    let mut path = Vec::new();
    if previous[end].is_some() {
        let mut cursor = end;
        loop {
            path.push(cursor);
            if cursor == start {
                break;
            }
            cursor = previous[cursor].context("Incomplete route")?;
        }
        path.reverse();
    }
    Ok(
        json!({"connected":!path.is_empty(),"faces":path,"triangles":path.iter().map(|&i|nav.triangles[i]).collect::<Vec<_>>()}),
    )
}

fn locate(triangles: &[Triangle], point: [f32; 3]) -> Option<usize> {
    let p = Vec3::from_array(point);
    triangles.iter().position(|t| {
        let a = Vec3::from_array(t[0]);
        let b = Vec3::from_array(t[1]);
        let c = Vec3::from_array(t[2]);
        let v0 = (b - a).truncate();
        let v1 = (c - a).truncate();
        let v2 = (p - a).truncate();
        let denominator = v0.perp_dot(v1);
        if denominator.abs() < 0.000001 {
            return false;
        }
        let u = v2.perp_dot(v1) / denominator;
        let v = v0.perp_dot(v2) / denominator;
        u >= -0.00001
            && v >= -0.00001
            && u + v <= 1.00001
            && (a.z + u * (b.z - a.z) + v * (c.z - a.z) - p.z).abs() < 0.6
    })
}

fn clip(polygon: Vec<[f32; 3]>, axis: usize, boundary: f32, greater: bool) -> Vec<[f32; 3]> {
    let mut output = Vec::new();
    if polygon.is_empty() {
        return output;
    }
    let inside = |p: [f32; 3]| {
        if greater {
            p[axis] >= boundary - 0.000001
        } else {
            p[axis] <= boundary + 0.000001
        }
    };
    let mut a = *polygon.last().unwrap_or(&[0.; 3]);
    for b in polygon {
        if inside(a) != inside(b) {
            let f = (boundary - a[axis]) / (b[axis] - a[axis]);
            let mut point = (Vec3::from_array(a).lerp(Vec3::from_array(b), f)).to_array();
            point[axis] = boundary;
            output.push(point);
        }
        if inside(b) {
            output.push(b);
        }
        a = b;
    }
    output
}

pub fn resource(c: &Compiled, nav: &Navigation) -> Result<Value> {
    let settings = &c.project.navigation;
    let rotation = Quat::from_rotation_z(c.project.yaw.to_radians());
    let origin = Vec3::from_array(c.project.origin);
    let world = |p: [f32; 3]| (rotation * Vec3::from_array(p) + origin).to_array();
    let mut groups: BTreeMap<(i32, i32), Vec<Triangle>> = BTreeMap::new();
    for triangle in &nav.triangles {
        let t = triangle.map(world);
        let minx = t.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
        let maxx = t.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max);
        let miny = t.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let maxy = t.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);
        for tx in (minx / TILE).floor() as i32..=(maxx / TILE).floor() as i32 {
            for ty in (miny / TILE).floor() as i32..=(maxy / TILE).floor() as i32 {
                let mut polygon = t.to_vec();
                for (axis, boundary, greater) in [
                    (0, tx as f32 * TILE, true),
                    (0, (tx + 1) as f32 * TILE, false),
                    (1, ty as f32 * TILE, true),
                    (1, (ty + 1) as f32 * TILE, false),
                ] {
                    polygon = clip(polygon, axis, boundary, greater);
                }
                if polygon.len() >= 3 {
                    for i in 1..polygon.len() - 1 {
                        let triangle = [polygon[0], polygon[i], polygon[i + 1]];
                        let a = Vec3::from_array(triangle[0]);
                        let b = Vec3::from_array(triangle[1]);
                        let d = Vec3::from_array(triangle[2]);
                        if (b - a).cross(d - a).length() > 0.000001 {
                            groups.entry((tx, ty)).or_default().push(triangle);
                        }
                    }
                }
            }
        }
    }
    ensure!(
        !groups.is_empty(),
        "No Human navigation surfaces were generated"
    );
    let mut buffers = Vec::new();
    let mut metadata = Vec::new();
    let links: Vec<_> = nav
        .links
        .iter()
        .cloned()
        .map(|mut l| {
            l.endpoints = l.endpoints.map(world);
            l
        })
        .collect();
    for (&(tx, ty), triangles) in &groups {
        let links: Vec<_> = links
            .iter()
            .filter(|l| {
                (l.endpoints[0][0] / TILE).floor() as i32 == tx
                    && (l.endpoints[0][1] / TILE).floor() as i32 == ty
            })
            .collect();
        let raw = encode_tile(triangles, tx, ty, settings, &links)?;
        let index = buffers.len();
        buffers
            .push(json!({"BufferId":(index+1).to_string(),"Flags":0,"Bytes":STANDARD.encode(raw)}));
        let mut data = json!({"$type":"worldNavigationTileData","tileX":tx,"tileY":ty,"tileIndex":index,"bufferIndex":index,"agentSize":"Human","tileRef":"9223372036854775807"});
        if !links.is_empty() {
            data["offMeshConnections"] = json!({"$type":"worldOffMeshConnectionsData","verts":links.iter().flat_map(|l|l.endpoints.iter().flat_map(|p|[p[0],p[2],p[1]])).collect::<Vec<_>>(),"radii":links.iter().map(|l|l.radius).collect::<Vec<_>>(),"flags":links.iter().map(|l|if l.enabled{3}else{0}).collect::<Vec<_>>(),"areas":links.iter().map(|l|l.area).collect::<Vec<_>>(),"directions":vec![1;links.len()],"ids":links.iter().map(|l|l.id.to_string()).collect::<Vec<_>>(),"tagIntervals":vec![0;links.len()*2],"globalNodeIDs":links.iter().map(|l|json!({"$type":"worldGlobalNodeID","hash":l.id.to_string()})).collect::<Vec<_>>(),"tagsX":[],"userData":links.iter().enumerate().map(|(i,_)|json!({"HandleId":(500000+index*100+i).to_string(),"Data":{"$type":"worldOffMeshUserData"}})).collect::<Vec<_>>()});
        }
        metadata.push(data);
    }
    let points: Vec<_> = groups.values().flatten().flatten().copied().collect();
    let lower = [
        groups.keys().map(|p| p.0).min().context("No tiles")? as f32 * TILE - origin.x,
        groups.keys().map(|p| p.1).min().context("No tiles")? as f32 * TILE - origin.y,
        points.iter().map(|p| p[2]).fold(f32::INFINITY, f32::min) - 1. - origin.z,
    ];
    let upper = [
        (groups.keys().map(|p| p.0).max().context("No tiles")? + 1) as f32 * TILE - origin.x,
        (groups.keys().map(|p| p.1).max().context("No tiles")? + 1) as f32 * TILE - origin.y,
        points
            .iter()
            .map(|p| p[2])
            .fold(f32::NEG_INFINITY, f32::max)
            + settings.height
            - origin.z,
    ];
    let vector = |p: [f32; 3]| json!({"$type":"Vector4","X":p[0],"Y":p[1],"Z":p[2],"W":0.});
    Ok(
        json!({"$type":"worldNavigationTileResource","cookingPlatform":"PLATFORM_PC","agentSize":"Human","localBoundingBox":{"$type":"Box","Min":vector(lower),"Max":vector(upper)},"tilesData":metadata,"tileBuffers":buffers}),
    )
}

fn encode_tile(
    triangles: &[Triangle],
    tx: i32,
    ty: i32,
    s: &Settings,
    links: &[&Link],
) -> Result<Vec<u8>> {
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    let mut lookup = BTreeMap::new();
    for triangle in triangles {
        let mut t = *triangle;
        if (t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[1][1] - t[0][1]) * (t[2][0] - t[0][0])
            > 0.
        {
            t.reverse();
        }
        let mut face = [0_u16; 3];
        for (i, p) in t.into_iter().enumerate() {
            let index = *lookup.entry(key(p)).or_insert_with(|| {
                let index = vertices.len();
                vertices.push(p);
                index
            });
            face[i] = u16::try_from(index)?;
        }
        faces.push(face);
    }
    let count = faces.len();
    ensure!(
        count < 32768 && vertices.len() + links.len() * 2 < 65535,
        "Native navigation tile exceeds polygon/index limits"
    );
    let low = [
        tx as f32 * TILE,
        vertices.iter().map(|p| p[2]).fold(f32::INFINITY, f32::min) - 1.,
        ty as f32 * TILE,
    ];
    let high = [
        (tx + 1) as f32 * TILE,
        vertices
            .iter()
            .map(|p| p[2])
            .fold(f32::NEG_INFINITY, f32::max)
            + s.height,
        (ty + 1) as f32 * TILE,
    ];
    let mut edges: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for (i, face) in faces.iter().enumerate() {
        for slot in 0..3 {
            let mut edge = [face[slot], face[(slot + 1) % 3]];
            edge.sort();
            edges.entry(edge).or_default().push((i, slot));
        }
    }
    let mut neighbors = vec![[0_u16; 3]; count];
    let mut portals = 0;
    for (edge, owners) in edges {
        ensure!(owners.len() <= 2, "Non-manifold navigation edge");
        if let [(a, sa), (b, sb)] = owners[..] {
            neighbors[a][sa] = u16::try_from(b + 1)?;
            neighbors[b][sb] = u16::try_from(a + 1)?;
        } else {
            let (i, slot) = owners[0];
            let a = vertices[edge[0] as usize];
            let b = vertices[edge[1] as usize];
            for (axis, boundary, side) in [
                (0, low[0], 4),
                (0, high[0], 0),
                (1, low[2], 6),
                (1, high[2], 2),
            ] {
                if (a[axis] - boundary).abs() < 0.0001 && (b[axis] - boundary).abs() < 0.0001 {
                    neighbors[i][slot] = 0x8000 | side;
                    portals += 1;
                    break;
                }
            }
        }
    }
    let capacity = count * 3 + portals * 2 + links.len() * 4;
    let mut offmesh = Vec::new();
    for link in links {
        let index = vertices.len();
        vertices.extend(link.endpoints);
        offmesh.push([index as u16, (index + 1) as u16]);
    }
    let mut bytes = b"VAND".to_vec();
    for value in [
        8,
        tx as u32,
        ty as u32,
        u32::from(s.layer),
        0,
        (count + links.len()) as u32,
        vertices.len() as u32,
        capacity as u32,
        count as u32,
        0,
        count as u32,
        (count * 2) as u32,
        links.len() as u32,
        count as u32,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in [
        s.height, s.radius, s.climb, low[0], low[1], low[2], high[0], high[1], high[2], 10.,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for p in &vertices {
        for v in [p[0], p[2], p[1]] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
    }
    for (face, neighbors) in faces.iter().zip(neighbors) {
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        for value in face.iter().chain(neighbors.iter()).copied().chain([3]) {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&[3, 1]);
    }
    for (face, link) in offmesh.iter().zip(links) {
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        for value in [
            face[0],
            face[1],
            0,
            0,
            0,
            0,
            if link.enabled { 3 } else { 0 },
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&[2, 64 | link.area]);
    }
    bytes.resize(bytes.len() + capacity * 16, 0);
    for i in 0..count {
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&(i as u32).to_le_bytes());
        bytes.extend_from_slice(&[0, 1, 0, 0]);
    }
    for _ in 0..count {
        bytes.extend_from_slice(&[0, 1, 2, 21]);
    }
    let mut items: Vec<_> = faces
        .iter()
        .enumerate()
        .map(|(i, face)| {
            let p = face.map(|v| {
                let p = vertices[v as usize];
                [p[0], p[2], p[1]]
            });
            let quantize = |v: f32, a: usize| ((v - low[a]) * 10.).clamp(0., 65535.) as u16;
            let lower = std::array::from_fn(|a| {
                quantize(p.iter().map(|p| p[a]).fold(f32::INFINITY, f32::min), a)
            });
            let upper = std::array::from_fn(|a| {
                quantize(p.iter().map(|p| p[a]).fold(f32::NEG_INFINITY, f32::max), a)
            });
            (lower, upper, i as i32)
        })
        .collect();
    let mut tree = Vec::new();
    bv(&mut items, &mut tree);
    tree.push(([0; 3], [0; 3], 0));
    for (lower, upper, index) in tree {
        for v in lower.into_iter().chain(upper) {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        bytes.extend_from_slice(&index.to_le_bytes());
    }
    for (i, link) in links.iter().enumerate() {
        let [a, b] = link.endpoints;
        for v in [a[0], a[2], a[1], b[0], b[2], b[1], link.radius] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        bytes.extend_from_slice(&((count + i) as u16).to_le_bytes());
        let dx = if b[0] < low[0] {
            -1
        } else {
            i32::from(b[0] >= high[0])
        };
        let dy = if b[1] < low[2] {
            -1
        } else {
            i32::from(b[1] >= high[2])
        };
        let side = match (dx, dy) {
            (0, 0) => 255,
            (1, 0) => 0,
            (1, 1) => 1,
            (0, 1) => 2,
            (-1, 1) => 3,
            (-1, 0) => 4,
            (-1, -1) => 5,
            (0, -1) => 6,
            (1, -1) => 7,
            _ => 255,
        };
        bytes.extend_from_slice(&[1, side]);
        bytes.extend_from_slice(&link.id.to_le_bytes());
    }
    Ok(bytes)
}

type BvNode = ([u16; 3], [u16; 3], i32);
fn bv(items: &mut [BvNode], out: &mut Vec<BvNode>) {
    if items.is_empty() {
        return;
    }
    let index = out.len();
    let lower = std::array::from_fn(|a| items.iter().map(|p| p.0[a]).min().unwrap_or(0));
    let upper = std::array::from_fn(|a| items.iter().map(|p| p.1[a]).max().unwrap_or(0));
    out.push((lower, upper, items[0].2));
    if items.len() > 1 {
        let axis = (0..3).max_by_key(|&a| upper[a] - lower[a]).unwrap_or(0);
        items.sort_by_key(|p| (p.0[axis], p.2));
        let split = items.len() / 2;
        bv(&mut items[..split], out);
        bv(&mut items[split..], out);
        out[index].2 = -((out.len() - index) as i32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cooked_wall_aperture_changes_clearance_routes() {
        let mut p: Project =
            serde_json::from_str(include_str!("../examples/workshop.building.json")).unwrap();
        p.rooms.clear();
        p.openings.clear();
        p.objects.clear();
        p.shell_rooms = vec![crate::interiors::ShellRoom {
            id: "surveyed".into(),
            position: [0.; 3],
            size: [6., 6., 3.],
            yaw: 0.,
            level: 0,
            retained_sides: ["north", "south", "east", "west"]
                .map(String::from)
                .into_iter()
                .collect(),
            openings: BTreeMap::new(),
        }];
        let mut c = crate::model::compile(&p, &crate::model::test_library().unwrap()).unwrap();
        let geometry =
            ghostline_red::physics::CollisionGeometry::solid_box([2.9, 0., 0.], [3.1, 6., 3.]);
        c.reference_collision.push(geometry.clone());
        assert_eq!(
            route(&compile(&c).unwrap(), [1.5, 3., 0.], [4.5, 3., 0.]).unwrap()["connected"],
            false
        );
        c.reference_collision[0] = geometry
            .subtract_solid(&[
                [1., 0., -0., -4.],
                [-1., 0., 0., 2.],
                [0., 1., 0., -4.],
                [0., -1., 0., 2.],
                [0., 0., 1., -2.4],
                [0., 0., -1., -0.1],
            ])
            .unwrap();
        let nav = compile(&c).unwrap();
        assert_eq!(
            route(&nav, [1.5, 3., 0.], [4.5, 3., 0.]).unwrap()["connected"],
            true
        );
    }

    #[test]
    fn guarded_stairs_connect_both_floors() {
        let p: Project =
            serde_json::from_str(include_str!("../examples/stairs-and-devices.building.json"))
                .unwrap();
        let c = crate::model::compile(&p, &crate::model::test_library().unwrap()).unwrap();
        let nav = compile(&c).unwrap();
        assert_eq!(
            route(&nav, [1.5, 1.5, 0.], [1.5, 1.5, 4.2]).unwrap()["connected"],
            true
        );
    }

    #[test]
    fn locked_device_blocks_the_same_doorway_that_an_unlocked_device_connects() {
        let mut p: Project =
            serde_json::from_str(include_str!("../examples/workshop.building.json")).unwrap();
        p.rooms[0].width = 1;
        p.rooms[0].depth = 1;
        let mut room = p.rooms[0].clone();
        room.id = "second".into();
        room.x = 1;
        p.rooms.push(room);
        p.openings.clear();
        p.openings.insert("0:y:1:0".into(), Opening::DoorManual);
        let library = crate::model::test_library().unwrap();
        let nav = compile(&crate::model::compile(&p, &library).unwrap()).unwrap();
        assert_eq!(
            route(&nav, [1.5, 1.5, 0.], [4.5, 1.5, 0.]).unwrap()["connected"],
            true
        );
        p.door_states
            .insert("0:y:1:0".into(), technical::DoorState::Locked);
        let nav = compile(&crate::model::compile(&p, &library).unwrap()).unwrap();
        assert_eq!(
            route(&nav, [1.5, 1.5, 0.], [4.5, 1.5, 0.]).unwrap()["connected"],
            false
        );
    }
}
