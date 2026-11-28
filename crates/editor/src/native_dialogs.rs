//! Native path selection. The editor calls this adapter from its dialog worker.
use module_engine_core::AUDIO_CUE_EXTENSIONS;
use rfd::FileDialog;
use std::path::{Path, PathBuf};

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
    Ok(project_dialog("Open ModuleEngine Project", directory).pick_file())
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
    save_selection(dialog.save_file())
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
    Ok(dialog(title, directory).pick_folder())
}

pub fn import_asset(directory: &Path) -> Result<Option<PathBuf>, String> {
    let render_formats = ["gltf", "glb", "png", "jpg", "jpeg"];
    let supported = [render_formats.as_slice(), &AUDIO_CUE_EXTENSIONS].concat();
    Ok(dialog("Import Asset", directory)
        .add_filter("Supported assets", &supported)
        .add_filter("3D models", &render_formats[..2])
        .add_filter("Textures", &render_formats[2..])
        .add_filter("Audio", &AUDIO_CUE_EXTENSIONS)
        .pick_file())
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
