#!/usr/bin/env python3
"""Run three consecutive native ModuleEngine benchmark passes."""
import argparse
import ctypes
from datetime import datetime
import json
import math
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys
import time
import urllib.request

from review_snapshot import capture, entry


ROOT = Path(__file__).resolve().parents[1]
SCENES = ("S1", "S2", "S3", "S4", "S5")
GATE_METRICS = {
    "S1": ("frame_interval_ms",),
    "S2": ("frame_interval_ms", "game_step_ms"),
    "S3": ("frame_interval_ms", "cpu_submit_ms", "ecs_tick_ms"),
    "S4": ("frame_interval_ms", "cpu_submit_ms"),
    "S5": ("frame_interval_ms", "cpu_submit_ms"),
}
LIMIT_PERCENT = 5.0
NVIDIA_QUERY = (
    "timestamp,index,name,driver_version,pstate,temperature.gpu,"
    "utilization.gpu,memory.total,memory.used,clocks.sm,power.draw"
)


def write_json(path, value):
    with path.open("x", encoding="utf-8", newline="\n") as output:
        json.dump(value, output, ensure_ascii=False, indent=2, allow_nan=False)
        output.write("\n")


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8")) if path.is_file() else {}


def command_output(*args):
    try:
        return subprocess.check_output(args, cwd=ROOT, text=True, stderr=subprocess.STDOUT,
                                       timeout=15).strip()
    except (OSError, subprocess.SubprocessError):
        return None


def stats(values):
    values = sorted(values)
    if not values:
        return None
    return {"samples": len(values), "mean_ms": statistics.mean(values),
            "p95_ms": values[math.ceil(len(values) * 0.95) - 1], "max_ms": values[-1]}


if os.name == "nt":
    from ctypes import wintypes

    class MemoryCounters(ctypes.Structure):
        _fields_ = [("cb", wintypes.DWORD), ("faults", wintypes.DWORD)] + [
            (name, ctypes.c_size_t)
            for name in ("peak_rss", "rss", "peak_paged_pool", "paged_pool",
                         "peak_nonpaged_pool", "nonpaged_pool", "pagefile", "peak_pagefile")
        ]

    class MemoryStatus(ctypes.Structure):
        _fields_ = [("length", wintypes.DWORD), ("load", wintypes.DWORD)] + [
            (name, ctypes.c_uint64)
            for name in ("physical", "available_physical", "commit", "available_commit",
                         "virtual", "available_virtual", "available_extended_virtual")
        ]

    KERNEL32 = ctypes.WinDLL("kernel32", use_last_error=True)
    OPEN_PROCESS = KERNEL32.OpenProcess
    OPEN_PROCESS.argtypes = (wintypes.DWORD, wintypes.BOOL, wintypes.DWORD)
    OPEN_PROCESS.restype = wintypes.HANDLE
    CLOSE_HANDLE = KERNEL32.CloseHandle
    CLOSE_HANDLE.argtypes = (wintypes.HANDLE,)
    GET_PROCESS_MEMORY = KERNEL32.K32GetProcessMemoryInfo
    GET_PROCESS_MEMORY.argtypes = (
        wintypes.HANDLE, ctypes.POINTER(MemoryCounters), wintypes.DWORD,
    )
    GET_PROCESS_MEMORY.restype = wintypes.BOOL
    GLOBAL_MEMORY_STATUS = KERNEL32.GlobalMemoryStatusEx
    GLOBAL_MEMORY_STATUS.argtypes = (ctypes.POINTER(MemoryStatus),)
    GLOBAL_MEMORY_STATUS.restype = wintypes.BOOL


def available_memory():
    if os.name == "nt":
        status = MemoryStatus()
        status.length = ctypes.sizeof(status)
        if GLOBAL_MEMORY_STATUS(ctypes.byref(status)):
            return min(status.available_physical, status.available_commit)
        return None
    try:
        for line in Path("/proc/meminfo").read_text().splitlines():
            if line.startswith("MemAvailable:"):
                return int(line.split()[1]) * 1024
    except OSError:
        pass
    return None


def peak_working_set(pid):
    if os.name == "nt":
        process = OPEN_PROCESS(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
        if process:
            try:
                counters = MemoryCounters()
                counters.cb = ctypes.sizeof(counters)
                if GET_PROCESS_MEMORY(process, ctypes.byref(counters), counters.cb):
                    return counters.peak_rss
            finally:
                CLOSE_HANDLE(process)
        return None
    try:
        with Path(f"/proc/{pid}/status").open() as status:
            for line in status:
                if line.startswith("VmHWM:"):
                    return int(line.split()[1]) * 1024
    except OSError:
        pass
    return None


def execute_scene(exe, scene, group, env):
    folder = group / scene
    args = [str(exe), "--bench-scene", scene, str(folder)]
    stdout = group / f"{scene}.log"
    print(f"{group.name}/{scene}", flush=True)
    started = time.monotonic()
    peak = None
    timed_out = False
    memory_available = available_memory()
    scene_env = env.copy()
    scene_env["MODULE_ENGINE_BENCH_AVAILABLE_RAM_BYTES"] = str(memory_available or "")
    gpu_started_at = datetime.now().astimezone().isoformat()
    with stdout.open("x", encoding="utf-8") as log:
        with subprocess.Popen(args, cwd=ROOT, env=scene_env, stdout=log, stderr=subprocess.STDOUT) as proc:
            try:
                while proc.poll() is None:
                    rss = peak_working_set(proc.pid)
                    if rss is not None:
                        peak = max(peak or rss, rss)
                    if time.monotonic() - started > 360:
                        timed_out = True
                        break
                    time.sleep(0.25)
            finally:
                if proc.poll() is None:
                    proc.kill()
                    proc.wait()
    raw_name = "native-frame-report.json" if scene in ("S1", "S2") else "scene.json"
    raw_path = folder / raw_name
    raw = read_json(raw_path)
    wall_duration_seconds = time.monotonic() - started
    status, metrics, constraints, signature = summarize_scene(scene, raw)
    if status == "complete" and signature["vsync_requested"] is not (
        scene_env["MODULE_ENGINE_GUI_VSYNC"] == "on"
    ):
        status = "incomplete"
        constraints.append("Reported GUI vsync request does not match the series configuration")
    if proc.returncode or timed_out:
        status = "failed"
    if peak is None:
        constraints.append("Peak working set unavailable")
    budgets_met = None
    if scene in ("S1", "S2") and status == "complete":
        timing = metrics["frame_interval_ms"]
        budgets_met = timing["mean_ms"] <= 1000 / 60 and timing["p95_ms"] <= 1000 / 60
        if scene == "S1":
            budgets_met = budgets_met and peak is not None and peak <= 1024 ** 3
    return {
        "scene": scene, "status": status, "command": args, "exit_code": proc.returncode,
        "timed_out": timed_out, "duration_seconds": wall_duration_seconds,
        "started_at": gpu_started_at, "memory_available_before_scene_bytes": memory_available,
        "error": raw.get("error") or raw.get("last_error") or raw.get("interrupted"),
        "log": stdout.name, "raw_report": f"{scene}/{raw_name}" if raw else None,
        "fixture": raw.get("fixture") or read_json(folder / "fixture.json"),
        "signature": signature,
        "metrics": metrics, "constraints": constraints, "budgets_met": budgets_met,
        "peak_working_set_bytes": peak, "peak_scope": "OS high-water mark observed while process was alive",
    }


def summarize_scene(scene, raw):
    if scene in ("S1", "S2"):
        statuses = raw.get("game_status_samples", [])
        player_valid = (
            raw.get("complete") is True and raw.get("timing_data_valid") is True
            and raw.get("physical_resolution_met") is True and raw.get("error_frames") == 0
            and raw.get("user_input_frames") == 0 and bool(statuses)
            and set(statuses) == {"Playing"}
            and all(value is not False for value in raw.get("window_visible_samples", []))
            and (scene != "S2" or raw.get("shipping_twenty_enemy_load_met") is True)
        )
        return (
            "complete" if player_valid else "incomplete",
            {"frame_interval_ms": raw.get("ui_observed_unique_frame_interval"),
             "game_step_ms": raw.get("game_step") if scene == "S2" else None,
             "render_ms": raw.get("renderer_content_render_readback")},
            [] if player_valid else ["Native Player workload or timing evidence is incomplete"],
            {"vsync_requested": raw.get("native_vsync_requested"),
             **{key: raw.get(key) for key in ("gui_graphics", "vulkan_device", "monitor_pixels",
                                             "monitor_refresh_millihertz", "offscreen_render_pixels",
                                             "measured_requested", "warmup_requested")}},
        )
    samples = raw.get("samples", [])
    metrics = dict(raw.get("metrics", {}))
    for key in ("frame_interval_ms", "cpu_submit_ms", "ecs_tick_ms",
                "transform_extract_ms", "observed_producer_latency_ms"):
        metrics[key] = stats(row[key] for row in samples if row[key] is not None)
    work = [sample["work"] for sample in samples]
    for key in ("draw_calls", "vertex_write_bytes", "atlas_upload_bytes"):
        metrics[key] = {
            "min": min(row[key] for row in work), "max": max(row[key] for row in work),
            "mean": statistics.mean(row[key] for row in work),
        } if work else None
    limits = raw.get("legacy_vertex_allocation", {})
    signature = {
        **{key: raw.get(key) for key in ("gui_graphics", "monitor", "render_pixels", "camera",
                                        "loaded_triangles", "loaded_draw_items", "warmup_requested",
                                        "measured_requested")},
        "adapter": limits.get("adapter"), "driver_version_raw": limits.get("driver_version_raw"),
        "vsync_requested": raw.get("gui_vsync"),
    }
    constraints = ["Legacy Forward cannot consume the fixture's 256 point lights"] if scene == "S4" else []
    status = raw.get("status", "incomplete")
    if status == "complete" and (len(samples) != raw.get("measured_requested")
                                 or raw.get("warmup_completed") != raw.get("warmup_requested")):
        status = "incomplete"
    return status, metrics, constraints, signature


def compare(runs):
    result = {}
    for scene in SCENES:
        cases = [run["results"][scene] for run in runs]
        comparable = (
            all(case["status"] == "complete" for case in cases)
            and all(case["signature"] == cases[0]["signature"] for case in cases)
            and all(case["fixture"] == cases[0]["fixture"] for case in cases)
        )
        comparisons = {}
        for key in GATE_METRICS[scene]:
            means = [(case["metrics"].get(key) or {}).get("mean_ms") for case in cases]
            spread = None
            if comparable and all(value is not None and value > 0 for value in means):
                spread = 100 * (max(means) - min(means)) / statistics.mean(means)
            comparisons[key] = {"means_ms": means, "spread_percent": spread,
                                "within_5_percent": spread <= LIMIT_PERCENT if spread is not None else None}
            if key == "frame_interval_ms":
                comparisons[key]["mean_fps"] = [1000 / value if value else None for value in means]
        result[scene] = {"comparable": comparable, "timing": comparisons,
                         "full_workload_measured": comparable and scene != "S4"}
    return result


def ensure_bench_assets():
    """Download pinned third-party models that the repository does not redistribute."""
    manifest_path = ROOT / "examples/bench/sponza/manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    cache = ROOT / ".tools/bench-assets/sponza"
    cache.mkdir(parents=True, exist_ok=True)
    downloaded = 0
    for item in manifest["files"]:
        name = item["path"]
        if "/" in name or "\\" in name or name.startswith("."):
            raise ValueError(f"Unsafe benchmark asset name: {name}")
        target = cache / name
        if target.is_file() and entry(cache, name)["sha256"] == item["sha256"]:
            continue
        print(f"Downloading benchmark asset {name}", flush=True)
        partial = cache / (name + ".part")
        with urllib.request.urlopen(manifest["base_url"] + name, timeout=120) as response, \
                partial.open("wb") as output:
            shutil.copyfileobj(response, output)
        if entry(cache, partial.name)["sha256"] != item["sha256"]:
            partial.unlink()
            raise ValueError(f"Downloaded {name} does not match the pinned SHA-256")
        partial.replace(target)
        downloaded += 1
    return {"manifest": str(manifest_path.relative_to(ROOT)), "commit": manifest["commit"],
            "files": len(manifest["files"]), "downloaded_now": downloaded,
            "manifest_sha256": entry(manifest_path.parent, manifest_path.name)["sha256"]}


def dxc_inputs(env):
    configured = env.get("MODULE_ENGINE_DXC")
    candidates = [Path(configured)] if configured else (
        [ROOT / "tools/dxc/dxc.exe"] if os.name == "nt" else
        [ROOT / "tools/dxc/bin/dxc", ROOT / "tools/dxc/dxc"]
    )
    dxc = next((p.resolve() for p in candidates if p.is_file()), None)
    if dxc is None and (found := shutil.which(configured or "dxc")):
        dxc = Path(found).resolve()
    if dxc is None:
        return None
    paths = [dxc]
    for name in ("dxcompiler.dll", "dxil.dll", "libdxcompiler.so", "libdxil.so"):
        candidates = (dxc.parent / name, dxc.parent.parent / "lib" / name)
        paths.extend(path for path in candidates if path.is_file())
    resolved = {path.resolve() for path in paths}
    return {str(path): entry(path.parent, path.name)["sha256"] for path in sorted(resolved)}


def built_editor(messages, host):
    bins = set()
    source = ROOT / "crates/editor/src/main.rs"
    allowed_folders = {ROOT / "target/release", ROOT / "target" / host / "release"}
    with messages.open(encoding="utf-8") as events:
        for line in events:
            event = json.loads(line)
            if event.get("reason") != "compiler-artifact":
                continue
            target = event["target"]
            if (target["name"] == "module-engine-editor" and "bin" in target["kind"]
                    and "bench" in event["features"]
                    and Path(target["src_path"]).resolve() == source and event.get("executable")):
                bins.add(Path(event["executable"]).resolve())
    if len(bins) != 1:
        raise ValueError("Cargo did not report exactly one bench Editor executable")
    exe = bins.pop()
    if exe.parent not in allowed_folders:
        raise ValueError(f"Cargo reported a non-native benchmark executable for {host}: {exe}")
    return exe


def run_series(series, diagnostic, vsync):
    env = os.environ.copy()
    for variable in ("EFRAME_SCREENSHOT_TO", "MODULE_ENGINE_UI_SCREENSHOT_TO",
                     "MODULE_ENGINE_CAPTURE_ATTACK", "MODULE_ENGINE_RESOURCE_ROOT"):
        env.pop(variable, None)
    env.update(MODULE_ENGINE_BENCHMARK_ARM="automatic", MODULE_ENGINE_BENCHMARK_FRAMES="240",
               MODULE_ENGINE_RESOURCE_ROOT=str(ROOT),
               MODULE_ENGINE_GUI_VSYNC="on" if vsync else "off")
    bench_assets = ensure_bench_assets()
    source = capture(ROOT)
    write_json(series / "source.json", source)
    build = ["cargo", "build", "--locked", "--release", "--features", "bench",
             "--message-format=json-render-diagnostics",
             "-p", "module-engine-editor", "--target-dir", str(ROOT / "target")]
    rustc = command_output("rustc", "-vV")
    host = next((line.removeprefix("host: ") for line in (rustc or "").splitlines()
                 if line.startswith("host: ")), None)
    if not host:
        raise ValueError("Cannot determine the native host from rustc -vV")
    smi = shutil.which("nvidia-smi")
    telemetry = [smi, f"--query-gpu={NVIDIA_QUERY}", "--format=csv"] if smi else None
    manifest = {
        "version": 1, "local_started": datetime.now().astimezone().isoformat(),
        "head": source["head"], "source_digest": source["source_digest"],
        "python": sys.version, "rustc": rustc, "native_host": host,
        "cargo": command_output("cargo", "--version"), "platform": platform.platform(),
        "processor": platform.processor(), "logical_cpus": os.cpu_count(),
        "dxc_inputs": dxc_inputs(env),
        "gui_environment": {key: env.get(key) for key in ("MODULE_ENGINE_GUI_RENDERER",
                            "MODULE_ENGINE_WINDOW_SYSTEM", "VK_DRIVER_FILES", "VK_ICD_FILENAMES")},
        "build": build, "scenes_in_order": SCENES, "runs": 3, "diagnostic": diagnostic,
        "gui_vsync": vsync, "bench_assets": bench_assets,
        "warmup_and_measured_frames": {"S1": [30, 240], "S2": [30, 240], "S3": [30, 240],
                                       "S4": [30, 240], "S5": [5, 30]},
        "warmup_order": "One unscored pass S1–S5, then per-process frame warmup in each scored pass",
        "resolution": [1920, 1080], "means_compared": GATE_METRICS,
        "spread": "100 * (max(run means) - min(run means)) / arithmetic_mean(run means)",
        "limit_percent_inclusive": LIMIT_PERCENT,
        "idle_gpu_sample": command_output(*telemetry) if telemetry else None,
    }
    write_json(series / "manifest.json", manifest)
    print("Building Release once before measurements", flush=True)
    messages = series / "build.jsonl"
    with (series / "build.log").open("x", encoding="utf-8") as log, messages.open("xb") as events:
        build_result = subprocess.run(build, cwd=ROOT, env=env, stdout=events, stderr=log)
    if build_result.returncode:
        write_json(series / "comparison.json", {"accepted": False, "error": "Build failed"})
        return 1
    executable = built_editor(messages, host)
    # Run the exact same bytes even if another process replaces target/release.
    exe = series / executable.name
    shutil.copy2(executable, exe)
    exe_hash = entry(series, exe.name)["sha256"]
    write_json(series / "binary.json", {
        "sha256": exe_hash, "size_bytes": exe.stat().st_size, "cargo_artifact": str(executable),
    })
    runs = []
    with (series / "gpu.csv").open("x", encoding="utf-8") as gpu_log:
        sampler = subprocess.Popen([*telemetry, "-l", "1"], stdout=gpu_log, stderr=subprocess.STDOUT) if telemetry else None
        try:
            for number in range(4):
                group = series / (f"run-{number:02}" if number else "prewarm")
                group.mkdir()
                run = {"run": number, "started_at": datetime.now().astimezone().isoformat(),
                       "source_digest": source["source_digest"], "binary_sha256": exe_hash, "results": {}}
                for scene in SCENES:
                    result = execute_scene(exe, scene, group, env)
                    run["results"][scene] = result
                    write_json(group / f"{scene}.json", result)
                write_json(group / "run.json", run)
                if number:
                    runs.append(run)
        finally:
            if sampler is not None and sampler.poll() is None:
                sampler.terminate()
                sampler.wait()
    metrics = compare(runs)
    source_matches = capture(ROOT)["source_digest"] == source["source_digest"]
    binary_matches = entry(series, exe.name)["sha256"] == exe_hash
    available = [datum["within_5_percent"] for item in metrics.values()
                 for datum in item["timing"].values() if datum["within_5_percent"] is not None]
    timing = [datum["within_5_percent"] for item in metrics.values() for datum in item["timing"].values()]
    accepted = (not diagnostic and source_matches and binary_matches
                and all(value is True for value in timing))
    write_json(series / "comparison.json", {
        "accepted": accepted, "diagnostic": diagnostic, "source_matches": source_matches,
        "source_digest": source["source_digest"], "binary_matches": binary_matches,
        "binary_sha256": exe_hash, "available_means_within_5_percent": bool(available) and all(available),
        "all_requested_means_within_5_percent": all(value is True for value in timing),
        "scenes": metrics,
    })
    return 0 if accepted else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--diagnostic", action="store_true", help="Record a series with known competing load")
    parser.add_argument("--vsync", action="store_true",
                        help="Keep GUI vsync; frame intervals are then capped by the monitor refresh rate")
    args = parser.parse_args()
    now = datetime.now().astimezone()
    series = ROOT / "outputs" / "bench" / now.date().isoformat() / now.strftime("%H%M%S-%f")
    series.mkdir(parents=True)
    print(f"Reports: {series}", flush=True)
    try:
        return run_series(series, args.diagnostic, args.vsync)
    except (ValueError, OSError, subprocess.SubprocessError, KeyError) as error:
        write_json(series / "failure.json", {"error": str(error)})
        print(f"bench: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
