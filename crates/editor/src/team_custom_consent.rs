//! Per-user consent kept outside the Team package and authored document.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Choice {
    Allow,
    Deny,
}

#[derive(Clone, Debug)]
pub(crate) struct Consent {
    record: Option<PathBuf>,
    choice: Option<Choice>,
}

pub(crate) fn state_directory() -> Option<PathBuf> {
    let absolute_env = |name| {
        std::env::var_os(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    let base = if cfg!(windows) {
        absolute_env("LOCALAPPDATA")
    } else {
        absolute_env("XDG_STATE_HOME")
            .or_else(|| absolute_env("HOME").map(|home| home.join(".local/state")))
    }?;
    Some(base.join("ModuleEngine/team-custom-rust"))
}

impl Consent {
    pub(crate) fn load(project: &Path, scope: &[u8], directory: Option<&Path>) -> Self {
        let record = directory.and_then(|directory| {
            let canonical = project.canonicalize().ok()?;
            let mut identity = canonical.to_str()?.as_bytes().to_vec();
            identity.push(0);
            identity.extend_from_slice(scope);
            let key = module_engine_project::hash_bytes(&identity);
            Some(directory.join(key).with_extension("decision"))
        });
        let choice = match record
            .as_ref()
            .and_then(|path| fs::read(path).ok())
            .as_deref()
        {
            Some(b"allow\n") => Some(Choice::Allow),
            Some(b"deny\n") => Some(Choice::Deny),
            _ => None,
        };
        Self { record, choice }
    }

    pub(crate) fn choice(&self) -> Option<Choice> {
        self.choice
    }

    pub(crate) fn record(&mut self, choice: Choice) -> Result<(), String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = self.record.as_ref().ok_or(
            "Custom Rust permission needs a saved project and an available per-user state directory",
        )?;
        let save = || -> std::io::Result<()> {
            fs::create_dir_all(path.parent().expect("consent record has a state directory"))?;
            let staged = path.with_extension(format!(
                "{}-{}.pending",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&staged)?;
            let contents = match choice {
                Choice::Allow => b"allow\n".as_slice(),
                Choice::Deny => b"deny\n".as_slice(),
            };
            let written = file.write_all(contents).and_then(|()| file.sync_all());
            drop(file);
            let published = written.and_then(|()| fs::rename(&staged, path));
            if published.is_err() {
                let _ = fs::remove_file(staged);
            }
            published
        };
        save().map_err(|error| format!("Could not save Custom Rust permission: {error}"))?;
        self.choice = Some(choice);
        Ok(())
    }
}
