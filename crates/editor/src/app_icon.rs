//! One editor mark for all native hosts. Player branding is independent.
use eframe::egui::IconData;
use std::sync::{Arc, OnceLock};

// Original installed ModuleEngine icon, retained byte-for-byte with all seven sizes.
const ORIGINAL_ICO: &[u8] = include_bytes!("../assets/editor.ico");

fn original_frame(size: u32) -> &'static [u8] {
    let count = u16::from_le_bytes(ORIGINAL_ICO[4..6].try_into().unwrap()) as usize;
    let entry = (0..count)
        .map(|index| &ORIGINAL_ICO[6 + index * 16..6 + (index + 1) * 16])
        .min_by_key(|entry| {
            let width = if entry[0] == 0 {
                256
            } else {
                u32::from(entry[0])
            };
            // Prefer an exact frame, otherwise the smallest larger original.
            (width < size, width.abs_diff(size))
        })
        .expect("embedded editor ICO contains frames");
    let length = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
    let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
    &ORIGINAL_ICO[offset..offset + length]
}

pub fn icon() -> Arc<IconData> {
    static ICON: OnceLock<Arc<IconData>> = OnceLock::new();
    ICON.get_or_init(|| {
        Arc::new(
            eframe::icon_data::from_png_bytes(original_frame(64))
                .expect("embedded original editor icon is a valid PNG"),
        )
    })
    .clone()
}

#[cfg(windows)]
pub fn apply_to_window(window: usize) -> Result<(), String> {
    windows::apply(window)
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::ffi::c_void;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn CreateIconFromResourceEx(
            bytes: *mut u8,
            length: u32,
            icon: i32,
            version: u32,
            width: i32,
            height: i32,
            flags: u32,
        ) -> *mut c_void;
        fn DestroyIcon(icon: *mut c_void) -> i32;
        fn GetSystemMetrics(index: i32) -> i32;
        fn GetWindowThreadProcessId(window: *mut c_void, process: *mut u32) -> u32;
        fn SendMessageW(window: *mut c_void, message: u32, wparam: usize, lparam: isize) -> isize;
    }
    struct Icons {
        big: usize,
        small: usize,
    }
    fn create(size: i32) -> Result<usize, String> {
        let size = size.clamp(16, 256);
        let mut bytes = original_frame(size as u32).to_vec();
        // Decode the selected original ICO frame; Windows scales only for non-original sizes.
        let handle = unsafe {
            CreateIconFromResourceEx(
                bytes.as_mut_ptr(),
                bytes.len() as u32,
                1,
                0x0003_0000,
                size,
                size,
                0,
            )
        };
        if handle.is_null() {
            Err("Cannot create editor window icon".into())
        } else {
            Ok(handle as usize)
        }
    }
    fn icons() -> Result<&'static Icons, String> {
        // Process-lifetime owners: every host may reference these handles until
        // shutdown. Never destroy an old icon returned by WM_SETICON: winit owns it.
        static ICONS: OnceLock<Result<Icons, String>> = OnceLock::new();
        ICONS
            .get_or_init(|| {
                let big = create(unsafe { GetSystemMetrics(11) })?; // SM_CXICON
                match create(unsafe { GetSystemMetrics(49) }) {
                    // SM_CXSMICON
                    Ok(small) => Ok(Icons { big, small }),
                    Err(error) => {
                        unsafe {
                            DestroyIcon(big as *mut c_void);
                        }
                        Err(error)
                    }
                }
            })
            .as_ref()
            .map_err(Clone::clone)
    }
    pub fn apply(window: usize) -> Result<(), String> {
        let window = window as *mut c_void;
        let mut process = 0;
        unsafe {
            GetWindowThreadProcessId(window, &mut process);
        }
        if process != std::process::id() {
            return Err("Editor icon target is not an owned window".into());
        }
        let icons = icons()?;
        unsafe {
            SendMessageW(window, 0x0080, 1, icons.big as isize); // WM_SETICON / ICON_BIG
            SendMessageW(window, 0x0080, 0, icons.small as isize); // ICON_SMALL
            if SendMessageW(window, 0x007f, 1, 0) != icons.big as isize
                || SendMessageW(window, 0x007f, 0, 0) != icons.small as isize
            {
                return Err("Editor window did not retain both icon sizes".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_frames_are_retained_and_icon_data_is_cached() {
        let first = icon();
        assert!(Arc::ptr_eq(&first, &icon()));
        assert_eq!((first.width, first.height), (64, 64));
        assert_eq!(
            first.rgba,
            eframe::icon_data::from_png_bytes(original_frame(64))
                .unwrap()
                .rgba
        );
        for size in [16, 24, 32, 48, 64, 128, 256] {
            let frame = eframe::icon_data::from_png_bytes(original_frame(size)).unwrap();
            assert_eq!((frame.width, frame.height), (size, size));
        }
        assert_eq!(original_frame(20), original_frame(24));
        assert_eq!(original_frame(96), original_frame(128));
        assert_eq!(original_frame(300), original_frame(256));
        if let Some(path) = std::env::var_os("MODULE_ENGINE_ICON_PREVIEW_PATH") {
            std::fs::write(path, original_frame(256)).unwrap();
        }
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "Native Win32 icon slots; run explicitly and sequentially"]
    fn both_icon_slots_are_set_on_each_exact_owned_window() {
        use std::ffi::c_void;
        #[link(name = "user32")]
        unsafe extern "system" {
            fn CreateWindowExW(
                ex: u32,
                class: *const u16,
                title: *const u16,
                style: u32,
                x: i32,
                y: i32,
                width: i32,
                height: i32,
                parent: *mut c_void,
                menu: *mut c_void,
                instance: *mut c_void,
                param: *mut c_void,
            ) -> *mut c_void;
            fn DestroyWindow(window: *mut c_void) -> i32;
            fn SendMessageW(
                window: *mut c_void,
                message: u32,
                wparam: usize,
                lparam: isize,
            ) -> isize;
        }
        struct Window(*mut c_void);
        impl Window {
            fn new() -> Self {
                let class = "STATIC\0".encode_utf16().collect::<Vec<_>>();
                let title = "ModuleEngine icon fixture\0"
                    .encode_utf16()
                    .collect::<Vec<_>>();
                let window = unsafe {
                    CreateWindowExW(
                        0x08000080,
                        class.as_ptr(),
                        title.as_ptr(),
                        0x80000000,
                        -30000,
                        -30000,
                        64,
                        64,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                };
                assert!(!window.is_null());
                Self(window)
            }
            fn slots(&self) -> [isize; 2] {
                unsafe {
                    [
                        SendMessageW(self.0, 0x007f, 0, 0),
                        SendMessageW(self.0, 0x007f, 1, 0),
                    ]
                }
            }
        }
        impl Drop for Window {
            fn drop(&mut self) {
                unsafe {
                    DestroyWindow(self.0);
                }
            }
        }
        let root = Window::new();
        let floating = Window::new();
        let before = floating.slots();
        apply_to_window(root.0 as usize).unwrap();
        assert!(root.slots().iter().all(|handle| *handle != 0));
        assert_eq!(
            floating.slots(),
            before,
            "setting root must not target another active window"
        );
        apply_to_window(floating.0 as usize).unwrap();
        assert_eq!(root.slots(), floating.slots());
        assert!(apply_to_window(0).is_err());
    }
}
