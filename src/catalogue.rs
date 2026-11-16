//! Published items catalogue adapter. Decoding and attachment evaluation stay in
//! red/renderer; the builder retains published identities and preview recipes.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, ensure};
use ghostline_red::assets::ResourceKey;
use ghostline_renderer::{AssemblyOptions, GameSource, SceneAssembly};
use glam::{Mat4, Vec3};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::model::{Bounds, Library, Module, Project};

#[derive(Deserialize)]
struct Descriptor {
    format: String,
    version: u32,
    collections: BTreeMap<String, Vec<String>>,
    sources: BTreeMap<String, String>,
}
#[derive(Deserialize)]
struct Prop {
    id: String,
    label: String,
    resource: ResourceKey,
    kind: String,
    category: String,
    selections: Vec<String>,
    #[serde(default)]
    assemblies: Vec<String>,
}
#[derive(Clone, Deserialize, serde::Serialize)]
struct Media {
    path: String,
    view: String,
}
#[derive(Deserialize, Default)]
struct PreviewContext {
    #[serde(default)]
    binding_contexts: Vec<ghostline_renderer::AssemblyReference>,
    #[serde(default)]
    slot_bone_overrides: BTreeMap<String, String>,
    #[serde(default)]
    standalone_origin_anchors: BTreeSet<String>,
    #[serde(default)]
    render: Value,
}
#[derive(Deserialize)]
struct Selection {
    id: String,
    appearance: Option<String>,
    resolution: String,
    preview_state: String,
    #[serde(default)]
    diagnostics: Vec<String>,
    #[serde(default)]
    media: Vec<Media>,
    #[serde(default)]
    context: PreviewContext,
}

pub struct Catalogue {
    root: PathBuf,
    source: Option<String>,
    props: BTreeMap<String, Prop>,
    selections: BTreeMap<String, Selection>,
    media: BTreeSet<String>,
}

fn safe_relative(name: &str) -> bool {
    let path = Path::new(name);
    !name.is_empty()
        && !name.contains('\\')
        && !name.contains(':')
        && path.components().all(|c| matches!(c, Component::Normal(_)))
}

fn relative(root: &Path, name: &str) -> Result<PathBuf> {
    ensure!(safe_relative(name), "Unsafe catalogue path {name}");
    let root = root.canonicalize()?;
    let resolved = root.join(name).canonicalize()?;
    ensure!(
        resolved.starts_with(root),
        "Catalogue path leaves its package"
    );
    Ok(resolved)
}

impl Catalogue {
    pub fn default_root() -> PathBuf {
        std::env::var_os("GHOSTLINE_ITEMS_CATALOGUE")
            .map_or_else(|| PathBuf::from("../items/dist/catalogue"), PathBuf::from)
    }

    pub fn open(root: &Path) -> Result<Option<Self>> {
        if !root.join("data/catalogue.json").exists() {
            return Ok(None);
        }
        let descriptor: Descriptor =
            serde_json::from_slice(&fs::read(root.join("data/catalogue.json"))?)?;
        ensure!(
            descriptor.format == "ghostline-items" && descriptor.version == 1,
            "Unsupported items catalogue"
        );
        let mut props = BTreeMap::new();
        for file in descriptor
            .collections
            .get("props")
            .context("Catalogue has no props collection")?
        {
            for prop in serde_json::from_slice::<Vec<Prop>>(&fs::read(relative(root, file)?)?)? {
                ensure!(
                    props.insert(prop.id.clone(), prop).is_none(),
                    "Duplicate prop identity"
                );
            }
        }
        let wanted: BTreeSet<_> = props
            .values()
            .flat_map(|p| p.selections.iter().cloned())
            .collect();
        let mut selections = BTreeMap::new();
        let mut media = BTreeSet::new();
        for file in descriptor
            .collections
            .get("selections")
            .context("Catalogue has no selections collection")?
        {
            for mut selection in
                serde_json::from_slice::<Vec<Selection>>(&fs::read(relative(root, file)?)?)?
            {
                if !wanted.contains(&selection.id) {
                    continue;
                }
                selection
                    .media
                    .retain(|m| m.path.starts_with("media/") && safe_relative(&m.path));
                media.extend(selection.media.iter().map(|m| m.path.clone()));
                ensure!(
                    selections.insert(selection.id.clone(), selection).is_none(),
                    "Duplicate selection identity"
                );
            }
        }
        Ok(Some(Self {
            root: root.canonicalize()?,
            source: descriptor.sources.get("props_source_identity").cloned(),
            props,
            selections,
            media,
        }))
    }

    pub fn count(&self) -> usize {
        self.props.len()
    }

    pub fn mesh_selection(&self, key: &ResourceKey) -> Result<(String, Vec<String>)> {
        let prop = self
            .props
            .values()
            .find(|p| p.resource.native_hash() == key.native_hash())
            .context("Native mesh is absent from published catalogue")?;
        Ok((
            prop.id.clone(),
            prop.selections
                .iter()
                .filter_map(|s| self.selections.get(s))
                .map(|s| s.appearance.clone().unwrap_or_default())
                .collect(),
        ))
    }

    pub fn media_path(&self, name: &str) -> Result<PathBuf> {
        ensure!(self.media.contains(name), "Unknown published preview");
        ensure!(name.ends_with(".png"), "Unsupported catalogue media");
        relative(&self.root, name)
    }

    pub fn search(&self, words: &[String], role: &str) -> Vec<Value> {
        let mut results = Vec::new();
        for prop in self.props.values() {
            let kind = if prop.kind == "assembly" {
                "assembly"
            } else if !prop.assemblies.is_empty() {
                "part"
            } else {
                "mesh"
            };
            let category = prop.category.to_ascii_lowercase();
            let family = match &prop.resource {
                ResourceKey::Path(path) => path.as_str(),
                _ => "",
            };
            let filename = family.rsplit('\\').next().unwrap_or("");
            let structural = [
                "doorway", "stairs", "ceiling", "floor", "roof", "pillar", "railing", "wall",
            ]
            .into_iter()
            .find(|role| filename.contains(role))
            .filter(|_| family.contains(r"\architecture\"));
            let text = format!("{} {family} {category} {kind}", prop.label).to_lowercase();
            if (!role.is_empty()
                && role != category
                && role != kind
                && Some(role) != structural
                && !(role == "equipment" && category == "electronics"))
                || !words.iter().all(|w| text.contains(w))
            {
                continue;
            }
            let selections: Vec<_> = prop
                .selections
                .iter()
                .filter_map(|s| self.selections.get(s))
                .collect();
            let appearances: Vec<_> = selections
                .iter()
                .map(|s| s.appearance.as_deref().unwrap_or(""))
                .collect();
            let previews: BTreeMap<_, _> = selections
                .iter()
                .map(|s| (s.appearance.as_deref().unwrap_or(""), &s.media))
                .collect();
            let states: BTreeMap<_,_> = selections.iter().map(|s| (s.appearance.as_deref().unwrap_or(""), json!({"resolution":s.resolution,"preview":s.preview_state,"diagnostics":s.diagnostics}))).collect();
            results.push(json!({"id":prop.id,"name":prop.label,"path":family,"role":structural.unwrap_or(&category),"kind":kind,"bounds":null,"appearances":appearances,"previews":previews,"states":states,"source_content_sha256":self.source}));
        }
        // Complete entities precede individual components in the same search.
        results.sort_by(|a, b| {
            (a["kind"] != "assembly", a["name"].as_str())
                .cmp(&(b["kind"] != "assembly", b["name"].as_str()))
        });
        results
    }

    /// Measures only selected native resources, then uses the same compiler as
    /// the authored kit library. No separate catalogue repair/import is needed.
    /// Catalogue provenance may predate the installed game; resolve and measure
    /// selected resources against the current installation instead of requiring
    /// an identical whole-game content fingerprint.
    pub fn hydrate(
        &self,
        project: &Project,
        library: &mut Library,
        source: &GameSource,
    ) -> Result<()> {
        let kit = crate::model::kit(&project.kit)?;
        for workstation in &project.workstations {
            ensure!(
                source
                    .resources()
                    .resource_exists(&ResourceKey::parse(&workstation.actor.workspot)),
                "Workstation workspot missing: {}",
                workstation.actor.workspot
            );
        }
        let mut requested = project
            .objects
            .iter()
            .filter(|o| !o.asset.starts_with("world:"))
            .map(|o| (o.asset.clone(), o.appearance.clone()))
            .collect::<BTreeSet<_>>();
        for piece in kit.pieces.values() {
            let hash = ResourceKey::parse(&piece.mesh).native_hash();
            let prop = self
                .props
                .values()
                .find(|p| p.resource.native_hash() == hash)
                .with_context(|| format!("Kit mesh absent from items catalogue: {}", piece.mesh))?;
            for id in &prop.selections {
                if let Some(selection) = self.selections.get(id)
                    && selection.resolution == "resolved"
                {
                    requested.insert((
                        prop.id.clone(),
                        selection.appearance.clone().unwrap_or_default(),
                    ));
                }
            }
        }
        for (asset, appearance) in requested {
            let prop = self
                .props
                .get(&asset)
                .with_context(|| format!("Unknown catalogue object {asset}"))?;
            if library
                .assets
                .iter()
                .any(|m| m.id == prop.id && m.catalogue_bounds.contains_key(&appearance))
            {
                continue;
            }
            let selection = prop
                .selections
                .iter()
                .filter_map(|id| self.selections.get(id))
                .find(|s| s.appearance.as_deref().unwrap_or("") == appearance)
                .context("Unknown catalogue appearance")?;
            ensure!(
                selection.resolution == "resolved" && selection.preview_state != "not_applicable",
                "Catalogue selection is unavailable: {}: {}",
                selection.id,
                selection.diagnostics.join("; ")
            );
            let entity = prop.kind == "assembly";
            let mut options = AssemblyOptions::default();
            let mut low = Vec3::splat(f32::INFINITY);
            let mut high = Vec3::splat(f32::NEG_INFINITY);
            if entity {
                let mut native =
                    source.resolve_assembly(&prop.resource, selection.appearance.as_deref())?;
                options = AssemblyOptions::standalone(&native);
                options.binding_contexts = selection.context.binding_contexts.clone();
                options.slot_bone_overrides = selection.context.slot_bone_overrides.clone();
                options
                    .origin_anchors
                    .extend(selection.context.standalone_origin_anchors.clone());
                if let Some(value) = selection.context.render.get("visibility_parameters") {
                    options.visibility_parameters = serde_json::from_value(value.clone())?;
                }
                if let Some(value) = selection.context.render.get("mesh_lods") {
                    options.mesh_lods = serde_json::from_value(value.clone())?;
                }
                source.add_binding_context(&mut native, &options)?;
                let scene = SceneAssembly::from_native(native, &options, |key, _| {
                    source
                        .load_geometry(key)?
                        .with_neutral_material()
                        .map(Arc::new)
                })?;
                let placements = scene.native_component_placements()?;
                for component in scene.native.components.iter().filter(|c| {
                    c.enabled
                        && c.chunk_mask != 0
                        && c.kind != "entAppearanceProxyMeshComponent"
                        && !c.is_collapsed()
                }) {
                    if let Some(mesh) = &component.mesh {
                        let matrix = placements
                            .get(&component.name)
                            .context("Missing native component transform")?;
                        measure(
                            source,
                            mesh,
                            Mat4::from_cols_array(matrix),
                            component.chunk_mask,
                            &mut low,
                            &mut high,
                        )?;
                    }
                }
            } else {
                measure(
                    source,
                    &prop.resource,
                    Mat4::IDENTITY,
                    u64::MAX,
                    &mut low,
                    &mut high,
                )?;
            }
            ensure!(
                low.is_finite() && high.is_finite() && (high - low).max_element() > 0.00001,
                "Catalogue object has no visible geometry: {}",
                selection.id
            );
            let bounds = Bounds {
                min: low.to_array(),
                max: high.to_array(),
                dimensions: (high - low).to_array(),
            };
            let index = if let Some(index) = library.assets.iter().position(|m| m.id == prop.id) {
                index
            } else {
                library.assets.push(Module {
                    id: prop.id.clone(),
                    path: match &prop.resource {
                        ResourceKey::Path(p) => p.clone(),
                        ResourceKey::Hash(h) => h.to_string(),
                    },
                    name: prop.label.clone(),
                    family: prop.category.clone(),
                    role: if let ResourceKey::Path(path) = &prop.resource {
                        let file = path.rsplit('\\').next().unwrap_or("");
                        ["ceiling", "floor", "wall"]
                            .into_iter()
                            .find(|r| file.contains(r))
                            .filter(|_| path.contains(r"\architecture\"))
                            .unwrap_or(&prop.category)
                            .to_lowercase()
                    } else {
                        prop.category.to_lowercase()
                    },
                    status: "prepared".into(),
                    bounds: bounds.clone(),
                    appearances: prop
                        .selections
                        .iter()
                        .filter_map(|id| self.selections.get(id))
                        .map(|s| s.appearance.clone().unwrap_or_default())
                        .collect(),
                    source_content_sha256: self.source.clone(),
                    entity,
                    catalogue_bounds: BTreeMap::new(),
                    assembly_options: BTreeMap::new(),
                });
                library.assets.len() - 1
            };
            library.assets[index]
                .catalogue_bounds
                .insert(appearance.clone(), bounds);
            library.assets[index]
                .assembly_options
                .insert(appearance, options);
        }
        Ok(())
    }
}

pub(crate) fn measure(
    source: &GameSource,
    key: &ResourceKey,
    matrix: Mat4,
    mask: u64,
    low: &mut Vec3,
    high: &mut Vec3,
) -> Result<()> {
    let geometry = source.load_geometry(key)?;
    if geometry.is_shadow_only() {
        return Ok(());
    }
    for chunk in geometry
        .chunks()
        .iter()
        .filter(|c| c.chunk_index < 64 && mask & (1u64 << c.chunk_index) != 0)
    {
        for &[x, y, z] in &chunk.positions {
            let p = matrix.transform_point3(Vec3::new(x, -z, y));
            *low = low.min(p);
            *high = high.max(p);
        }
    }
    Ok(())
}
