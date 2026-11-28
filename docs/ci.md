<a id="ci-moduleengine"></a>

# ModuleEngine CI

[English](ci.md) | [Русский](ru/ci.md) | [Documentation](README.md)

Ordinary PRs run the workspace tests, the minimal Player configuration, the
default Release Editor's CLI test, and separate `contracts` and `clippy`
checks on Windows and Linux.
Executable-source changes add native and package checks.
Documentation, `site/` and narrowly scoped CI policy changes can skip the build matrix. `CI required` succeeds after planning and
every job required by the selected route. Performance, desktop dialogs, hardware
GPU and full Full/Team/Player delivery are outside this CI scope.

<a id="что-выполняется"></a>

## What Runs

The [workflow](../.github/workflows/ci.yml) runs for all PRs, pushes to `main`, the merge queue,
and manual invocation; there are no workflow-level path filters. [ci_plan.py](../scripts/ci_plan.py)
reads the Git diff, including deleted files and both sides of renames, and routes it by
[.github/ci-routes.json](../.github/ci-routes.json): one folder is one route, the first matching rule wins,
and a path with no rule runs the full matrix. Routes:

| Route | Paths | Runs |
| --- | --- | --- |
| `docs` | `docs/**/*.md`, root `AGENTS.md` and README, README in a crate or example | Documentation and site checks only |
| `site` | `site/**` (roadmap and site generator, its templates and tests) | Documentation and site checks only |
| `policy` | In-place edits to `scripts/ci_plan.py` or `scripts/test_ci.py` with verified package pins and optional `docs` or `site` changes | Python CI policy tests in planning, plus documentation and site checks |
| `full` | everything else: `crates/`, `assets/`, `examples/`, `scripts/`, `tools/`, `.github/`, root manifests, non-Markdown files in `docs/` | The full matrix below, plus the documentation job |

These light routes accept a companion `.module-engine-package.json` update only for SHA-256 pins
in `sdk_files` or Full's `backend_files`. Every updated pin must refer to a regular file changed in
the same light diff and match the new Git blob byte for byte. Inventory keys and all other fields
must stay the same. The diff must include both the old and new file blobs. An updated pin may
correct a stale base pin. Manifest-only, unrelated, malformed and mixed changes run the full matrix.

A PR, a merge group and a push to `main` use the same rule on the diff against the event's base, so a
merge group or a push is the union of its changes: one code file anywhere selects the full matrix. An empty
diff, a non-regular or executable file, an initial push and a manual run also select it. Failure to
determine the diff fails planning. The planning step prints the chosen route and the files that selected
it in the run summary. `scripts/test_ci.py` requires every tracked file to match a rule, so a new folder
must be added to the routes file; do not commit `outputs/` or `work/`, they are ignored.

`Documentation and site checks` is a few-minute Linux job that runs on every route. It runs the
generator and Node model tests from `site/`, checks [roadmap EN/RU consistency](roadmap-map.md),
and builds both HTML pages. The
standalone Roadmap map workflow still checks Chromium and publishes Pages.

Planning also runs [check_doc_links.py](../scripts/check_doc_links.py) on
both routes. It compares documentation link destinations with the event base
and rejects new links into the checkout's `outputs/` or `work/` directories
or to absolute local paths. Existing historical references retain their
occurrence count per document. Keep compact acceptance evidence in tracked
files such as the [R2.4 receipt](evidence/r24/README.md).

An updated PR cancels its obsolete active run and replaces any pending run in
the same PR group. Each push to `main`, merge-queue event, and manual run has
its own group keyed by run ID. A second merge can start before the first
finishes and cannot cancel or replace its run.

The `full` route runs both platforms with native and package checks.

| Runner | Ordinary matrix | When sources change |
| --- | --- | --- |
| `windows-2025` | Workspace, minimal Player, Python unit checks, [license policy](licensing.md#third-party-code), `contracts`, `clippy` | `native-tools` and `package` |
| `ubuntu-24.04` | Workspace on Mesa lavapipe with pinned DXC, exact no-DXC check, minimal Player, Python unit checks, [license policy](licensing.md#third-party-code), `contracts`, `clippy` | `native` on Mesa lavapipe and `package` |

Every Rust job fetches locked dependencies. The workspace job runs the regular tests
with pinned cargo-nextest and the doc-tests with Cargo. It combines Nextest JUnit
results with doc-test counts and records capability skip reasons for each package.
Any ordinary test failure or a missing Custom SDK cache fails the job.
On Linux, the migration and Vulkan allocation-recovery tests and the Editor
tests for initial Scene preparation and Rust recovery and Apply with Game
hidden must pass without a skip in the workspace run. The Scene test uses a
native renderer without compiling gameplay code. The Rust test gives each of
the three compiler requests its own deadline. The preceding exact Vulkan check
runs with Mesa and no DXC and requires one test and its explicit DXC skip reason.

The minimal Player is built and tested in separate Cargo invocations with
`-p module-engine-player --no-default-features`. Its normal and build dependency
graph must exclude game packages and Vulkan. The Player's test-only Two Arenas
fixture is outside that graph.

The ordinary matrix plans and runs `clippy` separately on both platforms. On the pinned
native target it runs `--workspace --all-targets --all-features` and then
`--release -p module-engine-editor --bins` with the default features. Both
offline, locked runs pass `-D warnings`.
The workspace tests enable `verification` and run a real `--self-check`.
A separate `cargo test --offline --locked --release -p module-engine-editor --test verification_cli`
checks the feature requirement in the default build.
The launcher resolves `{native_host}` from the actual verbose Rust toolchain output
and rejects a release/target mismatch with the platform manifest before Cargo metadata.
Clippy does not run formatting, tests, or runtime scenarios; `contracts` still checks
formatting exactly once. A workflow change requires new hosted results for its own SHA.

`native-tools` comprises three existing checks of native linker/cache/executable
spellings without GPU or UI. This is part of `native`, not a replacement for its two GameUI tests.
Linux full `native` runs offscreen tests with software Vulkan, without a desktop
session. Windows GPU native acceptance remains a targeted local route.
Successful CI does not confirm hardware Linux, strict 60 FPS, cold-start, or
interactive desktop acceptance. `package` uses minimal Cargo fixtures,
not a real game export. Linux CI passes `--require-capabilities` so missing Vulkan or DXC fails the required offscreen check.

Each platform runs separate workspace, default Release Editor build and CLI test,
Clippy, contracts and minimal Player jobs. The plan can also select native and package
jobs independently. Each job gets its own hosted VM and TEMP. GPU and native tests
within a job run serially. Windows places Cargo products on a Dev Drive.
The release job also builds the Editor with `--no-default-features` and rejects game crates in its normal or build dependency tree.
Top-level workspace builds use rust-lld on Windows and lld on Linux. Release Editor,
Clippy and minimal Player invocations set the fast linker and `CARGO_PROFILE_DEV_DEBUG=0`.
The native and Custom SDK proofs retain their audited compiler environment.
Rust version/native target come from the existing Windows/Linux manifests.
Before the offline launcher, CI explicitly installs pinned Rust with the `rustfmt` and `clippy` components and runs
`cargo fetch --locked`. The Linux matrix installs system link libraries and Mesa,
then restores DXC using the archive and file hashes in `scripts/linux-toolchain.json`.
The test environment uses the pinned DXC library directory as its sole
`LD_LIBRARY_PATH` entry so the Custom SDK reuse checks can verify its contents.
An installation, fetch, compilation, or test failure fails CI, without a fallback version.

The launcher uses a short checkout-local `.work/v/<run-token>` and records
its resolved path in the report. The first Windows hosted run found LNK1181 in a nested
snapshot fixture with a long TEMP; the test and its criteria were preserved. Cargo hides the exact name
of the problematic object, so path length is a testable hypothesis,
and confirming the fix requires a new successful `package` on hosted Windows.

Reports and command logs from `outputs/verification` are published as an artifact for
each platform even after failure; if verification did not start, the GitHub job log
contains the preparation error. Retention is 14 days. Full source/artifact hashes and
scope remain in the launcher's normal `result.json`. The jobs share a dependency cache for each OS. Reports are named by suite,
OS and attempt.
[Measurement and cache boundaries](build-verification-performance.md) define compatible
keys and how to assess total hosted time, including restore/save costs.

<a id="включение-обязательности"></a>

## Making CI Required

The workflow file itself does not change GitHub branch protection. After the
workflow appears in the target GitHub repository and the first successful PR/push run:

1. Enable a PR requirement before merging in the ruleset/protected branch.
2. Add the required status check **`CI required`**, source GitHub Actions.
3. Check a test failure in a separate PR: a failed prerequisite must block
   `CI required`. A new PR version must pass again; the old SHA is insufficient.

The aggregate with `if: always()` rejects any required job result other than
`success` and rejects missing route outputs. It expects the unselected job to be
skipped. The ordinary matrix requires success on both OSes, including its selected
native/package steps. If branch protection/ruleset has not yet been
applied, CI provides a signal but does not technically prohibit merging. Enabling the rule
requires a known repository and appropriate permissions; local unit checks
do not prove execution of GitHub Actions.

The workflow token has only `contents: read`; checkout does not persist credentials.
There are no secrets, `pull_request_target`, privileged PR execution, or
unprepared self-hosted jobs. All external actions are pinned to full commit SHAs.
Fresh CI tools are downloaded only inside an explicitly started hosted job.

For [ModuleEngine/ModuleEngine](https://github.com/ModuleEngine/ModuleEngine), public
access is enabled by the owner's decision. `main` has required PRs, an up-to-date branch,
**`CI required` with GitHub Actions app ID 15368**, administrator protection, and resolution
of review discussions; force pushes and branch deletion are prohibited. Required human approvals = 0:
separate independent AI review is not presented as approval from another GitHub account.
In [service PR #2](https://github.com/ModuleEngine/ModuleEngine/pull/2), an invalid base SHA
caused planning failure, a skipped matrix, and failed aggregate; GitHub showed
`mergeable_state=blocked`. The PR was closed without merging. This proves prerequisite
failure, not a separate artificial Rust-test failure. Actual results
for each version must be checked in its [CI run](https://github.com/ModuleEngine/ModuleEngine/actions).

<a id="локальная-проверка-политики"></a>

## Local Policy Verification

```sh
python -B -m unittest discover -s scripts -p 'test_*.py' -v
python scripts/ci_plan.py --base <full-base-SHA>
python scripts/verify.py --profile clippy --plan
python scripts/verify.py --profile clippy
python scripts/verify.py --profile native-tools --plan
```

Policy fixtures check path selection, safe Git argv, and aggregate rejection on
error/skip/cancel. This is not hosted CI acceptance. A changed workflow must be checked
in GitHub, including fresh dependency restoration on both systems.

References: [required checks and skipped jobs](https://docs.github.com/en/pull-requests/how-tos/merge-and-close-pull-requests/troubleshooting-required-status-checks),
[minimal permissions and SHA pins](https://docs.github.com/en/actions/reference/security/secure-use),
[hosted runner images](https://github.com/actions/runner-images).
