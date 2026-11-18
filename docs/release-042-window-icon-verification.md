# Editor window icon verification

[English](release-042-window-icon-verification.md) | [Русский](ru/release-042-window-icon-verification.md) | [Documentation](README.md)

After the [area-host delivery](release-042-area-hosts-verification.md), the user
reported a missing running Editor taskbar/window icon and identified the existing
original icon. The initial invented cube was incorrect and has been removed.
The original installed ICO is copied byte-for-byte to
`crates/editor/assets/editor.ico`; SHA-256
`774d701d80d2f7ede634a228075d58627e2887d3776a08ed07136a3223200e9a`.
Its source was the Desktop Module Engine shortcut's IconLocation:
`C:\Users\MyJname\AppData\Local\ModuleEngine\Icons\editor-774d701d80d2f7ed.ico,0`.
The black rounded square and white modular hexagon are reused without redrawing.
All seven original PNG frames (16/24/32/48/64/128/256) remain inside the ICO.

Root and floating builders share one cached `IconData` decoded from the exact
64px frame through existing eframe support. Native Windows small/large icon sizes
come from system metrics: an exact original frame is selected when available,
otherwise the smallest larger original (or largest available) is decoded/scaled
by Windows. The 256px preview is an exact extracted original PNG, not a redraw.
No dependency, generated replacement image or branding change is introduced.

Pinned eframe 0.36.2 documents that winit sets only `ICON_SMALL`; its fallback
sets both slots on `GetActiveWindow` once, which does not bind every floating
host. The existing exact own-process HWND binding therefore assigns `ICON_BIG`
and `ICON_SMALL` on first binding/recreation. Two process-lifetime handles are
cached. Original PNG frames feed `CreateIconFromResourceEx`; borrowed old handles
are not destroyed. A partial new-pair failure releases only its new handle.
Invalid/foreign HWNDs are rejected; `WM_GETICON` verifies both assignments.

The exact ICO path is a required native source input, included in normal Full
source inventory and exports. The existing source-completeness test verifies
that sealing retains it, a missing file or omitted pin rejects completeness,
and an unrelated ICO does not become accepted implicitly. No broad asset rule
or manual manifest/pin bypass was added.

The release `app_icon::tests --include-ignored --test-threads=1` checks all seven
original frames and dimensions, exact decoded 64px pixels, fallback size choice,
shared cache and two actual hidden offscreen Win32 windows. Root assignment does
not change the other window; after both assignments both small/large slots are
nonzero and equal. Invalid HWND is rejected. These are native slot checks, not
observations of the actual taskbar rendering. Prior area functionality retains
its scoped independent evidence; its native suite is not repeated for this icon
correction.

Source, commands, preview and hashes are in `work/window-icon-original-042/` and
`outputs/reviews/window-icon-original-042/`. The corrected separate candidate is
under `outputs/area-host-parity-042/original-icon/`, with normally sealed Full
and normally exported Team. Previous packages remain unchanged.

Computer Use previously stopped on explicit owner input in an older Full window.
No further GUI activation, overwrite or closure of it is performed. Actual
running Editor titlebar/taskbar appearance, physical DPI and owner visual
acceptance remain pending. EXE file resources and Player branding are unchanged.
