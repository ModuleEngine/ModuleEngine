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
        lights: &'a [crate::SceneLight],
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
        self.apply_with_edits(update, |_| {})
    }

    /// Reports in-place model-only edits in draw order. The caller handles
    /// resets and edits that can change geometry, materials or draw order.
    pub fn apply_with_edits(
        &mut self,
        update: SceneUpdate<'_>,
        mut report_moved: impl FnMut(std::ops::Range<usize>),
    ) -> Result<(), String> {
        match update {
            SceneUpdate::Reset { revision, snapshot } => {
                self.scene = snapshot;
                self.revision = Some(revision);
            }
            SceneUpdate::Delta {
                base,
                revision,
                time,
                lights,
                changes,
            } => {
                if self.revision != Some(base) {
                    return Err("Retained render scene has a different base revision".into());
                }
                let mut next_group = 0;
                for change in changes {
                    let items = &self.scene.items;
                    let start = if items
                        .get(next_group)
                        .is_some_and(|item| item.id == change.id)
                    {
                        next_group
                    } else {
                        items.partition_point(|item| item.id < change.id)
                    };
                    let end = start
                        + items[start..]
                            .iter()
                            .take_while(|item| item.id == change.id)
                            .count();
                    next_group = end;
                    match change.kind {
                        SceneChangeKind::Edited {
                            moved,
                            material_changed,
                            mesh_changed,
                        } => {
                            debug_assert_eq!(end - start, change.items.len());
                            if moved && !material_changed && !mesh_changed {
                                report_moved(start..end);
                            }
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
                            next_group = start + change.items.len();
                        }
                    }
                }
                self.scene.time = time;
                self.scene.lights.clear();
                self.scene.lights.extend_from_slice(lights);
                self.revision = Some(revision);
            }
        }
        Ok(())
    }
}
