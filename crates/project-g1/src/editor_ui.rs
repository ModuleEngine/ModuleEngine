use module_engine_game_ui::authoring::{
    AuthoringRegistry, ProjectTemplateContribution, TemplateResources, ToolContribution,
};

pub fn install(registry: &mut AuthoringRegistry) -> Result<(), String> {
    registry.register_template(ProjectTemplateContribution {
        id: "g1",
        owner: "g1.rules",
        label: "Crate Garden",
        menu_label: "Crate Garden (G1)",
        engine_profile: module_engine_game_ui::authoring::EngineProfileContribution {
            id: "module-engine.g1",
            label: "Crate Garden (G1)",
            source_feature: "g1",
        },
        resources: TemplateResources::Example("examples/crate-garden"),
        populate: |mut document| {
            document.game = Some(crate::project());
            document
        },
    })?;
    registry.register_tool(ToolContribution {
        pane_id: 110, owner: "g1.rules", label: "Crate Garden Level Palette", package: "module-engine-g1",
        engine_only: false,
        description: "Paint this game's board, walls, crates and goals; propose validated authored edits through the existing tool lifecycle.",
        draw: crate::grid_ui::draw_tool,
    })
}
