//! Source package fixture assembled through the public runtime and UI contracts.
use module_engine_core::{
    Camera, ExecutableSystem, GameContribution, GameInput, GameProject, GraphDocument,
    ModuleToolResponse, SystemContext, SystemDescriptor, SystemNode, SystemRegistry, SystemStage,
};
use module_engine_game_ui::authoring::{
    AuthoringRegistry, EngineProfileContribution, ProjectTemplateContribution, TemplateResources,
    ToolContribution,
};
use module_engine_game_ui::{UiContribution, UiRegistry};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const OWNER: &str = "acceptance.independent.rules";
pub const TEMPLATE_ID: &str = "acceptance-independent";
pub const PANE_ID: u32 = 9821;
pub const PRESENTATION_ID: &str = "acceptance.independent.presentation";

struct IndependentRules;
impl ExecutableSystem for IndependentRules {
    fn step(&mut self, context: &mut SystemContext<'_>, _: &mut Value) -> Result<(), String> {
        let ticks = context
            .game
            .module_data
            .get(OWNER)
            .and_then(Value::as_u64)
            .unwrap_or(0);
        context
            .game
            .module_data
            .insert(OWNER.into(), json!(ticks + 1));
        Ok(())
    }
}

pub fn install(systems: &mut SystemRegistry) -> Result<(), String> {
    systems.register(
        SystemDescriptor {
            id: OWNER.into(),
            state_schema: "independent/1".into(),
            components: vec![],
            ports: vec![],
            requires: vec![],
        },
        |_| Ok(Box::new(IndependentRules)),
    )?;
    systems.register_game_contribution(GameContribution {
        id: PRESENTATION_ID.into(),
        activation_modules: BTreeSet::from([OWNER.into()]),
        begin_tick: |_| Ok(()),
        initial_camera: |_| Camera::default(),
        follow_target: |_| None,
        decorate_snapshot: |_, _| {},
        script_input: |_| Value::Null,
        validate_event: |_| Ok(()),
        validate_state: |_| Ok(()),
        service_requests: |_| Ok(vec![]),
        diagnostics: |state| json!({"accepted_ticks": state.module_data.get(OWNER)}),
    })
}

pub fn populate(mut document: GraphDocument) -> GraphDocument {
    let mut game = GameProject::empty();
    game.execution.stages.push(SystemStage {
        name: "Independent".into(),
        nodes: vec![SystemNode {
            id: "independent-tick".into(),
            module: OWNER.into(),
            params: Value::Null,
            bindings: BTreeMap::new(),
            after: vec![],
        }],
    });
    document.game = Some(game);
    document
}

pub fn authoring() -> AuthoringRegistry {
    let mut registry = AuthoringRegistry::default();
    registry
        .register_template(ProjectTemplateContribution {
            id: TEMPLATE_ID,
            owner: OWNER,
            label: "Independent acceptance",
            menu_label: "Independent acceptance",
            resources: TemplateResources::CurrentProject,
            engine_profile: EngineProfileContribution {
                id: "acceptance.independent.profile",
                label: "Independent acceptance",
                source_feature: "acceptance-independent",
            },
            populate,
        })
        .unwrap();
    registry
        .register_tool(ToolContribution {
            pane_id: PANE_ID,
            owner: OWNER,
            label: "Independent tool",
            package: "acceptance-fixture",
            engine_only: false,
            description: "Game-owned independent tool",
            draw: |ui, document, _, _| {
                ui.label("Independent source tool");
                Ok(ModuleToolResponse {
                    candidate: Some(document.clone()),
                    dirty: false,
                })
            },
        })
        .unwrap();
    registry
}

pub fn install_presentation(registry: &mut UiRegistry) -> Result<(), String> {
    registry.register(UiContribution {
        id: PRESENTATION_ID,
        input: |_, _, _, _, _| GameInput {
            actions: ["acceptance.independent.input".into()].into(),
            ..Default::default()
        },
        hud: |_, _, state, _, _| state.module_data.contains_key(OWNER),
        controls: None,
        controls_with_status: None,
        preview_aim: None,
        has_input: None,
        capture_input: None,
    })
}
