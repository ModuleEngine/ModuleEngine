#!/usr/bin/env python3
"""Compare the two S5 paths in one bench-enabled Release Editor."""
import argparse
from pathlib import Path
import platform
import subprocess
import sys

from bench import (
    ROOT, bench_env, command_output, execute_scene, judge_metric, read_json, write_json,
)
from review_snapshot import capture, entry


PATHS = {"baseline": "S5-r24", "candidate": "S5-r81"}


def scene_result(exe, path, folder, env, parity):
    case = execute_scene(exe, PATHS[path], folder, env)
    write_json(folder / "run.json", case)
    if case["status"] != "complete" or case["peak_working_set_bytes"] is None:
        raise ValueError(f"Incomplete result for {path} in {folder}. See run.json and process log.")
    raw = read_json(folder / case["raw_report"])
    device = dict(raw["r81_device"])
    if device.pop("extension_enabled") != (path == "candidate"):
        raise ValueError(f"Unexpected mesh extension state for {path} in {folder}")
    if (raw["fixture"], raw["camera"], raw["render_pixels"]) != (
            parity["fixture"], parity["camera"], parity["render_pixels"]):
        raise ValueError(f"Native input differs from the matched image check in {folder}")
    proof = parity["r81" if path == "candidate" else "r24"]["device"]
    if any(device[key] != proof[key]
           for key in ("vendor_id", "device_id", "device_uuid", "driver_version_raw")):
        raise ValueError(f"Native GPU differs from the matched image check in {folder}")
    if (case["signature"]["effective_present_mode"] != "Immediate"
            or raw["cpu_pixel_readback_bytes"] != 0
            or raw["window_pixels_at_close"] != raw["render_pixels"]):
        raise ValueError(f"Expected 1080p GPU-only presentation with Immediate mode in {folder}")
    profiles = case["metrics"]["profiling"]
    if (profiles["gpu_graph_ms"] or {}).get("samples") != raw["measured_requested"]:
        raise ValueError(f"Missing GPU timestamp samples in {folder}")
    geometry = next((row for row in profiles["gpu_pass_ms"] if row["id"] == "geometry"), None)
    if not geometry or geometry["samples"] != raw["measured_requested"] or geometry["bypassed"]:
        raise ValueError(f"Missing Forward geometry samples in {folder}")
    culling = next((row for row in profiles["gpu_pass_ms"] if row["id"] == "Scene culling"), None)
    if path == "baseline" and (not culling or culling["samples"] != raw["measured_requested"]):
        raise ValueError(f"Missing R2.4 GPU culling samples in {folder}")
    initial = raw["first_completed_frame_work"]
    upload_bytes = sum(initial[key] for key in (
        "vertex_write_bytes", "mesh_index_write_bytes", "instance_write_bytes",
        "material_row_write_bytes", "draw_write_bytes", "culling_input_write_bytes",
    )) if path == "baseline" else initial["meshlets"]["upload_bytes"]
    return {
        "folder": folder.name,
        "signature": {"device": device, **case["signature"]},
        "frame_interval_ms": case["metrics"]["frame_interval_ms"],
        "cpu_submit_ms": case["metrics"]["cpu_submit_ms"],
        "gpu_graph_ms": profiles["gpu_graph_ms"],
        "gpu_geometry_ms": geometry,
        "gpu_culling_ms": culling,
        "over_60hz_intervals": sum(row["frame_interval_ms"] > 1000 / 60 for row in raw["samples"]),
        "first_submission_ms": raw["first_submission_ms"],
        "first_producer_latency_ms": raw["first_producer_latency_ms"],
        "fixture_load_ms": raw["fixture_load_ms"],
        "retained_scene_prepare_ms": raw["retained_scene_prepare_ms"],
        "meshlet_build": raw.get("meshlet_build"),
        "first_geometry_upload_bytes": upload_bytes,
        "peak_working_set_bytes": case["peak_working_set_bytes"],
        "producer_allocator": raw["producer_allocator"],
    }


def run(exe, output, rounds):
    source = capture(ROOT)
    write_json(output / "source.json", source)
    exe_hash = entry(exe.parent, exe.name)["sha256"]
    env = bench_env(vsync=False, profiling=True)
    env.update(MODULE_ENGINE_GUI_BACKEND="wgpu")
    # The image gate and timing series use the same Release artifact.
    with (output / "visibility.log").open("x", encoding="utf-8") as log:
        subprocess.run([str(exe), "--bench-scene", "R81-visibility", str(output / "visibility")],
                       cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT,
                       timeout=360, check=True)
    parity = read_json(output / "visibility" / "comparison.json")
    if parity.get("status") != "complete" or parity.get("fxaa") is not True:
        raise ValueError("S5 image check is missing or failed")
    runs = {"baseline": [], "candidate": []}
    signature = None
    for number in range(1, rounds + 1):
        # Reverse which variant starts each pair.
        order = tuple(PATHS) if number % 2 else tuple(reversed(PATHS))
        for path in order:
            folder = output / f"{number:02}-{path}"
            folder.mkdir()
            result = scene_result(exe, path, folder, env, parity)
            if signature is None:
                signature = result["signature"]
            if result["signature"] != signature:
                raise ValueError(f"Device, presentation or fixture signature changed in {folder}")
            runs[path].append(result)
    if (capture(ROOT)["source_digest"] != source["source_digest"]
            or entry(exe.parent, exe.name)["sha256"] != exe_hash):
        raise ValueError("Source or benchmark executable changed during the series")
    comparisons = {}
    for metric in ("gpu_graph_ms", "gpu_geometry_ms", "frame_interval_ms", "cpu_submit_ms"):
        comparisons[metric] = {
            stat: judge_metric("timing", [row[metric][stat] for row in runs["baseline"]],
                               [row[metric][stat] for row in runs["candidate"]], False)
            for stat in ("mean_ms", "p95_ms")
        }
    comparisons["peak_working_set_bytes"] = judge_metric(
        "memory", [row["peak_working_set_bytes"] for row in runs["baseline"]],
        [row["peak_working_set_bytes"] for row in runs["candidate"]], False,
    )
    write_json(output / "comparison.json", {
        "status": "complete",
        "head": source["head"],
        "source_digest": source["source_digest"],
        "exe": str(exe),
        "exe_sha256": exe_hash,
        "platform": platform.platform(),
        "rustc": command_output("rustc", "-vV"),
        "gpu_inventory": command_output("nvidia-smi", "--query-gpu=name,driver_version,memory.total",
                                        "--format=csv,noheader"),
        "signature": signature,
        "image_comparison": "visibility/comparison.json",
        "comparisons": comparisons,
        "runs": runs,
    })


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exe", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--rounds", type=int, default=5)
    args = parser.parse_args()
    if args.rounds < 3:
        parser.error("--rounds must be at least 3")
    output = args.output.resolve()
    try:
        exe = args.exe.resolve(strict=True)
        output.mkdir(parents=True)
    except OSError as error:
        parser.exit(2, f"R8.1: {error}\n")
    try:
        run(exe, output, args.rounds)
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        write_json(output / "failure.json", {"error": str(error)})
        parser.exit(2, f"R8.1: {error}\n")
    print(output / "comparison.json")


if __name__ == "__main__":
    sys.exit(main())
