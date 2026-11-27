//! Source-installed authoring callbacks. The application owns validation and IO.
use module_engine_core::{GraphDocument, ModuleRegistry, ModuleToolContext, ModuleToolResponse};
use std::collections::BTreeMap;

pub type ToolCallback = fn(
    &mut egui::Ui,
    &GraphDocument,
    &ModuleRegistry,
    &ModuleToolContext,
) -> Result<ModuleToolResponse, String>;

pub struct ToolContribution {
    pub pane_id: u32,
    pub owner: &'static str,
    pub label: &'static str,
    pub package: &'static str,
    pub engine_only: bool,
    pub description: &'static str,
    pub draw: ToolCallback,
}

#[derive(Clone, Copy)]
pub enum TemplateResources {
    CurrentProject,
    Package,
    Example(&'static str),
}

pub struct ProjectTemplateContribution {
    pub id: &'static str,
    pub owner: &'static str,
    pub label: &'static str,
    pub menu_label: &'static str,
    pub engine_profile: EngineProfileContribution,
    pub resources: TemplateResources,
    pub populate: fn(GraphDocument) -> GraphDocument,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineProfileContribution {
    pub id: &'static str,
    pub label: &'static str,
    pub source_feature: &'static str,
}

#[derive(Default)]
pub struct AuthoringRegistry {
    templates: Vec<ProjectTemplateContribution>,
    tools: BTreeMap<u32, ToolContribution>,
}

impl AuthoringRegistry {
    pub fn register_template(&mut self, entry: ProjectTemplateContribution) -> Result<(), String> {
        if entry.id.is_empty()
            || entry.owner.is_empty()
            || entry.engine_profile.id.is_empty()
            || entry.engine_profile.source_feature.is_empty()
            || self
                .templates
                .iter()
                .any(|template| template.id == entry.id)
        {
            return Err(format!(
                "Invalid or duplicate project template {}",
                entry.id
            ));
        }
        if self.templates.iter().any(|template| {
            template.engine_profile.id == entry.engine_profile.id
                && template.engine_profile != entry.engine_profile
        }) {
            return Err(format!(
                "Conflicting engine profile {}",
                entry.engine_profile.id
            ));
        }
        self.templates.push(entry);
        Ok(())
    }

    pub fn register_tool(&mut self, entry: ToolContribution) -> Result<(), String> {
        if entry.owner.is_empty() || self.tools.contains_key(&entry.pane_id) {
            return Err(format!(
                "Invalid or duplicate authoring pane {}",
                entry.pane_id
            ));
        }
        self.tools.insert(entry.pane_id, entry);
        Ok(())
    }

    pub fn template(&self, id: &str) -> Result<&ProjectTemplateContribution, String> {
        self.templates
            .iter()
            .find(|template| template.id == id)
            .ok_or_else(|| format!("Project template '{id}' is not installed"))
    }

    pub fn templates(&self) -> impl Iterator<Item = &ProjectTemplateContribution> {
        self.templates.iter()
    }

    pub fn tools(&self) -> impl Iterator<Item = &ToolContribution> {
        self.tools.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn independent_template() -> ProjectTemplateContribution {
        ProjectTemplateContribution {
            id: "independent",
            owner: "independent.rules",
            label: "Independent game",
            menu_label: "Independent game",
            resources: TemplateResources::CurrentProject,
            engine_profile: EngineProfileContribution {
                id: "independent.profile",
                label: "Independent",
                source_feature: "independent",
            },
            populate: |mut document| {
                let mut project = module_engine_core::GameProject::empty();
                project.entry_scene = "independent".into();
                document.game = Some(project);
                document
            },
        }
    }

    #[test]
    fn independent_game_registers_and_duplicate_keeps_original_template() {
        let mut registry = AuthoringRegistry::default();
        registry.register_template(independent_template()).unwrap();
        let mut duplicate = independent_template();
        duplicate.populate = |_| panic!("Duplicate must never replace the first registration");
        assert!(registry.register_template(duplicate).is_err());
        let template = registry.template("independent").unwrap();
        let document = module_engine_core::demo_document(module_engine_core::WorldKind::Objects);
        let document = (template.populate)(document);
        assert_eq!(document.game.unwrap().entry_scene, "independent");
        assert!(registry.template("g1").is_err());
        assert_eq!(registry.templates().count(), 1);
    }
}
