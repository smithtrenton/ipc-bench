"""Sweep delivery depth and ring capacity with retained duration-calibration pilots."""

from __future__ import annotations
import argparse
import copy
import json
import random
from pathlib import Path
import benchmark_suite as suite


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--seconds", type=float, default=2.0, help="Target seconds per measured trial")
    parser.add_argument("--launches", type=int, default=5)
    parser.add_argument("--trials", type=int, default=3)
    parser.add_argument("--sizes", nargs="+", type=int, default=[64, 65536, 1048576])
    parser.add_argument("--depths", nargs="+", type=int, default=[1, 2, 8, 64, 256])
    parser.add_argument("--capacities", nargs="+", type=int, default=[8, 64, 256])
    parser.add_argument("--seed", type=int, default=20260904)
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument(
        "--methods",
        nargs="+",
        choices=("shm-ring-spin", "shm-ring-hybrid", "named-pipe-iocp"),
        default=["shm-ring-spin", "shm-ring-hybrid", "named-pipe-iocp"],
    )
    parser.add_argument("--binary-dir", type=Path, help="Use previously captured release binaries")
    options = parser.parse_args()
    if options.seconds <= 0 or options.launches < 1 or options.trials < 1:
        parser.error("duration, launches and trials must be positive")
    options.output.mkdir(parents=True, exist_ok=False)
    if not options.skip_build:
        suite.build("release")
    registry = {m["name"]: m for m in json.loads(suite.REGISTRY.read_text())["methods"]}
    args = argparse.Namespace(
        action="run",
        profile="release",
        warmup=256,
        trials=options.trials,
        timeout=120,
        validation="full",
        measurement="batch",
        workload="streaming",
        queue_depth=1,
        ring_capacity=64,
        stable_affinity=True,
        cpu_pair=None,
        topology=None,
        mailslot_count=None,
        features=[],
    )
    suite.atomic_json(
        options.output / "metadata.json",
        suite.metadata(args)
        | {"sweep": vars(options) | {"output": str(options.output), "binary_dir": str(options.binary_dir)}},
    )
    suite.snapshot_sources(options.output)
    configurations = []
    for method in options.methods:
        entry = registry[method].copy()
        if options.binary_dir:
            entry["binary"] = str(options.binary_dir.resolve() / (method + ".exe"))
        for mode in registry[method]["workloads"]:
            if mode == "round-trip":
                continue
            for capacity in options.capacities if method.startswith("shm-") else [64]:
                for depth in options.depths:
                    for size in options.sizes:
                        config = copy.copy(args)
                        config.workload, config.queue_depth, config.ring_capacity = mode, depth, capacity
                        gate = options.output / "gates" / f"{method}-{mode}-{capacity}-{depth}-{size}"
                        count = suite.calibrate(entry, size, 4096, config, gate, options.seconds)
                        configurations.append((entry, size, count, config))
    cases = [(index, launch) for index in range(len(configurations)) for launch in range(options.launches)]
    random.Random(options.seed).shuffle(cases)
    suite.atomic_json(
        options.output / "order.json",
        [
            {
                "configuration": i,
                "launch": launch,
                "method": configurations[i][0]["name"],
                "size": configurations[i][1],
                "count": configurations[i][2],
                "arguments": vars(configurations[i][3]),
            }
            for i, launch in cases
        ],
    )
    results = []
    for index, (config_index, launch) in enumerate(cases):
        entry, size, count, config = configurations[config_index]
        destination = options.output / "cases" / f"{index:05d}"
        destination.mkdir(parents=True)
        result = suite.run_case(entry, size, count, config, destination)
        results.append(result | {"launch": launch, "configuration": config_index})
        suite.atomic_json(options.output / "manifest.json", results)
        print(
            f"{index + 1}/{len(cases)} {result['status']} {entry['name']} {config.workload} {size} q{config.queue_depth} c{config.ring_capacity}",
            flush=True,
        )
    suite.publish(options.output)
    if any(r["status"] != "passed" for r in results):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
