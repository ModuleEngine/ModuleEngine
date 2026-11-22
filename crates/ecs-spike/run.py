"""Build, verify and compare the three S3 stores in fresh, sequential processes."""

import argparse
import hashlib
import itertools
import json
import math
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "crates" / "ecs-spike" / "Cargo.toml"
TARGET = ROOT / "target" / "ecs-spike"
SNAPSHOT = ROOT / "scripts" / "review_snapshot.py"
VARIANTS = ("core", "dynamic", "hybrid")
BATCHES = ("spawn", "despawn", "snapshot", "restore")


def capture(command, log):
    with log.open("wb") as stream:
        subprocess.run(command, cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT, check=True)


def run_process(binary, variant, stem):
    with stem.with_suffix(".json").open("wb") as stdout, stem.with_suffix(".stderr").open("wb") as stderr:
        subprocess.run([str(binary), variant], cwd=ROOT, stdout=stdout, stderr=stderr, check=True)
    report = json.loads(stem.with_suffix(".json").read_bytes())
    expected = {
        "variant": variant, "entities": 2000, "components_per_entity": 8,
        "moved_per_tick": 1000, "tick_warmup": 30, "batch_warmup": 5,
    }
    for key, value in expected.items():
        if report[key] != value:
            raise ValueError(f"{stem.name} {key}: expected {value!r}, got {report[key]!r}")
    if set(report["batch_ms"]) != set(BATCHES):
        raise ValueError(f"{stem.name}: incomplete operation list")
    timings = {"tick": report["tick_ms"], **report["batch_ms"]}
    for key, samples in timings.items():
        count = 240 if key == "tick" else 30
        if len(samples) != count or any(not math.isfinite(x) or x <= 0 for x in samples):
            raise ValueError(f"{stem.name} {key}: invalid timing samples")
    return timings


def stats(samples):
    return {
        "count": len(samples),
        "mean_ms": statistics.mean(samples),
        "p95_ms": sorted(samples)[math.ceil(0.95 * len(samples)) - 1],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="new run folder under outputs/bench")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True)
    source = output / "source.json"
    capture([sys.executable, str(SNAPSHOT), "capture", "--output", str(source)], output / "capture.log")
    capture(["rustc", "-Vv"], output / "rustc.log")
    capture(["cargo", "-V"], output / "cargo.log")
    cargo_args = ["--release", "--offline", "--locked", "--manifest-path", str(MANIFEST), "--target-dir", str(TARGET)]
    capture(["cargo", "test", *cargo_args], output / "test.log")
    capture(["cargo", "build", *cargo_args], output / "build.log")
    name = "module-engine-ecs-spike" + (".exe" if os.name == "nt" else "")
    binary = output / name
    shutil.copyfile(TARGET / "release" / name, binary)
    binary.chmod((TARGET / "release" / name).stat().st_mode)
    binary_sha = hashlib.sha256(binary.read_bytes()).hexdigest()

    orders = list(itertools.permutations(VARIANTS))
    for variant in VARIANTS:
        run_process(binary, variant, output / f"prewarm-{variant}")

    runs = []
    pooled = {variant: {key: [] for key in ("tick", *BATCHES)} for variant in VARIANTS}
    for number, order in enumerate(orders, 1):
        run = {"number": number, "order": order, "variants": {}}
        for variant in order:
            timings = run_process(binary, variant, output / f"run-{number:02}-{variant}")
            run["variants"][variant] = {key: stats(samples) for key, samples in timings.items()}
            for key, samples in timings.items():
                pooled[variant][key].extend(samples)
        runs.append(run)

    results = {variant: {key: stats(samples) for key, samples in times.items()} for variant, times in pooled.items()}
    speedup = {variant: results["core"]["tick"]["mean_ms"] / results[variant]["tick"]["mean_ms"]
               for variant in VARIANTS[1:]}
    if hashlib.sha256(binary.read_bytes()).hexdigest() != binary_sha:
        raise ValueError("Measured binary changed during the series")
    capture([sys.executable, str(SNAPSHOT), "check", str(source)], output / "check-source.log")
    summary = {
        "machine": {"platform": platform.platform(), "processor": platform.processor(), "logical_cpus": os.cpu_count()},
        "binary_sha256": binary_sha,
        "runs": runs,
        "pooled": results,
        "tick_speedup": speedup,
    }
    (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"pooled": results, "tick_speedup": speedup}, indent=2))


if __name__ == "__main__":
    main()
