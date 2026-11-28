# ModuleEngine patch for rfd 0.18.0

This copy comes from the published [rfd 0.18.0 crate](https://crates.io/crates/rfd/0.18.0), upstream commit `2d35d7d07ab4d021ca92e5378ae0e6ed5fbf8d01`. It carries one change in `src/backend/win_cid/file_dialog/dialog_ffi.rs`. The Windows picker still opens if `IFileDialog::SetFolder` rejects the optional starting directory. The same picker uses the system default folder and shows once. Replace this copy with the registry dependency when a release preserves that behavior.
