//! Document-owned parameter drafts, keyed by stable graph and node IDs.
//! Views share these drafts; personal layouts never serialize them.
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct NodeDrafts {
    entries: BTreeMap<Vec<String>, Draft>,
}
struct Draft {
    module: String,
    base: Value,
    text: String,
    pending: bool,
}
impl NodeDrafts {
    pub fn has_pending(&self) -> bool {
        self.entries.values().any(|draft| draft.pending)
    }
    pub fn pending_labels(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|(_, draft)| draft.pending)
            .map(|(key, _)| format!("Root / {}", key.join(" / ")))
            .collect()
    }
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    pub fn discard(&mut self, key: &[String]) {
        self.entries.remove(key);
    }

    /// Clean text follows live values. Pending text survives navigation and
    /// external changes, which require explicit conflict resolution.
    pub fn view(&mut self, key: &[String], module: &str, current: &Value) -> (String, bool, bool) {
        let draft = self.entries.entry(key.to_vec()).or_insert_with(|| Draft {
            module: module.into(),
            base: current.clone(),
            text: serde_json::to_string_pretty(current).unwrap_or_default(),
            pending: false,
        });
        if !draft.pending && (draft.base != *current || draft.module != module) {
            draft.base = current.clone();
            draft.module = module.into();
            draft.text = serde_json::to_string_pretty(current).unwrap_or_default();
        } else if draft.pending
            && draft.module == module
            && serde_json::from_str::<Value>(&draft.text).ok().as_ref() == Some(current)
        {
            draft.base = current.clone();
            draft.pending = false;
        }
        let result = (
            draft.text.clone(),
            draft.pending,
            draft.pending && (draft.base != *current || draft.module != module),
        );
        // Keep pending drafts, not a second copy of every browsed parameter set.
        if self.entries.len() > 256 {
            self.entries
                .retain(|old, draft| draft.pending || old == key);
        }
        result
    }
    pub fn update(&mut self, key: Vec<String>, module: &str, current: &Value, text: String) {
        self.view(&key, module, current);
        let draft = self.entries.get_mut(&key).unwrap();
        draft.pending = serde_json::from_str::<Value>(&text).ok().as_ref() != Some(&draft.base);
        draft.text = text;
    }
    pub fn candidate(
        &mut self,
        key: &[String],
        module: &str,
        current: &Value,
    ) -> Result<Option<Value>, String> {
        let (text, pending, conflict) = self.view(key, module, current);
        if conflict {
            return Err(
                "Parameters changed outside this draft. Discard the draft to load current values."
                    .into(),
            );
        }
        if !pending {
            return Ok(None);
        }
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|error| format!("Invalid parameter JSON: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scopes_conflicts_invalid_text_and_noop_keep_exact_values() {
        let mut drafts = NodeDrafts::default();
        let root = vec!["same-id".into()];
        let nested = vec!["renderer".into(), "same-id".into()];
        let initial = json!({"unknown":null,"value":1});
        let edit = "{\"unknown\":null,\"value\":2}";
        drafts.update(root.clone(), "scalar.value", &initial, edit.into());
        assert!(!drafts.view(&nested, "scalar.value", &initial).1);
        assert_eq!(drafts.view(&root, "scalar.value", &initial).0, edit);
        assert_eq!(
            drafts.candidate(&root, "scalar.value", &initial).unwrap(),
            Some(json!({"unknown":null,"value":2}))
        );
        let external = json!({"unknown":null,"value":3});
        assert!(drafts.candidate(&root, "scalar.value", &external).is_err());
        assert_eq!(drafts.view(&root, "scalar.value", &external).0, edit);
        drafts.discard(&root);
        assert_eq!(
            drafts.candidate(&root, "scalar.value", &external).unwrap(),
            None
        );
        drafts.update(root.clone(), "scalar.value", &external, "{".into());
        assert!(drafts.candidate(&root, "scalar.value", &external).is_err());
        assert!(drafts.has_pending());
        drafts.clear();
        assert!(!drafts.has_pending());
    }
}
