"""Run a matched pair of native ModuleEngine R3.1 painters at one monitor DPI."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
EXTENT = [1580, 940]
PAINTERS = ("ash", "thin")


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def layer_environment(validation):
    env = os.environ.copy()
    for setting in (
        "VK_INSTANCE_LAYERS", "VK_LOADER_LAYERS_ENABLE", "VK_LOADER_LAYERS_DISABLE",
        "VK_LOADER_LAYERS_ALLOW", "VK_LAYER_ENABLES", "VK_LAYER_DISABLES",
        "VK_LAYER_VALIDATE_SYNC",
    ):
        env.pop(setting, None)
    requested = {"VK_LOADER_DEBUG": "layer"}
    if validation:
        requested["VK_LAYER_VALIDATE_SYNC"] = "1"
    else:
        requested["VK_LOADER_LAYERS_DISABLE"] = "VK_LAYER_KHRONOS_validation"
    env.update(requested)
    return env, requested


def validate_layers(log, validation):
    lines = log.splitlines()
    active = "Khronos Validation Layer Active:" in log or any(
        "Insert instance layer" in line and "VK_LAYER_KHRONOS_validation" in line
        for line in lines
    )
    sync = any(
        "Current Enables:" in line and "SYNCHRONIZATION_VALIDATION" in line
        for line in lines
    )
    summary = "Vulkan debug summary:" in log
    status = {"validation_active": active, "synchronization_validation_active": sync}
    if validation:
        if not active or not sync or "Vulkan debug summary: validation=active " not in log:
            return status, "Khronos synchronization validation and its teardown must be observed"
        if "Vulkan validation ERROR" in log:
            return status, "The Vulkan validation layer reported an error"
    elif active or sync or summary:
        return status, "Scored timing requires a Release build without Vulkan diagnostics"
    return status, None


def run_case(exe, output, name, painter, scale, validation, env):
    destination = output / name
    log_path = output / f"{name}.log"
    args = [str(exe), "--r31-editor-vulkan", painter, scale, str(destination)]
    record = {"name": name, "command": args, "log": log_path.name, "errors": []}
    errors = record["errors"]
    with log_path.open("wb") as log:
        try:
            child = subprocess.run(
                args,
                cwd=ROOT,
                stdout=log,
                stderr=subprocess.STDOUT,
                env=env,
                timeout=120,
                creationflags=subprocess.CREATE_NO_WINDOW,  # Keep the native winit window visible.
            )
            record["exit_code"] = child.returncode
        except (subprocess.TimeoutExpired, OSError) as error:
            errors.append(str(error))
            return record
    try:
        report = json.loads((destination / "report.json").read_text(encoding="utf-8"))
        screenshot = destination / "editor.png"
        with screenshot.open("rb") as stream:
            header = stream.read(24)
        if header[:16] != b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR" or [
            int.from_bytes(header[16:20], "big"),
            int.from_bytes(header[20:24], "big"),
        ] != EXTENT:
            errors.append("Capture is not a PNG with the declared physical extent")
        record["capture_sha256"] = sha256(screenshot)
        record["fixture_sha256"] = sha256(destination / "project.json")
        record["report"] = f"{name}/report.json"
        record["conditions"] = {field: report[field] for field in (
            "native_scale", "pixels_per_point", "egui_zoom", "client_pixels",
            "refresh_millihertz", "device",
        )}
        samples = report["samples"]
        if (
            report["painter"] != painter
            or report["native_scale"] != float(scale)
            or report["pixels_per_point"] != float(scale)
            or report["egui_zoom"] != 1.0
            or report["client_pixels"] != EXTENT
            or report["warmup_frames"] != 30
            or len(samples) != 240
            or any(sample["interval_ms"] <= 0 or sample["indices"] <= 0 for sample in samples)
            or report["presented_frames"] != 271
            or report["capture_frame"] != 271
            or report["device"]["present_mode"] != "IMMEDIATE"
            or report["device"]["producer_slots"] != 2
            or not report["device"]["capabilities"]["tier_a"]
        ):
            errors.append("Report does not match the native DPI, window, draw or frame-count contract")
        record["timings"] = {field: report[field] for field in (
            "fps", "mean_interval_ms", "p95_interval_ms", "max_interval_ms", "intervals_over_budget",
        )}
        over_budget = sum(sample["interval_ms"] > 1000.0 / 60.0 for sample in samples)
        if over_budget != report["intervals_over_budget"]:
            errors.append("The interval budget count differs from the raw series")
        if not name.startswith("prewarm-") and not validation and over_budget:
            errors.append(f"{over_budget} intervals exceeded 1000/60 ms")
        if child.returncode != int(bool(over_budget)) or report["validation_errors"]:
            errors.append("The executable reported an unexpected failure")
        log = log_path.read_text(encoding="utf-8", errors="replace")
        record["layers"], layer_error = validate_layers(log, validation)
        if layer_error:
            errors.append(layer_error)
    except (OSError, KeyError, TypeError, ValueError) as error:
        errors.append(f"Invalid or missing native result: {error}")
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exe", type=Path, required=True)
    parser.add_argument("--dpi", choices=("1.0", "1.5"), required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--validation", action="store_true", help="One diagnostic pair with the Vulkan layer active")
    args = parser.parse_args()
    if sys.platform != "win32":
        parser.error("This spike requires a native Windows desktop")
    exe = args.exe.resolve(strict=True)
    output = args.out.resolve()
    output.mkdir(parents=True, exist_ok=False)
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    binary = sha256(exe)
    env, validation_settings = layer_environment(args.validation)
    cases = [("validation" if args.validation else "prewarm", painter) for painter in PAINTERS]
    if not args.validation:
        cases.extend((f"run-{repeat}", painter) for repeat, order in enumerate(
            (PAINTERS, PAINTERS[::-1], PAINTERS), 1,
        ) for painter in order)
    records = [
        run_case(exe, output, f"{label}-{painter}", painter, args.dpi, args.validation, env)
        for label, painter in cases
    ]
    baseline = records[0]
    matched = "conditions" in baseline and all(
        record.get("conditions") == baseline["conditions"]
        and record.get("fixture_sha256") == baseline.get("fixture_sha256")
        for record in records
    )
    same_binary = sha256(exe) == binary
    passed = matched and same_binary and not any(record["errors"] for record in records)
    summary = {
        "head": head,
        "os": platform.platform(),
        "exe": str(exe),
        "exe_sha256": binary,
        "native_scale_requested": float(args.dpi),
        "validation": args.validation,
        "vulkan_environment": validation_settings,
        "matched_conditions_and_fixture": matched,
        "binary_unchanged": same_binary,
        "passed": passed,
        "runs": records,
    }
    path = output / "series.json"
    path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(path)
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
