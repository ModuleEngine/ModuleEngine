#!/usr/bin/env python3
"""Run native ModuleEngine benchmarks."""
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
BUILD_COMMAND = ["cargo", "build", "--locked", "--release", "--features", "bench",
                 "--message-format=json-render-diagnostics",
                 "-p", "module-engine-editor"]
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


def nearest_rank(sorted_values, fraction):
    return sorted_values[math.ceil(len(sorted_values) * fraction) - 1]


def stats(values):
    values = sorted(values)
    if not values:
        return None
    return {"samples": len(values), "mean_ms": statistics.mean(values),
            "p95_ms": nearest_rank(values, 0.95), "p99_ms": nearest_rank(values, 0.99),
            "max_ms": values[-1]}


def with_tail(summary, samples):
    """Add a nearest-rank p99 to a Player summary when its raw samples are reported."""
    if not summary or not samples:
        return summary
    return {**summary, "p99_ms": nearest_rank(sorted(samples), 0.99)}


def summarize_frame_timings(observations):
    profiles = [row["timings"] for row in observations if row.get("timings") is not None]
    cpu = {}
    for key in ("cpu_tick_ms", "cpu_extract_ms", "cpu_record_ms", "cpu_submit_ms"):
        cpu[key] = stats(profile[key] for profile in profiles if profile.get(key) is not None)
    graph = []
    passes = {}
    unavailable = {}
    for profile in profiles:
        gpu = profile.get("gpu")
        if not gpu or gpu["status"] != "available":
            reason = gpu["reason"] if gpu else "No GPU timestamp data"
            unavailable[reason] = unavailable.get(reason, 0) + 1
            continue
        graph.append(gpu["graph_ms"])
        for render_pass in gpu["passes"]:
            key = (render_pass["id"], render_pass["bypassed"])
            passes.setdefault(key, []).append(render_pass["ms"])
    return {
        "profiled_frames": len(profiles), "cpu": cpu, "gpu_graph_ms": stats(graph),
        "gpu_pass_ms": [
            {"id": pass_id, "bypassed": bypassed, **stats(values)}
            for (pass_id, bypassed), values in passes.items()
        ] if graph else None,
        "gpu_unavailable_reasons": unavailable,
    }


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


def execute_scene(exe, scene, group, env, cwd=ROOT):
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
        with subprocess.Popen(args, cwd=cwd, env=scene_env, stdout=log, stderr=subprocess.STDOUT) as proc:
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
    if status == "complete" and signature["frame_profiling"] is not (
        scene_env["MODULE_ENGINE_BENCHMARK_PROFILING"] == "on"
    ):
        status = "incomplete"
        constraints.append("Reported frame profiling does not match the series configuration")
    if reason := raw.get("effective_present_mode_unavailable_reason"):
        constraints.append(reason)
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
    instrumentation = {key: raw.get(key) for key in ("effective_present_mode", "frame_profiling")}
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
            {"frame_interval_ms": with_tail(raw.get("ui_observed_unique_frame_interval"),
                                            raw.get("ui_observed_unique_frame_interval_samples_ms")),
             "game_step_ms": raw.get("game_step") if scene == "S2" else None,
             "render_ms": raw.get("renderer_content_render_readback"),
             "profiling": summarize_frame_timings(raw.get("presentation_samples", []))},
            [] if player_valid else ["Native Player workload or timing evidence is incomplete"],
            {"vsync_requested": raw.get("native_vsync_requested"), **instrumentation,
             **{key: raw.get(key) for key in ("gui_graphics", "vulkan_device", "monitor_pixels",
                                             "monitor_refresh_millihertz", "offscreen_render_pixels",
                                             "measured_requested", "warmup_requested")}},
        )
    samples = raw.get("samples", [])
    metrics = dict(raw.get("metrics", {}))
    metrics["profiling"] = summarize_frame_timings(samples)
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
        **instrumentation,
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


def ensure_bench_assets(root=None):
    """Download pinned third-party models that the repository does not redistribute.

    A compare worktree reuses verified files from the main checkout's cache
    before downloading anything."""
    root = root or ROOT
    manifest_path = root / "examples/bench/sponza/manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    cache = root / ".tools/bench-assets/sponza"
    seed = ROOT / ".tools/bench-assets/sponza"
    cache.mkdir(parents=True, exist_ok=True)
    downloaded = 0
    for item in manifest["files"]:
        name = item["path"]
        if "/" in name or "\\" in name or name.startswith("."):
            raise ValueError(f"Unsafe benchmark asset name: {name}")
        target = cache / name
        if target.is_file() and entry(cache, name)["sha256"] == item["sha256"]:
            continue
        if seed != cache and (seed / name).is_file() and entry(seed, name)["sha256"] == item["sha256"]:
            shutil.copy2(seed / name, target)
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
    return {"manifest": str(manifest_path.relative_to(root)), "commit": manifest["commit"],
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


def built_editor(messages, host, source_root=None, target_dir=None):
    bins = set()
    source = (source_root or ROOT) / "crates/editor/src/main.rs"
    target_dir = (target_dir or ROOT / "target").resolve()
    allowed_folders = {target_dir / "release", target_dir / host / "release"}
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


def bench_env(vsync, profiling, root=None):
    env = os.environ.copy()
    for variable in ("EFRAME_SCREENSHOT_TO", "MODULE_ENGINE_UI_SCREENSHOT_TO",
                     "MODULE_ENGINE_CAPTURE_ATTACK", "MODULE_ENGINE_RESOURCE_ROOT"):
        env.pop(variable, None)
    env.update(MODULE_ENGINE_BENCHMARK_ARM="automatic", MODULE_ENGINE_BENCHMARK_FRAMES="240",
               MODULE_ENGINE_RESOURCE_ROOT=str(root or ROOT),
               MODULE_ENGINE_BENCHMARK_PROFILING="on" if profiling else "off",
               MODULE_ENGINE_GUI_VSYNC="on" if vsync else "off")
    return env


def native_host():
    rustc = command_output("rustc", "-vV")
    host = next((line.removeprefix("host: ") for line in (rustc or "").splitlines()
                 if line.startswith("host: ")), None)
    if not host:
        raise ValueError("Cannot determine the native host from rustc -vV")
    return rustc, host


def run_series(series, diagnostic, vsync, profiling, once):
    env = bench_env(vsync, profiling)
    bench_assets = ensure_bench_assets()
    source = capture(ROOT)
    write_json(series / "source.json", source)
    build = [*BUILD_COMMAND, "--target-dir", str(ROOT / "target")]
    rustc, host = native_host()
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
        "build": build, "scenes_in_order": SCENES, "runs": 1 if once else 3,
        "diagnostic": diagnostic or once,
        "gui_vsync": vsync, "bench_assets": bench_assets,
        "frame_profiling": profiling,
        "warmup_and_measured_frames": {"S1": [30, 240], "S2": [30, 240], "S3": [30, 240],
                                       "S4": [30, 240], "S5": [5, 30]},
        "warmup_order": ("Per-process frame warmup only" if once else
                         "One unscored pass S1–S5, then per-process frame warmup in each scored pass"),
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
            for number in (range(1, 2) if once else range(4)):
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
    source_matches = capture(ROOT)["source_digest"] == source["source_digest"]
    binary_matches = entry(series, exe.name)["sha256"] == exe_hash
    if once:
        complete = (source_matches and binary_matches
                    and all(case["status"] == "complete" for case in runs[0]["results"].values()))
        write_json(series / "comparison.json", {
            "accepted": False, "diagnostic": True, "complete": complete,
            "source_matches": source_matches, "binary_matches": binary_matches,
            "source_digest": source["source_digest"], "binary_sha256": exe_hash,
            "reason": "One pass does not evaluate three-run repeatability",
        })
        return 0 if complete else 1
    metrics = compare(runs)
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


# --- Baseline comparison ("not worse than the baseline", docs/bench.md#not-worse) ---------

COMPARE_ROUNDS = 5
TIMING_FLOOR_PERCENT = 5.0
MEMORY_FLOOR_PERCENT = 1.0
SPREAD_FACTOR = 2.0
BORDERLINE_FRACTION = 0.25
SIDES = ("baseline", "candidate")
PASSING = ("same", "better")
# Every run must report these; a null present mode means unknown or changed during the run.
REQUIRED_EVIDENCE = ("effective_present_mode", "vsync_requested", "frame_profiling")
# Presentation-path fields that may differ between sides when declared (R2.6). The adapter,
# resolution, workload and present mode never may.
DECLARABLE_CHANGES = ("gui_graphics.backend", "gui_graphics.renderer",
                      "gui_graphics.requested_renderer", "gui_graphics.requested_gpu_backend")


def compare_rows(scene):
    """(metric, statistic, kind, blocking) for one scene, in report order."""
    rows = []
    for key in GATE_METRICS[scene]:
        rows += [(key, "mean_ms", "timing", True), (key, "p95_ms", "timing", True),
                 (key, "p99_ms", "tail", False)]
    rows.append(("peak_working_set_bytes", None, "memory", True))
    return rows


def metric_value(case, key, stat):
    if stat is None:
        return case.get(key)
    return ((case.get("metrics") or {}).get(key) or {}).get(stat)


def judge_metric(kind, baseline, candidate, blocking):
    """Median-based verdict; positive change means slower or larger."""
    row = {"kind": kind, "blocking": blocking, "baseline": baseline, "candidate": candidate}
    if not baseline or not candidate or any(
            value is None or value <= 0 for value in [*baseline, *candidate]):
        return {**row, "verdict": "missing", "borderline": False}
    base = statistics.median(baseline)
    cand = statistics.median(candidate)
    spread = 100 * (max(baseline) - min(baseline)) / base
    floor = MEMORY_FLOOR_PERCENT if kind == "memory" else TIMING_FLOOR_PERCENT
    tolerance = max(floor, SPREAD_FACTOR * spread)
    change = 100 * (cand - base) / base
    verdict = "worse" if change > tolerance else "better" if change < -tolerance else "same"
    return {**row, "baseline_median": base, "candidate_median": cand,
            "baseline_spread_percent": spread, "tolerance_percent": tolerance,
            "change_percent": change, "verdict": verdict,
            "borderline": abs(change - tolerance) <= BORDERLINE_FRACTION * tolerance}


def flatten(value, prefix):
    if isinstance(value, dict) and value:
        result = {}
        for key, item in value.items():
            result.update(flatten(item, f"{prefix}.{key}"))
        return result
    return {prefix: value}


def comparison_fields(case):
    return {**flatten(case.get("signature") or {}, "signature"),
            **flatten(case.get("fixture") or {}, "fixture")}


def signature_differences(cases, declared=()):
    """Dotted signature/fixture fields that differ from the first case, except declared ones."""
    ignored = {f"signature.{field}" for field in declared}
    first = comparison_fields(cases[0])
    keys = set()
    for case in cases[1:]:
        other = comparison_fields(case)
        keys |= {key for key in set(first) | set(other)
                 if key not in ignored and first.get(key) != other.get(key)}
    return sorted(keys)


def run_label(case):
    return f"{case.get('side', '?')} run {case.get('run', '?')}"


def missing_evidence(cases, declared=()):
    """Runs whose presentation evidence is unknown, e.g. a null or changing present mode.

    A declared change must be observed in every run, otherwise the report
    could accept a migration without knowing what the candidate used."""
    problems = []
    for case in cases:
        signature = case.get("signature") or {}
        fields = flatten(signature, "signature")
        unknown = [key for key in REQUIRED_EVIDENCE if signature.get(key) is None]
        unknown += [f"signature.{field} (declared)" for field in declared
                    if fields.get(f"signature.{field}") is None]
        if unknown:
            problems.append(f"{run_label(case)}: {', '.join(unknown)} unknown or changed")
    return problems


def compare_scene(scene, baseline_cases, candidate_cases, declared=()):
    cases = [*baseline_cases, *candidate_cases]
    reasons = []
    if not baseline_cases or not candidate_cases:
        reasons.append("No measured runs on one side")
    incomplete = [f"{run_label(case)}: {case['status']}"
                  for case in cases if case["status"] != "complete"]
    if incomplete:
        reasons.append("Incomplete runs: " + ", ".join(incomplete))
    if evidence := missing_evidence(cases, declared):
        reasons.append("Missing run evidence: " + "; ".join(evidence))
    for side_cases in (baseline_cases, candidate_cases):
        if side_cases and (differences := signature_differences(side_cases)):
            reasons.append(f"Signature or fixture changes within {side_cases[0].get('side', '?')} runs: "
                           + ", ".join(differences))
    declared_changes = {}
    if baseline_cases and candidate_cases:
        pair = [baseline_cases[0], candidate_cases[0]]
        if differences := signature_differences(pair, declared):
            reasons.append("Signature or fixture differs: " + ", ".join(differences))
        fields = [comparison_fields(case) for case in pair]
        declared_changes = {field: {"baseline": fields[0].get(f"signature.{field}"),
                                    "candidate": fields[1].get(f"signature.{field}")}
                            for field in declared}
    metrics = []
    for key, stat, kind, blocking in compare_rows(scene):
        verdict = judge_metric(kind, [metric_value(case, key, stat) for case in baseline_cases],
                               [metric_value(case, key, stat) for case in candidate_cases], blocking)
        metrics.append({"metric": key, "statistic": stat, **verdict})
    comparable = not reasons
    passed = comparable and all(row["verdict"] in PASSING for row in metrics if row["blocking"])
    return {"comparable": comparable, "reasons": reasons, "passed": passed,
            "borderline": comparable and any(row["blocking"] and row["borderline"] for row in metrics),
            "runs": {"baseline": len(baseline_cases), "candidate": len(candidate_cases)},
            "declared_changes": declared_changes, "metrics": metrics}


def compare_sides(runs, scenes, declared=()):
    """runs: [{"side", "run", "results": {scene: case}}] from every session that counts."""
    result = {}
    for scene in scenes:
        sides = {side: [{**run["results"][scene], "side": side, "run": run["run"]}
                        for run in runs if run["side"] == side and scene in run["results"]]
                 for side in SIDES}
        result[scene] = compare_scene(scene, sides["baseline"], sides["candidate"], declared)
    return result


def interleaved_schedule(rounds, candidate_first=False):
    order = ("candidate", "baseline") if candidate_first else SIDES
    return [(side, number) for number in range(1, rounds + 1) for side in order]


def format_metric_value(row, key):
    value = row.get(key)
    if value is None:
        return "—"
    if row["kind"] == "memory":
        return f"{value / 1024 ** 2:.1f} MiB"
    return f"{value:.3f} ms"


def render_table(scenes):
    lines = ["| Scene | Metric | Baseline median | Candidate median | Change | Tolerance | Verdict |",
             "| --- | --- | --- | --- | --- | --- | --- |"]
    for scene, item in scenes.items():
        if not item["comparable"]:
            lines.append(f"| {scene} | — | — | — | — | — | INCOMPARABLE: {'; '.join(item['reasons'])} |")
            continue
        for row in item["metrics"]:
            name = row["metric"] + (f" {row['statistic'].removesuffix('_ms')}" if row["statistic"] else "")
            change = row.get("change_percent")
            tolerance = row.get("tolerance_percent")
            verdict = row["verdict"].upper() if row["blocking"] else f"{row['verdict']} (tracked)"
            if row["borderline"] and row["blocking"]:
                verdict += ", borderline"
            lines.append(
                f"| {scene} | {name} | {format_metric_value(row, 'baseline_median')} | "
                f"{format_metric_value(row, 'candidate_median')} | "
                f"{'—' if change is None else f'{change:+.2f} %'} | "
                f"{'—' if tolerance is None else f'±{tolerance:.2f} %'} | {verdict} |")
    for scene, item in scenes.items():
        for field, values in item.get("declared_changes", {}).items():
            lines.append(f"\n{scene}: declared change `{field}`: "
                         f"{values['baseline']} → {values['candidate']}")
    return "\n".join(lines) + "\n"


def git_output(cwd, *args):
    return subprocess.check_output(["git", *args], cwd=cwd, text=True,
                                   stderr=subprocess.STDOUT, timeout=120).strip()


def resolve_commit(ref):
    try:
        return git_output(ROOT, "rev-parse", "--verify", f"{ref}^{{commit}}")
    except subprocess.CalledProcessError as error:
        raise ValueError(f"Unknown commit {ref!r}: {error.output.strip()}") from error


def provision_worktree(worktree):
    """Copy Git-ignored native tools (DXC) from the main checkout into a worktree."""
    ignored = git_output(ROOT, "ls-files", "--others", "--ignored", "--exclude-standard",
                         "-z", "--", "tools/dxc")
    copied = []
    for name in filter(None, ignored.split("\0")):
        target = worktree / name
        if not target.exists():
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / name, target)
            copied.append(name)
    return copied


def build_side(folder, root, env, host):
    """Build one side and copy the exact executable into the series."""
    folder.mkdir()
    # Shared Cargo products can remain fresh across checkouts with different sources.
    target_dir = folder / "target"
    build = [*BUILD_COMMAND, "--target-dir", str(target_dir)]
    messages = folder / "build.jsonl"
    with (folder / "build.log").open("x", encoding="utf-8") as log, messages.open("xb") as events:
        result = subprocess.run(build, cwd=root, env=env, stdout=events, stderr=log)
    if result.returncode:
        raise ValueError(f"Build failed in {root}; see {folder / 'build.log'}")
    executable = built_editor(messages, host, root, target_dir)
    exe = folder / executable.name
    shutil.copy2(executable, exe)
    return exe, entry(folder, exe.name)["sha256"]


def prepare_side(series, side, ref, vsync, profiling, host, worktrees):
    """Return the runtime description of one side; ref None means the current checkout."""
    if ref is None:
        root = ROOT
    else:
        root = ROOT / ".work" / "bench-compare" / f"{series.parent.name}-{series.name}-{side}"
        root.parent.mkdir(parents=True, exist_ok=True)
        git_output(ROOT, "worktree", "add", "--detach", str(root), ref)
        worktrees.append(root)
        provision_worktree(root)
    env = bench_env(vsync, profiling, root)
    assets = ensure_bench_assets(root)
    source = capture(root)
    write_json(series / f"{side}-source.json", source)
    print(f"Building {side} ({ref or 'current checkout'} at {source['head'][:12]})", flush=True)
    exe, exe_hash = build_side(series / f"{side}-build", root, env, host)
    return {"side": side, "ref": ref or "current checkout", "root": root, "env": env, "exe": exe,
            "head": source["head"], "source_digest": source["source_digest"],
            "binary_sha256": exe_hash, "bench_assets": assets}


def measure_pass(side, label, folder, scenes, number):
    folder.mkdir(parents=True)
    run = {"side": side["side"], "run": number, "started_at": datetime.now().astimezone().isoformat(),
           "source_digest": side["source_digest"], "binary_sha256": side["binary_sha256"],
           "results": {}}
    for scene in scenes:
        result = execute_scene(side["exe"], scene, folder, side["env"], side["root"])
        run["results"][scene] = result
        write_json(folder / f"{scene}.json", result)
    write_json(folder / "run.json", run)
    print(f"{label} done", flush=True)
    return run


def measure_session(series, sides, session, scenes, rounds, candidate_first=False):
    folder = series / f"session-{session}"
    runs = []
    for side_name, number in interleaved_schedule(rounds, candidate_first):
        name = f"run-{number:02}-{side_name}"
        runs.append(measure_pass(sides[side_name], f"session {session} {name}",
                                 folder / name, scenes, number + (session - 1) * rounds))
    return runs


def run_compare(series, baseline_ref, candidate_ref, rounds, scenes, vsync, profiling, declared=()):
    baseline = resolve_commit(baseline_ref)
    candidate = resolve_commit(candidate_ref) if candidate_ref else None
    _, host = native_host()
    worktrees = []
    try:
        sides = {"baseline": prepare_side(series, "baseline", baseline, vsync, profiling, host, worktrees)}
        sides["candidate"] = prepare_side(series, "candidate", candidate, vsync, profiling, host, worktrees)
        rule = {"rounds_per_side": rounds, "order": "interleaved ABAB…; a rerun starts with B",
                "statistic": "median of per-run values",
                "tolerance": "max(floor, 2 × baseline (max − min) / median)",
                "timing_floor_percent": TIMING_FLOOR_PERCENT,
                "memory_floor_percent": MEMORY_FLOOR_PERCENT, "spread_factor": SPREAD_FACTOR,
                "borderline": f"|change − tolerance| ≤ {BORDERLINE_FRACTION} × tolerance",
                "blocking": "mean and p95 of declared timing metrics, peak working set",
                "tracked": "p99 of declared timing metrics",
                "required_evidence": list(REQUIRED_EVIDENCE),
                "declared_changes": list(declared)}
        write_json(series / "compare-manifest.json", {
            "version": 1, "local_started": datetime.now().astimezone().isoformat(),
            "scenes": scenes, "gui_vsync": vsync, "frame_profiling": profiling, "rule": rule,
            "sides": {name: {key: str(value) if isinstance(value, Path) else value
                             for key, value in side.items() if key not in ("env",)}
                      for name, side in sides.items()},
        })
        result = compare_series(series, sides, scenes, rounds, rule, declared)
    finally:
        for worktree in worktrees:
            subprocess.run(["git", "worktree", "remove", "--force", str(worktree)], cwd=ROOT,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return result


def compare_series(series, sides, scenes, rounds, rule, declared=()):
    print("Unscored prewarm pass for each side", flush=True)
    for name in SIDES:
        measure_pass(sides[name], f"prewarm {name}", series / f"prewarm-{name}", scenes, 0)
    runs = measure_session(series, sides, 1, scenes, rounds)
    verdict = compare_sides(runs, scenes, declared)
    rerun = [scene for scene, item in verdict.items() if item["borderline"]]
    if rerun:
        print(f"Borderline result in {', '.join(rerun)}; repeating once", flush=True)
        runs += measure_session(series, sides, 2, rerun, rounds, candidate_first=True)
        verdict.update(compare_sides(runs, rerun, declared))
    integrity = {name: {
        "source_matches": capture(side["root"])["source_digest"] == side["source_digest"],
        "binary_matches": entry(side["exe"].parent, side["exe"].name)["sha256"] == side["binary_sha256"],
    } for name, side in sides.items()}
    intact = all(all(item.values()) for item in integrity.values())
    accepted = intact and all(item["passed"] for item in verdict.values())
    table = render_table(verdict)
    write_json(series / "compare.json", {
        "accepted": accepted, "integrity": integrity, "rerun_scenes": rerun, "rule": rule,
        "sides": {name: {key: side[key] for key in ("ref", "head", "source_digest", "binary_sha256")}
                  for name, side in sides.items()},
        "scenes": verdict,
    })
    with (series / "compare.md").open("x", encoding="utf-8", newline="\n") as output:
        output.write(table)
    # The report may contain glyphs unavailable in a redirected Windows console.
    encoding = sys.stdout.encoding or "utf-8"
    print(table.encode(encoding, "backslashreplace").decode(encoding), flush=True)
    if not intact:
        print("Sources or binaries changed during the series", flush=True)
    print("ACCEPTED: not worse than baseline" if accepted else "REJECTED", flush=True)
    return 0 if accepted else 1


def parse_scenes(text):
    scenes = [scene.strip().upper() for scene in text.split(",") if scene.strip()]
    unknown = [scene for scene in scenes if scene not in SCENES]
    if unknown or not scenes:
        raise argparse.ArgumentTypeError(f"Expected a comma-separated subset of {', '.join(SCENES)}")
    return [scene for scene in SCENES if scene in scenes]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--diagnostic", action="store_true", help="Record a series with known competing load")
    parser.add_argument("--vsync", action="store_true",
                        help="Keep GUI vsync; frame intervals are then capped by the monitor refresh rate")
    parser.add_argument("--no-profiling", action="store_true",
                        help="Disable CPU and GPU frame profiling for baseline comparisons")
    parser.add_argument("--once", action="store_true",
                        help="Run one diagnostic S1–S5 pass with per-scene warmup only")
    parser.add_argument("--compare", metavar="BASELINE",
                        help="Judge the candidate against this baseline commit by the not-worse rule")
    parser.add_argument("--candidate", metavar="REF",
                        help="Candidate commit for --compare (default: the current checkout as is)")
    parser.add_argument("--rounds", type=int, default=COMPARE_ROUNDS,
                        help=f"Interleaved runs per side for --compare (default {COMPARE_ROUNDS})")
    parser.add_argument("--scenes", type=parse_scenes, default=list(SCENES),
                        help="Comma-separated scenes for --compare, for example S2,S3")
    parser.add_argument("--declare-change", action="append", default=[], choices=DECLARABLE_CHANGES,
                        metavar="FIELD", help="Presentation field allowed to differ between sides "
                        f"under --compare (repeatable): {', '.join(DECLARABLE_CHANGES)}")
    args = parser.parse_args()
    if args.compare is None and (args.candidate or args.rounds != COMPARE_ROUNDS
                                 or args.scenes != list(SCENES) or args.declare_change):
        parser.error("--candidate, --rounds, --scenes and --declare-change require --compare")
    if args.compare is not None and (args.once or args.diagnostic):
        parser.error("--compare cannot be combined with --once or --diagnostic")
    if args.rounds < 3:
        parser.error("--rounds must be at least 3")
    now = datetime.now().astimezone()
    series = ROOT / "outputs" / "bench" / now.date().isoformat() / now.strftime("%H%M%S-%f")
    if args.compare is not None:
        series = series.with_name(series.name + "-compare")
    series.mkdir(parents=True)
    print(f"Reports: {series}", flush=True)
    try:
        if args.compare is not None:
            return run_compare(series, args.compare, args.candidate, args.rounds, args.scenes,
                               args.vsync, not args.no_profiling, tuple(args.declare_change))
        return run_series(series, args.diagnostic, args.vsync, not args.no_profiling, args.once)
    except (ValueError, OSError, subprocess.SubprocessError, KeyError) as error:
        write_json(series / "failure.json", {"error": str(error)})
        print(f"bench: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
