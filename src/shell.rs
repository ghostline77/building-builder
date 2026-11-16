//! Imported world shells, source selections and authoring operations.
use crate::{
    model::{Anchor, Bounds, Library, Module, Object, Project},
    retrofit::SectorPatch,
};
use anyhow::{Context, Result, ensure};
use ghostline_red::assets::ResourceKey;
use ghostline_renderer::{AssemblyOptions, GameSource};
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub id: String,
    pub sector: String,
    pub index: usize,
    pub instance: Option<usize>,
    pub actor: Option<usize>,
    pub shape: Option<usize>,
    pub kind: String,
    pub layer: String,
    pub label: String,
    pub resource: Option<ResourceKey>,
    pub appearance: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub bounds: Option<Bounds>,
    pub object: Option<String>,
    #[serde(default)]
    pub removed: bool,
    #[serde(default)]
    pub diagnostics: Vec<String>,
    #[serde(default)]
    pub native: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Floor {
    pub name: String,
    pub level: u32,
    pub position: [f32; 3],
    pub size: [f32; 3],
    pub yaw: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedShell {
    pub id: String,
    pub name: String,
    pub members: Vec<Member>,
    pub floors: Vec<Floor>,
    #[serde(default)]
    pub operations: Vec<Operation>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub members: Vec<String>,
    pub position: [f32; 3],
    pub size: [f32; 3],
    pub yaw: f32,
    pub preset: String,
    #[serde(default)]
    pub diagnostics: Vec<String>,
    #[serde(default)]
    pub stair: Option<crate::stairs::Stair>,
    #[serde(default)]
    pub navigation_masks: Vec<crate::terrain::Mask>,
}

pub struct EditedMesh {
    pub source: ResourceKey,
    pub document: Value,
    pub report: ghostline_red::mesh::CutReport,
}

#[derive(Serialize)]
pub struct CookedCollision {
    pub id: String,
    pub geometry: ghostline_red::physics::CollisionGeometry,
    pub materials: Vec<String>,
    pub material_names: Vec<String>,
    pub preset: String,
    pub preset_name: String,
    pub proxy: u8,
}

impl CookedCollision {
    pub fn document(&self) -> Result<Value> {
        use base64::Engine;
        let streams = self.geometry.cook()?;
        let face_materials = self
            .material_names
            .iter()
            .map(|m| json!({"$type":"CName","$storage":"string","$value":m}))
            .collect::<Vec<_>>();
        let colliders=streams.iter().enumerate().map(|(i,stream)|json!({"HandleId":(100+i).to_string(),"Data":{"$type":"physicsColliderMesh","localToBody":{"$type":"Transform","position":{"$type":"Vector4","X":0.,"Y":0.,"Z":0.,"W":0.},"orientation":{"$type":"Quaternion","i":0.,"j":0.,"k":0.,"r":1.}},"faceMaterials":face_materials,"compiledGeometryBuffer":{"BufferId":i.to_string(),"Flags":4063232,"Bytes":base64::engine::general_purpose::STANDARD.encode(stream)}}})).collect::<Vec<_>>();
        Ok(
            json!({"Header":{"DataType":"CR2W","GameVersion":2310},"Data":{"Version":195,"BuildVersion":0,"RootChunk":{"$type":"entEntityTemplate","cookingPlatform":"PLATFORM_PC","compiledData":{"BufferId":streams.len().to_string(),"Flags":4063232,"Type":"WolvenKit.RED4.Types.RedPackage","Data":{"Version":4,"Sections":7,"CruidIndex":-1,"CruidDict":{"0":"0","1":"2"},"Chunks":[{"$type":"entEntity"},{"$type":"entColliderComponent","name":{"$type":"CName","$storage":"string","$value":"retained_collision"},"id":"2","simulationType":"Static","isEnabled":1,"colliders":colliders,"filterData":{"HandleId":"99","Data":{"$type":"physicsFilterData","preset":{"$type":"CName","$storage":"string","$value":self.preset_name}}}}]}}},"EmbeddedFiles":[]}}),
        )
    }
}

#[derive(Deserialize)]
pub struct SurveyRequest {
    pub position: [f32; 3],
    pub radius: f32,
    #[serde(default)]
    pub sectors: Vec<String>,
}

pub fn survey(source: &GameSource, request: &SurveyRequest) -> Result<Value> {
    ensure!(
        request.radius.is_finite() && (1. ..=100.).contains(&request.radius),
        "Choose a survey radius between 1 and 100 metres"
    );
    let sectors = if request.sectors.is_empty() {
        crate::retrofit::nearby_sectors(source, request.position, request.radius)?["sectors"]
            .as_array()
            .context("Sector descriptors absent")?
            .iter()
            .filter_map(|v| {
                v["path"]
                    .as_str()
                    .map(|p| (p.to_string(), v["level"].as_u64().unwrap_or(0) == 0))
            })
            .collect::<Vec<_>>()
    } else {
        request.sectors.iter().map(|p| (p.clone(), true)).collect()
    };
    let mut sources = Vec::new();
    let mut members = Vec::new();
    let mut failures = Vec::new();
    for (path, visual_detail) in &sectors {
        match crate::retrofit::survey_members(
            source,
            path,
            request.position,
            request.radius,
            *visual_detail,
        ) {
            Ok((pin, mut found)) => {
                if !found.is_empty() {
                    sources.push(pin);
                    members.append(&mut found);
                }
            }
            Err(error) => failures.push(json!({"sector":path,"error":error.to_string()})),
        }
    }
    members.sort_by(|a, b| {
        (a.layer.as_str(), a.label.as_str(), a.id.as_str()).cmp(&(
            b.layer.as_str(),
            b.label.as_str(),
            b.id.as_str(),
        ))
    });
    Ok(
        json!({"sources":sources,"members":members,"failures":failures,"sectors_scanned":sectors.len(),"position":request.position,"radius":request.radius}),
    )
}

#[derive(Deserialize)]
pub struct ImportRequest {
    pub project: Project,
    pub name: String,
    pub sources: Vec<SectorPatch>,
    pub members: Vec<Member>,
    pub floors: Vec<Floor>,
}

#[derive(Deserialize)]
pub struct PreviewRequest {
    pub import: ImportRequest,
    pub options: crate::preview::Options,
}

#[derive(Deserialize)]
pub struct OperationRequest {
    pub project: Project,
    pub shell: String,
    pub operation: Operation,
    #[serde(default)]
    pub restore: bool,
}

pub fn operate(source: &GameSource, mut request: OperationRequest) -> Result<Project> {
    // Native masks are derived from selected source nodes and the edit volume.
    if !request.restore {
        request.operation.navigation_masks.clear();
    }
    let shell_index = request
        .project
        .shells
        .iter()
        .position(|s| s.id == request.shell)
        .context("Imported shell absent")?;
    crate::model::identifier(&request.operation.id)?;
    if request.restore {
        let index = request.project.shells[shell_index]
            .operations
            .iter()
            .position(|op| op.id == request.operation.id)
            .context("Change not found")?;
        let operation = request.project.shells[shell_index].operations.remove(index);
        for id in &operation.members {
            if request.project.shells[shell_index]
                .operations
                .iter()
                .any(|op| op.members.contains(id))
            {
                continue;
            }
            let member = request.project.shells[shell_index]
                .members
                .iter_mut()
                .find(|m| &m.id == id)
                .context("Change target absent")?;
            member.removed = false;
            if let Some(patch) = request
                .project
                .retrofit
                .iter_mut()
                .find(|p| p.path == member.sector)
            {
                remove_deletion(patch, member);
            }
        }
        request.project.door_devices.remove(&operation.id);
        request.project.door_states.remove(&operation.id);
        request.project.partitions.retain(|p| p.id != operation.id);
        request.project.stairs.retain(|s| s.id != operation.id);
    } else {
        ensure!(
            !request.project.shells[shell_index]
                .operations
                .iter()
                .any(|op| op.id == request.operation.id),
            "Change identity already exists"
        );
        ensure!(
            ["remove", "entrance", "replace_wall", "floor_connection"]
                .contains(&request.operation.kind.as_str()),
            "Unknown building operation"
        );
        ensure!(
            !request.operation.members.is_empty(),
            "Select the existing geometry affected by this change"
        );
        for id in &request.operation.members {
            let member = request.project.shells[shell_index]
                .members
                .iter_mut()
                .find(|m| &m.id == id)
                .context("Change target absent")?;
            ensure!(
                !member.removed || member.layer == "navigation",
                "This piece already belongs to a change; restore that change first"
            );
            let patch = request
                .project
                .retrofit
                .iter_mut()
                .find(|p| p.path == member.sector)
                .context("Source pin absent")?;
            crate::retrofit::pinned(source, patch)?;
            if member.layer == "navigation" {
                let op = &request.operation;
                let q = Quat::from_rotation_z(op.yaw.to_radians());
                let center = Vec3::from_array(op.position);
                let outline = [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]]
                    .map(|[x, y]| {
                        let p = center + q * Vec3::new(x * op.size[0], y * op.size[1], 0.);
                        [p.x, p.y]
                    })
                    .to_vec();
                request
                    .operation
                    .navigation_masks
                    .push(crate::terrain::Mask {
                        sector: patch.path.clone(),
                        sha256: patch.sha256.clone(),
                        expected_nodes: patch.expected_nodes,
                        placement_index: member.index,
                        resource_index: member.native["resource_index"]
                            .as_u64()
                            .context("Select an embedded Human navigation tile")?
                            as usize,
                        outline,
                        height_range: [op.position[2] - 0.2, op.position[2] + op.size[2]],
                    });
                request.project.navigation.export = true;
            }
            add_deletion(patch, member)?;
            member.removed = true;
        }
        if request.operation.kind == "entrance" {
            let op = &request.operation;
            ensure!(
                op.position.iter().chain(&op.size).all(|v| v.is_finite())
                    && op.size[0] >= 0.7
                    && op.size[2] >= 1.8,
                "Invalid entrance dimensions"
            );
            let (_, _, hinge, _) = crate::technical::door_preset(&op.preset)?;
            let angle = op.yaw.to_radians();
            let center = Vec3::from_array(op.position);
            let normal = Vec3::new(-angle.sin(), angle.cos(), 0.);
            request.project.door_devices.insert(
                op.id.clone(),
                crate::technical::DoorDevice {
                    preset: op.preset.clone(),
                    position: Some(
                        (center + Vec3::new(angle.cos(), angle.sin(), 0.) * hinge).to_array(),
                    ),
                    yaw: op.yaw,
                    endpoints: Some([
                        (center - normal * 0.8).to_array(),
                        (center + normal * 0.8).to_array(),
                    ]),
                    manual: true,
                    ..Default::default()
                },
            );
            request
                .project
                .door_states
                .insert(op.id.clone(), crate::technical::DoorState::Closed);
        }
        if request.operation.kind == "replace_wall" {
            let op = &request.operation;
            let half =
                Quat::from_rotation_z(op.yaw.to_radians()) * Vec3::new(op.size[0] / 2., 0., 0.);
            let center = Vec3::from_array(op.position);
            request.project.partitions.push(crate::interiors::Wall {
                id: op.id.clone(),
                from: (center - half).to_array(),
                to: (center + half).to_array(),
                height: op.size[2],
                thickness: op.size[1],
                openings: Vec::new(),
                level: request.project.shells[shell_index]
                    .floors
                    .iter()
                    .filter(|f| f.position[2] <= op.position[2] + 0.1)
                    .max_by(|a, b| a.position[2].total_cmp(&b.position[2]))
                    .map_or(0, |f| f.level),
            });
        }
        if request.operation.kind == "floor_connection" {
            let mut stair = request
                .operation
                .stair
                .clone()
                .context("Choose the stair flight for this floor connection")?;
            stair.id = request.operation.id.clone();
            request.project.stairs.push(stair);
        }
        request.project.shells[shell_index]
            .operations
            .push(request.operation);
    }
    request.project.revision += 1;
    Ok(request.project)
}

fn add_deletion(patch: &mut SectorPatch, member: &Member) -> Result<()> {
    let mut deletion = member.native.clone();
    deletion
        .as_object_mut()
        .context("Source selection absent")?
        .retain(|k, _| {
            matches!(
                k.as_str(),
                "index"
                    | "type"
                    | "expectedActors"
                    | "actorDeletions"
                    | "expectedInstances"
                    | "instanceDeletions"
                    | "removeWholeGroup"
            )
        });
    if let Some(existing) = patch
        .deletions
        .iter_mut()
        .find(|d| d["index"] == deletion["index"])
    {
        for key in ["actorDeletions", "instanceDeletions"] {
            if let Some(values) = deletion[key].as_array() {
                for value in values {
                    let array = existing[key]
                        .as_array_mut()
                        .context("Mixed native deletion kinds")?;
                    if !array.contains(value) {
                        array.push(value.clone());
                    }
                }
            }
        }
    } else {
        patch.deletions.push(deletion);
    }
    Ok(())
}
fn remove_deletion(patch: &mut SectorPatch, member: &Member) {
    if let Some(index) = patch
        .deletions
        .iter()
        .position(|d| d["index"] == member.index)
    {
        let deletion = &mut patch.deletions[index];
        let key = if member.actor.is_some() {
            "actorDeletions"
        } else {
            "instanceDeletions"
        };
        if let Some(array) = deletion[key].as_array_mut() {
            let selected = if let Some(actor) = member.actor {
                json!([actor, member.shape.unwrap_or(0)])
            } else {
                json!(member.instance)
            };
            array.retain(|v| v != &selected);
            if !array.is_empty() {
                return;
            }
        }
        patch.deletions.remove(index);
    }
}

pub fn import(source: &GameSource, mut request: ImportRequest) -> Result<Project> {
    ensure!(
        !request.name.trim().is_empty() && request.name.len() <= 200,
        "Give the imported building a name"
    );
    ensure!(
        !request.members.is_empty() && request.members.len() <= 2000,
        "Select between 1 and 2000 shell pieces"
    );
    let documents = request
        .sources
        .iter()
        .map(|pin| Ok((pin.path.clone(), crate::retrofit::pinned(source, pin)?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let id = format!("shell_{}", request.project.revision + 1);
    ensure!(
        !request.project.shells.iter().any(|s| s.id == id),
        "Shell identity already exists"
    );
    let inverse = Quat::from_rotation_z(-request.project.yaw.to_radians());
    for member in &mut request.members {
        let pin = request
            .sources
            .iter()
            .find(|p| p.path == member.sector)
            .context("Shell piece has no source pin")?;
        let actual = crate::retrofit::make_member(
            source,
            &pin.path,
            &documents[&pin.path],
            member.index,
            member.instance,
            member.actor,
            member.shape,
        )?;
        // Never trust client-supplied native transforms, resources or source identities.
        *member = actual;
        if member.resource.is_some()
            && member.bounds.is_some()
            && ["structure", "furnishings", "devices"].contains(&member.layer.as_str())
        {
            let object_id = format!("{}_{}", id, member.id);
            let asset = format!(
                "world:{}",
                member
                    .resource
                    .as_ref()
                    .context("Missing resource")?
                    .native_hash()
            );
            request.project.objects.push(Object {
                id: object_id.clone(),
                asset,
                appearance: member.appearance.clone(),
                position: (inverse
                    * (Vec3::from_array(member.position)
                        - Vec3::from_array(request.project.origin)))
                .to_array(),
                yaw: 0.,
                anchor: Anchor::Pivot,
                scale: member.scale,
                rotation: Some((inverse * Quat::from_array(member.rotation)).to_array()),
                reference_only: true,
                collision: false,
                source_content_sha256: None,
                level: Some(
                    request
                        .floors
                        .iter()
                        .filter(|f| {
                            f.position[2]
                                <= (inverse
                                    * (Vec3::from_array(member.position)
                                        - Vec3::from_array(request.project.origin)))
                                .z + 0.2
                        })
                        .max_by(|a, b| a.position[2].total_cmp(&b.position[2]))
                        .map_or(0, |f| f.level),
                ),
            });
            member.object = Some(object_id);
        }
    }
    for pin in request.sources {
        if !request.project.retrofit.iter().any(|p| p.path == pin.path) {
            request.project.retrofit.push(pin);
        }
    }
    for floor in &request.floors {
        add_floor(&mut request.project, &id, floor)?;
    }
    request.project.shells.push(ImportedShell {
        id,
        name: request.name,
        members: request.members,
        floors: request.floors,
        operations: Vec::new(),
    });
    request.project.revision += 1;
    Ok(request.project)
}

pub fn add_floor(project: &mut Project, shell: &str, floor: &Floor) -> Result<()> {
    ensure!(
        floor.level < 4
            && floor.size.iter().all(|v| v.is_finite() && *v > 0.)
            && floor.position.iter().all(|v| v.is_finite())
            && floor.yaw.is_finite(),
        "Invalid floor dimensions or position"
    );
    let id = format!("{shell}_floor_{}", floor.level);
    ensure!(
        !project.shell_rooms.iter().any(|r| r.id == id),
        "A floor at this level is already defined"
    );
    project.shell_rooms.push(crate::interiors::ShellRoom {
        id,
        position: floor.position,
        size: floor.size,
        yaw: floor.yaw,
        level: floor.level,
        retained_sides: ["north", "south", "east", "west", "floor", "ceiling"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        openings: BTreeMap::new(),
    });
    Ok(())
}

/// Prepares only native world references; new catalogue objects retain their published identities.
pub fn hydrate(project: &Project, library: &mut Library, source: &GameSource) -> Result<()> {
    let mut seen = BTreeSet::new();
    for shell in &project.shells {
        crate::model::identifier(&shell.id)?;
        for member in &shell.members {
            let Some(object_id) = &member.object else {
                continue;
            };
            let object = project
                .objects
                .iter()
                .find(|o| &o.id == object_id)
                .context("Imported reference object is missing")?;
            ensure!(
                object.reference_only,
                "An imported reference cannot be exported as a duplicate"
            );
            let expected = member_matrix(project, member);
            let actual = Mat4::from_scale_rotation_translation(
                Vec3::from_array(object.scale),
                Quat::from_array(
                    object
                        .rotation
                        .context("Imported reference quaternion absent")?,
                ),
                Vec3::from_array(object.position),
            );
            ensure!(
                expected
                    .to_cols_array()
                    .iter()
                    .zip(actual.to_cols_array())
                    .all(|(a, b)| (*a - b).abs() < 0.002)
                    && object.appearance == member.appearance,
                "Imported source geometry is locked; edit a change operation rather than moving a retained reference"
            );
            let key = member
                .resource
                .as_ref()
                .context("Reference resource missing")?;
            ensure!(
                object.asset == format!("world:{}", key.native_hash()),
                "Imported resource identity changed"
            );
            if !seen.insert((object.asset.clone(), object.appearance.clone()))
                || library.assets.iter().any(|m| {
                    m.id == object.asset && m.catalogue_bounds.contains_key(&object.appearance)
                })
            {
                continue;
            }
            let bounds = member
                .bounds
                .clone()
                .context("Native reference has no measured geometry")?;
            let mut options = AssemblyOptions::default();
            let entity = member.kind == "worldEntityNode";
            if entity {
                let native = source.resolve_assembly(
                    key,
                    (!object.appearance.is_empty()).then_some(object.appearance.as_str()),
                )?;
                options = AssemblyOptions::standalone(&native);
            }
            let module = Module {
                id: object.asset.clone(),
                path: match key {
                    ResourceKey::Path(p) => p.clone(),
                    ResourceKey::Hash(h) => h.to_string(),
                },
                name: member.label.clone(),
                family: "Imported world".into(),
                role: "shell_reference".into(),
                status: "prepared".into(),
                bounds: bounds.clone(),
                appearances: vec![object.appearance.clone()],
                source_content_sha256: None,
                entity,
                catalogue_bounds: BTreeMap::from([(object.appearance.clone(), bounds)]),
                assembly_options: BTreeMap::from([(object.appearance.clone(), options)]),
            };
            if let Some(existing) = library.assets.iter_mut().find(|m| m.id == module.id) {
                if !existing.appearances.contains(&object.appearance) {
                    existing.appearances.push(object.appearance.clone());
                }
                existing.catalogue_bounds.extend(module.catalogue_bounds);
                existing.assembly_options.extend(module.assembly_options);
            } else {
                library.assets.push(module);
            }
        }
    }
    Ok(())
}

pub fn local_bounds(project: &Project, member: &Member) -> Option<Bounds> {
    let bounds = member.bounds.as_ref()?;
    let matrix = Mat4::from_rotation_translation(
        Quat::from_rotation_z(-project.yaw.to_radians()),
        -Vec3::ZERO,
    ) * Mat4::from_translation(-Vec3::from_array(project.origin))
        * Mat4::from_scale_rotation_translation(
            Vec3::from_array(member.scale),
            Quat::from_array(member.rotation),
            Vec3::from_array(member.position),
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
    Some(Bounds {
        min: low.to_array(),
        max: high.to_array(),
        dimensions: (high - low).to_array(),
    })
}

fn member_matrix(project: &Project, member: &Member) -> Mat4 {
    Mat4::from_quat(Quat::from_rotation_z(-project.yaw.to_radians()))
        * Mat4::from_translation(-Vec3::from_array(project.origin))
        * Mat4::from_scale_rotation_translation(
            Vec3::from_array(member.scale),
            Quat::from_array(member.rotation),
            Vec3::from_array(member.position),
        )
}

fn brush_matrix(operation: &Operation) -> Mat4 {
    Mat4::from_rotation_translation(
        Quat::from_rotation_z(operation.yaw.to_radians()),
        Vec3::from_array(operation.position) + Vec3::Z * operation.size[2] / 2.,
    )
}

/// Compiles source edits once for both the GPU preview and native export.
pub fn compile_effects(compiled: &mut crate::model::Compiled, source: &GameSource) -> Result<()> {
    let project = &compiled.project;
    for shell in &project.shells {
        for member in &shell.members {
            if let Some(id) = &member.object {
                let p = compiled
                    .placements
                    .iter_mut()
                    .find(|p| &p.id == id)
                    .context("Shell placement missing")?;
                p.role = format!("existing_{}", member.layer);
                p.cutaway = member.label.contains("roof") || member.label.contains("ceiling");
                if !p.entity {
                    let binding = source
                        .resources()
                        .mesh_appearance_binding(&ResourceKey::parse(&p.mesh), &p.appearance)?;
                    if binding.used_native_default {
                        compiled.diagnostics.push(format!(
                            "{}: native mesh appearance {:?} falls back to {:?}",
                            member.label, p.appearance, binding.appearance
                        ));
                        p.appearance = binding.appearance;
                    }
                }
            }
            if member.layer == "collision" && !member.removed && member.native["shape_code"] == 0 {
                let bounds = local_bounds(project, member).context("Box dimensions absent")?;
                compiled.reference_solids.push(crate::model::Solid {
                    id: member.id.clone(),
                    center: ((Vec3::from_array(bounds.min) + Vec3::from_array(bounds.max)) / 2.)
                        .to_array(),
                    size: bounds.dimensions,
                    yaw: 0.,
                });
            }
            if project.navigation.export && member.layer == "collision" && !member.removed {
                compiled
                    .reference_collision
                    .push(native_collision(project, member, source)?.0);
            }
        }
        for op in &shell.operations {
            ensure!(
                op.position.iter().chain(&op.size).all(|v| v.is_finite())
                    && op.size.iter().all(|v| *v > 0.)
                    && op.yaw.is_finite(),
                "Invalid change volume"
            );
            for id in &op.members {
                let member = shell
                    .members
                    .iter()
                    .find(|m| &m.id == id)
                    .context("Changed member missing")?;
                if op.kind == "remove" {
                    continue;
                }
                if member.layer == "collision" {
                    let boxes = if member.native["shape_code"] == 0 {
                        cut_collision(project, member, op)?
                    } else {
                        None
                    };
                    if let Some(boxes) = boxes {
                        compiled.generated_collision.extend(boxes);
                    } else {
                        let key = ResourceKey::parse(&member.sector);
                        let document = source.resources().document(&key)?;
                        let root = &document["Data"]["RootChunk"];
                        let node_index = root["nodeData"]["Data"][member.index]["NodeIndex"]
                            .as_u64()
                            .context("Collision node index absent")?
                            as usize;
                        let resolved = ghostline_red::world::placement_node(
                            &root["nodes"][node_index]["Data"],
                            &root["nodeData"]["Data"][member.index],
                        )?;
                        let node = &resolved;
                        let shape = ghostline_red::world::collision_shapes(node)?
                            .into_iter()
                            .find(|s| {
                                Some(s.actor) == member.actor && Some(s.shape) == member.shape
                            })
                            .context("Collision source shape absent")?;
                        let mut geometry = if let Some(half) = shape.half_extents {
                            let half = Vec3::from_array(half);
                            ghostline_red::physics::CollisionGeometry::solid_box(
                                (-half).to_array(),
                                half.to_array(),
                            )
                        } else {
                            let sector = node["sectorHash"]
                                .as_u64()
                                .or_else(|| {
                                    node["sectorHash"].as_str().and_then(|s| s.parse().ok())
                                })
                                .context("Collision sector hash absent")?;
                            (*source.resources().collision_geometry(
                                sector,
                                shape.geometry.context(
                                    "Cut requires a box, triangle mesh or convex collider",
                                )?,
                            )?)
                            .clone()
                        };
                        let matrix = member_matrix(project, member);
                        for p in &mut geometry.vertices {
                            *p = matrix.transform_point3(Vec3::from_array(*p)).to_array();
                        }
                        let brush = brush_matrix(op).inverse();
                        let half = Vec3::from_array(op.size) / 2.;
                        let mut planes = Vec::new();
                        for axis in 0..3 {
                            let row = brush.row(axis);
                            planes.push([row.x, row.y, row.z, row.w - half[axis]]);
                            planes.push([-row.x, -row.y, -row.z, -row.w - half[axis]]);
                        }
                        let geometry = geometry.subtract_solid(&planes)?;
                        if !geometry.triangles.is_empty() {
                            compiled.cooked_collision.push(CookedCollision {
                                id: format!("{}_{}", op.id, member.id),
                                geometry,
                                materials: shape
                                    .materials
                                    .iter()
                                    .map(ToString::to_string)
                                    .collect(),
                                preset: shape.preset.to_string(),
                                material_names: shape
                                    .materials
                                    .iter()
                                    .map(|&hash| {
                                        source.resources().collision_name(hash).map_err(Into::into)
                                    })
                                    .collect::<Result<_>>()?,
                                preset_name: source.resources().collision_name(shape.preset)?,
                                proxy: shape.proxy,
                            });
                        }
                    }
                } else if let Some(object) = &member.object {
                    ensure!(
                        member.kind != "worldEntityNode",
                        "Cut the building's rigid shell rather than an animated entity; use Remove obstruction to remove a complete entity"
                    );
                    let resource = member.resource.as_ref().context("Changed mesh missing")?;
                    let mut document = (*source.resources().document(resource)?).clone();
                    // Decoded meshes are glTF Y-up; authoring volumes and native placements are RED Z-up.
                    let gltf_to_red = Mat4::from_cols(
                        Vec3::X.extend(0.),
                        Vec3::Z.extend(0.),
                        (-Vec3::Y).extend(0.),
                        glam::Vec4::W,
                    );
                    let matrix =
                        brush_matrix(op).inverse() * member_matrix(project, member) * gltf_to_red;
                    let half = Vec3::from_array(op.size) / 2.;
                    let mut planes = Vec::new();
                    for axis in 0..3 {
                        let row = matrix.row(axis);
                        planes.push([row.x, row.y, row.z, row.w - half[axis]]);
                        planes.push([-row.x, -row.y, -row.z, -row.w - half[axis]]);
                    }
                    // Entire pieces inside the brush have no replacement geometry.
                    if member.bounds.as_ref().is_some_and(|b| {
                        [b.min[0], b.max[0]].into_iter().all(|x| {
                            [b.min[1], b.max[1]].into_iter().all(|y| {
                                [b.min[2], b.max[2]].into_iter().all(|z| {
                                    let p = brush_matrix(op).inverse().transform_point3(
                                        member_matrix(project, member)
                                            .transform_point3(Vec3::new(x, y, z)),
                                    );
                                    p.abs().cmple(half + Vec3::splat(0.0001)).all()
                                })
                            })
                        })
                    }) {
                        continue;
                    }
                    let report =
                        ghostline_red::mesh::subtract_convex_volume(&mut document, &planes)
                            .with_context(|| format!("Cutting {}", member.label))?;
                    ensure!(
                        report.affected_triangles > 0,
                        "Change volume does not intersect {}; reposition or deselect this piece",
                        member.label
                    );
                    use sha2::{Digest, Sha256};
                    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&document)?));
                    let path = format!(
                        "mod\\building_builder\\{}\\cut_{}.mesh",
                        project.id,
                        &digest[..20]
                    );
                    let mut replacement = compiled
                        .placements
                        .iter()
                        .find(|p| &p.id == object)
                        .context("Original mesh placement missing")?
                        .clone();
                    replacement.id = format!("{}_{}", op.id, member.id);
                    replacement.role = "replacement".into();
                    replacement.mesh = path.clone();
                    replacement.reference_only = false;
                    compiled.placements.push(replacement);
                    compiled.edited_meshes.insert(
                        path,
                        EditedMesh {
                            source: resource.clone(),
                            document,
                            report,
                        },
                    );
                }
            }
        }
    }
    let inverse = Quat::from_rotation_z(-project.yaw.to_radians());
    for (i, b) in compiled.generated_collision.iter().enumerate() {
        let q = inverse * Quat::from_array(b.rotation);
        let size = Vec3::from_array(b.size);
        let extents = (q * Vec3::X).abs() * size.x
            + (q * Vec3::Y).abs() * size.y
            + (q * Vec3::Z).abs() * size.z;
        compiled.reference_solids.push(crate::model::Solid {
            id: format!("replacement_collision_{i}"),
            center: (inverse * (Vec3::from_array(b.position) - Vec3::from_array(project.origin)))
                .to_array(),
            size: extents.to_array(),
            yaw: 0.,
        });
        if project.navigation.export {
            let mut geometry = ghostline_red::physics::CollisionGeometry::solid_box(
                (-size / 2.).to_array(),
                (size / 2.).to_array(),
            );
            let center =
                inverse * (Vec3::from_array(b.position) - Vec3::from_array(project.origin));
            for p in &mut geometry.vertices {
                *p = (center + q * Vec3::from_array(*p)).to_array();
            }
            compiled.reference_collision.push(geometry);
        }
    }
    Ok(())
}

fn native_collision(
    project: &Project,
    member: &Member,
    source: &GameSource,
) -> Result<(
    ghostline_red::physics::CollisionGeometry,
    ghostline_red::world::CollisionShape,
)> {
    let document = source
        .resources()
        .document(&ResourceKey::parse(&member.sector))?;
    let root = &document["Data"]["RootChunk"];
    let index = root["nodeData"]["Data"][member.index]["NodeIndex"]
        .as_u64()
        .context("Collision node index absent")? as usize;
    let resolved = ghostline_red::world::placement_node(
        &root["nodes"][index]["Data"],
        &root["nodeData"]["Data"][member.index],
    )?;
    let node = &resolved;
    let shape = ghostline_red::world::collision_shapes(node)?
        .into_iter()
        .find(|s| Some(s.actor) == member.actor && Some(s.shape) == member.shape)
        .context("Collision source shape absent")?;
    let mut geometry = if let Some(half) = shape.half_extents {
        let half = Vec3::from_array(half);
        ghostline_red::physics::CollisionGeometry::solid_box((-half).to_array(), half.to_array())
    } else {
        let sector = node["sectorHash"]
            .as_u64()
            .or_else(|| node["sectorHash"].as_str().and_then(|s| s.parse().ok()))
            .context("Collision sector hash absent")?;
        (*source.resources().collision_geometry(
            sector,
            shape
                .geometry
                .context("Navigation requires a box, triangle or convex collider")?,
        )?)
        .clone()
    };
    let transform = member_matrix(project, member);
    for p in &mut geometry.vertices {
        *p = transform.transform_point3(Vec3::from_array(*p)).to_array();
    }
    Ok((geometry, shape))
}

fn cut_collision(
    project: &Project,
    member: &Member,
    op: &Operation,
) -> Result<Option<Vec<ghostline_red::world::CollisionBox>>> {
    let m = member_matrix(project, member);
    let (scale, q, center) = m.to_scale_rotation_translation();
    let bounds = member
        .bounds
        .as_ref()
        .context("Collision box dimensions absent")?;
    let half = Vec3::from_array(bounds.dimensions) * scale.abs() / 2.;
    let local = Mat4::from_rotation_translation(q, center).inverse() * brush_matrix(op);
    let (_, rotation, cut_center) = local.to_scale_rotation_translation();
    let axes = [rotation * Vec3::X, rotation * Vec3::Y, rotation * Vec3::Z];
    if !axes.iter().all(|a| a.abs().max_element() > 0.9999) {
        return Ok(None);
    }
    let cut_half = axes[0].abs() * op.size[0] / 2.
        + axes[1].abs() * op.size[1] / 2.
        + axes[2].abs() * op.size[2] / 2.;
    let low = (-half).max(cut_center - cut_half);
    let high = half.min(cut_center + cut_half);
    ensure!(
        low.cmplt(high).all(),
        "Change volume does not intersect collision {}",
        member.id
    );
    let mut fragments = Vec::new();
    let mut remaining_low = -half;
    let mut remaining_high = half;
    for axis in 0..3 {
        if low[axis] - remaining_low[axis] > 0.001 {
            let mut upper = remaining_high;
            upper[axis] = low[axis];
            fragments.push((remaining_low, upper));
            remaining_low[axis] = low[axis];
        }
        if remaining_high[axis] - high[axis] > 0.001 {
            let mut lower = remaining_low;
            lower[axis] = high[axis];
            fragments.push((lower, remaining_high));
            remaining_high[axis] = high[axis];
        }
    }
    let anchor = Quat::from_rotation_z(project.yaw.to_radians());
    Ok(Some(
        fragments
            .into_iter()
            .map(|(low, high)| ghostline_red::world::CollisionBox {
                position: (Vec3::from_array(project.origin)
                    + anchor * (center + q * ((low + high) / 2.)))
                    .to_array(),
                rotation: (anchor * q).to_array(),
                size: (high - low).to_array(),
            })
            .collect(),
    ))
}

pub fn visible(
    compiled: &crate::model::Compiled,
    p: &crate::model::Placement,
    options: &crate::preview::Options,
) -> bool {
    let member = compiled
        .project
        .shells
        .iter()
        .flat_map(|s| &s.members)
        .find(|m| m.object.as_ref() == Some(&p.id));
    if let Some(m) = member {
        if options.hidden_layers.contains(&m.layer) {
            return false;
        }
        if options.comparison == "proposed" && m.removed
            || options.comparison == "changes" && !m.removed
        {
            return false;
        }
        if let Some(level) = options.level {
            let floor = compiled
                .project
                .shells
                .iter()
                .flat_map(|s| &s.floors)
                .find(|f| f.level == level);
            if let (Some(f), Some(b)) = (floor, local_bounds(&compiled.project, m))
                && (b.max[2] < f.position[2] - 0.2 || b.min[2] > f.position[2] + f.size[2] + 0.2)
            {
                return false;
            }
        }
    } else if options.comparison == "existing" || options.level.is_some_and(|l| p.level != l) {
        return false;
    }
    !(options.cutaway && p.cutaway)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_wall_cut_preserves_outside_volume_and_native_orientation() {
        let mut project: Project =
            serde_json::from_str(include_str!("../examples/workshop.building.json")).unwrap();
        project.origin = [12., -20., 5.];
        project.yaw = 37.;
        let rotation = Quat::from_rotation_z(project.yaw.to_radians());
        let member:Member=serde_json::from_value(json!({"id":"wall","sector":"fixture","index":850,"instance":null,"actor":119,"shape":0,"kind":"worldCollisionNode","layer":"collision","label":"shared wall","resource":null,"appearance":"","position":(Vec3::from_array(project.origin)+rotation*Vec3::new(0.,0.,2.)).to_array(),"rotation":rotation.to_array(),"scale":[1.,1.,1.],"bounds":{"min":[-3.,-0.1,-2.],"max":[3.,0.1,2.],"dimensions":[6.,0.2,4.]},"object":null,"native":{"shape_code":0}})).unwrap();
        let operation:Operation=serde_json::from_value(json!({"id":"entrance","label":"Entrance","kind":"entrance","members":["wall"],"position":[0.,0.,0.],"size":[1.2,1.,2.3],"yaw":0.,"preset":"hinged_e"})).unwrap();
        let fragments = cut_collision(&project, &member, &operation)
            .unwrap()
            .unwrap();
        let volume: f32 = fragments
            .iter()
            .map(|b| b.size.iter().product::<f32>())
            .sum();
        assert!((volume - (6. * 0.2 * 4. - 1.2 * 0.2 * 2.3)).abs() < 0.0001);
        assert_eq!(fragments.len(), 3);
        for b in fragments {
            let native =
                ghostline_red::world::encode_boxes(std::slice::from_ref(&b), "fragment").unwrap();
            let decoded = ghostline_red::world::collision_shapes(&native).unwrap();
            assert!(
                (Vec3::from_array(decoded[0].position) - Vec3::from_array(b.position)).length()
                    < 0.0001
            );
            assert!(Quat::from_array(decoded[0].rotation).dot(rotation).abs() > 0.9999);
        }
    }
}
