mod com;
pub mod dialog_ffi;

use dialog_ffi::{IDialog, Result};

use crate::backend::DialogFutureType;
use crate::FileDialog;
use crate::FileHandle;

use std::path::PathBuf;

use super::utils::init_com;

fn async_thread<T, F>(f: F) -> DialogFutureType<Option<T>>
where
    F: FnOnce() -> Option<T>,
    F: Send + 'static,
    T: Send + 'static,
{
    Box::pin(async move {
        let (tx, rx) = crate::oneshot::channel();

        std::thread::spawn(move || {
            tx.send(f()).ok();
        });

        rx.await.ok()?
    })
}

fn select_path(
    options: &FileDialog,
    build: impl FnOnce(&FileDialog) -> Result<IDialog>,
) -> std::result::Result<Option<PathBuf>, String> {
    const CANCELLED: i32 = 0x8007_04C7_u32 as i32;
    init_com(|| {
        let dialog = build(options)?;
        match dialog.show() {
            Ok(()) => dialog.get_result().map(Some),
            Err(CANCELLED) => Ok(None),
            Err(error) => Err(error),
        }
    })
    .and_then(|selection| selection)
    .map_err(|error| {
        format!(
            "Windows file dialog failed (HRESULT 0x{:08X}).",
            error as u32
        )
    })
}

impl FileDialog {
    /// Pick a file, distinguishing a system failure from cancellation.
    pub fn try_pick_file(self) -> std::result::Result<Option<PathBuf>, String> {
        select_path(&self, IDialog::build_pick_file)
    }

    /// Pick a folder, distinguishing a system failure from cancellation.
    pub fn try_pick_folder(self) -> std::result::Result<Option<PathBuf>, String> {
        select_path(&self, IDialog::build_pick_folder)
    }

    /// Pick a save path, distinguishing a system failure from cancellation.
    pub fn try_save_file(self) -> std::result::Result<Option<PathBuf>, String> {
        select_path(&self, IDialog::build_save_file)
    }
}

//
// File Picker
//

use crate::backend::FilePickerDialogImpl;
impl FilePickerDialogImpl for FileDialog {
    fn pick_file(self) -> Option<PathBuf> {
        self.try_pick_file().ok().flatten()
    }

    fn pick_files(self) -> Option<Vec<PathBuf>> {
        fn run(opt: FileDialog) -> Result<Vec<PathBuf>> {
            init_com(|| {
                let dialog = IDialog::build_pick_files(&opt)?;
                dialog.show()?;
                dialog.get_results()
            })?
        }
        run(self).ok()
    }
}

use crate::backend::AsyncFilePickerDialogImpl;
impl AsyncFilePickerDialogImpl for FileDialog {
    fn pick_file_async(self) -> DialogFutureType<Option<FileHandle>> {
        async_thread(move || Self::pick_file(self).map(FileHandle::wrap))
    }

    fn pick_files_async(self) -> DialogFutureType<Option<Vec<FileHandle>>> {
        async_thread(move || {
            Self::pick_files(self).map(|res| res.into_iter().map(FileHandle::wrap).collect())
        })
    }
}

//
// Folder Picker
//

use crate::backend::FolderPickerDialogImpl;
impl FolderPickerDialogImpl for FileDialog {
    fn pick_folder(self) -> Option<PathBuf> {
        self.try_pick_folder().ok().flatten()
    }

    fn pick_folders(self) -> Option<Vec<PathBuf>> {
        fn run(opt: FileDialog) -> Result<Vec<PathBuf>> {
            init_com(|| {
                let dialog = IDialog::build_pick_folders(&opt)?;
                dialog.show()?;
                dialog.get_results()
            })?
        }
        run(self).ok()
    }
}

use crate::backend::AsyncFolderPickerDialogImpl;
impl AsyncFolderPickerDialogImpl for FileDialog {
    fn pick_folder_async(self) -> DialogFutureType<Option<FileHandle>> {
        async_thread(move || Self::pick_folder(self).map(FileHandle::wrap))
    }

    fn pick_folders_async(self) -> DialogFutureType<Option<Vec<FileHandle>>> {
        async_thread(move || {
            Self::pick_folders(self).map(|res| res.into_iter().map(FileHandle::wrap).collect())
        })
    }
}

//
// File Save
//

use crate::backend::FileSaveDialogImpl;
impl FileSaveDialogImpl for FileDialog {
    fn save_file(self) -> Option<PathBuf> {
        self.try_save_file().ok().flatten()
    }
}

use crate::backend::AsyncFileSaveDialogImpl;
impl AsyncFileSaveDialogImpl for FileDialog {
    fn save_file_async(self) -> DialogFutureType<Option<FileHandle>> {
        async_thread(move || Self::save_file(self).map(FileHandle::wrap))
    }
}
