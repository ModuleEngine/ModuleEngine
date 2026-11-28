#!/usr/bin/env bash
set -eo pipefail

artifacts=$1
toolchain=$2
alsa_sysroot=$3
zenity_sysroot=$4
alsa_libs="$alsa_sysroot/usr/lib/x86_64-linux-gnu"
zenity_libs="$zenity_sysroot/usr/lib/x86_64-linux-gnu"
source "$toolchain"
set -u

export CARGO_TARGET_DIR="$PWD/target/wslg-dialogs"
export PKG_CONFIG_LIBDIR="$alsa_libs/pkgconfig" PKG_CONFIG_SYSROOT_DIR="$alsa_sysroot"
export LD_LIBRARY_PATH="$alsa_libs:$alsa_libs/pulseaudio:$zenity_libs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PATH="$zenity_sysroot/usr/bin:$PATH"
export XDG_RUNTIME_DIR=/mnt/wslg/runtime-dir WAYLAND_DISPLAY=wayland-0 DISPLAY=:0 GTK_A11Y=none
export XDG_DATA_DIRS="$zenity_sysroot/usr/share:/usr/local/share:/usr/share"

printf 'Linux editor native picker on the WSLg desktop\n'
uname -sr
rustc -vV
zenity --version

build_log="$artifacts/cargo.jsonl"
cargo test --offline --locked --target x86_64-unknown-linux-gnu \
    -p module-engine-editor --no-default-features --features verification --bin module-engine-editor \
    --no-run --message-format=json > "$build_log"
binary=$(python3 - "$build_log" <<'PY'
import json
import sys

with open(sys.argv[1], encoding='utf-8') as stream:
    messages = [json.loads(line) for line in stream]
executables = [
    msg['executable'] for msg in messages
    if msg.get('reason') == 'compiler-artifact'
    and msg['target']['name'] == 'module-engine-editor'
    and msg['profile']['test'] and msg.get('executable')
]
assert len(executables) == 1, executables
print(executables[0])
PY
)
probe="$artifacts/native-dialogs-probe"
cp -- "$binary" "$probe"
(cd "$artifacts" && sha256sum native-dialogs-probe > probe.sha256)
cat "$artifacts/probe.sha256"
mkdir -- "$artifacts/fixture-tmp"
export TMPDIR="$artifacts/fixture-tmp"
setpriv --reuid 1000 --regid 1000 --clear-groups \
    timeout --kill-after=5s 240s "$probe" native_dialogs::tests::native_dialogs_choose_and_cancel \
    --exact --ignored --nocapture --test-threads=1
