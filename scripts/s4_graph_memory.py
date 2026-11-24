"""Compare the occupied Vulkan ranges recorded by the S4 floor benchmarks."""

import argparse
from collections import defaultdict
import json
from pathlib import Path


def occupied_bytes(report):
    """Union half-open ranges on each device memory binding."""
    bindings = defaultdict(list)
    for image in report["images"]:
        offset = image["offset_bytes"]
        size = image["required_bytes"]
        if offset < 0 or size <= 0:
            raise ValueError(f"Invalid image range for {image['resource']}")
        bindings[image["memory_binding"]].append((offset, offset + size))
    if not bindings:
        raise ValueError("No Vulkan image ranges")
    total = 0
    for ranges in bindings.values():
        covered_until = 0
        for start, end in sorted(ranges):
            total += max(0, end - max(start, covered_until))
            covered_until = max(covered_until, end)
    return total


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path, help="graph-memory.json from a benchmark run")
    parser.add_argument("--reference", type=Path, help="Saved graph-memory.json for the same graph")
    args = parser.parse_args()
    try:
        capture = json.loads(args.capture.read_text(encoding="utf-8"))
        used = occupied_bytes(capture)
        print(f"{used:,} bytes ({used / 2**20:.4f} MiB)")
        if args.reference:
            reference = json.loads(args.reference.read_text(encoding="utf-8"))
            if any(reference[key] != capture[key] for key in ("graph", "render_pixels", "slot")):
                raise ValueError("Compare the same graph, viewport and slot")
            baseline = occupied_bytes(reference)
            print(f"Reduction from {baseline:,} bytes: {100 * (1 - used / baseline):.2f}%")
    except (KeyError, OSError, ValueError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
