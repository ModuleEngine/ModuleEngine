use module_engine_game_ui::authoring::{
    AuthoringRegistry, ProjectTemplateContribution, TemplateResources,
};

pub fn install(registry: &mut AuthoringRegistry) -> Result<(), String> {
    registry.register_template(ProjectTemplateContribution {
        id: "two-arenas",
        owner: "game.input",
        label: "Two arenas",
        menu_label: "Two arenas example",
        engine_profile: module_engine_game_ui::authoring::EngineProfileContribution {
            id: "module-engine.arpg",
            label: "ARPG",
            source_feature: "arpg",
        },
        resources: TemplateResources::Package,
        populate: crate::document_with_rapier,
    })
}
