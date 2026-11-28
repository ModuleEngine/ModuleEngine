# Local verification queue on Windows

[English](local-queue.md) | [Русский](ru/local-queue.md) | [Documentation](README.md)

Use the shared queue on the Windows verification PC for Cargo test builds, Release
builds, GPU checks, native windows, captures and benchmarks. The GPU group runs one
job at a time. The light group runs up to two jobs and is reserved for read-only
checks that do not build or open windows. Coordinate desktop use and competing
workloads with the stand owner. Queue execution provides evidence for the author
and the package coordinator. Substantial changes still require [independent
review](review.md) and the applicable [verification profiles](verification.md).

## Submit and retrieve evidence

From PowerShell 7.5 or later, pass the absolute author checkout and its full,
committed HEAD. Use the owner-approved queue release. An installed release works
from any worktree, including branches created before these scripts land on main.

```powershell
$queue = 'E:\ModuleEngine-queue\releases\<approved-commit>\scripts\queue'
$checkout = (Get-Location).Path
$sha = git -C $checkout rev-parse HEAD
$id = & "$queue\submit.ps1" -Worktree $checkout -Commit $sha -Check render -Author R2

# Save the ID and continue working. Later, from any checkout:
& "$queue\result.ps1" -Id $id
& "$queue\result.ps1" -Id $id -FullLog
```

Submit prints the numeric Pueue ID on stdout. A linked worktree under the queue
root pins the commit; the daemon checks it out within the selected slot. The
author can edit, switch or commit in their own checkout immediately. Keep the
common Git repository and the queue's worktree registration available until the
job completes. Staged, dirty, untracked or ignored author files never travel with
the commit. Commit any required source or fixture first, and use approved
machine-level inputs for separately installed tools and external assets.

The deployed [allowlist](../scripts/queue/checks.json) defines commands, groups,
timeouts and artifacts. The input accepts a check name and an optional group.
Only a light check can go to light, and a light check can be promoted to gpu.
Extend the allowlist through review when a stage needs a different command or
fixture. Use the wrapper for submissions and retries. Direct Pueue task creation,
editing, immediate start and priority changes bypass these rules. This is a
same-account queue for trusted contributors and committed code, with the same
Windows permissions as its daemon.

| Check | Group | Evidence |
| --- | --- | --- |
| roadmap | light | Installed roadmap validator reads both languages and templates from the snapshot. |
| format | gpu | Workspace format check. |
| quick, contracts, clippy, native, package | gpu | Named verification profile and its own scope, logs and reports. |
| render | gpu | Offscreen profile with required capabilities, including retained Host and Vulkan checks. |
| r5-materials | gpu | Offscreen run of the original Khronos helmets, BC7/BC5, and the saved Editor material in the exported Player. |
| window-graph | gpu | Release Player graph probe in a native window with the selected adapter and presentation mode. |
| bench-once | gpu | S1–S5 diagnostic pass with benchmark artifacts. Three-run repeatability and acceptance require their own benchmark evidence. |
| queue-probe | gpu | Short diagnostic for ordering and Windows session evidence. It launches no renderer. |

Results are kept under **E:\ModuleEngine-queue\results\<id>**. Record the ID,
full commit, source digest, status, relevant conditions and report path in
your handoff. **report.json** records the received and observed SHA, before
and after source digests, timestamps, commands, status, exit and environment.
**log.txt** contains complete check stdout and stderr as captured. The
wrapper waits for this ID as needed, prints the report and tails 50 lines by
default. **artifacts/** contains the outputs declared by the route. A failed,
timed out or wrong-commit result returns a nonzero exit code. A task that the
daemon or OS interrupted may have only a request, a partial log and Pueue's
terminal status. Retain that evidence and resubmit for a fresh ID.

The byte inventory covers tracked files and non-ignored untracked sources as
described in [Independent Change Review](review.md#привязка-к-исходникам).
The queue checks its own checkout before and after the command. Ignored build
outputs and installed tools have separate provenance. The configured timeout
covers the commands after materialization and capability preparation; the
Windows job includes descendants, waits for the job to empty and terminates
the whole job on timeout. When the check has exited and the only remaining
members are verified MSVC telemetry uploaders (VCTIP), the runner closes those
processes and records their IDs in the task log.

Window routes require the logged-in user's unlocked interactive desktop.
Keep it available until the job finishes. Adapter names are taken from the
check's own artifacts where available. The OS video inventory gives the
installed driver versions; inventory alone cannot identify the adapter that a
check selected. The report distinguishes matched physical PCI adapters,
software renderers such as WARP or llvmpipe, and unverified matches. Review the
route's artifacts and conditions before making any physical-GPU, timing,
scanout or Linux hardware claim.

## Configure and operate the stand

The owner sets up the dedicated queue once on the verification PC. Install
[Pueue/Pueued 4.0.4](https://github.com/Nukesor/pueue/releases/tag/v4.0.4) for the
current account, verify the release SHA-256 values and make both executables
available in the queue's bin directory or on PATH. Install PowerShell 7.5+,
Git, Python 3.9.17+ and the [pinned Windows Rust and native tools](verification.md#запуск-и-окружение).
The tasks use the machine and user PATH and the current user's offline Cargo
cache. For the graphics routes the owner can place the pinned Windows DXC
and companion DLLs in the queue's bin directory. Verify their hashes against
the project's toolchain manifest. Benchmark fixtures follow the [S1–S5
contract](bench.md).

For `r5-materials`, put the original [Khronos inputs](content.md#r51-material-check)
under `fixtures/khronos/e124e8d8d7330a5372bc2b4d00d39a4a9d7997ea` inside
the queue root. Keep their relative `Models/...` paths. The Editor checks
every listed SHA-256 before import. The route builds and seals a Full
source delivery in its artifacts directory, keeps the exact Editor
and exported Players, and records the selected Vulkan adapter. Retain
the Full delivery at its recorded path for the authored project's binding.
Its generated `.work/player-native-target` is a build directory and can be
removed after acceptance and verification of the retained artifacts.

From a verified commit, publish an independent copy of the client and runner.
Keep the release directory path and its files fixed while jobs refer to it.

```powershell
$root = 'E:\ModuleEngine-queue'
$release = git rev-parse HEAD
$bundle = Join-Path $root "$release.zip"
$install = Join-Path $root "releases\$release"
git archive --format=zip --output=$bundle $release scripts/queue scripts/review_snapshot.py site/roadmap_map.py site/roadmap_events.py site/roadmap_moves.js
Expand-Archive -LiteralPath $bundle -DestinationPath $install
& "$install\scripts\queue\setup.ps1" -Python 'C:\path\to\python.exe' -RegisterAutostart
```

Setup uses a dedicated loopback configuration, state and logs under the queue
root on TCP port 16924. It applies gpu=1 and light=2. If the port is occupied,
choose a free port with **-Port** on initial setup. **-RegisterAutostart**
registers a per-user logon task with the interactive token. Start it via Task
Scheduler to verify registration without a Windows restart. To operate without
scheduled autostart, run **pueued --config E:\ModuleEngine-queue\pueue.yml** in
a dedicated PowerShell in the interactive account and rerun setup. An existing
unrelated Pueue installation keeps its own configuration.

```powershell
$pueue = 'E:\ModuleEngine-queue\bin\pueue.exe'
$config = 'E:\ModuleEngine-queue\pueue.yml'
& $pueue --config $config status --group gpu
& $pueue --config $config pause --group gpu --wait
& $pueue --config $config start --group gpu
```

In 4.0.4, **pause --group gpu --wait** pauses scheduling and lets the current
job finish. The command without **--wait** suspends the running process.
For a planned reboot or daemon maintenance, drain the running jobs and pause
the affected groups. Queued entries are stored in the dedicated state
directory. After sign-in, verify the expected IDs and group states before
resuming. Daemon restart and Windows reboot are separate checks. Submit a new
request for an interrupted job so the old number and evidence remain available.
Avoid **pueue restart**, **reset** and **clean** on this queue; the wrapper
binds each ID to its original request.

Keep result directories and the exact executables or packages referenced by
accepted evidence. Build intermediates live in the queue-owned snapshot,
including its Cargo target directory. After acceptance, save and hash required
artifacts outside the snapshot, ensure its processes have finished, verify the
exact registered path against the report and remove only that snapshot through
**git worktree remove** from its owning repository. An unfinished job still
needs its registered checkout. Preserve failure material needed for diagnosis.
