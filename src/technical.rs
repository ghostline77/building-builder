use crate::model::{Compiled, Opening, Project, identifier, kit};
use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Light {
    pub id: String,
    pub position: [f32; 3],
    pub color: [u8; 3],
    pub radius: f32,
    pub intensity: f32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AudioArea {
    pub id: String,
    pub position: [f32; 3],
    pub size: [f32; 3],
    pub preset: String,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DoorState {
    #[default]
    Closed,
    Opened,
    Locked,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct DoorDevice {
    pub preset: String,
    pub offset: [f32; 3],
    pub yaw_offset: f32,
    pub scale: [f32; 3],
    /// Explicit shell aperture placement; absent for a grid wall bay.
    pub position: Option<[f32; 3]>,
    pub yaw: f32,
    pub endpoints: Option<[[f32; 3]; 2]>,
    pub manual: bool,
}
impl Default for DoorDevice {
    fn default() -> Self {
        Self {
            preset: "sliding_entropy_c".into(),
            offset: [0.; 3],
            yaw_offset: 0.,
            scale: [1.; 3],
            position: None,
            yaw: 0.,
            endpoints: None,
            manual: false,
        }
    }
}
pub fn door_preset(name: &str) -> Result<(&'static str, &'static str, f32, &'static str)> {
    Ok(match name {
        "sliding_entropy_c" => (
            r"base\gameplay\devices\doors\final\single_door.ent",
            "doors_single_entropy_c",
            0.75,
            "1193887807258071040",
        ),
        "sliding_kitsch_double_f" => (
            r"base\gameplay\devices\doors\final\double_door.ent",
            "doors_double_kitsch_f",
            0.,
            "1193887807258071040",
        ),
        "hinged_h" => (
            r"base\gameplay\devices\doors\final\single_door_hinged.ent",
            "doors_single_hinged_h",
            89129. / 131072.,
            "1193887807258071040",
        ),
        "hinged_e" => (
            r"base\gameplay\devices\doors\final\single_door_hinged.ent",
            "doors_single_hinged_e",
            89129. / 131072.,
            "1193887807258071040",
        ),
        "garage_w300_h250" => (
            r"base\gameplay\devices\doors\gates\shutter_window_w300_h250.ent",
            "shutter_w300_h250_default",
            0.,
            "1563417374541762560",
        ),
        _ => anyhow::bail!("Unknown door preset {name}"),
    })
}

pub struct NativeNode {
    pub node: Value,
    pub position: [f32; 3],
    pub yaw: f32,
    pub radius: f32,
    pub reference: Option<String>,
    pub scale: [f32; 3],
}

pub fn validate(p: &Project) -> Result<()> {
    ensure!(
        p.lights.len() <= 64 && p.audio_areas.len() <= 32,
        "Too many lights or audio areas"
    );
    let mut ids = BTreeSet::new();
    for l in &p.lights {
        identifier(&l.id)?;
        ensure!(ids.insert(&l.id), "Duplicate technical layer identity");
        ensure!(
            l.position.iter().all(|v| v.is_finite() && v.abs() <= 8000.)
                && l.radius.is_finite()
                && (0.1..=20.).contains(&l.radius)
                && l.intensity.is_finite()
                && (0.1..=1000.).contains(&l.intensity),
            "Invalid practical light"
        );
    }
    for a in &p.audio_areas {
        identifier(&a.id)?;
        ensure!(ids.insert(&a.id), "Duplicate technical layer identity");
        ensure!(
            a.position.iter().all(|v| v.is_finite() && v.abs() <= 8000.)
                && a.size
                    .iter()
                    .all(|v| v.is_finite() && (0.1..=100.).contains(v))
                && ["residential-ground", "residential-upper"].contains(&a.preset.as_str()),
            "Invalid audio area or preset"
        );
    }
    for (key, device) in &p.door_devices {
        door_preset(&device.preset)?;
        ensure!(
            device.offset.iter().all(|v| v.is_finite() && v.abs() <= 3.)
                && device.yaw_offset.is_finite()
                && device.yaw.is_finite()
                && device
                    .scale
                    .iter()
                    .all(|v| v.is_finite() && (0.5..=2.).contains(v)),
            "Invalid door transform"
        );
        if let Some(position) = device.position {
            identifier(key)?;
            ensure!(
                position.iter().all(|v| v.is_finite() && v.abs() <= 1000.),
                "Invalid shell door position"
            );
            let endpoints = device
                .endpoints
                .ok_or_else(|| anyhow::anyhow!("Shell door requires explicit endpoints"))?;
            ensure!(
                endpoints.iter().flatten().all(|v| v.is_finite()),
                "Invalid shell door endpoints"
            );
        } else {
            ensure!(
                matches!(
                    p.openings.get(key),
                    Some(Opening::DoorAuto | Opening::DoorManual)
                ),
                "Door preset belongs to a non-device wall bay"
            );
            ensure!(
                !["sliding_kitsch_double_f", "garage_w300_h250"].contains(&device.preset.as_str()),
                "Wide door presets require an explicit shell aperture"
            );
        }
    }
    for key in p.door_states.keys() {
        ensure!(
            matches!(
                p.openings.get(key),
                Some(Opening::DoorAuto | Opening::DoorManual)
            ) || p
                .door_devices
                .get(key)
                .is_some_and(|d| d.position.is_some()),
            "Door state belongs to a non-device wall bay"
        );
    }
    Ok(())
}

pub fn typed(kind: &str, value: &str) -> Value {
    json!({"$type":kind,"$storage":"string","$value":value})
}
fn vector(p: [f32; 3]) -> Value {
    json!({"$type":"Vector3","X":p[0],"Y":p[1],"Z":p[2]})
}
fn resource(path: &str) -> Value {
    json!({"DepotPath":typed("ResourcePath",path),"Flags":"Default"})
}
pub fn node_hash(reference: &str) -> u64 {
    reference
        .encode_utf16()
        .filter(|c| *c != u16::from(b'#'))
        .fold(0xcbf29ce484222325, |h, c| {
            (h ^ u64::from(c)).wrapping_mul(0x100000001b3)
        })
}
struct Handles(u64);
impl Handles {
    fn wrap(&mut self, data: Value) -> Value {
        let id = self.0;
        self.0 += 1;
        json!({"HandleId":id.to_string(),"Data":data})
    }
}

fn outline(h: &mut Handles, size: [f32; 3]) -> Value {
    let [w, d, height] = size;
    let points = [
        [-w / 2., -d / 2., 0.],
        [w / 2., -d / 2., 0.],
        [w / 2., d / 2., 0.],
        [-w / 2., d / 2., 0.],
    ];
    let mut bytes = 4_u32.to_le_bytes().to_vec();
    for point in points {
        for value in [point[0], point[1], point[2], 1.] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes.extend_from_slice(&height.to_le_bytes());
    h.wrap(json!({"$type":"AreaShapeOutline","buffer":STANDARD.encode(bytes),"height":height,"points":points.map(vector)}))
}

pub fn nodes(c: &Compiled) -> Result<Vec<NativeNode>> {
    let mut out = Vec::new();
    let mut h = Handles(100000);
    let k = kit(&c.project.kit)?;
    for l in &c.project.lights {
        out.push(NativeNode{node:h.wrap(json!({"$type":"worldStaticLightNode","debugName":typed("CName",&l.id),"color":{"$type":"Color","Red":l.color[0],"Green":l.color[1],"Blue":l.color[2],"Alpha":255},"radius":l.radius,"intensity":l.intensity,"autoHideDistance":100.})),position:l.position,yaw:0.,radius:l.radius,reference:None,scale:[1.;3]});
    }
    for a in &c.project.audio_areas {
        let (value, event) = if a.preset == "residential-ground" {
            (
                0.9800000190734863,
                "amb_int_roomtone_apartment_large_warm_airy_01",
            )
        } else {
            (
                0.8999999761581421,
                "amb_int_roomtone_appartment_med_warm_02",
            )
        };
        let settings=h.wrap(json!({"$type":"audioAmbientAreaSettings","MetadataParent":typed("CName","amb_area_int_default_corridors"),"Priority":10,"EventsOnActive":[{"$type":"audioAudEventStruct","event":typed("CName",event)}],"Parameters":[{"$type":"audioAudParameter","name":typed("CName","amb_interior"),"value":value}],"Reverb":typed("CName","revb_interior_room_medium")}));
        let notifier=h.wrap(json!({"$type":"audioAmbientAreaNotifier","includeChannels":"TC_SoundAmbientArea","isEnabled":1,"occlusionEnabled":1,"Settings":settings}));
        let shape = outline(&mut h, a.size);
        out.push(NativeNode{node:h.wrap(json!({"$type":"worldAmbientAreaNode","debugName":typed("CName",&a.id),"outline":shape,"notifiers":[notifier]})),position:a.position,yaw:0.,radius:a.size.into_iter().fold(0.,f32::max),reference:None,scale:[1.;3]});
    }
    if c.project.interior {
        for room in &c.project.shell_rooms {
            let shape = outline(&mut h, room.size);
            let notifier=h.wrap(json!({"$type":"worldInteriorAreaNotifier","excludeChannels":0,"gameRestrictionIDs":[],"includeChannels":"TC_Player","isEnabled":1,"setTier2":0,"treatAsInterior":1}));
            let q = glam::Quat::from_rotation_z(room.yaw.to_radians());
            let center = glam::Vec3::from_array(room.position)
                + q * glam::Vec3::new(room.size[0] / 2., room.size[1] / 2., 0.);
            out.push(NativeNode{node:h.wrap(json!({"$type":"worldTriggerAreaNode","debugName":typed("CName",&format!("interior_{}",room.id)),"isHostOnly":0,"isVisibleInGame":1,"notifiers":[notifier],"outline":shape,"sourcePrefabHash":"0","tag":"None","tagExt":"None"})),position:center.to_array(),yaw:room.yaw,radius:room.size.into_iter().fold(0.,f32::max),reference:None,scale:[1.;3]});
        }
        for room in &c.project.rooms {
            let shape = outline(
                &mut h,
                [
                    room.width as f32 * 3.,
                    room.depth as f32 * 3.,
                    k.wall_height,
                ],
            );
            let notifier=h.wrap(json!({"$type":"worldInteriorAreaNotifier","excludeChannels":0,"gameRestrictionIDs":[],"includeChannels":"TC_Player","isEnabled":1,"setTier2":0,"treatAsInterior":1}));
            out.push(NativeNode{node:h.wrap(json!({"$type":"worldTriggerAreaNode","debugName":typed("CName",&format!("interior_{}",room.id)),"isHostOnly":0,"isVisibleInGame":1,"notifiers":[notifier],"outline":shape,"sourcePrefabHash":"0","tag":"None","tagExt":"None"})),position:[(room.x as f32+room.width as f32/2.)*3.,(room.y as f32+room.depth as f32/2.)*3.,room.level as f32*k.storey_height],yaw:0.,radius:room.width.max(room.depth) as f32*3.,reference:None,scale:[1.;3]});
        }
    }
    let mut openings = c.project.openings.clone();
    for (id, device) in &c.project.door_devices {
        if device.position.is_some() {
            openings.insert(
                id.clone(),
                if device.manual {
                    Opening::DoorManual
                } else {
                    Opening::DoorAuto
                },
            );
        }
    }
    for (key, mode) in &openings {
        let mode = match mode {
            Opening::DoorAuto => "AUTOMATIC",
            Opening::DoorManual => "INTERACTIVE",
            _ => continue,
        };
        let device = c.project.door_devices.get(key).cloned().unwrap_or_default();
        let (template, appearance, hinge, controller) = door_preset(&device.preset)?;
        let visual = c.placements.iter().find(|p| p.id == format!("wall:{key}"));
        let visual_yaw = visual.map_or(device.yaw, |v| v.yaw);
        let angle = visual_yaw.to_radians();
        let middle = if let Some(position) = device.position {
            position
        } else {
            let visual = visual.ok_or_else(|| anyhow::anyhow!("Door wall is absent"))?;
            [
                visual.position[0] - 1.5 * angle.cos(),
                visual.position[1] - 1.5 * angle.sin(),
                visual.position[2],
            ]
        };
        let depth = if c.edges.iter().any(|e| e.id == *key && e.shared) {
            k.wall_thickness / 2.
        } else {
            0.
        };
        let yaw = visual_yaw + device.yaw_offset;
        let facing = yaw.to_radians();
        let point = if let Some(position) = device.position {
            position
        } else {
            [
                middle[0] + hinge * facing.cos() - depth * angle.sin() + device.offset[0],
                middle[1] + hinge * facing.sin() + depth * angle.cos() + device.offset[1],
                middle[2] + device.offset[2],
            ]
        };
        let reference = format!(
            "$/03_night_city/#building_builder_{}/door_{}",
            c.project.id,
            key.replace(':', "_")
        );
        let connection = format!("{reference}_connection");
        let state = match c.project.door_states.get(key).copied().unwrap_or_default() {
            DoorState::Closed => "CLOSED",
            DoorState::Opened => "OPENED",
            DoorState::Locked => "LOCKED",
        };
        let mut ps=h.wrap(json!({"$type":"DoorControllerPS","doorProperties":{"$type":"DoorSetup","doorType":mode,"doorTypeSideOne":mode,"doorTypeSideTwo":mode,"initialDoorState":state,"automaticallyClosesItself":1,"canPlayerToggleLockState":0,"openingSpeed":1,"doorOpeningTime":1}}));
        if device.preset == "garage_w300_h250" {
            ps["Data"]["doorProperties"]["isShutter"] = json!(1);
            ps["Data"]["doorProperties"]["doorOpeningTime"] = json!(1.3);
        }
        let mut chunks = vec![
            json!({"$type":"DoorController","name":typed("CName","controller"),"id":controller,"persistentState":ps}),
            json!({"$type":"AIOffMeshConnectionComponent","name":typed("CName","offMeshConnection"),"id":"1455904520967634944","offMeshConnectionNodesRefs":[{"$type":"NodeRef","$storage":"uint64","$value":node_hash(&connection).to_string()}]}),
        ];
        if device.preset == "garage_w300_h250" && device.scale != [1.; 3] {
            chunks.push(json!({"$type":"entPhysicalMeshComponent","name":typed("CName","entropy_shutter_door_frame_w300_h_250_a"),"id":"2302343969809895424","visualScale":{"$type":"Vector3","X":device.scale[0],"Y":device.scale[1],"Z":device.scale[2]}}));
            chunks.push(json!({"$type":"entAnimatedComponent","name":typed("CName","anim"),"id":"2380823018022113280","rig":{"DepotPath":typed("ResourcePath",&format!("mod\\building_builder\\{}\\door_{}.rig",c.project.id,key)),"Flags":"Obligatory"}}));
            chunks.push(json!({"$type":"entColliderComponent","name":typed("CName","player_blocker"),"id":"2678189905129508864","localTransform":{"$type":"WorldTransform","Position":{"$type":"WorldPosition","x":{"$type":"FixedPoint","Bits":0},"y":{"$type":"FixedPoint","Bits":0},"z":{"$type":"FixedPoint","Bits":(1.25*device.scale[2]*131072.).round() as i32}},"Orientation":{"$type":"Quaternion","i":0.,"j":0.,"k":0.,"r":1.}},"colliders":[h.wrap(json!({"$type":"physicsColliderBox","halfExtents":{"$type":"Vector3","X":1.5*device.scale[0],"Y":0.02*device.scale[1],"Z":1.25*device.scale[2]}}))]}));
        }
        let cruid: serde_json::Map<String, Value> = chunks
            .iter()
            .enumerate()
            .map(|(i, c)| (i.to_string(), c["id"].clone()))
            .collect();
        let instance=h.wrap(json!({"$type":"entEntityInstanceData","buffer":{"BufferId":(20000+out.len()).to_string(),"Flags":4063232,"Type":"WolvenKit.RED4.Types.RedPackage","Data":{"Version":4,"Sections":7,"CruidIndex":-1,"CruidDict":cruid,"Chunks":chunks}}}));
        out.push(NativeNode{node:h.wrap(json!({"$type":"worldDeviceNode","debugName":typed("CName",&reference),"entityTemplate":resource(template),"appearanceName":typed("CName",appearance),"deviceClassName":typed("CName","DoorControllerPS"),"instanceData":instance,"deviceConnections":[],"entityLod":0,"isHostOnly":0,"isVisibleInGame":1})),position:point,yaw,radius:4.,reference:Some(reference),scale:device.scale});
        let mut points = Vec::new();
        let endpoints = device.endpoints.unwrap_or_else(|| {
            [-1., 1.].map(|direction| {
                [
                    middle[0] - direction * 0.8 * angle.sin(),
                    middle[1] + direction * 0.8 * angle.cos(),
                    middle[2],
                ]
            })
        });
        ensure!(
            (glam::Vec3::from_array(endpoints[0]) - glam::Vec3::from_array(endpoints[1])).length()
                <= 4.
                && (endpoints[0][2] - endpoints[1][2]).abs() <= 0.6,
            "Door connection is too long or steep"
        );
        let centre = if device.position.is_some() {
            [
                point[0] - hinge * facing.cos() * device.scale[0],
                point[1] - hinge * facing.sin() * device.scale[0],
            ]
        } else {
            [
                middle[0] - depth * angle.sin(),
                middle[1] + depth * angle.cos(),
            ]
        };
        let frame = |p: [f32; 3]| {
            let x = p[0] - centre[0];
            let y = p[1] - centre[1];
            [
                x * facing.cos() + y * facing.sin(),
                -x * facing.sin() + y * facing.cos(),
            ]
        };
        let [first, last] = endpoints.map(frame);
        ensure!(
            first[1] * last[1] < 0.,
            "Door endpoints must straddle its aperture plane"
        );
        let t = -first[1] / (last[1] - first[1]);
        let crossing = first[0] + (last[0] - first[0]) * t;
        let half_width =
            if ["sliding_kitsch_double_f", "garage_w300_h250"].contains(&device.preset.as_str()) {
                1.5
            } else {
                0.75
            } * device.scale[0];
        ensure!(
            crossing.abs() <= half_width - 0.15,
            "Door connection crosses outside the clear aperture"
        );
        for endpoint in endpoints {
            // Off-mesh spline placement has zero yaw, so express world-rotated
            // endpoints relative to the device without rotating them twice.
            let local = glam::Vec3::new(
                endpoint[0] - point[0],
                endpoint[1] - point[1],
                endpoint[2] - point[2],
            );
            let local = glam::Quat::from_rotation_z(c.project.yaw.to_radians()) * local;
            points.push(json!({"$type":"SplinePoint","automaticTangents":0,"continuousTangents":1,"id":0,"position":vector(local.to_array()),"rotation":{"$type":"Quaternion","i":0.,"j":0.,"k":0.,"r":1.},"tangents":{"Elements":[vector([0.;3]),vector([0.;3])]}}));
        }
        let spline = h.wrap(
            json!({"$type":"Spline","hasDirection":1,"looped":0,"points":points,"reversed":0}),
        );
        out.push(NativeNode{node:h.wrap(json!({"$type":"worldOffMeshConnectionNode","debugName":typed("CName",&connection),"type":"Door","radius":0.75,"isBidirectional":1,"isHostOnly":0,"isVisibleInGame":1,"sourcePrefabHash":"0","splineData":spline,"tag":"None","tagExt":"None"})),position:point,yaw:-c.project.yaw,radius:4.,reference:Some(connection),scale:[1.;3]});
    }
    for connection in &c.project.navigation.connections {
        let reference = format!(
            "$/03_night_city/#building_builder_{}/entrance_{}",
            c.project.id, connection.id
        );
        let delta = glam::Quat::from_rotation_z(c.project.yaw.to_radians())
            * (glam::Vec3::from_array(connection.to) - glam::Vec3::from_array(connection.from));
        let points=[[0.;3],delta.to_array()].map(|p|json!({"$type":"SplinePoint","automaticTangents":0,"continuousTangents":1,"id":0,"position":vector(p),"rotation":{"$type":"Quaternion","i":0.,"j":0.,"k":0.,"r":1.},"tangents":{"Elements":[vector([0.;3]),vector([0.;3])]}}));
        let spline = h.wrap(
            json!({"$type":"Spline","hasDirection":1,"looped":0,"points":points,"reversed":0}),
        );
        out.push(NativeNode{node:h.wrap(json!({"$type":"worldOffMeshConnectionNode","debugName":typed("CName",&reference),"type":"Floor","radius":0.75,"isBidirectional":1,"isHostOnly":0,"isVisibleInGame":1,"sourcePrefabHash":"0","splineData":spline,"tag":"None","tagExt":"None"})),position:connection.from,yaw:-c.project.yaw,radius:4.,reference:Some(reference),scale:[1.;3]});
    }
    out.extend(crate::encounters::nodes(c)?);
    Ok(out)
}
