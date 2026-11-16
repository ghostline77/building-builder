use std::{fs, path::Path};

use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use ghostline_red::{assets::ResourceKey, schema::RedSchema};
use glam::{Quat, Vec3};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::model::{Compiled, Placement, Solid};

fn typed(kind: &str, value: &str) -> Value {
    json!({"$type":kind,"$storage":"string","$value":value})
}
fn resource(path: &str) -> Value {
    json!({"DepotPath":typed("ResourcePath",path),"Flags":"Default"})
}
fn node_ref(value: u64) -> Value {
    json!({"$type":"NodeRef","$storage":"uint64","$value":value.to_string()})
}
fn vector(p: [f32; 3]) -> Value {
    json!({"$type":"Vector4","X":p[0],"Y":p[1],"Z":p[2],"W":0.})
}
fn quaternion(yaw: f32) -> Value {
    let q = Quat::from_rotation_z(yaw.to_radians());
    json!({"$type":"Quaternion","i":q.x,"j":q.y,"k":q.z,"r":q.w})
}

fn world(compiled: &Compiled, p: [f32; 3]) -> [f32; 3] {
    (Quat::from_rotation_z(compiled.project.yaw.to_radians()) * Vec3::from_array(p)
        + Vec3::from_array(compiled.project.origin))
    .to_array()
}

fn placement(
    index: usize,
    position: [f32; 3],
    yaw: f32,
    scale: [f32; 3],
    low: [f32; 3],
    high: [f32; 3],
) -> Value {
    json!({"Id":"0","NodeIndex":index,"Position":vector(position),"Orientation":quaternion(yaw),"Scale":{"$type":"Vector3","X":scale[0],"Y":scale[1],"Z":scale[2]},"Pivot":{"$type":"Vector3","X":position[0],"Y":position[1],"Z":position[2]},"Bounds":{"$type":"Box","Min":vector(low),"Max":vector(high)},"QuestPrefabRefHash":node_ref(0),"UkHash1":node_ref(0),"CookedPrefabData":{"DepotPath":{"$type":"ResourcePath","$storage":"uint64","$value":"0"},"Flags":"Default"},"MaxStreamingDistance":500.,"UkFloat1":400.,"Uk10":1024,"Uk11":512,"Uk12":0,"Uk13":"0","Uk14":"0"})
}

fn mesh_bounds(compiled: &Compiled, p: &Placement) -> ([f32; 3], [f32; 3]) {
    let rotation = Quat::from_rotation_z(compiled.project.yaw.to_radians())
        * p.rotation.map_or_else(
            || Quat::from_rotation_z(p.yaw.to_radians()),
            Quat::from_array,
        );
    let center = Vec3::from_array(world(compiled, p.position));
    let mut low = Vec3::splat(f32::INFINITY);
    let mut high = Vec3::splat(f32::NEG_INFINITY);
    for x in [p.bounds.min[0], p.bounds.max[0]] {
        for y in [p.bounds.min[1], p.bounds.max[1]] {
            for z in [p.bounds.min[2], p.bounds.max[2]] {
                let v = center + rotation * (Vec3::new(x, y, z) * Vec3::from_array(p.scale));
                low = low.min(v);
                high = high.max(v);
            }
        }
    }
    (low.to_array(), high.to_array())
}

fn collision(compiled: &Compiled, solid: &Solid, buffer: usize) -> Result<Value> {
    let mut bytes = Vec::new();
    for p in world(compiled, solid.center) {
        let value = (f64::from(p) * 131072.).round();
        ensure!(
            (i32::MIN as f64..=i32::MAX as f64).contains(&value),
            "Collision is outside native fixed-point range"
        );
        bytes.extend_from_slice(&(value as i32).to_le_bytes());
    }
    bytes.extend_from_slice(&0_i32.to_le_bytes());
    for q in Quat::from_rotation_z((solid.yaw + compiled.project.yaw).to_radians()).to_array() {
        bytes.extend_from_slice(&q.to_le_bytes());
    }
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&(-1_i16).to_le_bytes());
    bytes.extend_from_slice(&0_i16.to_le_bytes());
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    for size in solid.size {
        bytes.extend_from_slice(&(size / 2.).to_le_bytes());
    }
    for v in [0_i16, 0, 0, 1, -1, -1] {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    bytes.extend_from_slice(&[0, 11]);
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    for name in ["concrete.physmat", "World Static"] {
        // Physics names are case-sensitive FNV-1a values, unlike depot path hashes.
        let hash = name.bytes().fold(0xcbf29ce484222325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
        });
        bytes.extend_from_slice(&hash.to_le_bytes());
    }
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.push(0);
    let extent = solid.size.into_iter().fold(0_f32, f32::max);
    Ok(
        json!({"$type":"worldCollisionNode","debugName":typed("CName",&solid.id),"compiledData":{"BufferId":buffer.to_string(),"Flags":4063232,"Bytes":STANDARD.encode(bytes)},"numActors":1,"numShapeInfos":1,"numShapePositions":0,"numShapeRotations":0,"numScales":0,"numMaterials":1,"numPresets":1,"numMaterialIndices":1,"numShapeIndices":1,"sectorHash":"0","resourceVersion":2,"lod":0,"extents":vector([extent;3]),"staticCollisionShapeCategories":{"$type":"worldStaticCollisionShapeCategories_CollisionNode","arr":{"Elements":[{"Elements":[0,0,0,0,0,0]},{"Elements":[0,1,0,0,0,0]},{"Elements":[0,0,0,0,0,0]},{"Elements":[0,0,0,0,0,0]},{"Elements":[0,1,0,0,0,0]}]}}}),
    )
}

fn document(root: Value) -> Value {
    json!({"Header":{"DataType":"CR2W","GameVersion":2310},"Data":{"Version":195,"BuildVersion":0,"RootChunk":root,"EmbeddedFiles":[]}})
}

pub fn export(
    compiled: &Compiled,
    source: &ghostline_renderer::GameSource,
    output: &Path,
) -> Result<Value> {
    ensure!(
        compiled.placements.iter().any(|p| !p.reference_only)
            || compiled
                .project
                .retrofit
                .iter()
                .any(|p| !p.deletions.is_empty()),
        "There is no geometry to export"
    );
    ensure!(
        !output.exists(),
        "Export destination already exists; use a new directory"
    );
    source.validate_output(output)?;
    let retrofit = crate::retrofit::validate(
        source,
        &compiled.project.retrofit,
        &crate::terrain::masks(&compiled.project),
    )?;
    let staging = tempfile::tempdir()?;
    let root = staging.path().join("archive");
    let namespace = format!("mod\\building_builder\\{}", compiled.project.id);
    let depot = root.join("mod/building_builder").join(&compiled.project.id);
    fs::create_dir_all(&depot)?;
    let mut extra_resources = Vec::new();
    for (path, edit) in &compiled.edited_meshes {
        let name = path
            .rsplit('\\')
            .next()
            .context("Edited mesh filename absent")?;
        write_and_check(
            staging.path(),
            &depot.join(name),
            &edit.document,
            source.schema(),
        )?;
        extra_resources.push(name.to_owned());
    }
    for (id, device) in &compiled.project.door_devices {
        if device.preset != "garage_w300_h250" || device.scale == [1.; 3] {
            continue;
        }
        let (template, appearance, _, _) = crate::technical::door_preset(&device.preset)?;
        let assembly = source.resolve_assembly(&ResourceKey::parse(template), Some(appearance))?;
        let rig = assembly
            .components
            .iter()
            .find(|c| c.name == "anim")
            .and_then(|c| c.rig.as_ref())
            .context("Garage native anim rig absent")?;
        let mut document = (*source.resources().document(rig)?).clone();
        ghostline_red::assets::Rig::scale_root_document(&mut document, device.scale)?;
        let name = format!("door_{id}.rig");
        write_and_check(
            staging.path(),
            &depot.join(&name),
            &document,
            source.schema(),
        )?;
        extra_resources.push(name);
    }
    let mut nodes = Vec::new();
    let mut placements = Vec::new();
    let mut lower = Vec3::splat(f32::INFINITY);
    let mut upper = Vec3::splat(f32::NEG_INFINITY);
    for collision in &compiled.cooked_collision {
        let doc = collision.document()?;
        let hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&doc)?));
        let name = format!("collision_{}.ent", &hash[..20]);
        if !extra_resources.contains(&name) {
            write_and_check(staging.path(), &depot.join(&name), &doc, source.schema())?;
            extra_resources.push(name.clone());
        }
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for &p in &collision.geometry.vertices {
            let p = Vec3::from_array(world(compiled, p));
            lo = lo.min(p);
            hi = hi.max(p);
        }
        lower = lower.min(lo);
        upper = upper.max(hi);
        placements.push(placement(
            nodes.len(),
            compiled.project.origin,
            compiled.project.yaw,
            [1.; 3],
            lo.to_array(),
            hi.to_array(),
        ));
        nodes.push(json!({"HandleId":nodes.len().to_string(),"Data":{"$type":"worldEntityNode","debugName":typed("CName",&collision.id),"entityTemplate":resource(&format!("{namespace}\\{name}")),"appearanceName":typed("CName","None"),"isVisibleInGame":1,"isHostOnly":0}}));
    }
    for p in compiled.placements.iter().filter(|p| !p.reference_only) {
        ensure!(
            compiled.edited_meshes.contains_key(&p.mesh)
                || source
                    .resources()
                    .resource_exists(&ResourceKey::parse(&p.mesh)),
            "Missing native mesh {}",
            p.mesh
        );
        // Validate the selected appearance against current installed geometry.
        if p.entity {
            source.resolve_assembly(
                &ResourceKey::parse(&p.mesh),
                (!p.appearance.is_empty()).then_some(p.appearance.as_str()),
            )?;
        } else if !compiled.edited_meshes.contains_key(&p.mesh) {
            let geometry = source.load_geometry(&ResourceKey::parse(&p.mesh))?;
            ensure!(
                geometry.appearance_names().contains(&p.appearance.as_str()),
                "Native appearance is absent: {}#{}",
                p.mesh,
                p.appearance
            );
        }
        let (low, high) = mesh_bounds(compiled, p);
        lower = lower.min(Vec3::from_array(low));
        upper = upper.max(Vec3::from_array(high));
        placements.push(placement(
            nodes.len(),
            world(compiled, p.position),
            p.yaw + compiled.project.yaw,
            p.scale,
            low,
            high,
        ));
        if let Some(rotation) = p.rotation {
            let q = Quat::from_rotation_z(compiled.project.yaw.to_radians())
                * Quat::from_array(rotation);
            let entry = placements.last_mut().context("Mesh placement missing")?;
            entry["Orientation"] = json!({"$type":"Quaternion","i":q.x,"j":q.y,"k":q.z,"r":q.w});
        }
        if p.entity {
            let appearance = if p.appearance.is_empty() {
                "None"
            } else {
                &p.appearance
            };
            nodes.push(json!({"HandleId":nodes.len().to_string(),"Data":{"$type":"worldEntityNode","debugName":typed("CName",&p.id),"entityTemplate":resource(&p.mesh),"appearanceName":typed("CName",appearance),"isVisibleInGame":1,"isHostOnly":0}}));
        } else {
            nodes.push(json!({"HandleId":nodes.len().to_string(),"Data":{"$type":"worldMeshNode","debugName":typed("CName",&p.id),"mesh":resource(&p.mesh),"meshAppearance":typed("CName",&p.appearance),"isVisibleInGame":1,"isHostOnly":0,"castShadows":"Always","castLocalShadows":"Always","occluderType":"None","forceAutoHideDistance":500.}}));
        }
    }
    for (i, solid) in compiled.solids.iter().enumerate() {
        let position = world(compiled, solid.center);
        let radius = Vec3::from_array(solid.size).length() / 2.;
        let low = (Vec3::from_array(position) - Vec3::splat(radius)).to_array();
        let high = (Vec3::from_array(position) + Vec3::splat(radius)).to_array();
        lower = lower.min(Vec3::from_array(low));
        upper = upper.max(Vec3::from_array(high));
        placements.push(placement(nodes.len(), position, 0., [1.; 3], low, high));
        nodes.push(
            json!({"HandleId":nodes.len().to_string(),"Data":collision(compiled,solid,i+1)?}),
        );
    }
    for (i, b) in compiled.generated_collision.iter().enumerate() {
        let radius = Vec3::from_array(b.size).length() / 2.;
        let low = Vec3::from_array(b.position) - Vec3::splat(radius);
        let high = Vec3::from_array(b.position) + Vec3::splat(radius);
        lower = lower.min(low);
        upper = upper.max(high);
        placements.push(placement(
            nodes.len(),
            b.position,
            0.,
            [1.; 3],
            low.to_array(),
            high.to_array(),
        ));
        let mut node = ghostline_red::world::encode_boxes(
            std::slice::from_ref(b),
            &format!("retained_collision_{i}"),
        )?;
        node["compiledData"]["BufferId"] = json!((compiled.solids.len() + i + 1).to_string());
        nodes.push(json!({"HandleId":nodes.len().to_string(),"Data":node}));
    }
    if !lower.is_finite() {
        lower = Vec3::from_array(compiled.project.origin) - Vec3::ONE;
        upper = Vec3::from_array(compiled.project.origin) + Vec3::ONE;
    }
    let mut refs = Vec::new();
    let mut registry = Vec::new();
    let mut community_nodes = Vec::new();
    let mut community_placements = Vec::new();
    let mut community_refs = Vec::new();
    for extra in crate::technical::nodes(compiled)? {
        let point = world(compiled, extra.position);
        let low = (Vec3::from_array(point) - Vec3::splat(extra.radius)).to_array();
        let high = (Vec3::from_array(point) + Vec3::splat(extra.radius)).to_array();
        lower = lower.min(Vec3::from_array(low));
        upper = upper.max(Vec3::from_array(high));
        if extra.node["Data"]["$type"] == "worldCommunityRegistryNode" {
            let mut registry_placement =
                placement(community_nodes.len(), point, 0., [1.; 3], low, high);
            let reference = node_ref(crate::technical::node_hash(
                extra.node["Data"]["debugName"]["$value"]
                    .as_str()
                    .context("Registry identity absent")?,
            ));
            registry_placement["QuestPrefabRefHash"] = reference.clone();
            community_refs.push(reference);
            community_placements.push(registry_placement);
            community_nodes.push(extra.node);
            continue;
        }
        let mut p = placement(
            nodes.len(),
            point,
            extra.yaw + compiled.project.yaw,
            if extra.node["Data"]["entityTemplate"]["DepotPath"]["$value"]
                .as_str()
                .is_some_and(|s| s.contains("shutter_window_w300_h250"))
            {
                [1.; 3]
            } else {
                extra.scale
            },
            low,
            high,
        );
        if let Some(reference) = extra.reference {
            let reference = node_ref(crate::technical::node_hash(&reference));
            p["QuestPrefabRefHash"] = reference.clone();
            refs.push(reference);
            if extra.node["Data"]["$type"] == "worldDeviceNode" {
                registry.push(json!({"$type":"gameDeviceResourceData_Cls1","children":[],"className":extra.node["Data"]["deviceClassName"],"hash":p["QuestPrefabRefHash"]["$value"],"nodePosition":{"$type":"Vector3","X":point[0],"Y":point[1],"Z":point[2]},"parents":[]}));
            }
        }
        placements.push(p);
        nodes.push(extra.node);
    }
    let has_devices = !registry.is_empty();
    if has_devices {
        let devices = document(
            json!({"$type":"gameDeviceResource","data":{"HandleId":"90000","Data":{"$type":"gameDeviceResourceData","unk1":registry,"version":2}}}),
        );
        write_and_check(
            staging.path(),
            &depot.join("building.devices"),
            &devices,
            source.schema(),
        )?;
    }
    let mut sector = document(
        json!({"$type":"worldStreamingSector","category":"Exterior","cookingPlatform":"PLATFORM_PC","level":0,"localInplaceResource":[],"nodeData":{"BufferId":"0","Flags":0,"Type":"worldNodeDataBuffer","Data":placements},"nodeRefs":[],"nodes":nodes,"persistentNodeIndex":0,"persistentNodes":[],"variantIndices":[0],"variantNodes":[],"version":62}),
    );
    sector["Data"]["RootChunk"]["nodeRefs"] = json!(refs);
    let mut block = document(
        json!({"$type":"worldStreamingBlock","cookingPlatform":"PLATFORM_PC","descriptors":[{"$type":"worldStreamingSectorDescriptor","blockIndex":{"$type":"worldStreamingBlockIndex","oup":"Base","rldGridCell":0},"category":"Exterior","data":resource(&format!("{namespace}\\building.streamingsector")),"level":0,"numNodeRanges":1,"questPrefabNodeRef":"0","streamingBox":{"$type":"Box","Min":vector((lower-Vec3::splat(80.)).to_array()),"Max":vector((upper+Vec3::splat(80.)).to_array())},"variants":[]}],"index":{"$type":"worldStreamingBlockIndex","oup":"Base","rldGridCell":0}}),
    );
    block["Data"]["RootChunk"]["descriptors"][0]["questPrefabNodeRef"] = node_ref(0);
    if !community_nodes.is_empty() {
        let mut always = document(
            json!({"$type":"worldStreamingSector","category":"AlwaysLoaded","cookingPlatform":"PLATFORM_PC","level":0,"localInplaceResource":[],"nodeData":{"BufferId":"0","Flags":0,"Type":"worldNodeDataBuffer","Data":community_placements},"nodeRefs":[],"nodes":community_nodes,"persistentNodeIndex":0,"persistentNodes":[],"variantIndices":[0],"variantNodes":[],"version":62}),
        );
        always["Data"]["RootChunk"]["nodeRefs"] = json!(community_refs);
        write_and_check(
            staging.path(),
            &depot.join("communities.streamingsector"),
            &always,
            source.schema(),
        )?;
        extra_resources.push("communities.streamingsector".into());
        let mut descriptor = block["Data"]["RootChunk"]["descriptors"][0].clone();
        descriptor["category"] = json!("AlwaysLoaded");
        descriptor["data"] = resource(&format!("{namespace}\\communities.streamingsector"));
        block["Data"]["RootChunk"]["descriptors"]
            .as_array_mut()
            .context("Block descriptors absent")?
            .push(descriptor);
    }
    let has_navigation = compiled.project.navigation.export;
    if has_navigation {
        let nav = crate::navigation::compile(compiled)?;
        let nav_resource = crate::navigation::resource(compiled, &nav)?;
        let nav_path = format!("{namespace}\\building.navmesh");
        let embedded = json!({"DepotPath":typed("ResourcePath",&nav_path),"Flags":"Embedded"});
        let mut navigation = document(
            json!({"$type":"worldStreamingSector","category":"Navigation","cookingPlatform":"PLATFORM_PC","level":3,"localInplaceResource":[embedded],"nodeData":{"BufferId":"0","Flags":0,"Type":"worldNodeDataBuffer","Data":[placement(0,compiled.project.origin,0.,[1.;3],lower.to_array(),upper.to_array())]},"nodeRefs":[],"nodes":[{"HandleId":"800000","Data":{"$type":"worldNavigationNode","debugName":typed("CName","human_navigation"),"navigationTileResource":embedded,"isVisibleInGame":1}}],"persistentNodeIndex":0,"persistentNodes":[],"variantIndices":[0],"variantNodes":[],"version":62}),
        );
        navigation["Data"]["EmbeddedFiles"] =
            json!([{"FileName":typed("ResourcePath",&nav_path),"Content":nav_resource}]);
        write_and_check(
            staging.path(),
            &depot.join("navigation.streamingsector"),
            &navigation,
            source.schema(),
        )?;
        let mut descriptor = block["Data"]["RootChunk"]["descriptors"][0].clone();
        descriptor["category"] = json!("Navigation");
        descriptor["level"] = json!(3);
        descriptor["data"] = resource(&format!("{namespace}\\navigation.streamingsector"));
        block["Data"]["RootChunk"]["descriptors"]
            .as_array_mut()
            .context("Block descriptors absent")?
            .push(descriptor);
    }
    let terrain = crate::terrain::build(compiled, source)?;
    for (index, (terrain, report)) in terrain.iter().enumerate() {
        let filename = format!("retained_{index}.streamingsector");
        write_and_check(
            staging.path(),
            &depot.join(&filename),
            terrain,
            source.schema(),
        )?;
        extra_resources.push(filename.clone());
        let mut descriptor = block["Data"]["RootChunk"]["descriptors"][0].clone();
        descriptor["category"] = json!("Navigation");
        descriptor["level"] = json!(3);
        descriptor["data"] = resource(&format!("{namespace}\\{filename}"));
        let bounds = &terrain["Data"]["RootChunk"]["nodeData"]["Data"][0]["Bounds"];
        if bounds.is_object() {
            descriptor["streamingBox"] = bounds.clone();
        }
        block["Data"]["RootChunk"]["descriptors"]
            .as_array_mut()
            .context("Block descriptors absent")?
            .push(descriptor);
        fs::write(
            staging
                .path()
                .join(format!("terrain-mask-report-{index}.json")),
            serde_json::to_vec_pretty(report)?,
        )?;
    }
    for (extension, document) in [("streamingsector", &sector), ("streamingblock", &block)] {
        write_and_check(
            staging.path(),
            &depot.join(format!("building.{extension}")),
            document,
            source.schema(),
        )?;
    }
    // Publish only after both independent native readbacks have succeeded.
    fs::create_dir_all(output.parent().unwrap_or(Path::new(".")))?;
    let publish = tempfile::Builder::new()
        .prefix("building-export-")
        .tempdir_in(output.parent().unwrap_or(Path::new(".")))?;
    let archive = publish
        .path()
        .join(format!("{}.archive", compiled.project.id));
    ghostline_red::archive::pack(&root, &archive)?;
    let index = ghostline_red::archive::read_archive(&archive)?;
    ensure!(
        index.entries.len()
            == 2 + usize::from(has_devices) + usize::from(has_navigation) + extra_resources.len(),
        "Packed archive resource count differs"
    );
    for extension in ["streamingsector", "streamingblock"]
        .into_iter()
        .chain(has_devices.then_some("devices"))
    {
        let key = format!("{namespace}\\building.{extension}");
        let extracted = staging.path().join("unpacked");
        ghostline_red::archive::extract_path(&archive, &extracted, &key)?;
        ensure!(
            fs::read(extracted.join(key.replace('\\', "/")))?
                == fs::read(depot.join(format!("building.{extension}")))?,
            "Packed archive bytes differ"
        );
    }
    if has_navigation {
        let path = format!("{namespace}\\navigation.streamingsector");
        ghostline_red::archive::extract_path(&archive, &staging.path().join("unpacked"), &path)?;
        ensure!(
            fs::read(
                staging
                    .path()
                    .join("unpacked")
                    .join(path.replace('\\', "/"))
            )? == fs::read(depot.join("navigation.streamingsector"))?,
            "Packed navigation bytes differ"
        );
    }
    for name in &extra_resources {
        let key = format!("{namespace}\\{name}");
        ghostline_red::archive::extract_path(&archive, &staging.path().join("unpacked"), &key)?;
        ensure!(
            fs::read(staging.path().join("unpacked").join(key.replace('\\', "/")))?
                == fs::read(depot.join(name))?,
            "Packed extra resource bytes differ: {name}"
        );
    }
    let mut registration =
        json!({"streaming":{"blocks":[format!("{namespace}\\building.streamingblock")]}});
    if !retrofit.is_empty() {
        registration["streaming"]["sectors"] = json!(retrofit);
    }
    if has_devices {
        registration["resource"] = json!({"patch":{format!("{namespace}\\building.devices"):[r"base\worlds\03_night_city\_compiled\default\03_night_city.devices"]}});
    }
    fs::write(
        publish
            .path()
            .join(format!("{}.archive.xl", compiled.project.id)),
        serde_json::to_vec_pretty(&registration)?,
    )?;
    fs::write(
        publish
            .path()
            .join(format!("{}.building.json", compiled.project.id)),
        serde_json::to_vec_pretty(&compiled.project)?,
    )?;
    let sources = publish.path().join("source");
    if !compiled.project.workstations.is_empty() {
        fs::write(
            publish.path().join("workstation-bindings.json"),
            serde_json::to_vec_pretty(&crate::encounters::bindings(compiled))?,
        )?;
        fs::write(
            publish.path().join("quest-fragment.json"),
            serde_json::to_vec_pretty(&crate::encounters::quest_fragment(compiled))?,
        )?;
    }
    for index in 0..terrain.len() {
        fs::copy(
            staging
                .path()
                .join(format!("terrain-mask-report-{index}.json")),
            publish
                .path()
                .join(format!("terrain-mask-report-{index}.json")),
        )?;
    }
    crate::bundle::copy_tree(&root, &sources.join("archive"))?;
    crate::bundle::copy_tree(
        &staging.path().join("raw"),
        &sources
            .join("raw/mod/building_builder")
            .join(&compiled.project.id),
    )?;
    fs::create_dir_all(sources.join("resources"))?;
    fs::copy(
        publish
            .path()
            .join(format!("{}.archive.xl", compiled.project.id)),
        sources
            .join("resources")
            .join(format!("{}.archive.xl", compiled.project.id)),
    )?;
    fs::write(
        publish.path().join("project.json"),
        serde_json::to_vec_pretty(
            &json!({"schema_version":1,"id":compiled.project.id,"depot_prefixes":[namespace.replace('\\',"/")+"/"],"dependencies":[],"registration":format!("{}.archive.xl",compiled.project.id),"generator":"ghostline-building-builder"}),
        )?,
    )?;
    let title = compiled
        .project
        .name
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    fs::write(
        publish.path().join("Building.cpmodproj"),
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<CP77Mod><Name>{title}</Name><ModName>{}</ModName><Author>Ghostline</Author><Version>0.1.0</Version></CP77Mod>\n",
            compiled.project.id
        ),
    )?;
    let mut report = json!({"project":compiled.project.id,"revision":compiled.project.revision,"meshes":compiled.placements.iter().filter(|p|!p.reference_only).count(),"colliders":compiled.solids.len(),"verified_resources":2+usize::from(has_devices)+usize::from(has_navigation),"native_navigation":has_navigation,"archive_sha256":format!("{:x}",Sha256::digest(fs::read(&archive)?)),"game_content":source.content_identity(),"schema_sha256":source.schema_sha256(),"diagnostics":compiled.diagnostics,"runtime_verified":false});
    report["meshes"] = json!(
        compiled
            .placements
            .iter()
            .filter(|p| !p.reference_only && !p.entity)
            .count()
    );
    report["entities"] = json!(
        compiled
            .placements
            .iter()
            .filter(|p| !p.reference_only && p.entity)
            .count()
            + compiled.cooked_collision.len()
    );
    report["verified_resources"] = json!(index.entries.len());
    report["sector_patches"] = json!(
        compiled
            .project
            .retrofit
            .iter()
            .filter(|p| !p.deletions.is_empty())
            .count()
    );
    report["workstations"] = json!(compiled.project.workstations.len());
    report["colliders"] = json!(
        compiled.solids.len()
            + compiled.generated_collision.len()
            + compiled
                .cooked_collision
                .iter()
                .map(|c| c.geometry.triangles.len().div_ceil(15))
                .sum::<usize>()
    );
    report["cooked_collision"]=json!(compiled.cooked_collision.iter().map(|c|json!({"id":c.id,"triangles":c.geometry.triangles.len(),"materials":c.materials,"preset":c.preset,"proxy":c.proxy})).collect::<Vec<_>>());
    report["retrofit"] = json!({"retained_references":compiled.placements.iter().filter(|p|p.reference_only).count(),"shells":compiled.project.shells,"replacement_meshes":compiled.edited_meshes.iter().map(|(path,edit)|json!({"path":path,"cut":edit.report})).collect::<Vec<_>>(),"collision_fragments":compiled.generated_collision.len()});
    fs::write(
        publish.path().join("export-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    let zip = publish.path().join(format!("{}.zip", compiled.project.id));
    crate::bundle::write(publish.path(), &zip)?;
    report["zip_sha256"] = json!(format!("{:x}", Sha256::digest(fs::read(zip)?)));
    fs::write(
        publish.path().join("bundle-receipt.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    fs::rename(publish.path(), output)?;
    Ok(report)
}

fn write_and_check(
    staging: &Path,
    binary: &Path,
    expected: &Value,
    schema: &RedSchema,
) -> Result<()> {
    fs::create_dir_all(staging.join("raw"))?;
    let raw = staging.join("raw").join(format!(
        "{}.json",
        binary
            .file_name()
            .context("Resource filename missing")?
            .to_string_lossy()
    ));
    fs::write(&raw, serde_json::to_vec(expected)?)?;
    ghostline_red::writer::write(&raw, binary, schema)
        .with_context(|| format!("Writing {}", binary.display()))?;
    let actual = ghostline_red::codec::decode_wkit_from_bytes(&fs::read(binary)?, schema)
        .with_context(|| format!("Reading back {}", binary.display()))?;
    let actual_root = canonical_handles(&actual["Data"]["RootChunk"]);
    let expected_root = canonical_handles(&expected["Data"]["RootChunk"]);
    let a = &actual_root;
    let b = &expected_root;
    if let Some(embedded) = expected["Data"]["EmbeddedFiles"]
        .as_array()
        .filter(|a| !a.is_empty())
    {
        let actual_files = actual["Data"]["EmbeddedFiles"]
            .as_array()
            .context("Embedded native resources missing")?;
        ensure!(
            embedded.len() == actual_files.len(),
            "Embedded native resource count differs"
        );
        for (a, b) in actual_files.iter().zip(embedded) {
            verify(
                &canonical_handles(&a["Content"]),
                &canonical_handles(&b["Content"]),
                "embedded.resource",
            )?;
        }
    }
    ensure!(a["$type"] == b["$type"], "Native root readback differs");
    if b["$type"] == "worldStreamingBlock" {
        verify(&a["descriptors"], &b["descriptors"], "block.descriptors")?;
    } else if b["$type"] == "gameDeviceResource" {
        verify(&a["data"]["Data"], &b["data"]["Data"], "devices.data")?;
    } else if b["$type"] == "worldStreamingSector" {
        verify(&a["nodeRefs"], &b["nodeRefs"], "sector.nodeRefs")?;
        let aa = a["nodeData"]["Data"]
            .as_array()
            .context("Missing placement readback")?;
        let bb = b["nodeData"]["Data"]
            .as_array()
            .context("Missing authored placements")?;
        verify(&json!(aa), &json!(bb), "sector.placements")?;
        let aa = a["nodes"].as_array().context("Missing nodes")?;
        let bb = b["nodes"].as_array().context("Missing authored nodes")?;
        ensure!(aa.len() == bb.len(), "Native node count differs");
        for (a, b) in aa.iter().zip(bb) {
            verify(&a["Data"], &b["Data"], "sector.node")?;
            for field in ["$type", "debugName", "mesh", "meshAppearance"] {
                ensure!(
                    a["Data"][field] == b["Data"][field],
                    "Native node {field} readback differs"
                );
            }
            if b["Data"]["$type"] == "worldCollisionNode" {
                ensure!(
                    a["Data"]["compiledData"]["Bytes"] == b["Data"]["compiledData"]["Bytes"],
                    "Native collision bytes differ"
                );
            }
        }
    } else {
        verify(a, b, "resource")?;
    }
    Ok(())
}

// Native serialization renumbers handles. Compare the referenced declaration path,
// preserving graph identity rather than treating arbitrary handle integers as data.
fn canonical_handles(value: &Value) -> Value {
    fn declarations(
        value: &Value,
        path: &str,
        map: &mut std::collections::BTreeMap<String, String>,
    ) {
        match value {
            Value::Object(fields) => {
                if fields.contains_key("Data")
                    && let Some(id) = fields.get("HandleId").and_then(Value::as_str)
                {
                    map.insert(id.into(), path.into());
                }
                for (k, v) in fields {
                    declarations(v, &format!("{path}.{k}"), map);
                }
            }
            Value::Array(values) => {
                for (i, v) in values.iter().enumerate() {
                    declarations(v, &format!("{path}[{i}]"), map);
                }
            }
            _ => {}
        }
    }
    fn replace(value: &mut Value, map: &std::collections::BTreeMap<String, String>) {
        match value {
            Value::Object(fields) => {
                if let Some(reference) = fields.get_mut("HandleRefId")
                    && let Some(path) = reference.as_str().and_then(|id| map.get(id))
                {
                    *reference = json!(path);
                }
                for v in fields.values_mut() {
                    replace(v, map);
                }
            }
            Value::Array(values) => {
                for v in values {
                    replace(v, map);
                }
            }
            _ => {}
        }
    }
    let mut map = std::collections::BTreeMap::new();
    declarations(value, "root", &mut map);
    let mut result = value.clone();
    replace(&mut result, &map);
    result
}

fn verify(actual: &Value, expected: &Value, path: &str) -> Result<()> {
    if expected["$type"] == "TweakDBID" {
        let identity = |v: &Value| -> Result<u64> {
            let value = v["$value"].as_str().context("Invalid TweakDB identity")?;
            Ok(value
                .parse()
                .unwrap_or_else(|_| ghostline_red::tweakdb::TweakId::from_name(value).0))
        };
        ensure!(
            identity(actual)? == identity(expected)?,
            "Native TweakDB identity differs at {path}"
        );
        return Ok(());
    }
    if expected["$type"] == "NodeRef" {
        if expected["$value"] == "0" && actual["$value"] == "" {
            return Ok(());
        }
        ensure!(
            actual["$value"] == expected["$value"],
            "Native node reference differs at {path}: {actual} / {expected}"
        );
        return Ok(());
    }
    if expected["$type"] == "AreaShapeOutline" {
        ensure!(
            actual["buffer"] == expected["buffer"],
            "Native area outline bytes differ at {path}"
        );
        return Ok(());
    }
    match expected {
        Value::Object(fields) => {
            for (key, value) in fields {
                if ["HandleId", "BufferId", "Type"].contains(&key.as_str())
                    || (key == "Flags" && fields.contains_key("BufferId"))
                {
                    continue;
                }
                verify(&actual[key], value, &format!("{path}.{key}"))?;
            }
        }
        Value::Array(values) => {
            let array = actual
                .as_array()
                .with_context(|| format!("Readback array missing at {path}"))?;
            ensure!(
                array.len() == values.len(),
                "Readback array length differs at {path}: {} / {}",
                array.len(),
                values.len()
            );
            for (i, (a, b)) in array.iter().zip(values).enumerate() {
                verify(a, b, &format!("{path}[{i}]"))?;
            }
        }
        Value::Number(n) => {
            if let Some(value) = actual.as_bool() {
                ensure!(
                    n.as_u64() == Some(u64::from(value)),
                    "Readback boolean differs at {path}"
                );
                return Ok(());
            }
            let b = n.as_f64().context("Invalid authored number")?;
            let a = actual
                .as_f64()
                .with_context(|| format!("Readback number missing at {path}"))?;
            ensure!(
                (a - b).abs() <= 0.00001_f64.max(b.abs() * 0.0000002),
                "Readback number differs at {path}: {a} / {b}"
            );
        }
        _ => ensure!(
            actual == expected,
            "Readback differs at {path}: {actual} / {expected}"
        ),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readback_tracks_handle_targets_after_renumbering() {
        let source = json!({"definitions":[{"HandleId":"74","Data":{"value":1}},{"HandleId":"80","Data":{"value":2}}],"reference":{"HandleRefId":"74"}});
        let renamed = json!({"definitions":[{"HandleId":"6","Data":{"value":1}},{"HandleId":"7","Data":{"value":2}}],"reference":{"HandleRefId":"6"}});
        verify(
            &canonical_handles(&renamed),
            &canonical_handles(&source),
            "resource",
        )
        .unwrap();
        let mut incorrect = renamed;
        incorrect["reference"]["HandleRefId"] = json!("7");
        assert!(
            verify(
                &canonical_handles(&incorrect),
                &canonical_handles(&source),
                "resource"
            )
            .is_err()
        );
    }
}
