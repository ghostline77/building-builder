//! Native world survey, source-pinned ArchiveXL patches and shell references.
use crate::{
    catalogue::Catalogue,
    model::{Anchor, Object, Project},
};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use ghostline_red::assets::ResourceKey;
use ghostline_renderer::GameSource;
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SectorPatch {
    pub path: String,
    pub sha256: String,
    #[serde(rename = "expectedNodes")]
    pub expected_nodes: usize,
    #[serde(rename = "nodeDeletions", default)]
    pub deletions: Vec<Value>,
}
fn path(value: &str) -> Result<ResourceKey> {
    ensure!(
        (value.starts_with(r"base\worlds\") || value.starts_with(r"ep1\worlds\"))
            && value.ends_with(".streamingsector")
            && !value.contains(".."),
        "Survey requires a vanilla world sector depot path"
    );
    Ok(ResourceKey::parse(value))
}
fn root(document: &Value) -> &Value {
    &document["Data"]["RootChunk"]
}
fn placements(document: &Value) -> Result<&Vec<Value>> {
    root(document)["nodeData"]["Data"]
        .as_array()
        .context("Sector has no decoded placements")
}
fn node(document: &Value, placement: &Value) -> Result<Value> {
    let i = placement["NodeIndex"]
        .as_u64()
        .context("Invalid node index")? as usize;
    let native = root(document)["nodes"]
        .get(i)
        .map(|n| &n["Data"])
        .context("Missing sector node")?;
    Ok(ghostline_red::world::placement_node(native, placement)?)
}
fn number(value: &Value) -> Result<f32> {
    value
        .as_f64()
        .map(|v| v as f32)
        .filter(|v| v.is_finite())
        .context("Invalid native coordinate")
}
fn vector(value: &Value) -> Result<Vec3> {
    Ok(Vec3::new(
        number(&value["X"])?,
        number(&value["Y"])?,
        number(&value["Z"])?,
    ))
}
fn quaternion(value: &Value) -> Result<Quat> {
    let q = Quat::from_xyzw(
        number(&value["i"])?,
        number(&value["j"])?,
        number(&value["k"])?,
        number(&value["r"])?,
    );
    ensure!(q.length_squared() > 0.001, "Invalid native orientation");
    Ok(q.normalize())
}
fn u16_at(data: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        data.get(offset..offset + 2)
            .context("Truncated native buffer")?
            .try_into()?,
    ))
}
fn f32_at(data: &[u8], offset: usize) -> Result<f32> {
    Ok(f32::from_le_bytes(
        data.get(offset..offset + 4)
            .context("Truncated transform")?
            .try_into()?,
    ))
}
fn fixed(data: &[u8], offset: usize) -> Result<f32> {
    Ok(i32::from_le_bytes(
        data.get(offset..offset + 4)
            .context("Truncated position")?
            .try_into()?,
    ) as f32
        / 131072.)
}
fn collect<'a>(value: &'a Value, handles: &mut BTreeMap<String, &'a Value>) {
    match value {
        Value::Object(fields) => {
            if let Some(id) = value["HandleId"].as_str() {
                handles.insert(id.into(), &value["Data"]);
            }
            for child in fields.values() {
                collect(child, handles);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect(child, handles);
            }
        }
        _ => {}
    }
}
fn instances(document: &Value, node: &Value) -> Result<(Vec<u8>, usize, usize)> {
    let mut handles = BTreeMap::new();
    collect(document, &mut handles);
    let transforms = &node["worldTransformsBuffer"];
    let shared = &transforms["sharedDataBuffer"];
    let shared = if let Some(id) = shared["HandleRefId"].as_str() {
        *handles
            .get(id)
            .context("Unresolved instance transform handle")?
    } else {
        &shared["Data"]
    };
    let data = STANDARD.decode(
        shared["buffer"]["Bytes"]
            .as_str()
            .context("Instance transform bytes absent")?,
    )?;
    let start = transforms["startIndex"].as_u64().unwrap_or(0) as usize;
    let count = transforms["numElements"]
        .as_u64()
        .context("Instance count absent")? as usize;
    ensure!(
        count <= 1_000_000
            && start
                .checked_add(count)
                .and_then(|v| v.checked_mul(48))
                .is_some_and(|n| n <= data.len()),
        "Truncated instance buffer"
    );
    Ok((data, start, count))
}

fn resource_text(key: &ResourceKey) -> String {
    match key {
        ResourceKey::Path(p) => p.clone(),
        ResourceKey::Hash(h) => h.to_string(),
    }
}

fn measured(
    source: &GameSource,
    key: &ResourceKey,
    entity: bool,
    appearance: &str,
) -> Result<crate::model::Bounds> {
    let mut low = Vec3::splat(f32::INFINITY);
    let mut high = Vec3::splat(f32::NEG_INFINITY);
    if entity {
        let native = source.resolve_assembly(
            key,
            (!appearance.is_empty() && appearance != "None").then_some(appearance),
        )?;
        let options = ghostline_renderer::AssemblyOptions::standalone(&native);
        let assembly =
            ghostline_renderer::SceneAssembly::from_native(native, &options, |mesh, _| {
                source
                    .load_geometry(mesh)?
                    .with_neutral_material()
                    .map(std::sync::Arc::new)
            })?;
        let transforms = assembly.native_component_placements()?;
        for component in assembly
            .native
            .components
            .iter()
            .filter(|c| c.enabled && c.chunk_mask != 0 && !c.is_collapsed())
        {
            if let Some(mesh) = &component.mesh {
                crate::catalogue::measure(
                    source,
                    mesh,
                    Mat4::from_cols_array(
                        transforms
                            .get(&component.name)
                            .context("Native component placement absent")?,
                    ),
                    component.chunk_mask,
                    &mut low,
                    &mut high,
                )?;
            }
        }
    } else {
        let document = source.resources().document(key)?;
        let mesh = root(&document);
        ensure!(
            !mesh["isShadowMesh"].as_bool().unwrap_or(false)
                && !mesh["isPlayerShadowMesh"].as_bool().unwrap_or(false),
            "Shadow-only geometry"
        );
        if let (Ok(min), Ok(max)) = (
            vector(&mesh["boundingBox"]["Min"]),
            vector(&mesh["boundingBox"]["Max"]),
        ) && max.cmpge(min).all()
            && (max - min).max_element() > 0.00001
        {
            // World nodes use complete rigid meshes. Their native culling box
            // avoids decoding every vertex merely to select a building.
            low = min;
            high = max;
        } else {
            crate::catalogue::measure(source, key, Mat4::IDENTITY, u64::MAX, &mut low, &mut high)?;
        }
    }
    ensure!(
        low.is_finite() && high.is_finite(),
        "No visible native geometry"
    );
    Ok(crate::model::Bounds {
        min: low.to_array(),
        max: high.to_array(),
        dimensions: (high - low).to_array(),
    })
}

pub(crate) fn make_member(
    source: &GameSource,
    sector: &str,
    document: &Value,
    index: usize,
    instance: Option<usize>,
    actor: Option<usize>,
    shape: Option<usize>,
) -> Result<crate::shell::Member> {
    let p = placements(document)?
        .get(index)
        .context("Source placement absent")?;
    let resolved = node(document, p)?;
    let n = &resolved;
    let kind = n["$type"]
        .as_str()
        .context("Native node type absent")?
        .to_string();
    if let Some(actor) = actor {
        let selected = ghostline_red::world::collision_shapes(n)?
            .into_iter()
            .find(|s| s.actor == actor && s.shape == shape.unwrap_or(0))
            .context("Collision shape absent")?;
        let mut member = collision_member(sector, index, n, &selected);
        collision_bounds(source, n, &selected, &mut member)?;
        return Ok(member);
    }
    let entity = kind == "worldEntityNode";
    let key = ResourceKey::from_value(if entity {
        &n["entityTemplate"]
    } else {
        &n["mesh"]
    })?;
    let mut position = vector(&p["Position"])?;
    let mut rotation = quaternion(&p["Orientation"]).unwrap_or(Quat::IDENTITY);
    let mut scale = vector(&p["Scale"]).unwrap_or(Vec3::ONE);
    let mut native = json!({"type":kind,"index":index});
    if kind == "worldNavigationNode" {
        let target = &n["navigationTileResource"]["DepotPath"]["$value"];
        if let Some(index) = document["Data"]["EmbeddedFiles"]
            .as_array()
            .and_then(|files| {
                files.iter().position(|f| {
                    f["FileName"]["$value"] == *target && f["Content"]["agentSize"] == "Human"
                })
            })
        {
            native["resource_index"] = json!(index);
        }
    }
    let mut bounds = None;
    if kind == "worldNavigationNode" {
        if let Some(resource) = native["resource_index"]
            .as_u64()
            .and_then(|i| document["Data"]["EmbeddedFiles"].get(i as usize))
        {
            let native_bounds = &resource["Content"]["localBoundingBox"];
            let low = vector(&native_bounds["Min"])?;
            let high = vector(&native_bounds["Max"])?;
            bounds = Some(crate::model::Bounds {
                min: low.to_array(),
                max: high.to_array(),
                dimensions: (high - low).to_array(),
            });
        }
        if bounds.is_none()
            && let (Ok(low), Ok(high)) = (vector(&p["Bounds"]["Min"]), vector(&p["Bounds"]["Max"]))
            && high.cmpgt(low).all()
        {
            position = (low + high) / 2.;
            rotation = Quat::IDENTITY;
            scale = Vec3::ONE;
            bounds = Some(crate::model::Bounds {
                min: (low - position).to_array(),
                max: (high - position).to_array(),
                dimensions: (high - low).to_array(),
            });
        }
    }
    if let Some(instance) = instance {
        let (bytes, start, count) = instances(document, n)?;
        ensure!(instance < count, "Instance absent");
        let off = (start + instance) * 48;
        position = Vec3::new(
            fixed(&bytes, off)?,
            fixed(&bytes, off + 4)?,
            fixed(&bytes, off + 8)?,
        );
        rotation = Quat::from_xyzw(
            f32_at(&bytes, off + 16)?,
            f32_at(&bytes, off + 20)?,
            f32_at(&bytes, off + 24)?,
            f32_at(&bytes, off + 28)?,
        )
        .normalize();
        scale = Vec3::new(
            f32_at(&bytes, off + 32)?,
            f32_at(&bytes, off + 36)?,
            f32_at(&bytes, off + 40)?,
        );
        native["expectedInstances"] = json!(count);
        native["instanceDeletions"] = json!([instance]);
    }
    if kind == "worldInstancedOccluderNode" {
        let raw = STANDARD.decode(n["$rawTail"].as_str().context("Occluder data absent")?)?;
        let count = usize::from(raw.first().copied().unwrap_or(0));
        ensure!(raw.len() == 1 + count * 64, "Occluder matrices truncated");
        native["expectedInstances"] = json!(count);
        native["removeWholeGroup"] = json!(true);
        let mut low = Vec3::splat(f32::INFINITY);
        let mut high = Vec3::splat(f32::NEG_INFINITY);
        for i in 0..count {
            let mut values = [0.; 16];
            for (j, value) in values.iter_mut().enumerate() {
                *value = f32_at(&raw, 1 + i * 64 + j * 4)?;
            }
            let matrix = Mat4::from_cols_array(&values);
            for x in [-0.5, 0.5] {
                for y in [-0.5, 0.5] {
                    for z in [-0.5, 0.5] {
                        let v = matrix.transform_point3(Vec3::new(x, y, z));
                        low = low.min(v);
                        high = high.max(v);
                    }
                }
            }
        }
        if count > 0 {
            position = (low + high) / 2.;
            rotation = Quat::IDENTITY;
            scale = Vec3::ONE;
            bounds = Some(crate::model::Bounds {
                min: (low - position).to_array(),
                max: (high - position).to_array(),
                dimensions: (high - low).to_array(),
            });
        }
    }
    let mut appearance = n[if entity {
        "appearanceName"
    } else {
        "meshAppearance"
    }]["$value"]
        .as_str()
        .unwrap_or(if entity { "" } else { "default" })
        .to_owned();
    if n["meshAppearance"]["$storage"] == "uint64"
        && let Some(key) = &key
    {
        let hash = n["meshAppearance"]["$value"]
            .as_str()
            .and_then(|v| v.parse::<u64>().ok())
            .context("Invalid native appearance hash")?;
        appearance = if hash == 0 {
            "default".into()
        } else {
            source
                .resources()
                .mesh_appearance_names(key)?
                .into_iter()
                .find(|name| {
                    name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
                        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
                    }) == hash
                })
                // Preserve an unmatched request for the native first-appearance
                // rule already applied by compile_effects and the renderer.
                .unwrap_or_else(|| hash.to_string())
        };
    }
    if !entity
        && n.get("meshAppearance").is_none()
        && let Some(key) = &key
        && let Ok(geometry) = source.load_geometry(key)
        && let Some(name) = geometry.appearance_names().first()
    {
        appearance = (*name).to_owned();
    }
    let label = key.as_ref().map(resource_text).unwrap_or_else(|| {
        n["debugName"]["$value"]
            .as_str()
            .unwrap_or(&kind)
            .to_owned()
    });
    let lower = label.to_lowercase();
    let layer = if kind.contains("Collision") {
        "collision"
    } else if kind.contains("Occluder") {
        "occlusion"
    } else if kind.contains("Navigation") {
        "navigation"
    } else if entity {
        "devices"
    } else if lower.contains("architecture")
        || ["wall", "floor", "roof", "pillar", "stair", "ceiling"]
            .iter()
            .any(|w| lower.contains(w))
    {
        "structure"
    } else {
        "furnishings"
    };
    let mut diagnostics = Vec::new();
    if bounds.is_none()
        && let Some(resource) = &key
    {
        match measured(source, resource, entity, &appearance) {
            Ok(b) => bounds = Some(b),
            Err(error) => diagnostics.push(error.to_string()),
        }
    }
    Ok(crate::shell::Member {
        id: format!(
            "s{:x}_m{index}_i{}",
            ghostline_red::archive::depot_path_hash(sector),
            instance.map_or("n".into(), |v| v.to_string())
        ),
        sector: sector.into(),
        index,
        instance,
        actor,
        shape,
        kind,
        layer: layer.into(),
        label: label.rsplit('\\').next().unwrap_or(&label).into(),
        resource: key,
        appearance,
        position: position.to_array(),
        rotation: rotation.to_array(),
        scale: scale.to_array(),
        bounds,
        object: None,
        removed: false,
        diagnostics,
        native,
    })
}

fn collision_bounds(
    source: &GameSource,
    node: &Value,
    shape: &ghostline_red::world::CollisionShape,
    member: &mut crate::shell::Member,
) -> Result<()> {
    if let Some(hash) = shape.geometry {
        let sector = node["sectorHash"]
            .as_u64()
            .or_else(|| node["sectorHash"].as_str().and_then(|s| s.parse().ok()))
            .context("Collision sector hash absent")?;
        let geometry = source.resources().collision_geometry(sector, hash)?;
        let mut low = Vec3::splat(f32::INFINITY);
        let mut high = Vec3::splat(f32::NEG_INFINITY);
        for p in &geometry.vertices {
            let p = Vec3::from_array(*p);
            low = low.min(p);
            high = high.max(p);
        }
        if low.is_finite() && high.is_finite() {
            member.bounds = Some(crate::model::Bounds {
                min: low.to_array(),
                max: high.to_array(),
                dimensions: (high - low).to_array(),
            });
        }
    }
    Ok(())
}

fn collision_member(
    sector: &str,
    index: usize,
    node: &Value,
    shape: &ghostline_red::world::CollisionShape,
) -> crate::shell::Member {
    let bounds = shape.half_extents.map(|half| {
        let half = Vec3::from_array(half);
        crate::model::Bounds {
            min: (-half).to_array(),
            max: half.to_array(),
            dimensions: (half * 2.).to_array(),
        }
    });
    crate::shell::Member {
        id: format!(
            "s{:x}_m{index}_a{}_s{}",
            ghostline_red::archive::depot_path_hash(sector),
            shape.actor,
            shape.shape
        ),
        sector: sector.into(),
        index,
        instance: None,
        actor: Some(shape.actor),
        shape: Some(shape.shape),
        kind: node["$type"]
            .as_str()
            .unwrap_or("worldCollisionNode")
            .into(),
        layer: "collision".into(),
        label: format!("Collision actor {} / shape {}", shape.actor, shape.shape),
        resource: None,
        appearance: String::new(),
        position: shape.position,
        rotation: shape.rotation,
        scale: shape.scale,
        bounds,
        object: None,
        removed: false,
        diagnostics: Vec::new(),
        native: json!({"index":index,"type":node["$type"],"expectedActors":node["numActors"],"actorDeletions":[[shape.actor,shape.shape]],"shape_code":shape.kind,"geometry_hash":shape.geometry.map(|v|v.to_string())}),
    }
}

/// Inventories complete shapes by their measured world bounds, including pieces with distant pivots.
pub fn survey_members(
    source: &GameSource,
    sector: &str,
    center: [f32; 3],
    radius: f32,
    visual_detail: bool,
) -> Result<(SectorPatch, Vec<crate::shell::Member>)> {
    let key = path(sector)?;
    let bytes = source.resources().read_bytes(&key)?;
    let document = source.resources().document(&key)?;
    let pin = SectorPatch {
        path: sector.into(),
        sha256: format!("{:x}", Sha256::digest(bytes)),
        expected_nodes: placements(&document)?.len(),
        deletions: Vec::new(),
    };
    let mut output = Vec::new();
    for (index, p) in placements(&document)?.iter().enumerate() {
        let resolved = node(&document, p)?;
        let n = &resolved;
        let kind = n["$type"].as_str().unwrap_or("");
        if ![
            "worldMeshNode",
            "worldStaticMeshNode",
            "worldCookedStaticMeshNode",
            "worldCookedGenericProxyMeshNode",
            "worldCookedCollisionNode",
            "worldStaticOccluderMeshNode",
            "worldEntityNode",
            "worldInstancedMeshNode",
            "worldCollisionNode",
            "worldInstancedOccluderNode",
            "worldNavigationNode",
        ]
        .contains(&kind)
        {
            continue;
        }
        if !visual_detail
            && [
                "worldMeshNode",
                "worldStaticMeshNode",
                "worldInstancedMeshNode",
            ]
            .contains(&kind)
        {
            continue;
        }
        let bounds = &p["Bounds"];
        if let (Ok(low), Ok(high)) = (vector(&bounds["Min"]), vector(&bounds["Max"]))
            && high.cmpgt(low).all()
            && (Vec3::from_array(center).clamp(low, high) - Vec3::from_array(center)).length()
                > radius
        {
            continue;
        }
        if ["worldCollisionNode", "worldCookedCollisionNode"].contains(&kind) {
            for shape in ghostline_red::world::collision_shapes(n)? {
                let mut m = collision_member(sector, index, n, &shape);
                collision_bounds(source, n, &shape, &mut m)?;
                if member_near(&m, center, radius) {
                    output.push(m);
                }
            }
            continue;
        }
        if kind == "worldInstancedMeshNode" {
            let (bytes, start, count) = instances(&document, n)?;
            if count == 0 {
                continue;
            }
            let template = make_member(source, sector, &document, index, Some(0), None, None)?;
            for instance in 0..count {
                let mut m = template.clone();
                let off = (start + instance) * 48;
                m.id = format!(
                    "s{:x}_m{index}_i{instance}",
                    ghostline_red::archive::depot_path_hash(sector)
                );
                m.instance = Some(instance);
                m.native["instanceDeletions"] = json!([instance]);
                m.position = [
                    fixed(&bytes, off)?,
                    fixed(&bytes, off + 4)?,
                    fixed(&bytes, off + 8)?,
                ];
                m.rotation = Quat::from_xyzw(
                    f32_at(&bytes, off + 16)?,
                    f32_at(&bytes, off + 20)?,
                    f32_at(&bytes, off + 24)?,
                    f32_at(&bytes, off + 28)?,
                )
                .normalize()
                .to_array();
                m.scale = [
                    f32_at(&bytes, off + 32)?,
                    f32_at(&bytes, off + 36)?,
                    f32_at(&bytes, off + 40)?,
                ];
                if member_near(&m, center, radius) {
                    output.push(m);
                }
            }
        } else {
            let m = make_member(source, sector, &document, index, None, None, None)?;
            if member_near(&m, center, radius) {
                output.push(m);
            }
        }
    }
    Ok((pin, output))
}
fn member_near(m: &crate::shell::Member, center: [f32; 3], radius: f32) -> bool {
    if let Some(bounds) = &m.bounds {
        let matrix = Mat4::from_scale_rotation_translation(
            Vec3::from_array(m.scale),
            Quat::from_array(m.rotation),
            Vec3::from_array(m.position),
        );
        let mut low = Vec3::splat(f32::INFINITY);
        let mut high = Vec3::splat(f32::NEG_INFINITY);
        for x in [bounds.min[0], bounds.max[0]] {
            for y in [bounds.min[1], bounds.max[1]] {
                for z in [bounds.min[2], bounds.max[2]] {
                    let p = matrix.transform_point3(Vec3::new(x, y, z));
                    low = low.min(p);
                    high = high.max(p);
                }
            }
        }
        (Vec3::from_array(center).clamp(low, high) - Vec3::from_array(center)).length() <= radius
    } else {
        (Vec3::from_array(m.position) - Vec3::from_array(center)).length() <= radius
    }
}

pub fn survey(source: &GameSource, sector: &str, center: [f32; 3], radius: f32) -> Result<Value> {
    ensure!(
        center.iter().all(|v| v.is_finite())
            && radius.is_finite()
            && (0.1..=1000.).contains(&radius),
        "Invalid survey radius/position"
    );
    let key = path(sector)?;
    let bytes = source.resources().read_bytes(&key)?;
    let document = source.resources().document(&key)?;
    let mut candidates = Vec::new();
    let inside = |p: Vec3| (p - Vec3::from_array(center)).length() <= radius;
    for (index, placement) in placements(&document)?.iter().enumerate() {
        let resolved = node(&document, placement)?;
        let n = &resolved;
        let kind = n["$type"].as_str().unwrap_or("");
        if kind == "worldInstancedMeshNode" {
            let (data, start, count) = instances(&document, n)?;
            for instance in 0..count {
                let off = (start + instance) * 48;
                let p = Vec3::new(
                    fixed(&data, off)?,
                    fixed(&data, off + 4)?,
                    fixed(&data, off + 8)?,
                );
                if inside(p) {
                    candidates.push(json!({"index":index,"type":kind,"instance":instance,"expectedInstances":count,"position":p.to_array(),"mesh":n["mesh"]}));
                }
            }
        } else if ["worldCollisionNode", "worldCookedCollisionNode"].contains(&kind) {
            let data = STANDARD.decode(
                n["compiledData"]["Bytes"]
                    .as_str()
                    .context("Collision bytes absent")?,
            )?;
            let count = n["numActors"].as_u64().context("Collision count absent")? as usize;
            ensure!(
                count <= 1_000_000 && data.len() >= count * 48,
                "Truncated collision actors"
            );
            for actor in 0..count {
                let off = actor * 48;
                let p = Vec3::new(
                    fixed(&data, off)?,
                    fixed(&data, off + 4)?,
                    fixed(&data, off + 8)?,
                );
                if inside(p) {
                    candidates.push(json!({"index":index,"type":kind,"actor":actor,"expectedActors":count,"shape_count":u16_at(&data,off+34)?,"position":p.to_array()}));
                }
            }
        } else if inside(vector(&placement["Position"])?) {
            let resource_index = document["Data"]["EmbeddedFiles"]
                .as_array()
                .and_then(|files| {
                    files.iter().position(|file| {
                        file["FileName"]["$value"]
                            == n["navigationTileResource"]["DepotPath"]["$value"]
                            && file["Content"]["agentSize"] == "Human"
                    })
                });
            let mut candidate = json!({"index":index,"type":kind,"position":placement["Position"],"mesh":n["mesh"],"entityTemplate":n["entityTemplate"],"debugName":n["debugName"],"node":n});
            if kind == "worldInstancedOccluderNode" {
                let raw =
                    STANDARD.decode(n["$rawTail"].as_str().context("Occluder data absent")?)?;
                if let Some(count) = raw.first() {
                    candidate["expectedInstances"] = json!(count);
                }
            }
            if let Some(resource_index) = resource_index {
                candidate["resource_index"] = json!(resource_index);
            }
            candidates.push(candidate);
        }
    }
    Ok(
        json!({"selection":SectorPatch{path:sector.into(),sha256:format!("{:x}",Sha256::digest(bytes)),expected_nodes:placements(&document)?.len(),deletions:Vec::new()},"candidates":candidates,"indexing":"Indices address nodeData placements, not node definitions"}),
    )
}

/// Finds native sector descriptors intersecting a world-space survey volume.
pub fn nearby_sectors(source: &GameSource, position: [f32; 3], radius: f32) -> Result<Value> {
    ensure!(
        position.iter().all(|v| v.is_finite())
            && radius.is_finite()
            && (0.1..=1000.).contains(&radius),
        "Invalid survey position/radius"
    );
    let mut sectors = BTreeMap::new();
    for path in [
        r"base\worlds\03_night_city\_compiled\default\blocks\all.streamingblock",
        r"ep1\worlds\03_night_city\_compiled\default\blocks\all.streamingblock",
    ] {
        let key = ResourceKey::parse(path);
        if !source.resources().resource_exists(&key) {
            continue;
        }
        let document = source.resources().document(&key)?;
        for descriptor in root(&document)["descriptors"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let Some(path) = descriptor["data"]["DepotPath"]["$value"].as_str() else {
                continue;
            };
            let bounds = &descriptor["streamingBox"];
            let intersects = ["X", "Y", "Z"].iter().enumerate().all(|(i, axis)| {
                bounds["Min"][axis]
                    .as_f64()
                    .is_some_and(|v| v <= f64::from(position[i] + radius))
                    && bounds["Max"][axis]
                        .as_f64()
                        .is_some_and(|v| v >= f64::from(position[i] - radius))
            });
            if intersects && path.ends_with(".streamingsector") {
                sectors.insert(path.to_string(), json!({"path":path,"bounds":bounds,"level":descriptor["level"],"category":descriptor["category"]}));
            }
        }
    }
    ensure!(
        !sectors.is_empty(),
        "No native sectors found here. Check the world anchor, increase the radius, or enter a sector depot path."
    );
    Ok(json!({"sectors":sectors.into_values().collect::<Vec<_>>()}))
}

pub fn validate(
    source: &GameSource,
    patches: &[SectorPatch],
    terrain: &[&crate::terrain::Mask],
) -> Result<Vec<Value>> {
    ensure!(patches.len() <= 256, "At most 256 sector patches");
    let mut paths = BTreeSet::new();
    let mut output = Vec::new();
    for patch in patches {
        ensure!(paths.insert(&patch.path), "Duplicate sector patch");
        let document = pinned(source, patch)?;
        let placements = placements(&document)?;
        let mut indices = BTreeSet::new();
        ensure!(
            patch.deletions.len() <= 128,
            "At most 128 deletions per sector"
        );
        for deletion in &patch.deletions {
            let index = deletion["index"]
                .as_u64()
                .context("Deletion needs placement index")? as usize;
            ensure!(indices.insert(index), "Duplicate placement deletion");
            let resolved = node(
                &document,
                placements
                    .get(index)
                    .context("Deletion index out of bounds")?,
            )?;
            let n = &resolved;
            let kind = n["$type"].as_str().context("Node type absent")?;
            ensure!(deletion["type"] == kind, "Surveyed node type changed");
            match kind {
                "worldCollisionNode" | "worldCookedCollisionNode" => {
                    let count = n["numActors"].as_u64().context("Actor count absent")?;
                    ensure!(deletion["expectedActors"] == count, "Actor count changed");
                    let actors = deletion["actorDeletions"]
                        .as_array()
                        .filter(|a| !a.is_empty())
                        .context("Choose collision actors or actor/shape pairs")?;
                    let data = STANDARD.decode(
                        n["compiledData"]["Bytes"]
                            .as_str()
                            .context("Collision data absent")?,
                    )?;
                    let mut selected = BTreeSet::new();
                    for actor in actors {
                        ensure!(
                            selected.insert(actor.to_string()),
                            "Duplicate collision selection"
                        );
                        let i = actor
                            .as_u64()
                            .or_else(|| actor[0].as_u64())
                            .context("Invalid actor index")?;
                        ensure!(i < count, "Collision actor out of bounds");
                        if let Some(pair) = actor.as_array() {
                            ensure!(
                                pair.len() == 2
                                    && pair[1].as_u64().is_some_and(|shape| shape
                                        < u64::from(
                                            u16_at(&data, i as usize * 48 + 34).unwrap_or(0)
                                        )),
                                "Collision shape out of bounds"
                            );
                        }
                    }
                }
                "worldInstancedMeshNode" | "worldInstancedDestructibleMeshNode" => {
                    let field = if kind == "worldInstancedMeshNode" {
                        "worldTransformsBuffer"
                    } else {
                        "cookedInstanceTransforms"
                    };
                    let count = n[field]["numElements"]
                        .as_u64()
                        .context("Instance count absent")?;
                    ensure!(
                        deletion["expectedInstances"] == count,
                        "Instance count changed"
                    );
                    let selected = deletion["instanceDeletions"]
                        .as_array()
                        .filter(|a| !a.is_empty())
                        .context("Select individual render instances")?;
                    let mut used = BTreeSet::new();
                    for value in selected {
                        let i = value.as_u64().context("Invalid render instance index")?;
                        ensure!(
                            i < count && used.insert(i),
                            "Invalid/duplicate render instance"
                        );
                    }
                }
                "worldInstancedOccluderNode" => {
                    let raw = STANDARD.decode(
                        n["$rawTail"]
                            .as_str()
                            .context("Occluder matrix appendix absent")?,
                    )?;
                    let count = *raw.first().context("Empty occluder group")?;
                    ensure!(
                        count < 128
                            && raw.len() == 1 + 64 * usize::from(count)
                            && deletion["expectedInstances"] == count
                            && deletion["removeWholeGroup"] == true,
                        "Explicit whole occluder-group acknowledgement required"
                    );
                }
                "worldNavigationNode" => ensure!(
                    terrain
                        .iter()
                        .any(|m| m.sector == patch.path && m.placement_index == index),
                    "Navigation removal requires matching retained terrain replacement"
                ),
                "worldMeshNode"
                | "worldStaticMeshNode"
                | "worldCookedStaticMeshNode"
                | "worldCookedGenericProxyMeshNode"
                | "worldStaticOccluderMeshNode"
                | "worldEntityProxyMeshNode"
                | "worldGenericProxyMeshNode"
                | "worldEntityNode" => {}
                _ => anyhow::bail!("Unsupported world patch node {kind}"),
            }
        }
        if !patch.deletions.is_empty() {
            output.push(json!({"path":patch.path,"expectedNodes":patch.expected_nodes,"nodeDeletions":patch.deletions}));
        }
    }
    Ok(output)
}

pub fn pinned(source: &GameSource, patch: &SectorPatch) -> Result<Value> {
    let key = path(&patch.path)?;
    let bytes = source.resources().read_bytes(&key)?;
    ensure!(
        format!("{:x}", Sha256::digest(&bytes)) == patch.sha256,
        "Surveyed sector source changed; survey again"
    );
    let document = (*source.resources().document(&key)?).clone();
    ensure!(
        placements(&document)?.len() == patch.expected_nodes,
        "Surveyed placement count changed"
    );
    Ok(document)
}

fn same_source_member(saved: &crate::shell::Member, current: &crate::shell::Member) -> bool {
    let close = |a: &[f32], b: &[f32]| {
        a.len() == b.len() && a.iter().zip(b).all(|(a, b)| (*a - *b).abs() < 0.001)
    };
    let bounds_match = match (&saved.bounds, &current.bounds) {
        (Some(a), Some(b)) => close(&a.min, &b.min) && close(&a.max, &b.max),
        (None, None) => true,
        _ => false,
    };
    saved.kind == current.kind
        && saved.resource.as_ref().map(ResourceKey::native_hash)
            == current.resource.as_ref().map(ResourceKey::native_hash)
        && saved.appearance == current.appearance
        && saved.native == current.native
        && close(&saved.position, &current.position)
        && close(&saved.scale, &current.scale)
        && Quat::from_array(saved.rotation)
            .dot(Quat::from_array(current.rotation))
            .abs()
            > 0.999999
        && bounds_match
}

/// Refreshes changed sector pins only when the saved source selections still
/// identify the same native geometry. Authored objects and operations are retained.
pub fn refresh_sources(project: &mut Project, source: &GameSource) -> Result<()> {
    let mut refreshed = Vec::new();
    for (index, patch) in project.retrofit.iter().enumerate() {
        let key = path(&patch.path)?;
        let bytes = source.resources().read_bytes(&key)?;
        let hash = format!("{:x}", Sha256::digest(&bytes));
        if hash == patch.sha256 {
            continue;
        }
        let document = source.resources().document(&key)?;
        ensure!(
            placements(&document)?.len() == patch.expected_nodes,
            "Surveyed placement count changed in {}; survey again",
            patch.path
        );
        let members = project
            .shells
            .iter()
            .flat_map(|s| &s.members)
            .filter(|m| m.sector == patch.path)
            .collect::<Vec<_>>();
        ensure!(
            !members.is_empty(),
            "Changed sector has no saved shell selections to revalidate: {}; survey again",
            patch.path
        );
        for member in &members {
            let current = make_member(
                source,
                &patch.path,
                &document,
                member.index,
                member.instance,
                member.actor,
                member.shape,
            )?;
            ensure!(
                same_source_member(member, &current),
                "Surveyed piece {} changed in {}; survey again",
                member.id,
                patch.path
            );
        }
        for deletion in &patch.deletions {
            let placement = deletion["index"]
                .as_u64()
                .context("Patch placement index absent")?;
            let selected = members
                .iter()
                .filter(|m| m.index as u64 == placement)
                .collect::<Vec<_>>();
            ensure!(
                !selected.is_empty(),
                "Changed patch target has no saved shell selection; survey again"
            );
            if let Some(instances) = deletion["instanceDeletions"].as_array() {
                ensure!(
                    instances.iter().all(|i| selected
                        .iter()
                        .any(|m| m.instance.map(|i| i as u64) == i.as_u64())),
                    "Changed patch instance has no saved shell selection; survey again"
                );
            }
            if let Some(actors) = deletion["actorDeletions"].as_array() {
                ensure!(
                    actors
                        .iter()
                        .all(
                            |a| selected.iter().any(|m| a.as_array().is_some_and(|pair| pair
                                .len()
                                == 2
                                && m.actor.map(|i| i as u64) == pair[0].as_u64()
                                && m.shape.map(|i| i as u64) == pair[1].as_u64()))
                        ),
                    "Changed collision target has no saved shape selection; survey again"
                );
            }
        }
        ensure!(
            !crate::terrain::masks(project)
                .iter()
                .any(|m| m.sector == patch.path),
            "Surveyed navigation sector changed; survey its navigation again"
        );
        refreshed.push((index, hash));
    }
    for (index, hash) in refreshed {
        project.retrofit[index].sha256 = hash;
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct ImportSelection {
    pub index: usize,
    pub instance: Option<usize>,
    pub id: String,
    pub level: u32,
}
pub fn import(
    project: &mut Project,
    source: &GameSource,
    catalogue: &Catalogue,
    patch: &SectorPatch,
    selection: &[ImportSelection],
) -> Result<()> {
    ensure!(selection.len() <= 1000, "Too many shell references");
    let document = pinned(source, patch)?;
    let inverse = Quat::from_rotation_z(-project.yaw.to_radians());
    for item in selection {
        crate::model::identifier(&item.id)?;
        let p = placements(&document)?
            .get(item.index)
            .context("Reference placement absent")?;
        let resolved = node(&document, p)?;
        let n = &resolved;
        let key: ResourceKey = serde_json::from_value(
            n["mesh"]["DepotPath"]
                .get("$value")
                .and_then(Value::as_str)
                .map_or_else(
                    || json!({"Hash":n["mesh"]["DepotPath"]["$value"]}),
                    |p| json!({"Path":p}),
                ),
        )?;
        let (position, rotation, scale) = if let Some(instance) = item.instance {
            let (data, start, count) = instances(&document, n)?;
            ensure!(instance < count, "Reference instance out of bounds");
            let off = (start + instance) * 48;
            (
                Vec3::new(
                    fixed(&data, off)?,
                    fixed(&data, off + 4)?,
                    fixed(&data, off + 8)?,
                ),
                Quat::from_xyzw(
                    f32_at(&data, off + 16)?,
                    f32_at(&data, off + 20)?,
                    f32_at(&data, off + 24)?,
                    f32_at(&data, off + 28)?,
                )
                .normalize(),
                Vec3::new(
                    f32_at(&data, off + 32)?,
                    f32_at(&data, off + 36)?,
                    f32_at(&data, off + 40)?,
                ),
            )
        } else {
            ensure!(
                ["worldMeshNode", "worldStaticMeshNode"]
                    .contains(&n["$type"].as_str().unwrap_or("")),
                "Select an explicit static mesh or instance"
            );
            (
                vector(&p["Position"])?,
                quaternion(&p["Orientation"])?,
                vector(&p["Scale"])?,
            )
        };
        let (asset, appearances) = catalogue.mesh_selection(&key)?;
        let appearance = n["meshAppearance"]["$value"].as_str().unwrap_or("default");
        ensure!(
            appearances.iter().any(|a| a == appearance),
            "Surveyed mesh appearance {appearance} is absent in catalogue"
        );
        project.objects.push(Object {
            id: item.id.clone(),
            asset,
            appearance: appearance.into(),
            position: (inverse * (position - Vec3::from_array(project.origin))).to_array(),
            yaw: 0.,
            anchor: Anchor::Pivot,
            scale: scale.to_array(),
            rotation: Some((inverse * rotation).to_array()),
            reference_only: true,
            collision: false,
            source_content_sha256: None,
            level: Some(item.level),
        });
    }
    Ok(())
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;

    #[test]
    fn source_revalidation_preserves_edits_but_rejects_changed_targets() {
        let saved: crate::shell::Member = serde_json::from_value(json!({
            "id":"piece","sector":"base\\worlds\\test.streamingsector","index":1,
            "instance":null,"actor":null,"shape":null,"kind":"worldStaticMeshNode",
            "layer":"structure","label":"Wall","resource":{"Hash":"42"},
            "appearance":"default","position":[1.,2.,3.],"rotation":[0.,0.,0.,1.],
            "scale":[1.,1.,1.],"bounds":null,"object":"authored_object","removed":true,
            "native":{"type":"worldStaticMeshNode","index":1}
        }))
        .unwrap();
        let mut current = saved.clone();
        current.object = None;
        current.removed = false;
        current.rotation = [0., 0., 0., -1.];
        assert!(same_source_member(&saved, &current));
        current.position[0] += 1.;
        assert!(!same_source_member(&saved, &current));
        current.position = saved.position;
        current.resource = Some(ResourceKey::Hash(43));
        assert!(!same_source_member(&saved, &current));
        current.resource = saved.resource.clone();
        current.native["expectedActors"] = json!(2);
        assert!(!same_source_member(&saved, &current));
    }
}
