//! Retains surveyed Human navigation while disabling only covered polygon flags.
use crate::{model::Compiled, retrofit::SectorPatch};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use ghostline_renderer::GameSource;
use glam::{Quat, Vec2, Vec3};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Mask {
    pub sector: String,
    pub sha256: String,
    pub expected_nodes: usize,
    pub placement_index: usize,
    pub resource_index: usize,
    pub outline: Vec<[f32; 2]>,
    pub height_range: [f32; 2],
}
fn u32_at(data: &[u8], off: usize) -> Result<usize> {
    Ok(u32::from_le_bytes(
        data.get(off..off + 4)
            .context("Truncated VAND header")?
            .try_into()?,
    ) as usize)
}
fn u16_at(data: &[u8], off: usize) -> Result<usize> {
    Ok(u16::from_le_bytes(
        data.get(off..off + 2)
            .context("Truncated VAND polygon")?
            .try_into()?,
    ) as usize)
}
fn f32_at(data: &[u8], off: usize) -> Result<f32> {
    let n = f32::from_le_bytes(
        data.get(off..off + 4)
            .context("Truncated VAND vertex")?
            .try_into()?,
    );
    ensure!(n.is_finite(), "Invalid navigation vertex");
    Ok(n)
}
fn cross(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    (b - a).perp_dot(c - a)
}
fn intersects(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    cross(a, b, c) * cross(a, b, d) < -0.0000001 && cross(c, d, a) * cross(c, d, b) < -0.0000001
}
fn contains(polygon: &[Vec2], p: Vec2) -> bool {
    let mut inside = false;
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];
        if cross(a, b, p).abs() < 0.00001 && (p - a).dot(p - b) <= 0.00001 {
            return true;
        }
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}

pub fn masks(project: &crate::model::Project) -> Vec<&Mask> {
    project
        .navigation
        .terrain_mask
        .iter()
        .chain(
            project
                .shells
                .iter()
                .flat_map(|s| &s.operations)
                .flat_map(|op| &op.navigation_masks),
        )
        .collect()
}

pub fn build(c: &Compiled, source: &GameSource) -> Result<Vec<(Value, Value)>> {
    let mut groups = std::collections::BTreeMap::<_, Vec<_>>::new();
    for mask in masks(&c.project) {
        groups
            .entry((&mask.sector, mask.placement_index, mask.resource_index))
            .or_default()
            .push(mask);
    }
    groups
        .values()
        .enumerate()
        .map(|(index, masks)| build_tile(c, source, masks, index))
        .collect()
}

fn build_tile(
    c: &Compiled,
    source: &GameSource,
    masks: &[&Mask],
    tile_index: usize,
) -> Result<(Value, Value)> {
    let mask = masks
        .first()
        .context("Navigation replacement has no masks")?;
    let mut polygons = Vec::new();
    for mask in masks {
        ensure!(
            mask.sha256 == masks[0].sha256 && mask.expected_nodes == masks[0].expected_nodes,
            "Navigation masks disagree about their native source"
        );
        ensure!(
            (3..=64).contains(&mask.outline.len())
                && mask
                    .outline
                    .iter()
                    .flatten()
                    .all(|v| v.is_finite() && v.abs() <= 1000.)
                && mask.height_range.iter().all(|v| v.is_finite())
                && mask.height_range[0] < mask.height_range[1],
            "Invalid terrain mask outline/heights"
        );
        let polygon: Vec<_> = mask.outline.iter().copied().map(Vec2::from_array).collect();
        for i in 0..polygon.len() {
            for j in i + 1..polygon.len() {
                ensure!(
                    !intersects(
                        polygon[i],
                        polygon[(i + 1) % polygon.len()],
                        polygon[j],
                        polygon[(j + 1) % polygon.len()]
                    ),
                    "Terrain mask self-intersects"
                );
            }
        }
        polygons.push((polygon, mask.height_range));
    }
    let patch = SectorPatch {
        path: mask.sector.clone(),
        sha256: mask.sha256.clone(),
        expected_nodes: mask.expected_nodes,
        deletions: Vec::new(),
    };
    let mut document = crate::retrofit::pinned(source, &patch)?;
    ensure!(
        c.project.retrofit.iter().any(|p| p.path == mask.sector
            && p.deletions
                .iter()
                .any(|d| d["index"] == mask.placement_index && d["type"] == "worldNavigationNode")),
        "Terrain replacement must remove its original navigation node"
    );
    let root = &document["Data"]["RootChunk"];
    let mut placement = root["nodeData"]["Data"]
        .get(mask.placement_index)
        .context("Terrain placement absent")?
        .clone();
    let index = placement["NodeIndex"]
        .as_u64()
        .context("Terrain node index absent")? as usize;
    let mut node = root["nodes"]
        .get(index)
        .context("Terrain node absent")?
        .clone();
    ensure!(
        node["Data"]["$type"] == "worldNavigationNode",
        "Selected terrain node changed"
    );
    let embedded = document["Data"]["EmbeddedFiles"]
        .get_mut(mask.resource_index)
        .context("Terrain embedded resource absent")?;
    let original = embedded["FileName"]["$value"].clone();
    ensure!(
        node["Data"]["navigationTileResource"]["DepotPath"]["$value"] == original,
        "Terrain resource belongs to another node"
    );
    let resource = &mut embedded["Content"];
    ensure!(
        resource["agentSize"] == "Human",
        "Terrain mask requires Human navigation"
    );
    let inverse = Quat::from_rotation_z(-c.project.yaw.to_radians());
    let origin = Vec3::from_array(c.project.origin);
    let mut changes = Vec::new();
    for (index, buffer) in resource["tileBuffers"]
        .as_array_mut()
        .context("Terrain buffers absent")?
        .iter_mut()
        .enumerate()
    {
        let mut bytes = STANDARD.decode(
            buffer["Bytes"]
                .as_str()
                .context("Terrain buffer bytes absent")?,
        )?;
        let original_bytes = bytes.clone();
        ensure!(
            bytes.get(..4) == Some(b"VAND") && u32_at(&bytes, 4)? == 8,
            "Unsupported terrain VAND version"
        );
        let faces = u32_at(&bytes, 24)?;
        let vertices = u32_at(&bytes, 28)?;
        let face_start = 100usize
            .checked_add(vertices.checked_mul(12).context("VAND count overflow")?)
            .context("VAND count overflow")?;
        ensure!(
            faces <= 1_000_000
                && face_start
                    .checked_add(faces * 20)
                    .is_some_and(|n| n <= bytes.len()),
            "Truncated VAND polygons"
        );
        let mut disabled = Vec::new();
        for face in 0..faces {
            let offset = face_start + face * 20;
            let count = usize::from(bytes[offset + 18]);
            if count != 3 || bytes[offset + 19] & 0xc0 != 0 || u16_at(&bytes, offset + 16)? == 0 {
                continue;
            }
            let mut triangle = Vec::new();
            for vertex in 0..3 {
                let v = u16_at(&bytes, offset + 4 + vertex * 2)?;
                ensure!(v < vertices, "Terrain polygon vertex out of bounds");
                let off = 100 + v * 12;
                let world = Vec3::new(
                    f32_at(&bytes, off)?,
                    f32_at(&bytes, off + 8)?,
                    f32_at(&bytes, off + 4)?,
                );
                let local = inverse * (world - origin);
                triangle.push(local);
            }
            let covered = polygons.iter().any(|(polygon, height_range)| {
                let mut covered = triangle.iter().all(|p| {
                    p.z >= height_range[0]
                        && p.z <= height_range[1]
                        && contains(polygon, p.truncate())
                });
                if !covered {
                    return false;
                }
                for edge in 0..3 {
                    for p in 0..polygon.len() {
                        covered &= !intersects(
                            triangle[edge].truncate(),
                            triangle[(edge + 1) % 3].truncate(),
                            polygon[p],
                            polygon[(p + 1) % polygon.len()],
                        );
                    }
                }
                covered
            });
            if covered {
                bytes[offset + 16] = 0;
                bytes[offset + 17] = 0;
                disabled.push(face);
            }
        }
        let allowed: std::collections::BTreeSet<_> = disabled
            .iter()
            .flat_map(|i| [face_start + i * 20 + 16, face_start + i * 20 + 17])
            .collect();
        ensure!(
            bytes.len() == original_bytes.len()
                && bytes
                    .iter()
                    .zip(&original_bytes)
                    .enumerate()
                    .all(|(i, (a, b))| a == b || allowed.contains(&i)),
            "Terrain mask changed bytes outside polygon flags"
        );
        if !disabled.is_empty() {
            changes.push(json!({"buffer":index,"polygons":disabled,"before_sha256":format!("{:x}",Sha256::digest(&original_bytes)),"after_sha256":format!("{:x}",Sha256::digest(&bytes))}));
        }
        buffer["Bytes"] = json!(STANDARD.encode(bytes));
    }
    ensure!(
        !changes.is_empty(),
        "Terrain mask selected no complete terrain polygons"
    );
    let depot = format!(
        "mod\\building_builder\\{}\\retained_{tile_index}.navmesh",
        c.project.id
    );
    let reference = json!({"DepotPath":{"$type":"ResourcePath","$storage":"string","$value":depot},"Flags":"Embedded"});
    embedded["FileName"] = reference["DepotPath"].clone();
    let embedded = embedded.clone();
    node["Data"]["navigationTileResource"] = reference.clone();
    placement["NodeIndex"] = json!(0);
    document["Data"]["RootChunk"]["nodes"] = json!([node]);
    document["Data"]["RootChunk"]["nodeData"]["Data"] = json!([placement]);
    document["Data"]["RootChunk"]["nodeRefs"] = json!([]);
    document["Data"]["RootChunk"]["localInplaceResource"] = json!([reference]);
    document["Data"]["EmbeddedFiles"] = json!([embedded]);
    Ok((
        document,
        json!({"source_sector":mask.sector,"source_sha256":mask.sha256,"placement_index":mask.placement_index,"resource_index":mask.resource_index,"masks":masks,"changes":changes,"original_resource":original}),
    ))
}
