//! Capability preflight for tests that build actual Custom SDK workers.
use crate::ScriptCompiler;
use std::{path::Path, process::Command, sync::OnceLock};

fn offline_sdk() -> Result<Option<String>, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let compiler = ScriptCompiler::new(root.clone(), root.join(".work/test-temp"));
    let mut command = Command::new(
        compiler
            .cargo_binary
            .as_deref()
            .unwrap_or(Path::new("cargo")),
    );
    command
        .current_dir(&root)
        .args(["fetch", "--offline", "--locked"]);
    if let Some(home) = &compiler.cargo_home {
        command.env("CARGO_HOME", home);
    }
    if let Some(home) = &compiler.rustup_home {
        command.env("RUSTUP_HOME", home);
    }
    if let Some(toolchain) = &compiler.rustup_toolchain {
        command.env("RUSTUP_TOOLCHAIN", toolchain);
    }
    let result = command.output().map_err(|error| error.to_string())?;
    if result.status.success() {
        return Ok(None);
    }
    let stderr = String::from_utf8_lossy(&result.stderr);
    let missing_index_entry =
        stderr.contains("no matching package named") && stderr.contains("--offline");
    let missing_archive = stderr.contains("failed to download")
        && stderr.contains("attempting to make an HTTP request, but --offline was specified");
    if missing_index_entry || missing_archive {
        let home = compiler.cargo_home.as_ref().map_or_else(
            || "Cargo's default home".into(),
            |path| path.display().to_string(),
        );
        Ok(Some(format!(
            "Custom SDK: offline Cargo dependencies are missing. Run cargo fetch --locked from the checkout using {home}."
        )))
    } else {
        Err(stderr.into_owned())
    }
}

pub fn offline_sdk_available() -> bool {
    static OFFLINE_SDK: OnceLock<Result<Option<String>, String>> = OnceLock::new();
    match OFFLINE_SDK.get_or_init(offline_sdk) {
        Ok(None) => true,
        Ok(Some(reason)) => {
            let current = std::thread::current();
            let name = current
                .name()
                .expect("the SDK preflight runs on a libtest thread");
            let reason = reason.replace(['\r', '\n', '\t'], " ");
            println!("\nMODULEENGINE_TEST_SKIP\t{name}\t{reason}");
            false
        }
        Err(error) => panic!("Custom SDK offline cache probe failed: {error}"),
    }
}

/// Place this before creating fixtures or starting a real SDK build.
#[macro_export]
macro_rules! requires_offline_sdk {
    () => {
        if !$crate::verification::offline_sdk_available() {
            return;
        }
    };
}
