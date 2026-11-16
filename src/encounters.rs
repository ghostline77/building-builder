//! Workstation actor communities, native terminals and quest-builder bindings.
use crate::{
    model::{Compiled, Project, identifier},
    technical::{NativeNode, node_hash, typed},
};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    pub record: String,
    pub appearance: String,
    pub workspot: String,
    pub offset: [f32; 3],
    pub yaw_offset: f32,
    pub active_on_start: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Computer {
    pub offset: [f32; 3],
    pub yaw_offset: f32,
    pub title: String,
    pub text: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Workstation {
    pub id: String,
    pub desk: String,
    pub chair: String,
    pub actor: Actor,
    pub computer: Computer,
}
pub fn validate(p: &Project) -> Result<()> {
    ensure!(p.workstations.len() <= 8, "At most eight workstations");
    let mut ids = BTreeSet::new();
    for w in &p.workstations {
        identifier(&w.id)?;
        ensure!(ids.insert(&w.id), "Duplicate workstation identity");
        ensure!(
            w.desk != w.chair
                && [&w.desk, &w.chair]
                    .iter()
                    .all(|id| p.objects.iter().any(|o| o.id == ***id && !o.reference_only)),
            "Workstation requires distinct placed desk and chair objects"
        );
        ensure!(
            w.actor.record.strip_prefix("Character.").is_some_and(
                |s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
            ),
            "Invalid actor character record"
        );
        ensure!(
            w.actor.workspot.starts_with(r"base\workspots\")
                && w.actor.workspot.ends_with(".workspot")
                && !w.actor.workspot.contains(".."),
            "Actor needs a native workspot"
        );
        ensure!(
            w.actor
                .offset
                .iter()
                .chain(&w.computer.offset)
                .all(|v| v.is_finite() && v.abs() <= 3.)
                && w.actor.yaw_offset.is_finite()
                && w.computer.yaw_offset.is_finite()
                && w.computer.title.len() <= 200
                && w.computer.text.len() <= 16000,
            "Invalid workstation offset or document content"
        );
    }
    Ok(())
}
fn resource(path: &str) -> Value {
    json!({"DepotPath":typed("ResourcePath",path),"Flags":"Default"})
}
fn reference(path: &str) -> Value {
    json!({"$type":"NodeRef","$storage":"uint64","$value":node_hash(path).to_string()})
}
fn wrap(index: &mut u32, data: Value) -> Value {
    *index += 1;
    json!({"HandleId":index.to_string(),"Data":data})
}
pub fn bindings(c: &Compiled) -> Value {
    let items:Vec<_>=c.project.workstations.iter().map(|w|{
        let prefix=format!("$/03_night_city/#building_builder_{}/workstation_{}",c.project.id,w.id);
        json!({"id":w.id,"community":format!("{prefix}_community"),"entry":"worker","phase":"default","workspot":format!("{prefix}_seat"),"computer":format!("{prefix}_computer"),"character":w.actor.record,"read_fact":format!("{}_{}_file_read",c.project.id.replace('-',"_"),w.id.replace('-',"_")),"active_on_start":w.actor.active_on_start})
    }).collect();
    json!({"schema_version":1,"layout_id":c.project.id,"area":format!("$/03_night_city/#building_builder_{}/office_area",c.project.id),"workstations":items})
}
pub fn quest_fragment(c: &Compiled) -> Value {
    let binding = bindings(c);
    let items = binding["workstations"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    json!({"schema_version":1,"kind":"building-quest-binding-fragment","layout_id":c.project.id,"area_trigger":binding["area"],"actors":items.iter().map(|w|json!({"community":w["community"],"entry":w["entry"],"phase":w["phase"],"workspot":w["workspot"],"requires_character_spawned_gate":true})).collect::<Vec<_>>(),"terminal_stages":items.iter().map(|w|json!({"id":format!("read_{}_file",w["id"].as_str().unwrap_or("")),"type":"read_terminal_document","computer":w["computer"],"completion_fact":w["read_fact"]})).collect::<Vec<_>>(),"required_from_owning_quest":["phase_resource","objective journal path","activation and completion lifecycle"],"cleanup_communities":items.iter().map(|w|w["community"].clone()).collect::<Vec<_>>()})
}
pub fn nodes(c: &Compiled) -> Result<Vec<NativeNode>> {
    let mut out = Vec::new();
    let mut handle = 300000;
    let bindings = bindings(c);
    for (index, w) in c.project.workstations.iter().enumerate() {
        let b = &bindings["workstations"][index];
        let community = b["community"]
            .as_str()
            .context("Missing community identity")?;
        let seat = b["workspot"].as_str().context("Missing seat identity")?;
        let computer = b["computer"]
            .as_str()
            .context("Missing computer identity")?;
        let position = |id: &str, offset: [f32; 3], yaw: f32| -> Result<(Vec3, f32)> {
            let anchor = c
                .project
                .objects
                .iter()
                .find(|o| o.id == id)
                .context("Workstation anchor absent")?;
            Ok((
                Vec3::from_array(anchor.position)
                    + Quat::from_rotation_z(anchor.yaw.to_radians()) * Vec3::from_array(offset),
                anchor.yaw + yaw,
            ))
        };
        let (actor, actor_yaw) = position(&w.chair, w.actor.offset, w.actor.yaw_offset)?;
        let (terminal, terminal_yaw) = position(&w.desk, w.computer.offset, w.computer.yaw_offset)?;
        let world = Quat::from_rotation_z(c.project.yaw.to_radians()) * actor
            + Vec3::from_array(c.project.origin);
        let spot = wrap(
            &mut handle,
            json!({"$type":"AIActionSpot","ActorBodytypeE3":"Undefined","clippingSpaceOrientation":180.,"clippingSpaceRange":120.,"enabledWhenMasterOccupied":0,"resource":resource(&w.actor.workspot),"snapToGround":0,"useClippingSpace":0}),
        );
        out.push(NativeNode{node:wrap(&mut handle,json!({"$type":"worldAISpotNode","debugName":typed("CName",seat),"isHostOnly":0,"isVisibleInGame":1,"isWorkspotInfinite":1,"isWorkspotStatic":1,"disableBumps":0,"spot":spot,"markings":[],"tag":"None","tagExt":"None"})),position:actor.to_array(),yaw:actor_yaw,radius:3.,reference:Some(seat.into()),scale:[1.;3]});
        let area = wrap(
            &mut handle,
            json!({"$type":"communityArea","entriesData":[{"$type":"communityCommunityEntrySpotsData","entryName":typed("CName","worker"),"phasesData":[{"$type":"communityCommunityEntryPhaseSpotsData","entryPhaseName":typed("CName","default"),"timePeriodsData":[{"$type":"communityCommunityEntryPhaseTimePeriodData","isSequence":0,"periodName":typed("CName","Day"),"spotNodeIds":[{"$type":"worldGlobalNodeID","hash":node_hash(seat).to_string()}]}]}]}]}),
        );
        out.push(NativeNode{node:wrap(&mut handle,json!({"$type":"worldCompiledCommunityAreaNode_Streamable","debugName":typed("CName",community),"area":area,"sourceObjectId":{"$type":"entEntityID","hash":node_hash(community).to_string()},"streamingDistance":0.,"isHostOnly":0,"isVisibleInGame":1})),position:actor.to_array(),yaw:0.,radius:3.,reference:Some(community.into()),scale:[1.;3]});
        let phase = wrap(
            &mut handle,
            json!({"$type":"communitySpawnPhase","alwaysSpawned":"true","appearances":[typed("CName",&w.actor.appearance)],"phaseName":typed("CName","default"),"prefetchAppearance":0,"timePeriods":[{"$type":"communityPhaseTimePeriod","categories":[],"hour":"Day","isSequence":0,"markings":[],"quantity":1,"spotNodeRefs":[reference(seat)]}]}),
        );
        let entry = wrap(
            &mut handle,
            json!({"$type":"communitySpawnEntry","characterRecordId":typed("TweakDBID",&w.actor.record),"entryName":typed("CName","worker"),"initializers":[],"phases":[phase],"spawnInView":"default__true_"}),
        );
        let template = wrap(
            &mut handle,
            json!({"$type":"communityCommunityTemplateData","crowdEntries":[],"entries":[entry],"spawnSetReference":typed("CName","None")}),
        );
        out.push(NativeNode{node:wrap(&mut handle,json!({"$type":"worldCommunityRegistryNode","debugName":typed("CName",&format!("{community}_registry")),"communitiesData":[{"$type":"worldCommunityRegistryItem","communityAreaType":"Regular","communityId":{"$type":"gameCommunityID","entityId":{"$type":"entEntityID","hash":node_hash(community).to_string()}},"entriesInitialState":[{"$type":"worldCommunityEntryInitialState","entryActiveOnStart":u8::from(w.actor.active_on_start),"entryName":typed("CName","worker"),"initialPhaseName":typed("CName","default")}],"template":template}],"representsCrowd":0,"spawnSetNameToCommunityID":{"$type":"gameCommunitySpawnSetNameToID","entries":[]},"workspotsPersistentData":[{"$type":"AISpotPersistentData","globalNodeId":{"$type":"worldGlobalNodeID","hash":node_hash(seat).to_string()},"isEnabled":1,"worldPosition":{"$type":"WorldPosition","x":{"$type":"FixedPoint","Bits":(world.x*131072.).round() as i32},"y":{"$type":"FixedPoint","Bits":(world.y*131072.).round() as i32},"z":{"$type":"FixedPoint","Bits":(world.z*131072.).round() as i32}},"yaw":actor_yaw+c.project.yaw}],"isHostOnly":0,"isVisibleInGame":1})),position:actor.to_array(),yaw:0.,radius:3.,reference:None,scale:[1.;3]});
        let checks = wrap(&mut handle, json!({"$type":"HackEngContainer"}));
        let ps = wrap(
            &mut handle,
            json!({"$type":"ComputerControllerPS","contentScale":{"$type":"TweakDBID","$storage":"uint64","$value":"164817071718"},"terminalSkillChecks":checks,"computerSetup":{"$type":"ComputerSetup","mailsStructure":[],"filesStructure":[{"$type":"gamedeviceGenericDataContent","name":"Documents","content":[{"$type":"gamedeviceDataElement","title":w.computer.title,"content":w.computer.text,"wasRead":0,"questInfo":{"$type":"gamedeviceQuestInfo","isHighlighted":1,"factName":typed("CName",b["read_fact"].as_str().unwrap_or(""))}}]}]}}),
        );
        let instance = wrap(
            &mut handle,
            json!({"$type":"entEntityInstanceData","buffer":{"BufferId":(40000+index).to_string(),"Flags":4063232,"Type":"WolvenKit.RED4.Types.RedPackage","Data":{"Version":4,"Sections":7,"CruidIndex":0,"CruidDict":{"0":"0","1":"1131680419258347532"},"Chunks":[{"$type":"Computer","contentScale":{"$type":"TweakDBID","$storage":"uint64","$value":"164817071718"}},{"$type":"ComputerController","persistentState":ps}]}}}),
        );
        out.push(NativeNode{node:wrap(&mut handle,json!({"$type":"worldDeviceNode","debugName":typed("CName",computer),"entityTemplate":resource(r"base\gameplay\devices\masters\computers\computer_1.ent"),"appearanceName":typed("CName","computer_computer_1"),"deviceClassName":typed("CName","ComputerControllerPS"),"instanceData":instance,"deviceConnections":[],"isHostOnly":0,"isVisibleInGame":1})),position:terminal.to_array(),yaw:terminal_yaw,radius:3.,reference:Some(computer.into()),scale:[1.;3]});
    }
    if !c.project.workstations.is_empty() {
        let mut low = Vec3::splat(f32::INFINITY);
        let mut high = Vec3::splat(f32::NEG_INFINITY);
        for object in &c.project.objects {
            let point = Vec3::from_array(object.position);
            low = low.min(point - Vec3::new(2., 2., 0.));
            high = high.max(point + Vec3::new(2., 2., 3.));
        }
        let size = high - low;
        let center = (low + high) * 0.5;
        let mut bytes = 4u32.to_le_bytes().to_vec();
        for [x, y] in [
            [-size.x / 2., -size.y / 2.],
            [size.x / 2., -size.y / 2.],
            [size.x / 2., size.y / 2.],
            [-size.x / 2., size.y / 2.],
        ] {
            for v in [x, y, 0., 1.] {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
        }
        bytes.extend_from_slice(&size.z.to_le_bytes());
        let outline = wrap(
            &mut handle,
            json!({"$type":"AreaShapeOutline","buffer":STANDARD.encode(bytes)}),
        );
        let notifier = wrap(
            &mut handle,
            json!({"$type":"questTriggerNotifier_Quest","excludeChannels":0,"includeChannels":"TC_Default","isEnabled":1}),
        );
        out.push(NativeNode{node:wrap(&mut handle,json!({"$type":"worldTriggerAreaNode","debugName":typed("CName","office_area"),"outline":outline,"notifiers":[notifier],"isHostOnly":0,"isVisibleInGame":1})),position:[center.x,center.y,low.z],yaw:0.,radius:size.length(),reference:bindings["area"].as_str().map(str::to_owned),scale:[1.;3]});
    }
    Ok(out)
}
