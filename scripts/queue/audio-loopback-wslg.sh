#!/usr/bin/env bash
set -eo pipefail

artifacts=$1
toolchain=$2
sysroot=$3
test_name=$4
libs="$sysroot/usr/lib/x86_64-linux-gnu"
source "$toolchain"
set -u

export CARGO_TARGET_DIR="$PWD/target/wslg-audio"
export PKG_CONFIG_LIBDIR="$libs/pkgconfig" PKG_CONFIG_SYSROOT_DIR="$sysroot"
export LD_LIBRARY_PATH="$libs:$libs/pulseaudio${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export ALSA_CONFIG_PATH="$artifacts/alsa.conf" ALSA_PLUGIN_DIR="$libs/alsa-lib"
export PULSE_SERVER=unix:/mnt/wslg/runtime-dir/pulse/native
export MODULE_ENGINE_AUDIO_LOOPBACK_DEVICE=alsa:r6_monitor
export MODULE_ENGINE_AUDIO_PROBE_RECORDING="$artifacts/capture.f32le"

cat > "$ALSA_CONFIG_PATH" <<'ALSA'
pcm.!default {
    type pulse
    server "unix:/mnt/wslg/runtime-dir/pulse/native"
    device "RDPSink"
    hint { show on description "WSLg RDP output" }
}
pcm.r6_monitor {
    type pulse
    server "unix:/mnt/wslg/runtime-dir/pulse/native"
    device "RDPSink.monitor"
    hint { show on description "WSLg RDP output monitor" }
}
ALSA

printf 'WSLg virtual endpoint\n'
uname -sr
rustc -vV
pkg-config --print-errors --modversion alsa
as_desktop=(setpriv --reuid 1000 --regid 1000 --clear-groups)
"${as_desktop[@]}" "$sysroot/usr/bin/pactl" list short sinks
"${as_desktop[@]}" "$sysroot/usr/bin/pactl" list short sources

build_log="$artifacts/cargo.jsonl"
cargo test --offline --locked --target x86_64-unknown-linux-gnu \
    -p module-engine-host-services --features audio --lib --no-run \
    --message-format=json > "$build_log"
binary=$(python3 - "$build_log" <<'PY'
import json
import sys

with open(sys.argv[1], encoding='utf-8') as stream:
    messages = [json.loads(line) for line in stream]
executables = [
    msg['executable'] for msg in messages
    if msg.get('reason') == 'compiler-artifact'
    and msg['target']['name'] == 'module_engine_host_services'
    and msg['profile']['test'] and msg.get('executable')
]
assert len(executables) == 1, executables
print(executables[0])
PY
)
probe="$artifacts/audio-loopback-probe"
cp -- "$binary" "$probe"
(cd "$artifacts" && sha256sum audio-loopback-probe alsa.conf > probe.sha256)
cat "$artifacts/probe.sha256"
status=0
"${as_desktop[@]}" "$probe" "$test_name" --exact --ignored --nocapture --test-threads=1 || status=$?
if [[ -f "$MODULE_ENGINE_AUDIO_PROBE_RECORDING" ]]; then
    (cd "$artifacts" && sha256sum capture.f32le > capture.sha256)
fi
exit "$status"
