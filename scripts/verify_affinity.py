"""Read back native/Python placement for every supported topology on this host."""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path
from unittest.mock import patch

import benchmark_suite as suite

sys.path.insert(0, str(suite.ROOT))
from benchmarks.methods.python import benchmark_adapter as adapter


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("debug", "release"), default="release")
    parser.add_argument("--output", type=Path, required=True)
    options = parser.parse_args()
    options.output.mkdir(parents=True, exist_ok=False)
    registry = {m["name"]: m for m in json.loads(suite.REGISTRY.read_text())["methods"]}
    results = []
    for placement in ("smt", "separate-core", "separate-cache", "unpinned", "explicit"):
        environment = {key: value for key, value in os.environ.items() if not key.startswith("IPC_BENCH_")}
        environment["IPC_BENCH_TOPOLOGY"] = placement if placement != "explicit" else "separate-core"
        try:
            with patch.dict(os.environ, environment, clear=True):
                expected = (
                    (adapter.effective_affinity(), adapter.effective_affinity())
                    if placement == "unpinned"
                    else adapter._resolve_stable_affinity_pair()
                )
        except (ValueError, OSError) as error:
            results.append({"topology": placement, "status": "unsupported", "reason": str(error)})
            continue
        args = argparse.Namespace(
            action="run",
            profile=options.profile,
            warmup=4,
            trials=1,
            timeout=15,
            validation="full",
            measurement="batch",
            workload="round-trip",
            queue_depth=1,
            ring_capacity=64,
            stable_affinity=False,
            cpu_pair=None,
            topology=placement,
            mailslot_count=None,
        )
        if placement == "explicit":
            args.topology = None
            args.cpu_pair = ",".join(str(mask.bit_length() - 1) for mask in expected)
        for name in ("copy-roundtrip", "anon-pipe", "shm-ring-spin", "py-multiprocessing-pipe"):
            destination = options.output / "cases" / f"{len(results):05d}"
            destination.mkdir(parents=True)
            result = suite.run_case(registry[name], 64, 32, args, destination)
            if result["status"] == "passed":
                report = json.loads((destination / "report.json").read_text())
                if report["effective_parent_affinity"] != expected[0] or (
                    name != "copy-roundtrip" and report["effective_child_affinity"] != expected[1]
                ):
                    result["status"], result["error"] = "failed", "effective placement does not match resolved topology"
            results.append(result | {"topology": placement, "expected_masks": expected})
            suite.atomic_json(options.output / "manifest.json", results)
            print(f"{placement} {name}: {result['status']}", flush=True)
    suite.atomic_json(options.output / "manifest.json", results)
    if any(result["status"] == "failed" for result in results):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
