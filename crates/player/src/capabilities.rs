//! Read-only join of source registries. No service/system/renderer factory runs.
use module_engine_core::*;
use module_engine_host_services::HostServiceRegistry;
use module_engine_project::EngineProfile;
use std::collections::BTreeSet;

pub fn capability_inventory(
    document: &GraphDocument,
    composition: &CompositionContext,
    services: &HostServiceRegistry,
) -> CapabilityInventory {
    let profile = EngineProfile::from_document(document).and_then(|profile| {
        profile.validate_document(document, composition)?;
        Ok(profile)
    });
    let offered = profile
        .as_ref()
        .ok()
        .map(|p| &p.blueprint.available_modules);
    let mut inventory = composition.capability_inventory(document, offered);
    if let Err(error) = &profile {
        inventory
            .diagnostics
            .push(format!("Engine profile availability is unknown: {error}"));
    }
    let activity = composition.activity(document);
    for contribution in composition.systems.game_contributions() {
        let metadata = composition
            .systems
            .contribution_metadata(&contribution.id)
            .cloned()
            .unwrap_or_else(|| {
                CapabilityMetadata::unknown(
                    &contribution.id,
                    "Installed host hooks; purpose and ownership not described by their source",
                    CapabilityKind::HostContribution,
                )
            });
        inventory.records.push(CapabilityRecord {
            id: contribution.id.clone(), metadata, installed: true, version: None,
            available: offered.map_or_else(
                || CapabilityFact::unknown("No checked Engine profile module library"),
                |offered| CapabilityFact::known(contribution.activation_modules.iter().any(|id| offered.contains(id)), "At least one contribution activation module offered by the Engine profile")),
            participation: match &activity {
                Ok(_) => CapabilityFact::known(document.game.as_ref().is_some_and(|g| contribution.is_active(&g.execution)),
                    "GameContribution activation modules in the authored schedule; UI installation and running hooks not implied"),
                Err(error) => CapabilityFact::unknown(error),
            },
            dependencies: contribution.activation_modules.iter().cloned().map(|id| CapabilityDependency { kind: CapabilityDependencyKind::ActivatedByAny, id }).collect(),
            provided_components: BTreeSet::new(), consumed_components: BTreeSet::new(),
        });
    }
    let requests = activity.as_ref().map_err(Clone::clone).and_then(|_| {
        let mut result = Vec::new();
        if let Some(game) = &document.game {
            for contribution in composition
                .systems
                .active_game_contributions(&game.execution)
            {
                result.extend((contribution.service_requests)(game)?);
            }
            if let Some(request) = player_preferences_service_request(game)?
                && !result.iter().any(|existing| existing.id == request.id)
            {
                // Preferences have a separate runtime owner. Inventory joins
                // participation by capability ID without merging their configs.
                result.push(request);
            }
        }
        Ok(result)
    });
    let service_inventory = services.capability_inventory(&requests);
    inventory.records.extend(service_inventory.records);
    inventory.diagnostics.extend(service_inventory.diagnostics);
    #[cfg(feature = "g1")]
    for library in module_engine_g1::library_capabilities() {
        let consumers: BTreeSet<_> = inventory
            .records
            .iter()
            .filter(|r| r.metadata.libraries.contains(library.id))
            .map(|r| r.id.clone())
            .collect();
        if consumers.is_empty() {
            continue;
        }
        let participation = match &activity {
            Ok(activity) => CapabilityFact::known(
                consumers.iter().any(|id| activity.modules.contains(id)),
                "Declared library dependency of a participating source module; not an executable library instance",
            ),
            Err(error) => CapabilityFact::unknown(error),
        };
        inventory.records.push(CapabilityRecord {
            id: library.id.into(),
            metadata: CapabilityMetadata::new(library.label, library.purpose,
                if library.gameplay { CapabilityOwner::Gameplay } else { CapabilityOwner::Engine },
                CapabilityKind::Library, "module-engine-game-kit")
                .configured_by([CapabilityConfiguration::NativeSource]),
            version: Some(module_engine_g1::library_package_version().into()), installed: true,
            available: CapabilityFact::known(true, "Compile-selected helper API linked by an installed source package; not a graph module"),
            participation, dependencies: BTreeSet::new(), provided_components: BTreeSet::new(), consumed_components: BTreeSet::new(),
        });
    }
    let libraries: BTreeSet<_> = inventory
        .records
        .iter()
        .filter(|r| r.metadata.kind == CapabilityKind::Library)
        .map(|r| r.id.as_str())
        .collect();
    for record in &inventory.records {
        for library in &record.metadata.libraries {
            if !libraries.contains(library.as_str()) {
                inventory.diagnostics.push(format!("Library '{library}' referenced by '{}' has no supplied linked-library evidence", record.id));
            }
        }
    }
    inventory.records.sort_by(|a, b| a.id.cmp(&b.id));
    inventory.diagnostics.sort();
    inventory.diagnostics.dedup();
    inventory
}

#[cfg(test)]
mod tests {
    use super::*;
    use module_engine_project::EngineBlueprint;
    use std::path::Path;

    fn document(composition: &CompositionContext, game: Option<GameProject>) -> GraphDocument {
        let mut document = demo_document(WorldKind::Ecs);
        document.root.nodes.retain(|n| {
            !matches!(
                n.module.as_str(),
                "custom.rust" | "clock.time" | "scalar.value"
            )
        });
        let ids: BTreeSet<_> = document.root.nodes.iter().map(|n| n.id.clone()).collect();
        document
            .root
            .edges
            .retain(|e| ids.contains(&e.from_node) && ids.contains(&e.to_node));
        document.custom_sources.clear();
        document.game = game;
        let profile = EngineProfile::new(
            "fixture.inventory",
            "1",
            EngineBlueprint {
                root: document.root.clone(),
                engine: document.engine.clone(),
                custom_sources: Default::default(),
                shader_sources: Default::default(),
                default_execution: GameProject::empty().execution,
                available_modules: composition.module_versions().into_keys().collect(),
            },
        )
        .unwrap();
        document.engine_profile = Some(profile.document_value().unwrap());
        composition
            .capture_lock(&document, profile.identity, None)
            .unwrap()
            .apply_to(&mut document);
        document
    }
    fn find<'a>(inventory: &'a CapabilityInventory, id: &str) -> &'a CapabilityRecord {
        inventory.records.iter().find(|r| r.id == id).unwrap()
    }

    #[test]
    fn neutral_inventory_preserves_json_and_does_not_create_a_state_root() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let composition = crate::composition_at(&root).unwrap();
        let document = document(&composition, None);
        let before = serde_json::to_vec(&document).unwrap();
        let state_root = root
            .join(".work")
            .join(format!("inventory-must-not-create-{}", std::process::id()));
        assert!(!state_root.exists());
        let services = crate::service_registry_at(state_root.clone()).unwrap();
        let inventory = capability_inventory(&document, &composition, &services);
        assert!(
            inventory.diagnostics.is_empty(),
            "{:?}",
            inventory.diagnostics
        );
        assert_eq!(
            inventory,
            capability_inventory(&document, &composition, &services)
        );
        assert_eq!(serde_json::to_vec(&document).unwrap(), before);
        assert!(!state_root.exists());
        assert!(
            inventory
                .records
                .iter()
                .all(|r| r.metadata.owner != CapabilityOwner::Unknown),
            "{:?}",
            inventory
                .records
                .iter()
                .filter(|r| r.metadata.owner == CapabilityOwner::Unknown)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            find(&inventory, "world.ecs").participation.value,
            Some(true)
        );
        #[cfg(feature = "arpg")]
        {
            assert_eq!(
                find(&inventory, "game.combat").metadata.owner,
                CapabilityOwner::Gameplay
            );
            assert_eq!(
                find(&inventory, "game.audio").participation.value,
                Some(false)
            );
        }
        #[cfg(feature = "audio")]
        {
            assert_eq!(
                find(&inventory, "audio.wav").participation.value,
                Some(false)
            );
            assert_eq!(
                find(&inventory, "audio.wav").metadata.owner,
                CapabilityOwner::Engine
            );
            assert_eq!(
                find(&inventory, "audio.wav").available.value,
                Some(module_engine_host_services::WAV_PLAYBACK_SUPPORTED)
            );
        }
        #[cfg(feature = "g1")]
        assert_eq!(
            find(&inventory, "game-kit.campaign").participation.value,
            Some(false)
        );
    }

    #[test]
    fn smaller_registered_composition_has_no_omitted_game_render_or_helper_entries() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let composition = crate::composition_for_features(&root, &BTreeSet::new()).unwrap();
        let document = document(&composition, None);
        let inventory = capability_inventory(&document, &composition, &HostServiceRegistry::new());
        assert!(
            inventory.diagnostics.is_empty(),
            "{:?}",
            inventory.diagnostics
        );
        for id in [
            "game.combat",
            "g1.rules",
            "puzzle.turn",
            "render.vulkan",
            "audio.wav",
            "persistence.json",
            "game-kit.campaign",
        ] {
            assert!(
                !inventory.records.iter().any(|r| r.id == id),
                "Omitted capability {id} must not be advertised"
            );
        }
    }

    #[cfg(feature = "g1")]
    #[test]
    fn g1_requests_services_and_library_dependencies_without_executable_helper_nodes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let composition = crate::composition_at(&root).unwrap();
        let mut game = module_engine_g1::project();
        game.player_preferences = Some(PlayerPreferencesConfig {
            namespace: "fixture.capability-preferences".into(),
        });
        let document = document(&composition, Some(game));
        let inventory = capability_inventory(
            &document,
            &composition,
            &crate::service_registry_at(root.join(".work/unused-inventory-g1")).unwrap(),
        );
        if cfg!(all(feature = "audio", feature = "persistence")) {
            assert!(
                inventory.diagnostics.is_empty(),
                "{:?}",
                inventory.diagnostics
            );
        } else {
            assert!(
                inventory
                    .diagnostics
                    .iter()
                    .any(|d| d.contains("not installed"))
            );
        }
        assert_eq!(
            find(&inventory, "g1.rules").metadata.owner,
            CapabilityOwner::Project
        );
        assert_eq!(
            find(&inventory, "g1.presentation").participation.value,
            Some(true)
        );
        for library in module_engine_g1::library_capabilities() {
            let record = find(&inventory, library.id);
            assert_eq!(record.metadata.kind, CapabilityKind::Library);
            assert_eq!(record.participation.value, Some(true));
            assert!(composition.systems.descriptor(library.id).is_none());
            assert!(composition.modules.descriptor(library.id).is_none());
        }
        #[cfg(feature = "audio")]
        assert_eq!(
            find(&inventory, "audio.wav").participation.value,
            Some(true)
        );
        #[cfg(feature = "persistence")]
        assert_eq!(
            find(&inventory, "persistence.json").participation.value,
            Some(true)
        );
    }

    #[cfg(feature = "persistence")]
    #[test]
    fn explicit_preferences_participate_without_gameplay_service_or_storage_access() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let composition = crate::composition_at(&root).unwrap();
        let mut game = GameProject::empty();
        game.player_preferences = Some(PlayerPreferencesConfig {
            namespace: "fixture.capability-preferences".into(),
        });
        let document = document(&composition, Some(game));
        let inventory = capability_inventory(
            &document,
            &composition,
            &crate::service_registry_at(root.join(".work/unused-inventory-preferences")).unwrap(),
        );
        assert!(
            inventory.diagnostics.is_empty(),
            "{:?}",
            inventory.diagnostics
        );
        assert_eq!(
            find(&inventory, "persistence.json").participation.value,
            Some(true)
        );
        assert!(!root.join(".work/unused-inventory-preferences").exists());
    }

    #[test]
    fn invalid_activity_and_unchecked_profiles_are_explicit_unknowns() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let composition = crate::composition_at(&root).unwrap();
        let mut document = document(&composition, None);
        document.engine_profile = None;
        let no_profile = capability_inventory(&document, &composition, &HostServiceRegistry::new());
        assert_eq!(find(&no_profile, "world.ecs").available.value, None);
        let mut game = GameProject::empty();
        game.execution.stages.push(SystemStage {
            name: "Unknown".into(),
            nodes: vec![SystemNode {
                id: "unknown".into(),
                module: "foreign.not.installed".into(),
                params: serde_json::Value::Null,
                bindings: Default::default(),
                after: vec![],
            }],
        });
        document.game = Some(game);
        let unknown = capability_inventory(
            &document,
            &composition,
            &crate::service_registry_at(root.join(".work/unused-inventory-invalid")).unwrap(),
        );
        assert!(!unknown.diagnostics.is_empty());
        assert!(
            unknown
                .records
                .iter()
                .all(|r| r.participation.value.is_none())
        );
    }
}
