use std::path::Path;
use std::process::Command;

#[test]
fn self_check_matches_editor_features() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture =
        module_engine_script::TestArtifactGuard::new(&root.join(".work/test-temp"), "editor-cli")
            .unwrap();
    let report = fixture.path().join("self-check");
    let output = Command::new(env!("CARGO_BIN_EXE_module-engine-editor"))
        .current_dir(&root)
        .arg("--self-check")
        .arg(&report)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if cfg!(feature = "verification") {
        assert!(output.status.success(), "{stderr}");
        let data = std::fs::read(report.join("report.json")).unwrap();
        let data: serde_json::Value = serde_json::from_slice(&data).unwrap();
        assert_eq!(data["world_snapshots_equal"].as_bool(), Some(true));
        assert_eq!(data["world_frames_equal"].as_bool(), Some(true));
        assert!(
            data["geometry_pixels"]
                .as_u64()
                .is_some_and(|count| count > 0)
        );
    } else {
        assert_eq!(output.status.code(), Some(1), "{stderr}");
        assert!(
            stderr.lines().any(|line| line
                == "ModuleEngine: --self-check requires an Editor built with --features verification"),
            "{stderr}"
        );
        assert!(
            !report.exists(),
            "rejected self-check created an output folder"
        );
    }
}
