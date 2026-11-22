use crate::{RenderItem, SceneSnapshot};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SceneSync {
    #[default]
    SnapshotOnly,
    NeedsReset,
    At(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneChangeKind {
    Spawned,
    Removed,
    Replaced,
    Edited {
        moved: bool,
        material_changed: bool,
        mesh_changed: bool,
    },
}

/// An object owns a contiguous, ordered group of drawable primitives. A
/// replacement can change the number of primitives or reuse a stable ID.
pub struct SceneChange<'a> {
    pub id: u64,
    pub kind: SceneChangeKind,
    pub items: &'a [RenderItem],
}

pub enum SceneUpdate<'a> {
    /// Items are ordered by stable ID, with primitives in source order.
    Reset {
        revision: u64,
        snapshot: SceneSnapshot,
    },
    Delta {
        base: u64,
        revision: u64,
        time: f32,
        changes: &'a [SceneChange<'a>],
    },
}

/// CPU-side receiver for the stock renderer. It retains draw order and an
/// already assembled drawable list for the existing geometry passes.
#[derive(Default)]
pub struct RenderScene {
    revision: Option<u64>,
    scene: SceneSnapshot,
}

impl RenderScene {
    pub fn sync(&self) -> SceneSync {
        self.revision.map_or(SceneSync::NeedsReset, SceneSync::At)
    }

    pub fn snapshot(&self) -> Result<&SceneSnapshot, String> {
        self.revision
            .map(|_| &self.scene)
            .ok_or_else(|| "Retained render scene needs a reset".into())
    }

    pub fn apply(&mut self, update: SceneUpdate<'_>) -> Result<(), String> {
        match update {
            SceneUpdate::Reset { revision, snapshot } => {
                self.scene = snapshot;
                self.revision = Some(revision);
            }
            SceneUpdate::Delta {
                base,
                revision,
                time,
                changes,
            } => {
                if self.revision != Some(base) {
                    return Err("Retained render scene has a different base revision".into());
                }
                for change in changes {
                    let start = self.scene.items.partition_point(|item| item.id < change.id);
                    let end = start
                        + self.scene.items[start..].partition_point(|item| item.id == change.id);
                    match change.kind {
                        SceneChangeKind::Edited {
                            moved,
                            material_changed,
                            mesh_changed,
                        } => {
                            debug_assert_eq!(end - start, change.items.len());
                            for (item, next) in
                                self.scene.items[start..end].iter_mut().zip(change.items)
                            {
                                if moved {
                                    item.model = next.model;
                                }
                                if material_changed {
                                    item.color = next.color;
                                    item.material = next.material.clone();
                                }
                                if mesh_changed {
                                    item.mesh.clone_from(&next.mesh);
                                }
                            }
                        }
                        _ => {
                            self.scene
                                .items
                                .splice(start..end, change.items.iter().cloned());
                        }
                    }
                }
                self.scene.time = time;
                self.revision = Some(revision);
            }
        }
        Ok(())
    }
}
