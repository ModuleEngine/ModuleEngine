//! Mutable authored scenes. Core snapshots stay hydrated; only this IO boundary
//! turns scene bodies into references. Imported resource hashes are unrelated.
use crate::{hash_bytes, normalize_relative, resolve_relative};
use module_engine_core::{AuthoredScene, GraphDocument, SCHEMA_VERSION, SceneAssetBinding};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

pub const SCENE_STORAGE_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub enum SceneSourceEdit {
    New {
        name: String,
        duplicate: Option<String>,
    },
    Rename {
        scene: String,
        name: String,
        filename: String,
    },
    Move {
        scene: String,
        folder: String,
    },
    Delete {
        scene: String,
    },
    SetEntry {
        scene: String,
    },
    Convert,
    Restore {
        trash_id: String,
    },
}

pub(crate) fn validate_scene_path(input: &str) -> Result<String, String> {
    let path = normalize_relative(input)?;
    if path != input || !path.starts_with("scenes/") || !path.ends_with(".scene.json") {
        return Err(format!(
            "Authored scene needs a normalized scenes/*.scene.json path: {input}"
        ));
    }
    for part in path.split('/') {
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || ((stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.len() == 4
                && matches!(stem.as_bytes()[3], b'1'..=b'9'));
        if part.ends_with(['.', ' '])
            || part
                .chars()
                .any(|c| c.is_control() || matches!(c, '<' | '>' | '"' | '|' | '?' | '*'))
            || device
        {
            return Err(format!(
                "Scene path contains a nonportable filename: {part}"
            ));
        }
    }
    Ok(path)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneFile {
    scene_asset_version: u32,
    scene_id: String,
    asset_id: String,
    scene: AuthoredScene,
}

/// Accepted disk generation, never rebased through authored history or Assets
/// refresh. None means an operation-owned path must still be absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectFileState {
    pub project: String,
    root: PathBuf,
    pub(crate) files: BTreeMap<String, Option<String>>,
    pub(crate) scene_paths: BTreeSet<String>,
    scene_ids: BTreeMap<String, String>,
    sizes: BTreeMap<String, u64>,
}

pub struct LoadedProject {
    pub document: GraphDocument,
    pub files: ProjectFileState,
}

pub(crate) struct ProjectSourcePlan {
    pub changes: BTreeMap<String, Option<Vec<u8>>>,
    pub expected: BTreeMap<String, Option<String>>,
    pub accepted: ProjectFileState,
}

fn schema(value: &Value) -> Result<(), String> {
    let version = value
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or("Project needs an integer schema_version")?;
    if version != u64::from(SCHEMA_VERSION) {
        return Err(if (1..u64::from(SCHEMA_VERSION)).contains(&version) {
            format!(
                "Migration required: document format {version}; expected {SCHEMA_VERSION}. Use explicit migrate-copy"
            )
        } else {
            format!("Unsupported document format {version}; expected exact format {SCHEMA_VERSION}")
        });
    }
    Ok(())
}

/// Canonical ownership check for source reads and prospective writes. Source
/// scene files live outside the immutable Assets scanner, in scenes/.
pub(crate) fn source_target(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let relative = normalize_relative(relative)?;
    let target = root.join(&relative);
    let mut ancestor = target.as_path();
    while !ancestor.exists() {
        // Reject dangling redirects as well as redirects with existing targets.
        if fs::symlink_metadata(ancestor).is_ok() {
            return Err(format!("Source path is redirected: {relative}"));
        }
        ancestor = ancestor.parent().ok_or("Source path has no parent")?;
    }
    let canonical = ancestor.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(root) {
        return Err(format!("Source path leaves project: {relative}"));
    }
    if canonical
        .to_string_lossy()
        .replace('\\', "/")
        .to_lowercase()
        != ancestor.to_string_lossy().replace('\\', "/").to_lowercase()
    {
        return Err(format!(
            "Authored source uses a redirected/aliased path: {relative}"
        ));
    }
    if fs::symlink_metadata(&target).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(format!("Source file is a symbolic link: {relative}"));
    }
    Ok(target)
}

fn bindings(document: &GraphDocument) -> Result<BTreeMap<String, SceneAssetBinding>, String> {
    let Some(game) = &document.game else {
        return Ok(BTreeMap::new());
    };
    if game.scene_assets.is_empty() {
        return Ok(BTreeMap::new());
    }
    game.validate_scene_data()?;
    let mut paths = BTreeSet::new();
    for binding in game.scene_assets.values() {
        let path = validate_scene_path(&binding.path)?;
        if !paths.insert(path.to_lowercase()) {
            return Err("Scene file paths alias each other".into());
        }
    }
    let manifest = crate::AssetManifest::from_value(&document.asset_manifest)?;
    if game
        .scene_assets
        .values()
        .any(|binding| manifest.assets.contains_key(&binding.id))
    {
        return Err("Scene asset identity collides with an imported resource".into());
    }
    for asset in manifest.assets.values() {
        if paths.contains(&normalize_relative(&asset.path)?.to_lowercase()) {
            return Err("Mutable authored scene cannot also be an immutable imported asset".into());
        }
    }
    Ok(game.scene_assets.clone())
}

pub fn validate_scene_sources(document: &GraphDocument) -> Result<(), String> {
    bindings(document).map(|_| ())
}

pub fn read_project_source(path: &Path) -> Result<LoadedProject, String> {
    let root = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let project = path
        .file_name()
        .and_then(|p| p.to_str())
        .ok_or("Project filename is not UTF-8")?
        .to_owned();
    let raw = fs::read(source_target(&root, &project)?)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut value = crate::engine_migration::parse_source(&raw)?;
    schema(&value)?;
    let storage = value
        .as_object_mut()
        .ok_or("Project must be an object")?
        .remove("scene_storage_version");
    let mut accepted = ProjectFileState {
        project: project.clone(),
        root: root.clone(),
        files: BTreeMap::from([(project, Some(hash_bytes(&raw)))]),
        scene_paths: BTreeSet::new(),
        scene_ids: BTreeMap::new(),
        sizes: BTreeMap::from([(
            path.file_name().unwrap().to_string_lossy().into_owned(),
            raw.len() as u64,
        )]),
    };
    if let Some(version) = storage {
        if version.as_u64() != Some(u64::from(SCENE_STORAGE_VERSION)) {
            return Err("Unsupported scene_storage_version; expected exact version 1".into());
        }
        let game = value
            .get_mut("game")
            .and_then(Value::as_object_mut)
            .ok_or("External scene project requires game data")?;
        if game.contains_key("scenes") {
            return Err("External scene project cannot also embed scenes".into());
        }
        let sources: BTreeMap<String, SceneAssetBinding> = serde_json::from_value(
            game.get("scene_assets")
                .cloned()
                .ok_or("External scene project requires scene_assets")?,
        )
        .map_err(|e| e.to_string())?;
        if sources.is_empty() || sources.len() > 256 {
            return Err("External scene project needs between 1 and 256 scenes".into());
        }
        let mut scenes = BTreeMap::new();
        let mut identities = BTreeSet::new();
        let mut canonical_paths = BTreeSet::new();
        for (id, binding) in &sources {
            let normalized = validate_scene_path(&binding.path)?;
            if binding.id.trim().is_empty()
                || !identities.insert(&binding.id)
                || !accepted.scene_paths.insert(normalized.to_lowercase())
            {
                return Err("Duplicate or empty scene asset identity/path".into());
            }
            let target = source_target(&root, &binding.path)?;
            let canonical = target
                .canonicalize()
                .map_err(|e| format!("Scene {id} ({}): {e}", binding.path))?;
            if !canonical_paths.insert(canonical.to_string_lossy().to_lowercase()) {
                return Err("Scene file paths resolve to the same file".into());
            }
            let bytes = fs::read(resolve_relative(&root, &binding.path)?)
                .map_err(|e| format!("Scene {id}: {e}"))?;
            let file: SceneFile =
                serde_json::from_value(crate::engine_migration::parse_source(&bytes)?)
                    .map_err(|e| format!("Scene {id} ({}): {e}", binding.path))?;
            if file.scene_asset_version != SCENE_STORAGE_VERSION
                || file.scene_id != *id
                || file.asset_id != binding.id
            {
                return Err(format!(
                    "Scene file version/identity differs from project binding: {}",
                    binding.path
                ));
            }
            accepted
                .files
                .insert(binding.path.clone(), Some(hash_bytes(&bytes)));
            accepted
                .scene_ids
                .insert(binding.id.clone(), binding.path.clone());
            accepted
                .sizes
                .insert(binding.path.clone(), bytes.len() as u64);
            scenes.insert(id.clone(), file.scene);
        }
        // Retain the authored spelling, not the case-folded uniqueness index.
        accepted.scene_paths = sources.values().map(|s| s.path.clone()).collect();
        game.insert(
            "scenes".into(),
            serde_json::to_value(scenes).map_err(|e| e.to_string())?,
        );
    } else if value
        .pointer("/game/scene_assets")
        .and_then(Value::as_object)
        .is_some_and(|v| !v.is_empty())
    {
        return Err("Scene bindings require explicit scene_storage_version: 1; hydrated snapshots are not source files".into());
    }
    let document: GraphDocument =
        serde_json::from_value(value).map_err(|e| format!("{}: {e}", path.display()))?;
    bindings(&document)?;
    Ok(LoadedProject {
        document,
        files: accepted,
    })
}

impl ProjectFileState {
    /// Bind an already read candidate to actual source bytes. A second read may
    /// detect a concurrent edit; it must never silently bless different scenes.
    pub fn observe(path: &Path, document: &GraphDocument) -> Result<Self, String> {
        if path.exists() {
            let loaded = read_project_source(path)?;
            if loaded.document != *document {
                return Err("Project changed while opening; reload before saving".into());
            }
            return Ok(loaded.files);
        }
        let project = path
            .file_name()
            .and_then(|p| p.to_str())
            .ok_or("Project filename is not UTF-8")?
            .to_owned();
        let raw = if path.exists() {
            Some(fs::read(path).map_err(|e| e.to_string())?)
        } else {
            None
        };
        let bytes = raw.as_deref().map(hash_bytes);
        let sizes = raw
            .as_ref()
            .map(|b| BTreeMap::from([(project.clone(), b.len() as u64)]))
            .unwrap_or_default();
        let root = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .canonicalize()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            project: project.clone(),
            root,
            files: BTreeMap::from([(project, bytes)]),
            scene_paths: BTreeSet::new(),
            scene_ids: BTreeMap::new(),
            sizes,
        })
    }

    pub fn require_project_path(&self, path: &Path) -> Result<(), String> {
        let root = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if root != self.root
            || path.file_name().and_then(|v| v.to_str()) != Some(self.project.as_str())
        {
            return Err("Accepted source generation belongs to another project path".into());
        }
        if path.exists() && !path.is_file() {
            return Err("Project path is not a file".into());
        }
        Ok(())
    }
    pub fn source_size(&self, path: &str) -> Option<u64> {
        self.sizes.get(path).copied()
    }
    pub fn verify(&self, root: &Path) -> Result<(), String> {
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        if root != self.root {
            return Err("Accepted source generation belongs to another project root".into());
        }
        for (path, expected) in &self.files {
            let target = source_target(&root, path)?;
            let actual = if target.exists() {
                Some(hash_bytes(&fs::read(&target).map_err(|e| e.to_string())?))
            } else {
                None
            };
            if &actual != expected {
                return Err(format!(
                    "Authored source changed outside this editor; reload or save a separate project: {path}"
                ));
            }
        }
        Ok(())
    }
}

/// Explicitly allocate a source group. Selection/ordinary inline Save never
/// calls this. A same-folder copy asks for a fresh group and retains identities.
fn file_slug(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '-'
            }
        })
        .take(48)
        .collect();
    let slug = slug.trim_matches('-');
    let slug = if slug.is_empty() { "Scene" } else { slug };
    if validate_scene_path(&format!("scenes/{slug}.scene.json")).is_ok() {
        slug.into()
    } else {
        format!("Scene-{slug}")
    }
}
fn common_scene_parent(sources: &BTreeMap<String, SceneAssetBinding>) -> String {
    let mut common: Option<Vec<&str>> = None;
    for binding in sources.values() {
        let parent: Vec<_> = binding
            .path
            .rsplit_once('/')
            .map_or("scenes", |v| v.0)
            .split('/')
            .collect();
        if let Some(current) = &mut common {
            let count = current
                .iter()
                .zip(&parent)
                .take_while(|(a, b)| a == b)
                .count();
            current.truncate(count);
        } else {
            common = Some(parent);
        }
    }
    common
        .filter(|v| !v.is_empty())
        .map(|v| v.join("/"))
        .unwrap_or_else(|| "scenes".into())
}
fn available_scene_path(parent: &str, name: &str, occupied: &BTreeSet<String>) -> String {
    let slug = file_slug(name);
    let mut suffix = 1;
    loop {
        let name = if suffix == 1 {
            slug.clone()
        } else {
            format!("{slug}-{suffix}")
        };
        let path = format!("{parent}/{name}.scene.json");
        if !occupied.contains(&path.to_lowercase()) {
            return path;
        }
        suffix += 1;
    }
}
pub fn externalize_scenes(
    document: &GraphDocument,
    fresh_group: bool,
) -> Result<GraphDocument, String> {
    validate_scene_sources(document)?;
    let mut candidate = document.clone();
    let project_name = candidate.name.clone();
    let Some(game) = &mut candidate.game else {
        return Ok(candidate);
    };
    game.validate_scene_data()?;
    if !game.scene_assets.is_empty() && !fresh_group {
        return Ok(candidate);
    }
    let token = hash_bytes(crate::history::unique_id().as_bytes());
    let parent = format!("scenes/{}-{}", file_slug(&project_name), &token[..12]);
    let previous = game.scene_assets.clone();
    let old_parent = common_scene_parent(&previous);
    let mut occupied = BTreeSet::new();
    game.scene_assets.clear();
    for (id, scene) in &game.scenes {
        let asset = previous
            .get(id)
            .map(|v| v.id.clone())
            .unwrap_or_else(|| format!("scene-{}", crate::history::unique_id()));
        let path = if let Some(old) = previous.get(id) {
            let suffix = old
                .path
                .strip_prefix(&format!("{old_parent}/"))
                .ok_or("Scene source parent differs")?;
            format!("{parent}/{suffix}")
        } else {
            available_scene_path(
                &parent,
                if scene.name.is_empty() {
                    id
                } else {
                    &scene.name
                },
                &occupied,
            )
        };
        validate_scene_path(&path)?;
        occupied.insert(path.to_lowercase());
        game.scene_assets
            .insert(id.clone(), SceneAssetBinding { id: asset, path });
    }
    Ok(candidate)
}

/// A new scene is ordinary authored data; no gameplay component is installed.
pub fn new_scene_source(
    document: &GraphDocument,
    name: &str,
    duplicate: Option<&str>,
) -> Result<(GraphDocument, String), String> {
    if name.trim().is_empty() {
        return Err("Enter a scene name".into());
    }
    let mut candidate = document.clone();
    let game = candidate
        .game
        .as_mut()
        .ok_or("Create a game project before adding a scene")?;
    game.validate_scene_data()?;
    let id = format!("scene-{}", crate::history::unique_id());
    let mut scene = if let Some(source) = duplicate {
        game.scenes
            .get(source)
            .ok_or("Source scene is missing")?
            .clone()
    } else {
        AuthoredScene::default()
    };
    scene.name = name.into();
    game.scenes.insert(id.clone(), scene);
    if !game.scene_assets.is_empty() {
        let parent = common_scene_parent(&game.scene_assets);
        let occupied = game
            .scene_assets
            .values()
            .map(|b| b.path.to_lowercase())
            .collect();
        let path = available_scene_path(&parent, name, &occupied);
        let asset = format!("scene-{}", crate::history::unique_id());
        game.scene_assets
            .insert(id.clone(), SceneAssetBinding { path, id: asset });
    }
    let candidate = externalize_scenes(&candidate, false)?;
    Ok((candidate, id))
}

pub(crate) fn decode_scene_asset(
    bytes: &[u8],
    scene_id: &str,
    binding: &SceneAssetBinding,
) -> Result<AuthoredScene, String> {
    let file: SceneFile = serde_json::from_value(crate::engine_migration::parse_source(bytes)?)
        .map_err(|e| e.to_string())?;
    if file.scene_asset_version != SCENE_STORAGE_VERSION
        || file.scene_id != scene_id
        || file.asset_id != binding.id
    {
        return Err("Scene file identity/version differs".into());
    }
    Ok(file.scene)
}

fn equivalent_project_metadata(a: &[u8], b: &[u8]) -> Result<bool, String> {
    fn descriptor(raw: &[u8]) -> Result<GraphDocument, String> {
        let mut value = crate::engine_migration::parse_source(raw)?;
        let storage = value
            .as_object_mut()
            .ok_or("Project must be an object")?
            .remove("scene_storage_version");
        if let Some(version) = storage {
            if version.as_u64() != Some(u64::from(SCENE_STORAGE_VERSION)) {
                return Err("Unsupported scene storage".into());
            }
            let game = value
                .get_mut("game")
                .and_then(Value::as_object_mut)
                .ok_or("Missing game")?;
            if game.contains_key("scenes") {
                return Err("Mixed scene storage".into());
            }
            game.insert("scenes".into(), serde_json::json!({}));
        }
        serde_json::from_value(value).map_err(|e| e.to_string())
    }
    Ok(descriptor(a)? == descriptor(b)?)
}

/// One unambiguous project DTO and current authored bodies, not stale disk copies.
/// Encoded project DTO and its map of relative scene paths to source bytes.
pub type EncodedProjectSources = (Vec<u8>, BTreeMap<String, Vec<u8>>);

pub fn encode_project_sources(document: &GraphDocument) -> Result<EncodedProjectSources, String> {
    let sources = bindings(document)?;
    let mut value = serde_json::to_value(document).map_err(|e| e.to_string())?;
    let mut files = BTreeMap::new();
    if !sources.is_empty() {
        value.as_object_mut().unwrap().insert(
            "scene_storage_version".into(),
            Value::from(SCENE_STORAGE_VERSION),
        );
        value
            .pointer_mut("/game")
            .and_then(Value::as_object_mut)
            .unwrap()
            .remove("scenes");
        let game = document.game.as_ref().unwrap();
        for (id, binding) in sources {
            let file = SceneFile {
                scene_asset_version: SCENE_STORAGE_VERSION,
                scene_id: id.clone(),
                asset_id: binding.id,
                scene: game.scenes[&id].clone(),
            };
            let mut bytes = serde_json::to_vec_pretty(&file).map_err(|e| e.to_string())?;
            bytes.push(b'\n');
            files.insert(binding.path, bytes);
        }
    }
    let mut bytes = if files.is_empty() {
        serde_json::to_vec_pretty(document)
    } else {
        serde_json::to_vec_pretty(&value)
    }
    .map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    Ok((bytes, files))
}

pub fn save_project_with_state(
    path: &Path,
    document: &GraphDocument,
    accepted: &ProjectFileState,
) -> Result<crate::AssetOperation, String> {
    let root = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    accepted.require_project_path(path)?;
    crate::AssetWorkspace::new(root, Vec::<String>::new())?
        .with_project_source(accepted.clone())
        .publish_project_source(document, document, "Save authored project")
}

pub(crate) fn prepare_project_sources(
    root: &Path,
    document: &GraphDocument,
    accepted: &ProjectFileState,
    preferred: &BTreeMap<String, Vec<u8>>,
) -> Result<ProjectSourcePlan, String> {
    accepted.verify(root)?;
    let (mut project, mut scenes) = encode_project_sources(document)?;
    let sources = bindings(document)?;
    for binding in sources.values() {
        let canonical = scenes
            .get(&binding.path)
            .ok_or("Scene serialization missing")?;
        let desired: SceneFile = serde_json::from_slice(canonical).map_err(|e| e.to_string())?;
        let original = if let Some(bytes) = preferred.get(&binding.path) {
            Some(bytes.clone())
        } else if let Some(path) = accepted.scene_ids.get(&binding.id) {
            Some(fs::read(source_target(root, path)?).map_err(|e| e.to_string())?)
        } else {
            None
        };
        if let Some(bytes) = original {
            let original: SceneFile =
                serde_json::from_value(crate::engine_migration::parse_source(&bytes)?)
                    .map_err(|e| e.to_string())?;
            if original == desired {
                scenes.insert(binding.path.clone(), bytes);
            } else if preferred.contains_key(&binding.path) {
                return Err("Recovery scene bytes differ from target authored data".into());
            }
        }
    }
    let original = if let Some(bytes) = preferred.get(&accepted.project) {
        Some(bytes.clone())
    } else if accepted
        .files
        .get(&accepted.project)
        .is_some_and(|v| v.is_some())
    {
        Some(fs::read(source_target(root, &accepted.project)?).map_err(|e| e.to_string())?)
    } else {
        None
    };
    if let Some(bytes) = original {
        if equivalent_project_metadata(&bytes, &project).unwrap_or(false) {
            project = bytes;
        } else if preferred.contains_key(&accepted.project) {
            return Err("Recovery project bytes differ from target source metadata".into());
        }
    }
    let mut changes: BTreeMap<_, _> = scenes.into_iter().map(|(p, b)| (p, Some(b))).collect();
    let current_paths: BTreeSet<_> = changes.keys().cloned().collect();
    for path in accepted.scene_paths.difference(&current_paths) {
        changes.insert(path.clone(), None);
    }
    if changes.contains_key(&accepted.project) {
        return Err("Scene file conflicts with project metadata".into());
    }
    changes.insert(accepted.project.clone(), Some(project));
    let mut expected = BTreeMap::new();
    let mut state = accepted.clone();
    state.scene_paths = current_paths;
    state.scene_ids = sources
        .values()
        .map(|b| (b.id.clone(), b.path.clone()))
        .collect();
    for (path, bytes) in &changes {
        source_target(root, path)?;
        expected.insert(
            path.clone(),
            accepted.files.get(path).cloned().unwrap_or(None),
        );
        state
            .files
            .insert(path.clone(), bytes.as_deref().map(hash_bytes));
        if let Some(bytes) = bytes {
            state.sizes.insert(path.clone(), bytes.len() as u64);
        } else {
            state.sizes.remove(path);
        }
    }
    changes.retain(|path, bytes| bytes.as_deref().map(hash_bytes) != expected[path]);
    Ok(ProjectSourcePlan {
        changes,
        expected,
        accepted: state,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use module_engine_core::{AuthoredObject, GameProject, Transform, WorldKind, WorldObject};
    use serde_json::json;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "module-scene-source-{}",
                crate::history::unique_id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn project(&self) -> PathBuf {
            self.0.join("project.json")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            crate::history::remove_test_directory(&self.0);
        }
    }
    fn document() -> GraphDocument {
        let mut d = module_engine_core::demo_document(WorldKind::Objects);
        let mut game = GameProject::empty();
        let mut a = AuthoredObject::new(WorldObject {
            id: 7,
            parent: None,
            transform: Transform::default(),
            color: [10, 20, 30, 255],
        });
        a.name = module_engine_core::AuthoredName::Text("Crate / ящик".into());
        a.components.insert(
            "future.namespace".into(),
            json!({"nullable":null,"array":[1,{"x":false}]}),
        );
        let mut b = AuthoredObject::new(WorldObject {
            id: 9,
            parent: Some(7),
            transform: Transform::default(),
            color: [90, 80, 70, 255],
        });
        b.references.insert("target".into(), 7);
        b.name = module_engine_core::AuthoredName::Null;
        game.scenes.get_mut("empty").unwrap().objects = vec![a, b];
        let mut second = game.scenes["empty"].clone();
        second.name = "Second".into();
        game.scenes.insert("second".into(), second);
        d.game = Some(game);
        d
    }
    fn write(f: &Fixture, d: &GraphDocument) -> ProjectFileState {
        let initial = ProjectFileState::observe(&f.project(), d).unwrap();
        save_project_with_state(&f.project(), d, &initial)
            .unwrap()
            .source_files
            .unwrap()
    }
    #[test]
    fn external_files_have_one_authority_and_exact_hydrated_roundtrip() {
        let f = Fixture::new();
        let d = externalize_scenes(&document(), false).unwrap();
        let state = write(&f, &d);
        let raw: Value = serde_json::from_slice(&fs::read(f.project()).unwrap()).unwrap();
        assert_eq!(raw["schema_version"], 4);
        assert_eq!(raw["scene_storage_version"], 1);
        assert!(raw.pointer("/game/scenes").is_none());
        assert_eq!(state.scene_paths.len(), 2);
        assert_eq!(read_project_source(&f.project()).unwrap().document, d);
        let checkpoint = crate::CheckpointStore::new(f.0.join("checkpoints"));
        let saved = checkpoint.save("Complete hydrated scenes", &d).unwrap();
        assert_eq!(checkpoint.load(&saved.id).unwrap(), d);
        assert_eq!(
            d.game.as_ref().unwrap().scenes["empty"].objects[0].components["future.namespace"]["nullable"],
            Value::Null
        );
    }
    #[test]
    fn inline_open_save_retains_storage_and_absent_object_names() {
        let f = Fixture::new();
        let mut d = document();
        d.game
            .as_mut()
            .unwrap()
            .scenes
            .get_mut("empty")
            .unwrap()
            .objects[0]
            .name = module_engine_core::AuthoredName::Absent;
        crate::save_document_with_backup(&f.project(), &d).unwrap();
        let before = fs::read(f.project()).unwrap();
        let loaded = read_project_source(&f.project()).unwrap();
        assert_eq!(fs::read(f.project()).unwrap(), before);
        assert_eq!(loaded.document, d);
        save_project_with_state(&f.project(), &d, &loaded.files).unwrap();
        let raw: Value = serde_json::from_slice(&fs::read(f.project()).unwrap()).unwrap();
        assert!(raw.get("scene_storage_version").is_none());
        assert!(raw.pointer("/game/scenes/empty/objects/0/name").is_none());
        assert_eq!(
            raw["game"]["scenes"]["empty"]["objects"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }
    #[test]
    fn ordinary_save_preserves_bindings_and_rejects_external_edits_without_publication() {
        let f = Fixture::new();
        let d = externalize_scenes(&document(), false).unwrap();
        let state = write(&f, &d);
        let mut next = d.clone();
        next.game
            .as_mut()
            .unwrap()
            .scenes
            .get_mut("empty")
            .unwrap()
            .objects[0]
            .name = module_engine_core::AuthoredName::Text("Changed".into());
        let op = save_project_with_state(&f.project(), &next, &state).unwrap();
        let state = op.source_files.unwrap();
        assert_eq!(read_project_source(&f.project()).unwrap().document, next);
        assert_eq!(
            next.game.as_ref().unwrap().scene_assets,
            d.game.as_ref().unwrap().scene_assets
        );
        let scene_path =
            f.0.join(&next.game.as_ref().unwrap().scene_assets["empty"].path);
        let mut changed = fs::read(&scene_path).unwrap();
        changed.push(b' ');
        fs::write(&scene_path, &changed).unwrap();
        let project_before = fs::read(f.project()).unwrap();
        assert!(
            save_project_with_state(&f.project(), &d, &state)
                .unwrap_err()
                .contains("changed outside")
        );
        assert_eq!(fs::read(&scene_path).unwrap(), changed);
        assert_eq!(fs::read(f.project()).unwrap(), project_before);
    }
    #[test]
    fn external_reader_rejects_missing_identity_mixed_and_unsupported_storage() {
        let f = Fixture::new();
        let d = externalize_scenes(&document(), false).unwrap();
        write(&f, &d);
        let original = fs::read(f.project()).unwrap();
        let mut raw: Value = serde_json::from_slice(&original).unwrap();
        raw["game"]["scenes"] = json!({});
        fs::write(f.project(), serde_json::to_vec(&raw).unwrap()).unwrap();
        assert!(
            read_project_source(&f.project())
                .err()
                .unwrap()
                .contains("cannot also embed")
        );
        raw.as_object_mut().unwrap().remove("game");
        raw["scene_storage_version"] = json!(2);
        fs::write(f.project(), serde_json::to_vec(&raw).unwrap()).unwrap();
        assert!(
            read_project_source(&f.project())
                .err()
                .unwrap()
                .contains("Unsupported scene_storage_version")
        );
        fs::write(f.project(), &original).unwrap();
        let binding = &d.game.as_ref().unwrap().scene_assets["empty"];
        let scene_path = f.0.join(&binding.path);
        let bytes = fs::read(&scene_path).unwrap();
        let mut file: Value = serde_json::from_slice(&bytes).unwrap();
        file["asset_id"] = json!("different");
        fs::write(&scene_path, serde_json::to_vec(&file).unwrap()).unwrap();
        assert!(
            read_project_source(&f.project())
                .err()
                .unwrap()
                .contains("identity differs")
        );
        fs::remove_file(scene_path).unwrap();
        assert!(read_project_source(&f.project()).is_err());
    }
    #[test]
    fn aliases_escape_and_conflicting_target_refuse_before_source_changes() {
        let f = Fixture::new();
        let original = document();
        crate::save_document_with_backup(&f.project(), &original).unwrap();
        let state = ProjectFileState::observe(&f.project(), &original).unwrap();
        let mut d = externalize_scenes(&original, false).unwrap();
        let game = d.game.as_mut().unwrap();
        game.scene_assets.get_mut("empty").unwrap().path = "../outside.scene.json".into();
        assert!(save_project_with_state(&f.project(), &d, &state).is_err());
        d = externalize_scenes(&original, false).unwrap();
        let path = d.game.as_ref().unwrap().scene_assets["empty"].path.clone();
        let target = f.0.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, b"unowned").unwrap();
        let before = fs::read(f.project()).unwrap();
        assert!(save_project_with_state(&f.project(), &d, &state).is_err());
        assert_eq!(fs::read(f.project()).unwrap(), before);
        assert_eq!(fs::read(target).unwrap(), b"unowned");
        let game = d.game.as_mut().unwrap();
        let path = game.scene_assets["empty"].path.to_uppercase();
        game.scene_assets.get_mut("second").unwrap().path = path;
        assert!(game.validate_scene_data().is_err());
    }
    #[test]
    fn fresh_source_group_keeps_ids_but_does_not_share_mutable_files() {
        let d = externalize_scenes(&document(), false).unwrap();
        let copy = externalize_scenes(&d, true).unwrap();
        let a = d.game.as_ref().unwrap();
        let b = copy.game.as_ref().unwrap();
        assert_eq!(a.scenes, b.scenes);
        for id in a.scenes.keys() {
            assert_eq!(a.scene_assets[id].id, b.scene_assets[id].id);
            assert_ne!(a.scene_assets[id].path, b.scene_assets[id].path);
        }
    }
    #[test]
    fn explicit_source_creation_makes_fresh_nested_parent_and_rejects_invalid_data_before_io() {
        let fixture = Fixture::new();
        let doc = externalize_scenes(&document(), false).unwrap();
        let path = fixture.0.join("new-parent/nested/project.json");
        assert!(!path.parent().unwrap().exists());
        crate::save_document_with_backup(&path, &doc).unwrap();
        let loaded = read_project_source(&path).unwrap();
        assert!(loaded.document == doc);
        let value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["scene_storage_version"], 1);
        assert!(value["game"].get("scenes").is_none());
        for binding in doc.game.as_ref().unwrap().scene_assets.values() {
            assert!(path.parent().unwrap().join(&binding.path).is_file());
        }
        let invalid = fixture.0.join("invalid-parent/nested/project.json");
        let mut broken = doc.clone();
        broken
            .game
            .as_mut()
            .unwrap()
            .scene_assets
            .values_mut()
            .next()
            .unwrap()
            .path = "assets/wrong.scene.json".into();
        assert!(crate::save_document_with_backup(&invalid, &broken).is_err());
        assert!(!fixture.0.join("invalid-parent").exists());
        let bytes = fs::read(&path).unwrap();
        fs::write(fixture.0.join("blocked"), b"user bytes").unwrap();
        assert!(
            crate::save_document_with_backup(&fixture.0.join("blocked/nested/project.json"), &doc)
                .is_err()
        );
        assert_eq!(fs::read(fixture.0.join("blocked")).unwrap(), b"user bytes");
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
    #[test]
    fn new_duplicate_delete_and_restart_trash_restore_preserve_source_identity() {
        let f = Fixture::new();
        let original = externalize_scenes(&document(), false).unwrap();
        let state = write(&f, &original);
        let (duplicated, id) = new_scene_source(&original, "Copied room", Some("second")).unwrap();
        let copy = &duplicated.game.as_ref().unwrap().scenes[&id];
        assert_eq!(
            copy.objects,
            original.game.as_ref().unwrap().scenes["second"].objects
        );
        let state = save_project_with_state(&f.project(), &duplicated, &state)
            .unwrap()
            .source_files
            .unwrap();
        let workspace = crate::AssetWorkspace::new(&f.0, Vec::<String>::new())
            .unwrap()
            .with_project_source(state);
        let deleted = workspace.delete_scene_source(&duplicated, &id).unwrap();
        assert!(
            !read_project_source(&f.project())
                .unwrap()
                .document
                .game
                .as_ref()
                .unwrap()
                .scenes
                .contains_key(&id)
        );
        let trash_id = deleted.record.id.clone();
        let loaded = read_project_source(&f.project()).unwrap();
        let workspace = crate::AssetWorkspace::new(&f.0, Vec::<String>::new())
            .unwrap()
            .with_project_source(loaded.files);
        assert_eq!(
            workspace.scene_trash_identity(&trash_id).unwrap(),
            Some(id.clone())
        );
        let restored = workspace
            .restore_scene_source(&loaded.document, &trash_id)
            .unwrap();
        assert_eq!(restored.document, duplicated);
        assert_eq!(
            read_project_source(&f.project()).unwrap().document,
            duplicated
        );
        assert_eq!(
            restored.document.game.as_ref().unwrap().scene_assets[&id],
            duplicated.game.as_ref().unwrap().scene_assets[&id]
        );
        assert!(workspace.delete_scene_source(&duplicated, "empty").is_err());
    }
    #[test]
    fn immutable_asset_move_publishes_references_in_scene_files_and_project_together() {
        let f = Fixture::new();
        fs::create_dir_all(f.0.join("assets")).unwrap();
        fs::write(
            f.0.join("assets/original.png"),
            b"project-tools resource bytes",
        )
        .unwrap();
        let mut d = document();
        let mut manifest = crate::AssetManifest::default();
        manifest
            .register("texture", "assets/original.png", &f.0)
            .unwrap();
        d.asset_manifest = serde_json::to_value(manifest).unwrap();
        d.game
            .as_mut()
            .unwrap()
            .scenes
            .get_mut("empty")
            .unwrap()
            .objects[0]
            .components
            .insert(
                "material".into(),
                json!({"base_color_texture":"assets/original.png"}),
            );
        let before = externalize_scenes(&d, false).unwrap();
        let state = write(&f, &before);
        let workspace = crate::AssetWorkspace::new(&f.0, Vec::<String>::new())
            .unwrap()
            .with_project_source(state);
        let moved = workspace
            .move_asset(&before, "texture", "assets/moved.png")
            .unwrap();
        assert!(moved.record.files.iter().any(|v| v.path == "project.json"));
        let scene_path = &before.game.as_ref().unwrap().scene_assets["empty"].path;
        assert!(moved.record.files.iter().any(|v| v.path == *scene_path));
        let loaded = read_project_source(&f.project()).unwrap();
        assert_eq!(loaded.document, moved.document);
        assert_eq!(
            loaded.document.game.as_ref().unwrap().scenes["empty"].objects[0].components["material"]
                ["base_color_texture"],
            json!("assets/moved.png")
        );
        let undone = crate::AssetWorkspace::new(&f.0, Vec::<String>::new())
            .unwrap()
            .with_project_source(moved.source_files.unwrap())
            .replay_project_operation(&moved.record, &moved.document, false)
            .unwrap();
        assert_eq!(undone.document, before);
        assert_eq!(read_project_source(&f.project()).unwrap().document, before);
        assert!(f.0.join("assets/original.png").is_file());
        assert!(!f.0.join("assets/moved.png").exists());
    }
    #[test]
    fn unchanged_save_does_not_rewrite_any_scene_or_publish_another_journal() {
        let f = Fixture::new();
        let d = externalize_scenes(&document(), false).unwrap();
        let state = write(&f, &d);
        let count = fs::read_dir(f.0.join(".asset-history")).unwrap().count();
        let operation = save_project_with_state(&f.project(), &d, &state).unwrap();
        assert!(operation.record.files.is_empty());
        assert_eq!(
            fs::read_dir(f.0.join(".asset-history")).unwrap().count(),
            count
        );
        let raw: Value = serde_json::from_slice(
            &fs::read(f.0.join(&d.game.as_ref().unwrap().scene_assets["empty"].path)).unwrap(),
        )
        .unwrap();
        assert_eq!(raw["scene"]["objects"][1]["name"], Value::Null);
        assert!(
            raw["scene"]["objects"][1]
                .as_object()
                .unwrap()
                .contains_key("name")
        );
    }
    #[test]
    fn each_scene_move_publication_failure_rolls_back_all_authored_files() {
        for at in 0..3 {
            let f = Fixture::new();
            let before = externalize_scenes(&document(), false).unwrap();
            let state = write(&f, &before);
            let binding = &before.game.as_ref().unwrap().scene_assets["empty"];
            let mut candidate = before.clone();
            candidate
                .game
                .as_mut()
                .unwrap()
                .scene_assets
                .get_mut("empty")
                .unwrap()
                .path = format!("scenes/new-place/{}.scene.json", binding.id);
            let new_path = candidate.game.as_ref().unwrap().scene_assets["empty"]
                .path
                .clone();
            let workspace = crate::AssetWorkspace::new(&f.0, Vec::<String>::new())
                .unwrap()
                .with_project_source(state.clone());
            let result = crate::asset_workspace::with_source_write_failure(at, || {
                workspace.publish_project_source(&before, &candidate, "Move scene")
            });
            assert!(result.err().unwrap().contains("Injected"), "boundary {at}");
            state.verify(&f.0).unwrap();
            assert_eq!(read_project_source(&f.project()).unwrap().document, before);
            assert!(!f.0.join(new_path).exists());
            assert!(f.0.join(&binding.path).is_file());
        }
    }
    #[test]
    fn source_filename_rejects_portable_aliases_and_device_names() {
        for path in [
            "scenes/room./a.scene.json",
            "scenes/NUL.scene.json",
            "scenes/a/CON.txt.scene.json",
            "scenes/a/room .scene.json?",
            "scenes/../other.scene.json",
        ] {
            assert!(validate_scene_path(path).is_err(), "{path}");
        }
        assert_eq!(
            validate_scene_path("scenes/комната/Room.scene.json").unwrap(),
            "scenes/комната/Room.scene.json"
        );
    }
    #[test]
    fn observe_inline_rejects_a_second_different_read_and_state_is_full_path_scoped() {
        let f = Fixture::new();
        let initial = document();
        crate::save_document_with_backup(&f.project(), &initial).unwrap();
        let loaded = read_project_source(&f.project()).unwrap();
        let mut external = initial.clone();
        external.name = "External author change".into();
        crate::save_document_with_backup(&f.project(), &external).unwrap();
        assert!(ProjectFileState::observe(&f.project(), &initial).is_err());
        assert!(save_project_with_state(&f.project(), &initial, &loaded.files).is_err());
        assert_eq!(
            read_project_source(&f.project()).unwrap().document,
            external
        );
        let other = Fixture::new();
        crate::save_document_with_backup(&other.project(), &initial).unwrap();
        let accepted = read_project_source(&f.project()).unwrap().files;
        assert!(save_project_with_state(&other.project(), &initial, &accepted).is_err());
        let wrong = f.0.join("another.json");
        assert!(save_project_with_state(&wrong, &initial, &accepted).is_err());
        assert!(!wrong.exists());
        assert_eq!(
            read_project_source(&other.project()).unwrap().document,
            initial
        );
    }
    #[test]
    fn hand_formatted_source_survives_noop_move_undo_and_trash_restore_byte_exact() {
        let f = Fixture::new();
        let d = externalize_scenes(&document(), false).unwrap();
        write(&f, &d);
        let binding = &d.game.as_ref().unwrap().scene_assets["second"];
        let path = f.0.join(&binding.path);
        let value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let mut raw = vec![0xef, 0xbb, 0xbf];
        raw.extend(serde_json::to_vec(&value).unwrap());
        raw.extend(b" \r\n");
        fs::write(&path, &raw).unwrap();
        let loaded = read_project_source(&f.project()).unwrap();
        let saved = save_project_with_state(&f.project(), &loaded.document, &loaded.files).unwrap();
        assert!(saved.record.files.is_empty());
        assert_eq!(fs::read(&path).unwrap(), raw);
        let workspace = crate::AssetWorkspace::new(&f.0, Vec::<String>::new())
            .unwrap()
            .with_project_source(saved.source_files.unwrap());
        let moved = workspace
            .edit_scene_source(
                &d,
                &SceneSourceEdit::Move {
                    scene: "second".into(),
                    folder: "scenes/hand-formatted".into(),
                },
            )
            .unwrap();
        let new_path =
            f.0.join(&moved.document.game.as_ref().unwrap().scene_assets["second"].path);
        assert_eq!(fs::read(&new_path).unwrap(), raw);
        let undone = crate::AssetWorkspace::new(&f.0, Vec::<String>::new())
            .unwrap()
            .with_project_source(moved.source_files.unwrap())
            .replay_project_operation(&moved.record, &moved.document, false)
            .unwrap();
        assert_eq!(fs::read(&path).unwrap(), raw);
        let deleted = crate::AssetWorkspace::new(&f.0, Vec::<String>::new())
            .unwrap()
            .with_project_source(undone.source_files.unwrap())
            .delete_scene_source(&d, "second")
            .unwrap();
        let restored = crate::AssetWorkspace::new(&f.0, Vec::<String>::new())
            .unwrap()
            .with_project_source(deleted.source_files.unwrap())
            .restore_scene_source(&deleted.document, &deleted.record.id)
            .unwrap();
        assert_eq!(fs::read(&path).unwrap(), raw);
        assert_eq!(restored.document, d);
    }
    #[test]
    fn file_move_history_after_later_save_uses_accepted_generation_and_hydrated_snapshot() {
        let f = Fixture::new();
        let before = externalize_scenes(&document(), false).unwrap();
        let initial = write(&f, &before);
        let mut after = before.clone();
        let binding = after
            .game
            .as_mut()
            .unwrap()
            .scene_assets
            .get_mut("empty")
            .unwrap();
        binding.path = format!("scenes/renamed/{}.scene.json", binding.id);
        let workspace = crate::AssetWorkspace::new(&f.0, Vec::<String>::new())
            .unwrap()
            .with_project_source(initial);
        let moved = workspace
            .publish_project_source(&before, &after, "Move scene file")
            .unwrap();
        let record = moved.record;
        let moved_state = moved.source_files.unwrap();
        assert_eq!(read_project_source(&f.project()).unwrap().document, after);
        let mut later = after.clone();
        later
            .game
            .as_mut()
            .unwrap()
            .scenes
            .get_mut("empty")
            .unwrap()
            .objects[0]
            .name = module_engine_core::AuthoredName::Text("Saved later".into());
        let late_state = save_project_with_state(&f.project(), &later, &moved_state)
            .unwrap()
            .source_files
            .unwrap();
        // Ordinary in-memory Undo returned to the move boundary, while disk
        // legitimately still contains our later save.
        let undone = crate::AssetWorkspace::new(&f.0, Vec::<String>::new())
            .unwrap()
            .with_project_source(late_state)
            .replay_project_operation(&record, &after, false)
            .unwrap();
        assert_eq!(undone.document, before);
        assert_eq!(read_project_source(&f.project()).unwrap().document, before);
        let redone = crate::AssetWorkspace::new(&f.0, Vec::<String>::new())
            .unwrap()
            .with_project_source(undone.source_files.unwrap())
            .replay_project_operation(&record, &before, true)
            .unwrap();
        assert_eq!(redone.document, after);
        assert_eq!(read_project_source(&f.project()).unwrap().document, after);
        let current_path =
            f.0.join(&after.game.as_ref().unwrap().scene_assets["empty"].path);
        let mut external = fs::read(&current_path).unwrap();
        external.push(b' ');
        fs::write(&current_path, &external).unwrap();
        let old_json = fs::read(f.project()).unwrap();
        assert!(
            crate::AssetWorkspace::new(&f.0, Vec::<String>::new())
                .unwrap()
                .with_project_source(redone.source_files.unwrap())
                .replay_project_operation(&record, &after, false)
                .is_err()
        );
        assert_eq!(fs::read(current_path).unwrap(), external);
        assert_eq!(fs::read(f.project()).unwrap(), old_json);
    }
}
