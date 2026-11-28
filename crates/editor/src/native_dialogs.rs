//! Native path selection. The editor calls this adapter from its dialog worker.
use module_engine_core::AUDIO_CUE_EXTENSIONS;
use rfd::FileDialog;
use std::path::{Path, PathBuf};

// rfd exposes detailed outcomes for the Windows and Linux backends used by the editor.
#[cfg(not(any(windows, target_os = "linux")))]
trait OptionalPicker {
    fn try_pick_file(self) -> Result<Option<PathBuf>, String>;
    fn try_pick_folder(self) -> Result<Option<PathBuf>, String>;
    fn try_save_file(self) -> Result<Option<PathBuf>, String>;
}

#[cfg(not(any(windows, target_os = "linux")))]
impl OptionalPicker for FileDialog {
    fn try_pick_file(self) -> Result<Option<PathBuf>, String> {
        Ok(self.pick_file())
    }

    fn try_pick_folder(self) -> Result<Option<PathBuf>, String> {
        Ok(self.pick_folder())
    }

    fn try_save_file(self) -> Result<Option<PathBuf>, String> {
        Ok(self.save_file())
    }
}

pub(crate) fn initial_directory(path: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        windows::folder_hint_path(path)
    }
    #[cfg(not(windows))]
    {
        let directory = path.canonicalize().ok()?;
        directory.is_dir().then_some(directory)
    }
}

fn dialog(title: &str, directory: &Path) -> FileDialog {
    let dialog = FileDialog::new().set_title(title);
    match initial_directory(directory) {
        Some(directory) => dialog.set_directory(directory),
        None => dialog,
    }
}

fn project_dialog(title: &str, directory: &Path) -> FileDialog {
    dialog(title, directory).add_filter("ModuleEngine project", &["json"])
}

pub fn open_project(directory: &Path) -> Result<Option<PathBuf>, String> {
    project_dialog("Open ModuleEngine Project", directory).try_pick_file()
}

pub fn save_project(initial_path: &Path) -> Result<Option<PathBuf>, String> {
    let directory = initial_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut dialog = project_dialog("Save Project As", directory);
    if let Some(name) = initial_path.file_name().and_then(|name| name.to_str()) {
        dialog = dialog.set_file_name(name);
    }
    save_selection(dialog.try_save_file()?)
}

fn save_selection(path: Option<PathBuf>) -> Result<Option<PathBuf>, String> {
    if let Some(path) = &path
        && !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        // The native picker confirmed the exact path it returned. Changing the
        // extension here could select a different existing file.
        return Err("Choose a project filename ending in .json.".into());
    }
    Ok(path)
}

pub fn pick_folder(title: &str, directory: &Path) -> Result<Option<PathBuf>, String> {
    dialog(title, directory).try_pick_folder()
}

pub fn import_asset(directory: &Path) -> Result<Option<PathBuf>, String> {
    let render_formats = ["gltf", "glb", "png", "jpg", "jpeg"];
    let supported = [render_formats.as_slice(), &AUDIO_CUE_EXTENSIONS].concat();
    dialog("Import Asset", directory)
        .add_filter("Supported assets", &supported)
        .add_filter("3D models", &render_formats[..2])
        .add_filter("Textures", &render_formats[2..])
        .add_filter("Audio", &AUDIO_CUE_EXTENSIONS)
        .try_pick_file()
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        ffi::OsString,
        os::windows::ffi::{OsStrExt, OsStringExt},
    };
    /// The Shell hint needs ordinary absolute Win32 spelling. This conversion
    /// never changes an IO path, selection, binding, or process current directory.
    fn ordinary_shell_path(path: &Path) -> Option<PathBuf> {
        use std::path::{Component, Prefix};
        let mut units: Vec<_> = path.as_os_str().encode_wide().collect();
        if units.contains(&0) {
            return None;
        }
        for unit in &mut units {
            if *unit == b'/' as u16 {
                *unit = b'\\' as u16;
            }
        }
        let normalized = PathBuf::from(OsString::from_wide(&units));
        let ordinary = match normalized.components().next() {
            Some(Component::Prefix(prefix)) => match prefix.kind() {
                Prefix::VerbatimDisk(_) | Prefix::VerbatimUNC(_, _) => {
                    // Names with significant verbatim-only trailing characters
                    // cannot safely be represented as ordinary Shell paths.
                    if normalized.components().any(|component| {
                        if let Component::Normal(name) = component {
                            let name: Vec<_> = name.encode_wide().collect();
                            name != [b'.' as u16]
                                && name != [b'.' as u16, b'.' as u16]
                                && matches!(name.last(), Some(32 | 46))
                        } else {
                            false
                        }
                    }) {
                        return None;
                    }
                    if let Prefix::VerbatimUNC(server, share) = prefix.kind() {
                        let mut ordinary = OsString::from("\\\\");
                        ordinary.push(server);
                        ordinary.push("\\");
                        ordinary.push(share);
                        let count = prefix.as_os_str().encode_wide().count();
                        ordinary.push(OsString::from_wide(&units[count..]));
                        ordinary
                    } else {
                        OsString::from_wide(&units[4..])
                    }
                }
                Prefix::DeviceNS(_) | Prefix::Verbatim(_) => return None,
                Prefix::Disk(_) | Prefix::UNC(_, _) => normalized.into_os_string(),
            },
            _ => normalized.into_os_string(),
        };
        Some(PathBuf::from(ordinary))
    }

    pub(super) fn folder_hint_path(path: &Path) -> Option<PathBuf> {
        // Validate the namespace first, but canonicalize the ORIGINAL IO path.
        // This resolves relative/slash/dot spellings without silently changing
        // verbatim-only path semantics. Unrepresentable hints are simply absent.
        ordinary_shell_path(path)?;
        let canonical = path.canonicalize().ok()?;
        let ordinary = ordinary_shell_path(&canonical)?;
        if !ordinary.is_dir() || ordinary.canonicalize().ok()? != canonical {
            return None;
        }
        Some(ordinary)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn shell_hint_spelling_preserves_utf16_and_only_maps_drive_or_unc_namespaces() {
            for (input, expected) in [
                (r"E:/Игры/assets", r"E:\Игры\assets"),
                (r"\\?\E:\Игры\assets", r"E:\Игры\assets"),
                (r"\\server\share\Игры", r"\\server\share\Игры"),
                (r"\\?\UNC\server\share\Игры", r"\\server\share\Игры"),
            ] {
                assert_eq!(
                    ordinary_shell_path(Path::new(input)),
                    Some(PathBuf::from(expected)),
                    "{input}"
                );
            }
            let mut units: Vec<_> = r"\\?\E:\folder-".encode_utf16().collect();
            units.push(0xd800); // Unpaired UTF-16 is valid in an OsString.
            let input = PathBuf::from(OsString::from_wide(&units));
            assert_eq!(
                ordinary_shell_path(&input)
                    .unwrap()
                    .as_os_str()
                    .encode_wide()
                    .collect::<Vec<_>>(),
                units[4..]
            );
            for path in [
                r"\\.\C:\folder",
                r"\\?\Volume{00000000-0000-0000-0000-000000000000}\folder",
                r"\\?\GLOBALROOT\Device\HarddiskVolume1\folder",
                r"\\?\E:\folder.\child",
                r"\\?\E:\folder \child",
                r"\\?\UNC\server\share\folder.",
            ] {
                assert!(ordinary_shell_path(Path::new(path)).is_none(), "{path}");
            }
            let null = PathBuf::from(OsString::from_wide(&[b'E' as u16, 0, b'x' as u16]));
            assert!(ordinary_shell_path(&null).is_none());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod contracts {
        use super::*;
        #[test]
        fn directory_hint_resolves_the_same_unicode_directory_and_missing_hints_are_optional() {
            let fixture = module_engine_script::TestArtifactGuard::new(
                &std::env::temp_dir(),
                "r64-dialog-hints",
            )
            .unwrap();
            let directory = fixture.path().join("Игры");
            std::fs::create_dir(&directory).unwrap();
            let current = std::env::current_dir().unwrap();
            let normalized = initial_directory(&directory.join("."))
                .expect("existing hint")
                .canonicalize()
                .unwrap();
            assert_eq!(normalized, directory.canonicalize().unwrap());
            assert!(initial_directory(&directory.join("missing")).is_none());
            assert_eq!(std::env::current_dir().unwrap(), current);
        }

        #[test]
        fn save_choice_retains_the_exact_confirmed_json_path_and_cancel() {
            let path = PathBuf::from("Игры/новый проект.JSON");
            assert_eq!(save_selection(Some(path.clone())).unwrap(), Some(path));
            assert!(save_selection(None).unwrap().is_none());
            assert!(
                save_selection(Some("existing".into()))
                    .unwrap_err()
                    .contains(".json")
            );
        }

        #[cfg(target_os = "linux")]
        #[test]
        fn linux_picker_distinguishes_backend_errors_and_cancellation() {
            const CHILD: &str = "MODULE_ENGINE_MISSING_PICKERS_TEST_CHILD";
            if std::env::var_os(CHILD).is_some() {
                use std::{fs, os::unix::fs::PermissionsExt};

                let error = open_project(Path::new(".")).unwrap_err();
                assert!(error.contains("XDG file chooser"), "{error}");
                assert!(error.contains("Zenity failed"), "{error}");

                let fixture = Path::new("zenity");
                let set_picker = |script: &str| {
                    fs::write(fixture, format!("#!/bin/sh\n{script}\n")).unwrap();
                    fs::set_permissions(fixture, fs::Permissions::from_mode(0o700)).unwrap();
                };
                set_picker("exit 2");
                let error = open_project(Path::new(".")).unwrap_err();
                assert!(error.contains("exit status: 2"), "{error}");
                set_picker("exit 1");
                assert_eq!(open_project(Path::new(".")).unwrap(), None);
                set_picker("exit 0");
                let error = open_project(Path::new(".")).unwrap_err();
                assert!(error.contains("no local file path"), "{error}");
                set_picker("printf '/tmp/project.json\\n'");
                assert_eq!(
                    open_project(Path::new(".")).unwrap(),
                    Some(PathBuf::from("/tmp/project.json"))
                );
                return;
            }
            let fixture = module_engine_script::TestArtifactGuard::new(
                &std::env::temp_dir(),
                "r64-missing-pickers",
            )
            .unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "native_dialogs::tests::contracts::linux_picker_distinguishes_backend_errors_and_cancellation",
                ])
                .current_dir(fixture.path())
                .env(CHILD, "1")
                .env("PATH", fixture.path())
                .env(
                    "DBUS_SESSION_BUS_ADDRESS",
                    format!("unix:path={}", fixture.path().join("no-bus").display()),
                )
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                output.status.success() && stdout.contains("1 passed"),
                "{stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[test]
    #[ignore = "Requires a desktop and an operator to accept and cancel native selections"]
    fn native_dialogs_choose_and_cancel() {
        let fixture = module_engine_script::TestArtifactGuard::new(
            &std::env::temp_dir(),
            "r64-native-dialogs",
        )
        .unwrap();
        let root = fixture.path();
        let project = root.join("project.json");
        let copy = root.join("copy.json");
        let asset = root.join("tone.ogg");
        let selected_folder = root.join("Selected");
        std::fs::write(&project, b"{}").unwrap();
        std::fs::write(&asset, b"picker fixture").unwrap();
        std::fs::create_dir(&selected_folder).unwrap();
        let current = std::env::current_dir().unwrap();
        eprintln!("Native picker fixture {}", root.display());
        eprintln!("Open ModuleEngine Project: select project.json");
        assert_eq!(
            open_project(root).unwrap().unwrap().canonicalize().unwrap(),
            project.canonicalize().unwrap()
        );
        eprintln!("Save Project As: accept suggested copy.json");
        let saved = save_project(&copy).unwrap().unwrap();
        assert_eq!(saved.file_name(), copy.file_name());
        assert_eq!(
            saved.parent().unwrap().canonicalize().unwrap(),
            root.canonicalize().unwrap()
        );
        assert!(!copy.exists());
        eprintln!(
            "Save Project As: choose existing project.json, decline replacement, then cancel"
        );
        assert!(save_project(&project).unwrap().is_none());
        assert_eq!(std::fs::read(&project).unwrap(), b"{}");
        eprintln!("Save Project As: choose existing project.json and confirm replacement");
        assert_eq!(
            save_project(&project)
                .unwrap()
                .unwrap()
                .canonicalize()
                .unwrap(),
            project.canonicalize().unwrap()
        );
        eprintln!("Pick folder: select Selected");
        assert_eq!(
            pick_folder("Select an output folder", root)
                .unwrap()
                .unwrap()
                .canonicalize()
                .unwrap(),
            selected_folder.canonicalize().unwrap()
        );
        eprintln!("Import Asset: select tone.ogg");
        assert_eq!(
            import_asset(root).unwrap().unwrap().canonicalize().unwrap(),
            asset.canonicalize().unwrap()
        );
        eprintln!("Open ModuleEngine Project: cancel");
        assert!(open_project(root).unwrap().is_none());
        assert_eq!(std::fs::read(&project).unwrap(), b"{}");
        assert_eq!(std::env::current_dir().unwrap(), current);
    }
}
