//! Application assembly. Generic Editor code consumes these source registrations.
use module_engine_game_ui::authoring::AuthoringRegistry;
use std::sync::OnceLock;

#[cfg(feature = "arpg")]
pub use module_engine_arpg as arpg;
#[cfg(feature = "g1")]
pub use module_engine_g1 as g1;
#[cfg(feature = "puzzle")]
pub use module_engine_project_puzzle as puzzle;
#[cfg(feature = "arpg")]
pub use module_engine_two_arenas as two_arenas;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectTemplate(pub &'static str);
#[allow(non_upper_case_globals)]
impl ProjectTemplate {
    pub const Empty: Self = Self("empty");
    pub const TwoArenas: Self = Self("two-arenas");
    pub const Puzzle: Self = Self("puzzle");
    pub const G1: Self = Self("g1");

    pub fn installed(self) -> bool {
        self == Self::Empty || authoring().template(self.0).is_ok()
    }
}

pub fn authoring() -> &'static AuthoringRegistry {
    static REGISTRY: OnceLock<AuthoringRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let registry = AuthoringRegistry::default();
        #[cfg(any(feature = "arpg", feature = "puzzle", feature = "g1"))]
        let mut registry = registry;
        #[cfg(feature = "arpg")]
        two_arenas::authoring::install(&mut registry).expect("Unique example registrations");
        #[cfg(feature = "puzzle")]
        puzzle::authoring::install(&mut registry).expect("Unique puzzle registrations");
        #[cfg(feature = "g1")]
        g1::editor_ui::install(&mut registry).expect("Unique G1 registrations");
        registry
    })
}
