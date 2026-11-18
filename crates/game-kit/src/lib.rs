//! Independently selected, pure game-building libraries.
//!
//! These modules do not register scheduled systems or native services. A source
//! package calls the selected APIs from its own real consumers and declares its
//! graph/schema/service contracts at the integration boundary.

#[cfg(feature = "actions")]
pub mod actions;
#[cfg(feature = "campaign")]
pub mod campaign;
#[cfg(feature = "grid")]
pub mod grid;
#[cfg(feature = "history")]
pub mod history;

/// Compile-selected library metadata. Presence means an API is linked, not that
/// an executable system, native service or gameplay instance has been started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LibraryCapability {
    pub id: &'static str,
    pub label: &'static str,
    pub purpose: &'static str,
    pub gameplay: bool,
}

pub fn library_package_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn library_capabilities() -> Vec<LibraryCapability> {
    vec![
        #[cfg(feature = "actions")]
        LibraryCapability {
            id: "game-kit.actions",
            label: "Action mapping library",
            purpose: "Map held/pressed physical and UI inputs to source-defined named actions; the consumer owns their meaning.",
            gameplay: false,
        },
        #[cfg(feature = "grid")]
        LibraryCapability {
            id: "game-kit.grid",
            label: "Grid coordinates library",
            purpose: "Provide bounded cells, layered occupancy, atomic relocation and grid/world projection without crate or combat rules.",
            gameplay: false,
        },
        #[cfg(feature = "history")]
        LibraryCapability {
            id: "game-kit.history",
            label: "Undo journal library",
            purpose: "Keep and serialize accepted typed snapshots; the consumer decides which operations and data can be undone.",
            gameplay: false,
        },
        #[cfg(feature = "campaign")]
        LibraryCapability {
            id: "game-kit.campaign",
            label: "Campaign progression library",
            purpose: "Provide ordered level unlocks, replay and records; each game owns its levels, scoring meaning and attempt payload.",
            gameplay: true,
        },
    ]
}
