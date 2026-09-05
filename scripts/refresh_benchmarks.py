"""Rebuild and rerun every published schema-2 performance campaign sequentially."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

import benchmark_suite as suite


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="Fresh directory for raw campaigns and logs")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    python = sys.executable
    scripts = suite.ROOT / "scripts"
    stages = [
        ("build", ["cargo", "build", "--locked", "--release", "--workspace"]),
        (
            "round-trip",
            [
                python,
                str(scripts / "benchmark_suite.py"),
                "run",
                "--skip-build",
                "--output",
                str(output / "round-trip"),
                "--sizes",
                "64",
                "1024",
                "4096",
                "16384",
                "32704",
                "--count",
                "10000",
                "--warmup",
                "1000",
                "--trials",
                "3",
                "--launches",
                "5",
                "--min-trial-seconds",
                "0.1",
                "--validation",
                "sampled",
                "--stable-affinity",
            ],
        ),
        (
            "latency",
            [
                python,
                str(scripts / "benchmark_suite.py"),
                "run",
                "--skip-build",
                "--output",
                str(output / "latency"),
                "--measurement",
                "latency",
                "--sizes",
                "64",
                "65536",
                "--count",
                "10000",
                "--warmup",
                "1000",
                "--trials",
                "1",
                "--launches",
                "5",
                "--stable-affinity",
                "--methods",
                "shm-ring-spin",
                "named-pipe-overlapped",
                "py-multiprocessing-pipe",
            ],
        ),
    ]
    for name, methods, sizes, depths, capacity, seconds in (
        ("throughput-rings", ["shm-ring-spin", "shm-ring-hybrid"], [64, 65536], [1, 2, 8, 64, 256], 64, 1),
        ("throughput-iocp", ["named-pipe-iocp"], [64, 65536], [1, 2, 8, 64, 256], 64, 1),
        ("capacity-large-payload", ["shm-ring-spin", "shm-ring-hybrid"], [65536, 1048576], [1, 8, 256], 8, 0.5),
    ):
        stages.append(
            (
                name,
                [
                    python,
                    str(scripts / "throughput_suite.py"),
                    "--skip-build",
                    "--output",
                    str(output / name),
                    "--seconds",
                    str(seconds),
                    "--trials",
                    "1",
                    "--launches",
                    "5",
                    "--capacities",
                    str(capacity),
                    "--sizes",
                    *map(str, sizes),
                    "--depths",
                    *map(str, depths),
                    "--methods",
                    *methods,
                ],
            )
        )
    # Variants rebuild target/release, so run them after all default-binary campaigns.
    stages += [
        (
            "experiments",
            [
                python,
                str(scripts / "experiment_suite.py"),
                "--output",
                str(output / "experiments"),
                "--count",
                "10000",
                "--launches",
                "5",
                "--sizes",
                "64",
                "4096",
                "32704",
                "--profiles",
                "release-thin",
                "release-lto",
                "native",
            ],
        ),
        ("restore-default-build", ["cargo", "build", "--locked", "--release", "--workspace"]),
    ]
    suite.atomic_json(output / "commands.json", [{"stage": name, "command": command} for name, command in stages])
    import clang

    environment = os.environ.copy()
    environment["LIBCLANG_PATH"] = str(Path(clang.__file__).parent / "native")
    completed = []
    for name, command in stages:
        print(f"Starting {name}; log: {output / (name + '.log')}", flush=True)
        with (output / (name + ".log")).open("w", encoding="utf-8") as log:
            result = subprocess.run(command, cwd=suite.ROOT, env=environment, stdout=log, stderr=subprocess.STDOUT)
        completed.append({"stage": name, "exit_code": result.returncode})
        suite.atomic_json(output / "stages.json", completed)
        if result.returncode:
            raise SystemExit(f"{name} failed; see its retained log")
        if name not in ("build", "restore-default-build"):
            manifest = json.loads((output / name / "manifest.json").read_text())
            if not manifest or any(case["status"] != "passed" for case in manifest):
                raise SystemExit(f"{name} contains failed or missing measurements")
        print(f"Completed {name}", flush=True)


if __name__ == "__main__":
    main()
