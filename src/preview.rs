use std::{collections::BTreeMap, io::Cursor, sync::Arc};

use anyhow::{Context, Result};
use ghostline_red::assets::ResourceKey;
use ghostline_renderer::{
    Camera, GameSource, Node, PreparedScene, RenderOptions, Renderer, Scene, View,
};
use serde::Deserialize;

use crate::model::{Compiled, placement_transform, transform};

pub fn inspect(compiled: &Compiled, source: &GameSource, out: &std::path::Path) -> Result<()> {
    source.validate_output(out)?;
    std::fs::create_dir_all(out)?;
    let mut subjects = Vec::new();
    for placement in &compiled.placements {
        if placement.entity {
            let key = ResourceKey::parse(&placement.mesh);
            std::fs::write(
                out.join(format!("entity-{}.json", key.native_hash())),
                serde_json::to_vec_pretty(&*source.resources().document(&key)?)?,
            )?;
            let native = source.resolve_assembly(
                &key,
                (!placement.appearance.is_empty()).then_some(placement.appearance.as_str()),
            )?;
            for component in native
                .components
                .iter()
                .filter(|c| c.enabled && c.chunk_mask != 0)
            {
                if let Some(mesh) = &component.mesh {
                    let mut subject = placement.clone();
                    subject.entity = false;
                    subject.mesh = match mesh {
                        ResourceKey::Path(path) => path.clone(),
                        ResourceKey::Hash(hash) => hash.to_string(),
                    };
                    subject.appearance = component.mesh_appearance.clone();
                    subjects.push(subject);
                }
            }
        } else {
            subjects.push(placement.clone());
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut repositories = std::collections::BTreeSet::new();
    for p in &subjects {
        if !seen.insert((&p.mesh, &p.appearance)) {
            continue;
        }
        let key = ResourceKey::parse(&p.mesh);
        let directory = out.join(format!("{}_{}", key.native_hash(), p.appearance));
        std::fs::create_dir_all(&directory)?;
        let asset = source.load_shared_mesh_resource(&key, &p.appearance)?;
        if let Some(provenance) = asset.material_provenance()?
            && let Some(root) = provenance["MaterialRepo"].as_str()
        {
            repositories.insert(std::path::PathBuf::from(root));
        }
        let geometry = source.load_geometry(&key)?;
        std::fs::write(
            directory.join("native.json"),
            serde_json::to_vec_pretty(&*source.resources().document(&key)?)?,
        )?;
        std::fs::write(
            directory.join("materials.json"),
            serde_json::to_vec_pretty(&asset.material_provenance()?)?,
        )?;
        std::fs::write(
            directory.join("prepared.txt"),
            format!("{:#?}", asset.materials()),
        )?;
        let chunks:Vec<_>=geometry.chunks().iter().map(|c|serde_json::json!({"chunk":c.chunk_index,"materials":c.material_names,"uv0":c.tex_coords,"uv1":c.tex_coords_1})).collect();
        std::fs::write(
            directory.join("uvs.json"),
            serde_json::to_vec_pretty(&chunks)?,
        )?;
        for (i, m) in asset.materials().iter().enumerate() {
            for (kind, path) in [
                ("base", &m.base_color_texture),
                ("normal", &m.normal_texture),
                ("surface", &m.metallic_roughness_texture),
            ] {
                if let Some(path) = path {
                    std::fs::copy(path, directory.join(format!("{i}-{kind}.png")))?;
                }
            }
        }
    }
    for (i, path) in repositories.iter().enumerate() {
        copy_repository(path, &out.join(format!("repository-{i}")))?;
    }
    Ok(())
}

fn copy_repository(source: &std::path::Path, target: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let destination = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_repository(&entry.path(), &destination)?;
        } else {
            std::fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}

#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    pub browser_scene: bool,
    pub comparison: String,
    pub hidden_layers: Vec<String>,
    pub size: u32,
    pub cutaway: bool,
    pub level: Option<u32>,
    pub view: View,
    pub camera: Option<Camera>,
    pub lighting: Option<ghostline_renderer::Lighting>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            browser_scene: false,
            comparison: "proposed".into(),
            hidden_layers: Vec::new(),
            size: 1024,
            cutaway: true,
            level: None,
            view: View::Hero,
            camera: None,
            lighting: None,
        }
    }
}

pub struct Session {
    pub source: Arc<GameSource>,
    renderer: Renderer,
    prepared: Option<PreparedScene>,
    signature: Vec<(String, String, String)>,
    nodes: Vec<ghostline_renderer::NodeId>,
    devices: Vec<(ghostline_renderer::NodeId, [f32; 3], f32)>,
    entities: BTreeMap<(String, String), Scene>,
}

impl Session {
    pub fn new(source: GameSource) -> Result<Self> {
        Ok(Self {
            source: Arc::new(source),
            renderer: pollster::block_on(Renderer::new())?,
            prepared: None,
            signature: Vec::new(),
            nodes: Vec::new(),
            devices: Vec::new(),
            entities: BTreeMap::new(),
        })
    }

    pub fn render(&mut self, compiled: &Compiled, options: Options) -> Result<Vec<u8>> {
        anyhow::ensure!(
            (64..=4096).contains(&options.size),
            "Preview size must be 64–4096 pixels"
        );
        anyhow::ensure!(
            compiled
                .placements
                .iter()
                .any(|p| crate::shell::visible(compiled, p, &options)),
            "No visible geometry on the selected floor"
        );
        let mut signature: Vec<_> = compiled
            .placements
            .iter()
            .map(|p| (p.id.clone(), p.mesh.clone(), p.appearance.clone()))
            .collect();
        let technical = crate::technical::nodes(compiled)?;
        for extra in &technical {
            if extra.node["Data"]["$type"] == "worldDeviceNode" {
                signature.push((
                    extra.node["Data"]["debugName"]["$value"]
                        .as_str()
                        .context("Missing device identity")?
                        .into(),
                    extra.node["Data"]["entityTemplate"]["DepotPath"]["$value"]
                        .as_str()
                        .context("Missing device template")?
                        .into(),
                    extra.node["Data"]["appearanceName"]["$value"]
                        .as_str()
                        .context("Missing device appearance")?
                        .into(),
                ));
            }
        }
        if self.prepared.is_none() || self.signature != signature {
            let mut scene = Scene::new();
            let mut nodes = Vec::new();
            let mut devices = Vec::new();
            let mut assets = BTreeMap::new();
            for p in &compiled.placements {
                if p.entity {
                    let key = (p.mesh.clone(), p.appearance.clone());
                    if !self.entities.contains_key(&key) {
                        let mut native = self.source.resolve_assembly(
                            &ResourceKey::parse(&p.mesh),
                            (!p.appearance.is_empty()).then_some(p.appearance.as_str()),
                        )?;
                        self.source
                            .add_binding_context(&mut native, &p.assembly_options)?;
                        let assembly = ghostline_renderer::SceneAssembly::from_native(
                            native,
                            &p.assembly_options,
                            |resource, appearance| {
                                self.source.load_shared_mesh_resource(resource, appearance)
                            },
                        )?;
                        self.entities.insert(key.clone(), assembly.scene);
                    }
                    nodes.push(
                        scene.append_scene(
                            self.entities
                                .get(&key)
                                .context("Native prop cache absent")?,
                            &p.id,
                            placement_transform(p),
                        )?,
                    );
                    continue;
                }
                let key = (&p.mesh, &p.appearance);
                let asset = if let Some(id) = assets.get(&key) {
                    *id
                } else {
                    let mesh = if let Some(edit) = compiled.edited_meshes.get(&p.mesh) {
                        Arc::new(
                            self.source
                                .load_shared_mesh_resource(&edit.source, &p.appearance)?
                                .with_document(&edit.document)?,
                        )
                    } else {
                        self.source
                            .load_shared_mesh_resource(&ResourceKey::parse(&p.mesh), &p.appearance)
                            .with_context(|| format!("Preparing {} ({})", p.id, p.mesh))?
                    };
                    let id = scene.add_shared_asset(mesh);
                    assets.insert(key, id);
                    id
                };
                nodes.push(scene.add_node(
                    Node::new(&p.id, Some(asset)).with_transform(placement_transform(p)),
                )?);
            }
            for extra in &technical {
                if extra.node["Data"]["$type"] != "worldDeviceNode" {
                    continue;
                }
                let mesh = extra.node["Data"]["entityTemplate"]["DepotPath"]["$value"]
                    .as_str()
                    .context("Missing native device template")?;
                let appearance = extra.node["Data"]["appearanceName"]["$value"]
                    .as_str()
                    .context("Missing native device appearance")?;
                let key = (mesh.to_owned(), appearance.to_owned());
                if !self.entities.contains_key(&key) {
                    let native = self
                        .source
                        .resolve_assembly(&ResourceKey::parse(mesh), Some(appearance))?;
                    let options = ghostline_renderer::AssemblyOptions::standalone(&native);
                    let assembly = ghostline_renderer::SceneAssembly::from_native(
                        native,
                        &options,
                        |resource, appearance| {
                            self.source.load_shared_mesh_resource(resource, appearance)
                        },
                    )?;
                    self.entities.insert(key.clone(), assembly.scene);
                }
                let native = self
                    .entities
                    .get(&key)
                    .context("Native entity cache absent")?;
                let node = scene.append_scene(
                    native,
                    "door",
                    transform(extra.position, extra.yaw, extra.scale),
                )?;
                devices.push((node, extra.position, extra.yaw));
            }
            self.prepared = Some(self.renderer.prepare_scene(&scene)?);
            self.signature = signature;
            self.nodes = nodes;
            self.devices = devices;
        }
        let scene = self
            .prepared
            .as_mut()
            .context("Preview has no prepared scene")?;
        let lighting = options.lighting.clone().unwrap_or_else(|| {
            let mut lights = ghostline_renderer::Lighting {
                ambient: [0.45; 3],
                sky: 0.05,
                ..Default::default()
            };
            if !compiled.project.lights.is_empty() {
                lights.lights = compiled
                    .project
                    .lights
                    .iter()
                    .map(|light| {
                        let linear = |v: u8| {
                            let v = f32::from(v) / 255.;
                            if v <= 0.04045 {
                                v / 12.92
                            } else {
                                ((v + 0.055) / 1.055).powf(2.4)
                            }
                        };
                        ghostline_renderer::Light::Point {
                            position: [light.position[0], light.position[2], -light.position[1]],
                            color: light.color.map(|v| linear(v) * light.intensity),
                            radius: light.radius,
                        }
                    })
                    .collect();
            }
            lights
        });
        scene.set_lighting(lighting)?;
        for (index, p) in compiled.placements.iter().enumerate() {
            let node = self.nodes[index];
            scene.set_transform(node, placement_transform(p))?;
            scene.set_visible(node, crate::shell::visible(compiled, p, &options))?;
        }
        for ((node, _, _), extra) in self.devices.iter().zip(
            technical
                .iter()
                .filter(|extra| extra.node["Data"]["$type"] == "worldDeviceNode"),
        ) {
            scene.set_transform(*node, transform(extra.position, extra.yaw, extra.scale))?;
            let storey = crate::model::kit(&compiled.project.kit)?.storey_height;
            scene.set_visible(
                *node,
                options.comparison != "existing"
                    && options
                        .level
                        .is_none_or(|level| (extra.position[2] / storey).floor() as u32 == level),
            )?;
        }
        if options.browser_scene {
            return Ok(scene.browser_glb()?);
        }
        let mut low = glam::Vec3::splat(f32::INFINITY);
        let mut high = glam::Vec3::splat(f32::NEG_INFINITY);
        for p in compiled
            .placements
            .iter()
            .filter(|p| crate::shell::visible(compiled, p, &options))
        {
            let m = placement_transform(p);
            for x in [p.bounds.min[0], p.bounds.max[0]] {
                for y in [p.bounds.min[1], p.bounds.max[1]] {
                    for z in [p.bounds.min[2], p.bounds.max[2]] {
                        let point = m.transform_point3(glam::Vec3::new(x, z, -y));
                        low = low.min(point);
                        high = high.max(point);
                    }
                }
            }
        }
        let center = (low + high) / 2.;
        let radius = (high - low).length() / 2.;
        let direction = match options.view {
            View::Hero => glam::Vec3::new(0.8, 1., 1.),
            View::Front => glam::Vec3::new(0., 0.2, 1.),
            View::Back => glam::Vec3::new(0., 0.2, -1.),
            View::Left => glam::Vec3::new(-1., 0.2, 0.),
            View::Right | View::Side => glam::Vec3::new(1., 0.2, 0.),
        }
        .normalize();
        let camera = Camera::Orthographic {
            eye: (center + direction * radius * 4.).to_array(),
            target: center.to_array(),
            up: [0., 1., 0.],
            half_height: radius * 0.95,
            near: radius * 0.01,
            far: radius * 10.,
        };
        let rendered = self.renderer.render(
            scene,
            RenderOptions {
                size: options.size,
                camera: options.camera.unwrap_or(camera),
            },
        )?;
        let mut output = Cursor::new(Vec::new());
        rendered
            .image
            .write_to(&mut output, image::ImageFormat::Png)?;
        Ok(output.into_inner())
    }
}
