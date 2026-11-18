//! Read-only source descriptions. These are not authored contracts or activity flags.
use crate::{CompositionContext, GraphDocument};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityOwner {
    Engine,
    Gameplay,
    Project,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    SourceContract,
    WorldStorage,
    Renderer,
    RenderPass,
    ExecutableSystem,
    ComponentData,
    HostContribution,
    Library,
    Service,
    EditorTool,
}

/// Navigation hints only. Permission, candidate validation and rebuild rules stay
/// with their existing owners; a hint never grants an editing capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityConfiguration {
    RootGraph,
    RenderGraph,
    ExecutionGraph,
    ComponentFields,
    ProjectSettings,
    RustSource,
    HlslSource,
    NativeSource,
    ModuleTool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CapabilityMetadata {
    pub label: String,
    pub purpose: String,
    pub owner: CapabilityOwner,
    pub kind: CapabilityKind,
    pub source_package: Option<String>,
    pub configuration: BTreeSet<CapabilityConfiguration>,
    /// Declared source dependencies, not executable systems or extra module pins.
    pub libraries: BTreeSet<String>,
}
impl CapabilityMetadata {
    pub fn new(
        label: &str,
        purpose: &str,
        owner: CapabilityOwner,
        kind: CapabilityKind,
        package: &str,
    ) -> Self {
        Self {
            label: label.into(),
            purpose: purpose.into(),
            owner,
            kind,
            source_package: Some(package.into()),
            configuration: BTreeSet::new(),
            libraries: BTreeSet::new(),
        }
    }
    pub fn configured_by(
        mut self,
        routes: impl IntoIterator<Item = CapabilityConfiguration>,
    ) -> Self {
        self.configuration.extend(routes);
        self
    }
    pub fn uses_libraries(mut self, libraries: impl IntoIterator<Item = String>) -> Self {
        self.libraries.extend(libraries);
        self
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.label.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self
                .source_package
                .as_ref()
                .is_none_or(|p| p.trim().is_empty())
            || self.libraries.iter().any(|id| id.trim().is_empty())
        {
            return Err("Source capability metadata needs a label, purpose and package; library IDs cannot be empty".into());
        }
        Ok(())
    }
    pub fn unknown(label: &str, purpose: &str, kind: CapabilityKind) -> Self {
        Self {
            label: label.into(),
            purpose: purpose.into(),
            owner: CapabilityOwner::Unknown,
            kind,
            source_package: None,
            configuration: BTreeSet::new(),
            libraries: BTreeSet::new(),
        }
    }
}

pub(crate) fn describe_capability(
    descriptions: &mut BTreeMap<String, CapabilityMetadata>,
    id: &str,
    metadata: CapabilityMetadata,
) -> Result<(), String> {
    metadata.validate()?;
    if descriptions.get(id).is_some_and(|old| old != &metadata) {
        return Err(format!("Conflicting source capability metadata for '{id}'"));
    }
    descriptions.insert(id.into(), metadata);
    Ok(())
}

/// Each fact identifies its evidence. None is unknown, not false.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CapabilityFact {
    pub value: Option<bool>,
    pub evidence: String,
}
impl CapabilityFact {
    pub fn known(value: bool, evidence: impl Into<String>) -> Self {
        Self {
            value: Some(value),
            evidence: evidence.into(),
        }
    }
    pub fn unknown(evidence: impl Into<String>) -> Self {
        Self {
            value: None,
            evidence: evidence.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityDependencyKind {
    ActiveModule,
    PrecedingSystem,
    Library,
    ActivatedByAny,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct CapabilityDependency {
    pub kind: CapabilityDependencyKind,
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CapabilityRecord {
    pub id: String,
    pub metadata: CapabilityMetadata,
    pub version: Option<String>,
    pub installed: bool,
    /// Offered source capability, not permission, compilation or device readiness.
    pub available: CapabilityFact,
    /// Authored participation/request/dependency, never a running-instance claim.
    pub participation: CapabilityFact,
    pub dependencies: BTreeSet<CapabilityDependency>,
    pub provided_components: BTreeSet<String>,
    pub consumed_components: BTreeSet<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct CapabilityInventory {
    pub records: Vec<CapabilityRecord>,
    pub diagnostics: Vec<String>,
}

impl CompositionContext {
    /// Inspect the existing installation and canonical activity calculation.
    /// `offered` is supplied from a checked Engine profile by its owner. None
    /// means no checked availability evidence, not an unrestricted profile.
    pub fn capability_inventory(
        &self,
        document: &GraphDocument,
        offered: Option<&BTreeSet<String>>,
    ) -> CapabilityInventory {
        let activity = self.activity(document);
        let mut inventory = CapabilityInventory::default();
        if let Err(error) = &activity {
            inventory.diagnostics.push(error.clone());
        }
        let mut ids: BTreeSet<_> = self
            .modules
            .descriptors()
            .iter()
            .map(|d| d.id.clone())
            .collect();
        ids.extend(crate::effective_modules(document));
        if let Some(game) = &document.game {
            ids.extend(
                game.execution
                    .stages
                    .iter()
                    .flat_map(|s| &s.nodes)
                    .map(|n| n.module.clone()),
            );
        }
        for id in ids {
            let descriptor = self.modules.descriptor(&id);
            let installed = descriptor.is_some();
            let kind = if self.systems.descriptor(&id).is_some() {
                CapabilityKind::ExecutableSystem
            } else if self.modules.is_world_provider(&id) {
                CapabilityKind::WorldStorage
            } else if self.modules.is_renderer(&id) {
                CapabilityKind::Renderer
            } else {
                CapabilityKind::SourceContract
            };
            let metadata = self
                .modules
                .capability_metadata(&id)
                .cloned()
                .unwrap_or_else(|| {
                    CapabilityMetadata::unknown(
                        descriptor.map_or(id.as_str(), |d| &d.label),
                        descriptor.map_or("Source contract is not installed", |d| &d.description),
                        kind,
                    )
                });
            let mut dependencies = BTreeSet::new();
            if let Some(required) = self.systems.module_dependencies().get(&id) {
                dependencies.extend(required.iter().cloned().map(|id| CapabilityDependency {
                    kind: CapabilityDependencyKind::ActiveModule,
                    id,
                }));
            }
            let consumed_components =
                self.systems
                    .descriptor(&id)
                    .map_or_else(BTreeSet::new, |d| {
                        dependencies.extend(d.requires.iter().cloned().map(|id| {
                            CapabilityDependency {
                                kind: CapabilityDependencyKind::PrecedingSystem,
                                id,
                            }
                        }));
                        d.components.iter().cloned().collect()
                    });
            dependencies.extend(metadata.libraries.iter().cloned().map(|id| {
                CapabilityDependency {
                    kind: CapabilityDependencyKind::Library,
                    id,
                }
            }));
            inventory.records.push(CapabilityRecord {
                version: self.module_version(&id).map(Into::into),
                available: if !installed {
                    CapabilityFact::known(false, "No installed source contract")
                } else {
                    offered.map_or_else(
                        || {
                            CapabilityFact::unknown(
                                "No checked Engine profile module library supplied",
                            )
                        },
                        |offered| {
                            CapabilityFact::known(
                                offered.contains(&id),
                                "Engine profile module library",
                            )
                        },
                    )
                },
                participation: match &activity {
                    Ok(activity) => CapabilityFact::known(
                        activity.modules.contains(&id),
                        "CompositionContext::activity; authored participation only",
                    ),
                    Err(error) => CapabilityFact::unknown(format!(
                        "Authored participation could not be determined: {error}"
                    )),
                },
                provided_components: self
                    .systems
                    .components()
                    .schemas()
                    .filter(|s| s.owner_module == id)
                    .map(|s| s.id.clone())
                    .collect(),
                id,
                metadata,
                installed,
                dependencies,
                consumed_components,
            });
        }
        inventory
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    fn record<'a>(inventory: &'a CapabilityInventory, id: &str) -> &'a CapabilityRecord {
        inventory.records.iter().find(|r| r.id == id).unwrap()
    }

    #[test]
    fn inspection_uses_existing_activity_without_factories_or_document_changes() {
        let called = Arc::new(AtomicUsize::new(0));
        let counter = called.clone();
        let mut systems = SystemRegistry::builtin();
        systems
            .register(
                SystemDescriptor {
                    id: "fixture.step".into(),
                    state_schema: "fixture/1".into(),
                    components: vec![],
                    ports: vec![],
                    requires: vec![],
                },
                move |_| {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Err("Inspection must not construct a system".into())
                },
            )
            .unwrap();
        systems
            .describe_module(
                "fixture.step",
                CapabilityMetadata::new(
                    "Fixture step",
                    "A source-owned test operation",
                    CapabilityOwner::Project,
                    CapabilityKind::ExecutableSystem,
                    "fixture",
                )
                .uses_libraries(["fixture.library".into()]),
            )
            .unwrap();
        let composition = CompositionContext::new(ModuleRegistry::builtin(), systems).unwrap();
        let mut document = demo_document(WorldKind::Ecs);
        let mut game = GameProject::empty();
        game.execution = ExecutionGraph {
            resources: vec![],
            stages: vec![SystemStage {
                name: "Step".into(),
                nodes: vec![SystemNode {
                    id: "run".into(),
                    module: "fixture.step".into(),
                    params: serde_json::json!({"opaque":null}),
                    bindings: BTreeMap::new(),
                    after: vec![],
                }],
            }],
        };
        document.game = Some(game);
        let before = serde_json::to_vec(&document).unwrap();
        let versions = composition.module_versions();
        let offered = versions.keys().cloned().collect();
        let first = composition.capability_inventory(&document, Some(&offered));
        assert_eq!(
            first,
            composition.capability_inventory(&document, Some(&offered))
        );
        assert!(first.diagnostics.is_empty(), "{:?}", first.diagnostics);
        assert_eq!(record(&first, "world.ecs").participation.value, Some(true));
        assert_eq!(
            record(&first, "world.objects").participation.value,
            Some(false)
        );
        assert_eq!(
            record(&first, "fixture.step").participation.value,
            Some(true)
        );
        assert_eq!(
            record(&first, "fixture.step").metadata.owner,
            CapabilityOwner::Project
        );
        assert!(
            record(&first, "fixture.step")
                .dependencies
                .contains(&CapabilityDependency {
                    kind: CapabilityDependencyKind::Library,
                    id: "fixture.library".into()
                })
        );
        assert!(composition.systems.descriptor("fixture.library").is_none());
        assert!(composition.modules.descriptor("fixture.library").is_none());
        assert_eq!(called.load(Ordering::SeqCst), 0);
        assert_eq!(before, serde_json::to_vec(&document).unwrap());
        assert_eq!(versions, composition.module_versions());
    }

    #[test]
    fn unknown_evidence_is_not_a_known_empty_profile_or_inactive_graph() {
        let composition =
            CompositionContext::new(ModuleRegistry::builtin(), SystemRegistry::builtin()).unwrap();
        let mut document = demo_document(WorldKind::Ecs);
        let unknown = composition.capability_inventory(&document, None);
        assert_eq!(record(&unknown, "world.ecs").available.value, None);
        let empty = composition.capability_inventory(&document, Some(&BTreeSet::new()));
        assert_eq!(record(&empty, "world.ecs").available.value, Some(false));
        assert_eq!(record(&empty, "world.ecs").participation.value, Some(true));
        let mut game = GameProject::empty();
        game.execution.stages = vec![SystemStage {
            name: "Missing".into(),
            nodes: vec![SystemNode {
                id: "missing".into(),
                module: "foreign.uninstalled".into(),
                params: serde_json::Value::Null,
                bindings: BTreeMap::new(),
                after: vec![],
            }],
        }];
        document.game = Some(game);
        let invalid = composition.capability_inventory(&document, Some(&BTreeSet::new()));
        assert!(!invalid.diagnostics.is_empty());
        assert!(
            invalid
                .records
                .iter()
                .all(|r| r.participation.value.is_none())
        );
        assert!(!record(&invalid, "foreign.uninstalled").installed);
        assert_eq!(
            record(&invalid, "foreign.uninstalled").metadata.owner,
            CapabilityOwner::Unknown
        );
    }

    #[test]
    fn third_party_ownership_is_not_inferred_and_conflicting_metadata_is_rejected() {
        let mut systems = SystemRegistry::builtin();
        systems
            .register(
                SystemDescriptor {
                    id: "render.quest".into(),
                    state_schema: "quest/1".into(),
                    components: vec![],
                    ports: vec![],
                    requires: vec![],
                },
                |_| Err("Not executed".into()),
            )
            .unwrap();
        let mut composition = CompositionContext::new(ModuleRegistry::builtin(), systems).unwrap();
        let document = demo_document(WorldKind::Ecs);
        let inventory = composition.capability_inventory(&document, None);
        let unknown = record(&inventory, "render.quest");
        assert_eq!(unknown.metadata.owner, CapabilityOwner::Unknown);
        assert_eq!(unknown.metadata.kind, CapabilityKind::ExecutableSystem);
        assert_eq!(unknown.metadata.source_package, None);
        let metadata = CapabilityMetadata::new(
            "Quest rules",
            "Update quest goals",
            CapabilityOwner::Gameplay,
            CapabilityKind::ExecutableSystem,
            "quest-package",
        );
        composition
            .modules
            .describe_module("render.quest", metadata.clone())
            .unwrap();
        let mut conflicting = metadata.clone();
        conflicting.owner = CapabilityOwner::Engine;
        assert!(
            composition
                .modules
                .describe_module("render.quest", conflicting)
                .is_err()
        );
        assert_eq!(
            composition.modules.capability_metadata("render.quest"),
            Some(&metadata)
        );
        assert!(
            composition
                .modules
                .describe_module("not.installed", metadata)
                .is_err()
        );
    }
}
