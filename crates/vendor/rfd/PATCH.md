# ModuleEngine patch for rfd 0.18.0

This copy comes from the published [rfd 0.18.0 crate](https://crates.io/crates/rfd/0.18.0), upstream commit `2d35d7d07ab4d021ca92e5378ae0e6ed5fbf8d01`.

The Windows picker still opens if `IFileDialog::SetFolder` rejects the optional starting directory. The same picker uses the system default folder and shows once.

The Windows and Linux XDG backends expose `try_pick_file`, `try_pick_folder` and `try_save_file`. These methods distinguish cancellation from a Windows dialog error, a portal failure or a Zenity launch or execution error. A selection without a local path is also reported as an error. Replace this copy with the registry dependency when a release provides both behaviors.
