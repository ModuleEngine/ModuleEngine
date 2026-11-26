#!/bin/sh
# Full native Linux suite, including offscreen Vulkan/GameUI tests; no package
# manager, tool installation or native desktop-window acceptance in this script.
set -eu
if [ "$#" -gt 0 ]; then
    verification_script_root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
    verification_python=${MODULE_ENGINE_VERIFY_PYTHON:-python3}
    exec "$verification_python" "$verification_script_root/scripts/verify.py" "$@"
fi
cd "$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
if [ "$(uname -s)" != Linux ]; then echo 'Use verify.ps1 on Windows.' >&2; exit 2; fi
verification_python=${MODULE_ENGINE_VERIFY_PYTHON:-python3}
"$verification_python" -B scripts/python_runtime.py >/dev/null
"$verification_python" scripts/linux_prerequisites.py --require build
host=$(rustc -vV | sed -n 's/^host: //p')
case "$host" in *linux*) ;; *) echo 'A native Linux Rust toolchain is required.' >&2; exit 2 ;; esac
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/.work/linux-target/$host}"
export CARGO_INCREMENTAL=0
export MODULE_ENGINE_VERIFY_SERIAL=1
mkdir -p .work/test-temp
validation_log=$(mktemp .work/test-temp/legacy-vulkan.XXXXXX)
trap 'rm -f -- "$validation_log"' 0
verify_tests() {
    test_status=0
    cargo test "$@" 2>"$validation_log" || test_status=$?
    cat "$validation_log" >&2
    if grep -q '^Vulkan validation ERROR:' "$validation_log"; then
        echo 'Vulkan validation reported errors during legacy tests' >&2
        return 1
    fi
    return "$test_status"
}
cargo fmt --all -- --check
cargo check --offline --locked --release --target "$host" --workspace --features module-engine-editor/verification --all-targets
verify_tests --offline --locked --release --target "$host" --workspace --features module-engine-editor/verification --lib --bins -- --show-output --test-threads=1
verify_tests --offline --locked --release --target "$host" -p module-engine-editor --features verification game_ui::tests::interactive_game_ui_keeps_drawing_and_applying_rust_during_slow_ticks -- --ignored --exact --show-output --test-threads=1
cargo clippy --offline --locked --release --target "$host" --workspace --features module-engine-editor/verification --all-targets -- -D warnings
cargo test --offline --locked --release --target "$host" --workspace --features module-engine-editor/verification --doc
cargo build --offline --locked --release --target "$host" -p module-engine-editor --features verification
"$CARGO_TARGET_DIR/$host/release/module-engine-editor" --self-check outputs/linux-verification/cpu
echo 'Build, workspace tests with reported capability skips, and CPU self-check passed. Native desktop-window/export checks remain separate; see docs/linux-support.md.'
