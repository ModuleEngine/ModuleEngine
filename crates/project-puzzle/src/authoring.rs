use module_engine_game_ui::authoring::{
    AuthoringRegistry, ProjectTemplateContribution, TemplateResources,
};

pub fn install(registry: &mut AuthoringRegistry) -> Result<(), String> {
    registry.register_template(ProjectTemplateContribution {
        id: "puzzle",
        owner: "puzzle.turn",
        label: "Crate puzzle",
        menu_label: "Crate puzzle example",
        engine_profile: module_engine_game_ui::authoring::EngineProfileContribution {
            id: "module-engine.puzzle",
            label: "Puzzle",
            source_feature: "puzzle",
        },
        resources: TemplateResources::CurrentProject,
        populate: |mut document| {
            document.game = Some(crate::project());
            document
        },
    })
}
