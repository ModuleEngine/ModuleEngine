use super::*;

type Model = [[f32; 4]; 4];

struct ObjectPose {
    identity: PresentationIdentity,
    local: Transform,
    model: Model,
    color: [u8; 4],
}

#[derive(Clone, PartialEq)]
pub(super) struct Sample {
    pub scene: String,
    pub tick: u64,
    pub time: f64,
    pub target: Option<[f32; 3]>,
}

struct Drawables {
    lifetime: u64,
    items: Vec<RenderItem>,
}

#[derive(Default)]
pub(super) struct RetainedPresentation {
    cursor: RenderChangeCursor,
    objects: BTreeMap<u64, ObjectPose>,
    children: BTreeMap<u64, BTreeSet<u64>>,
    pub current: Option<Sample>,
    pub previous: Option<Sample>,
    pub hierarchy_changed: bool,
    pub models_changed: bool,
    previous_models: BTreeMap<u64, Model>,
    projected_models: BTreeMap<u64, Model>,
    dirty: BTreeSet<u64>,
    last_interpolated: BTreeMap<u64, Model>,
    appearance: BTreeSet<u64>,
    animated: BTreeSet<u64>,
    projected_time: Option<f32>,
    drawables: BTreeMap<u64, Drawables>,
    revision: u64,
    base: u64,
    delta: BTreeMap<u64, SceneChangeKind>,
}

impl RetainedPresentation {
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Advance the read cursor only over committed storage. Capture at both
    /// sides of a single accepted tick to preserve its sparse pose endpoints.
    pub fn observe(
        &mut self,
        session: &mut GameSession,
        target: Option<[f32; 3]>,
        scopes: &[GamePresentationScope],
        adjacent: bool,
    ) -> Result<bool, String> {
        let changes = session.read_render_changes(&mut self.cursor).unwrap();
        if changes.reset {
            self.objects.clear();
            self.children.clear();
            self.projected_models.clear();
            self.dirty.extend(self.drawables.keys().copied());
        }
        let state = session.state();
        let world = &state.world;
        let sample = Sample {
            scene: state.scene.clone(),
            tick: state.tick,
            time: state.time,
            target,
        };
        let mut model_roots = BTreeSet::new();
        let mut identity_roots = BTreeSet::new();
        let mut old_models = BTreeMap::new();
        let mut removed = false;
        for id in changes.removed {
            if !world.contains(id)
                && let Some(object) = self.objects.remove(&id)
            {
                unlink(&mut self.children, id, object.identity.parent);
                self.projected_models.remove(&id);
                self.dirty.insert(id);
                removed = true;
            }
        }
        let mut spawned = false;
        let mut recolored = false;
        let check_sources = changes.reset
            || !changes.changed.is_empty()
            || removed
            || self.current.as_ref() != Some(&sample);
        for id in changes.changed {
            let object = world.object(id).unwrap();
            let identity = PresentationIdentity {
                lifetime: world.lifetime(id).unwrap(),
                parent: object.parent,
            };
            let previous = self.objects.get(&id);
            spawned |= previous.is_none();
            recolored |= previous.is_some_and(|old| old.color != object.color);
            if previous.is_none_or(|old| old.identity != identity) {
                if let Some(old) = previous {
                    unlink(&mut self.children, id, old.identity.parent);
                }
                if let Some(parent) = identity.parent {
                    self.children.entry(parent).or_default().insert(id);
                }
                identity_roots.insert(id);
            }
            if previous.is_none_or(|old| old.local != object.transform) {
                model_roots.insert(id);
            }
            self.dirty.insert(id);
            self.objects.insert(
                id,
                ObjectPose {
                    identity,
                    local: object.transform,
                    model: previous.map_or_else(identity_matrix, |old| old.model),
                    color: object.color,
                },
            );
        }
        let incompatible = self.descendants(&identity_roots);
        model_roots.extend(identity_roots.iter().copied());
        let mut resolved = BTreeMap::new();
        for id in self.descendants(&model_roots) {
            let model = world.render_model(id, &mut resolved);
            let object = self.objects.get_mut(&id).unwrap();
            if object.model != model {
                old_models.insert(
                    id,
                    self.projected_models
                        .get(&id)
                        .copied()
                        .unwrap_or(object.model),
                );
                object.model = model;
                self.dirty.insert(id);
            }
        }
        if check_sources && !scopes.is_empty() {
            let mut scoped = BTreeSet::new();
            let mut appearance = BTreeSet::new();
            for scope in scopes {
                scoped.extend((scope.pose_objects)(state));
                appearance.extend((scope.appearance_objects)(state));
            }
            self.dirty
                .extend(self.appearance.union(&appearance).copied());
            self.appearance = appearance;
            let mut input = SceneSnapshot {
                time: sample.time as f32,
                items: scoped
                    .into_iter()
                    .filter_map(|id| self.raw_item(id, self.objects.get(&id)?.model))
                    .collect(),
            };
            session.project_presentation_models(&mut input)?;
            let mut source_models = BTreeMap::new();
            for item in input.items {
                if item.model != self.objects[&item.id].model {
                    source_models.insert(item.id, item.model);
                }
            }
            for id in self
                .projected_models
                .keys()
                .chain(source_models.keys())
                .copied()
            {
                let model = self.objects[&id].model;
                let previous = self.projected_models.get(&id).copied().unwrap_or(model);
                let next = source_models.get(&id).copied().unwrap_or(model);
                if previous != next {
                    old_models.entry(id).or_insert(previous);
                    self.dirty.insert(id);
                }
            }
            self.projected_models = source_models;
        }
        old_models.retain(|&id, old| *old != self.model(id));
        let hierarchy_changed = changes.reset || removed || !identity_roots.is_empty();
        let model_list_changed = removed || spawned || !old_models.is_empty();
        let changed = hierarchy_changed
            || model_list_changed
            || recolored
            || self.current.as_ref() != Some(&sample);
        let can_interpolate = adjacent
            && !changes.reset
            && self.current.as_ref().is_some_and(|before| {
                before.tick.checked_add(1) == Some(sample.tick) && before.scene == sample.scene
            });
        if can_interpolate {
            self.previous = self.current.take();
            self.previous_models = old_models;
            self.previous_models
                .retain(|id, _| !incompatible.contains(id));
            self.hierarchy_changed = hierarchy_changed;
            self.models_changed = model_list_changed;
        } else if changed {
            self.previous = None;
            self.previous_models.clear();
        }
        self.current = Some(sample);
        Ok(changed && !can_interpolate)
    }

    fn descendants(&self, roots: &BTreeSet<u64>) -> BTreeSet<u64> {
        let mut result = BTreeSet::new();
        let mut pending = roots.iter().copied().collect::<Vec<_>>();
        while let Some(id) = pending.pop() {
            if result.insert(id)
                && let Some(children) = self.children.get(&id)
            {
                pending.extend(children);
            }
        }
        result
    }

    fn model(&self, id: u64) -> Model {
        self.projected_models
            .get(&id)
            .copied()
            .unwrap_or(self.objects[&id].model)
    }

    pub fn has_objects(&self) -> bool {
        !self.objects.is_empty()
    }

    fn raw_item(&self, id: u64, model: Model) -> Option<RenderItem> {
        Some(RenderItem {
            id,
            mesh: Mesh::cube(),
            model,
            color: self.objects.get(&id)?.color,
            material: Material::default(),
        })
    }

    /// Only models that differ from the current accepted endpoint can make an
    /// extra presentation eligible at a new alpha.
    pub fn interpolate(&self, alpha: f64) -> BTreeMap<u64, Model> {
        if alpha == 1. {
            return BTreeMap::new();
        }
        self.previous_models
            .iter()
            .filter_map(|(&id, &previous)| {
                let current = self.model(id);
                let model = interpolate_model(previous, current, alpha as f32)?;
                (model != current).then_some((id, model))
            })
            .collect()
    }

    pub fn project(
        &mut self,
        models: &BTreeMap<u64, Model>,
        time: f32,
        enrich: impl FnOnce(SceneSnapshot) -> Result<(SceneSnapshot, BTreeSet<u64>), String>,
    ) -> Result<(), String> {
        let mut dirty = self.dirty.clone();
        dirty.extend(self.appearance.iter().copied());
        if self.projected_time != Some(time) {
            dirty.extend(self.animated.iter().copied());
        }
        for id in self.last_interpolated.keys().chain(models.keys()) {
            if self.last_interpolated.get(id) != models.get(id) {
                dirty.insert(*id);
            }
        }
        if dirty.is_empty() {
            return Ok(());
        }
        let (snapshot, animated) = enrich(SceneSnapshot {
            time,
            items: dirty
                .iter()
                .filter_map(|&id| {
                    self.objects.get(&id)?;
                    self.raw_item(
                        id,
                        models.get(&id).copied().unwrap_or_else(|| self.model(id)),
                    )
                })
                .collect(),
        })?;
        self.animated.retain(|id| !dirty.contains(id));
        self.animated.extend(animated);
        self.projected_time = Some(time);
        let mut items = snapshot.items.into_iter().peekable();
        let mut delta = BTreeMap::new();
        for id in dirty {
            let mut group = Vec::new();
            while items.peek().is_some_and(|item| item.id == id) {
                group.extend(items.next());
            }
            if let Some(object) = self.objects.get(&id) {
                let kind = match self.drawables.get(&id) {
                    None => SceneChangeKind::Spawned,
                    Some(old)
                        if old.lifetime != object.identity.lifetime
                            || old.items.len() != group.len() =>
                    {
                        SceneChangeKind::Replaced
                    }
                    Some(old) => {
                        let moved = old
                            .items
                            .iter()
                            .zip(&group)
                            .any(|(a, b)| a.model != b.model);
                        let material_changed = old
                            .items
                            .iter()
                            .zip(&group)
                            .any(|(a, b)| a.color != b.color || a.material != b.material);
                        let mesh_changed =
                            old.items.iter().zip(&group).any(|(a, b)| a.mesh != b.mesh);
                        if !(moved || material_changed || mesh_changed) {
                            continue;
                        }
                        SceneChangeKind::Edited {
                            moved,
                            material_changed,
                            mesh_changed,
                        }
                    }
                };
                self.drawables.insert(
                    id,
                    Drawables {
                        lifetime: object.identity.lifetime,
                        items: group,
                    },
                );
                delta.insert(id, kind);
            } else if self.drawables.remove(&id).is_some() {
                delta.insert(id, SceneChangeKind::Removed);
            }
        }
        debug_assert!(items.next().is_none());
        self.dirty.clear();
        self.last_interpolated.clone_from(models);
        if !delta.is_empty() {
            self.base = self.revision;
            self.revision = next_presentation_revision();
            self.delta = delta;
        }
        Ok(())
    }

    pub fn expanded_snapshot(&self, time: f32) -> Option<SceneSnapshot> {
        // Deformed positions change with each pose. The expanded draw stream
        // owns them per frame and avoids interning them in the resident pool.
        (!self.animated.is_empty()).then(|| self.snapshot(time))
    }

    fn snapshot(&self, time: f32) -> SceneSnapshot {
        SceneSnapshot {
            time,
            items: self
                .drawables
                .values()
                .flat_map(|object| object.items.iter().cloned())
                .collect(),
        }
    }

    pub fn sync(&self, renderer: &mut dyn FrameRenderer, time: f32) -> Result<(), String> {
        let ready = renderer.scene_sync();
        let changes;
        let update = if ready == SceneSync::At(self.revision) {
            SceneUpdate::Delta {
                base: self.revision,
                revision: self.revision,
                time,
                changes: &[],
            }
        } else if ready == SceneSync::At(self.base) {
            changes = self
                .delta
                .iter()
                .map(|(&id, &kind)| SceneChange {
                    id,
                    kind,
                    items: self
                        .drawables
                        .get(&id)
                        .map_or(&[], |object| object.items.as_slice()),
                })
                .collect::<Vec<_>>();
            SceneUpdate::Delta {
                base: self.base,
                revision: self.revision,
                time,
                changes: &changes,
            }
        } else {
            SceneUpdate::Reset {
                revision: self.revision,
                snapshot: self.snapshot(time),
            }
        };
        renderer.update_scene(update)?;
        if renderer.scene_sync() != SceneSync::At(self.revision) {
            return Err("Renderer did not acknowledge the retained scene revision".into());
        }
        Ok(())
    }
}

fn unlink(children: &mut BTreeMap<u64, BTreeSet<u64>>, id: u64, parent: Option<u64>) {
    if let Some(parent) = parent
        && let Some(siblings) = children.get_mut(&parent)
    {
        siblings.remove(&id);
        if siblings.is_empty() {
            children.remove(&parent);
        }
    }
}
