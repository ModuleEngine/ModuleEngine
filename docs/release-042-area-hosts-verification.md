# Area host parity verification

[English](release-042-area-hosts-verification.md) | [Русский](ru/release-042-area-hosts-verification.md) | [Documentation](README.md)

Implemented scope: a floating native host contains its own Area tree. The same
menu, recursive renderer and leaf operations serve main and floating hosts.
Split and type choice stay in the addressed host. Area Close removes one exact
leaf; OS × closes all current members of that host. Native host identity remains
stable after closing its original leaf. Return is absent from current UI.
Final-main Close remains disabled, with fallback help on disabled hover rather
than a permanent paragraph. Explicit drag docking can target either host kind.

Legacy flat detached records retain their singleton IDs/editor/geometry on load.
The optional `tree` is authoritative for new host membership; legacy `id` is the
stable host identity, and legacy `editor` is not a leaf-membership index.
Host identities and area identities have separate namespaces; `next_id` allocates
both, and leaf/split IDs remain unique across all trees. New layouts are not a
supported downgrade format. No authored JSON or SDK format migration is made.
Closing the origin host during tear-off finishes the transaction, closes current
members and keeps the moved leaf in its new host. Later Escape cannot revive
that host. Unrelated closes prune their leaves from rollback snapshots.

The shared compact cutoff is 480 logical points wide or 240 high, replacing the
earlier 800×360 host cutoff so the ordinary 760×510 floating window shows a real
split. Below that cutoff the active area/switcher remains available. The OS title,
system ×, native resize frame and right vector Area menu are retained. SSAO,
FXAA, 100% physical Scene resolution, source checks and Full/Team rights are
unchanged. There is no new claim about held-resize jitter, slow physical drags,
mixed DPI, Linux hardware, cold start or game FPS.

On Windows, docking resolves the top visible native HWND before selecting a
leaf in its host. Root identity comes from eframe Frame; child identity is bound
to the unique own-process HWND created synchronously for that viewport, and is
rebound after recreation. Titles and coincident rectangles are not identities.
The moving host/leaf is excluded; surviving leaves of its original host remain
eligible. A foreign or unbound window accepting pointer input blocks docking into hidden
areas. A layered `WS_EX_TRANSPARENT` window passes mouse input through and is
skipped, following [Win32 layered-window hit testing](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features#layered-windows).
The native failed-drop probe recorded a foreign transient overlay with style
`0x080800a8`; correct host bindings/leaf bounds were present, but the old resolver
stopped at that input-passthrough overlay. Its exact process identity was not
established. The fix excludes only the layered-plus-transparent combination;
ordinary opaque covers still block hidden targets.
The saved source slot handles fast out-and-back independently of live targets.
If creation produces an ambiguous HWND delta, no identity is guessed; docking
waits for a valid binding. No additional dependency or title suffix is introduced.

Evidence is recorded under `work/area-host-parity-042/` and
`outputs/reviews/area-host-parity-042/`; runnable isolated packages and owner
instructions are under `outputs/area-host-parity-042/`. Initial source digest:
`6b5f10837ab41325f4fb48d89bcf0c453b5e3fa3ddaf47644a7f549226634a40`.
The final source/artifact binding and review state are recorded with the delivery.
