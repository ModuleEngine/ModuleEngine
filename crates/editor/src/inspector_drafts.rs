//! Object-scoped, source-aware Inspector drafts; never serialized into a scene.
use module_engine_core::{same_component_json, same_component_payloads};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

type Values = BTreeMap<String, Value>;
type Key = (String, u64);
#[derive(Default)]
struct Entry {
    base: Values,
    values: Values,
    schemas: Values,
    conflicts: BTreeSet<String>,
    raw_base: String,
    raw: String,
    raw_conflict: bool,
}
#[derive(Default)]
pub struct InspectorDrafts {
    active: Option<Key>,
    entries: BTreeMap<Key, Entry>,
}
impl InspectorDrafts {
    /// Reconcile clean values with Undo/source updates while retaining local
    /// edits. A conflicting candidate remains visible until explicit Discard.
    pub fn activate(
        &mut self,
        key: Key,
        base: Values,
        schemas: Values,
        raw_base: String,
        values: &mut Values,
        raw: &mut String,
    ) {
        if let Some(entry) = self
            .active
            .as_ref()
            .and_then(|key| self.entries.get_mut(key))
        {
            entry.values.clone_from(values);
            entry.raw.clone_from(raw);
        }
        let entry = self.entries.entry(key.clone()).or_insert_with(|| Entry {
            values: base.clone(),
            base: base.clone(),
            schemas: schemas.clone(),
            raw: raw_base.clone(),
            raw_base: raw_base.clone(),
            ..Default::default()
        });
        let names: BTreeSet<_> = entry
            .base
            .keys()
            .chain(entry.values.keys())
            .chain(base.keys())
            .cloned()
            .collect();
        for name in names {
            let clean = same_component_json(entry.values.get(&name), entry.base.get(&name));
            let accepted = same_component_json(entry.values.get(&name), base.get(&name));
            if clean || accepted {
                if let Some(value) = base.get(&name) {
                    entry.base.insert(name.clone(), value.clone());
                    entry.values.insert(name.clone(), value.clone());
                } else {
                    entry.base.remove(&name);
                    entry.values.remove(&name);
                }
                entry.conflicts.remove(&name);
            } else if !same_component_json(base.get(&name), entry.base.get(&name))
                || schemas.get(&name) != entry.schemas.get(&name)
            {
                entry.conflicts.insert(name.clone());
            }
        }
        if entry.raw == entry.raw_base || entry.raw == raw_base {
            entry.raw.clone_from(&raw_base);
            entry.raw_base.clone_from(&raw_base);
            entry.raw_conflict = false;
        } else if entry.raw_base != raw_base {
            entry.raw_conflict = true;
        }
        entry.schemas = schemas;
        values.clone_from(&entry.values);
        raw.clone_from(&entry.raw);
        self.active = Some(key.clone());
        // Browsing many objects must not retain a second copy of every clean
        // object/schema. Only pending drafts and the current baseline survive.
        self.entries.retain(|cached, entry| {
            cached == &key
                || !same_component_payloads(&entry.values, &entry.base)
                || entry.raw != entry.raw_base
        });
    }
    pub fn has_drafts(&self, values: &Values, raw: &str) -> bool {
        self.entries.iter().any(|(key, entry)| {
            let (values, raw) = if self.active.as_ref() == Some(key) {
                (values, raw)
            } else {
                (&entry.values, entry.raw.as_str())
            };
            !same_component_payloads(values, &entry.base) || raw != entry.raw_base
        })
    }
    pub fn changed(&self, name: &str, values: &Values) -> bool {
        self.active
            .as_ref()
            .and_then(|k| self.entries.get(k))
            .is_some_and(|e| !same_component_json(e.base.get(name), values.get(name)))
    }
    pub fn conflict(&self, name: &str) -> bool {
        self.active
            .as_ref()
            .and_then(|k| self.entries.get(k))
            .is_some_and(|e| e.conflicts.contains(name))
    }
    pub fn raw_conflict(&self) -> bool {
        self.active
            .as_ref()
            .and_then(|k| self.entries.get(k))
            .is_some_and(|e| e.raw_conflict)
    }
    pub fn accept(&mut self, name: &str, value: Option<Value>, values: &mut Values) {
        if let Some(entry) = self.active.as_ref().and_then(|k| self.entries.get_mut(k)) {
            if let Some(value) = value {
                values.insert(name.into(), value.clone());
                entry.base.insert(name.into(), value);
            } else {
                values.remove(name);
                entry.base.remove(name);
            }
            entry.conflicts.remove(name);
        }
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn accept_raw(&mut self, raw: &str) {
        if let Some(entry) = self.active.as_ref().and_then(|k| self.entries.get_mut(k)) {
            entry.raw = raw.into();
            entry.raw_base = raw.into();
            entry.raw_conflict = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn component_apply_and_selection_retain_other_drafts_and_detect_source_conflicts() {
        let mut cache = InspectorDrafts::default();
        let mut values = Values::new();
        let mut raw = String::new();
        let base = Values::from([
            ("actor".into(), json!({"speed":6})),
            ("health".into(), json!({"hp":100})),
        ]);
        let schemas = Values::from([
            ("actor".into(), json!("v1")),
            ("health".into(), json!("v1")),
        ]);
        cache.activate(
            ("arena".into(), 1),
            base.clone(),
            schemas.clone(),
            "raw".into(),
            &mut values,
            &mut raw,
        );
        values.get_mut("actor").unwrap()["speed"] = json!(9);
        values.get_mut("health").unwrap()["hp"] = json!(70);
        cache.accept("actor", Some(json!({"speed":9})), &mut values);
        let applied = Values::from([
            ("actor".into(), json!({"speed":9})),
            ("health".into(), json!({"hp":100})),
        ]);
        cache.activate(
            ("arena".into(), 2),
            base.clone(),
            schemas.clone(),
            "raw2".into(),
            &mut values,
            &mut raw,
        );
        cache.activate(
            ("arena".into(), 1),
            applied.clone(),
            schemas.clone(),
            "raw-applied".into(),
            &mut values,
            &mut raw,
        );
        assert_eq!(values["health"]["hp"], 70);
        assert_eq!(values["actor"]["speed"], 9);
        assert!(cache.has_drafts(&values, &raw));
        assert!(!cache.conflict("health"));
        let mut changed = applied;
        changed.insert("health".into(), json!({"hp":110}));
        cache.activate(
            ("arena".into(), 1),
            changed,
            schemas,
            "raw-external".into(),
            &mut values,
            &mut raw,
        );
        assert_eq!(values["health"]["hp"], 70);
        assert!(cache.conflict("health"));
        cache.clear();
        assert!(!cache.has_drafts(&Values::new(), ""));
    }
}
