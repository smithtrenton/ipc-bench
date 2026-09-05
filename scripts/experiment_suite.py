"""Build isolated A/B variants, gate correctness, and interleave retained launches."""

from __future__ import annotations
import argparse
import copy
import hashlib
import json
import os
import random
import shutil
import statistics
import subprocess
import time
from pathlib import Path
import benchmark_suite as suite

METHODS = [
    "copy-roundtrip",
    "shm-mailbox-spin",
    "shm-mailbox-hybrid",
    "shm-ring-spin",
    "shm-ring-hybrid",
    "iceoryx2-publish-subscribe-loan",
    "iceoryx2-request-response-loan",
    "rpc",
]
EXPERIMENTS = {
    "padded-layout": METHODS[1:5],
    "copy-elision": METHODS,
    "borrowed-response": METHODS[1:3] + METHODS[5:7],
    "conditional-wake": ["shm-mailbox-hybrid", "shm-ring-hybrid"],
    "cached-cursors": METHODS[3:5],
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--count", type=int, default=10000)
    parser.add_argument("--launches", type=int, default=5)
    parser.add_argument("--sizes", nargs="+", type=int, default=[64, 4096, 32704])
    parser.add_argument("--profiles", nargs="*", choices=("release-thin", "release-lto", "native"), default=[])
    parser.add_argument("--seed", type=int, default=20260904)
    parser.add_argument(
        "--resume", action="store_true", help="Resume an interrupted campaign with identical native sources"
    )
    parser.add_argument(
        "--topology", choices=("smt", "separate-core", "separate-cache", "unpinned"), default="separate-core"
    )
    parser.add_argument("--spin-budget", type=int, default=256)
    arguments = parser.parse_args()
    output = arguments.output.resolve()
    output.mkdir(parents=True, exist_ok=arguments.resume)
    native_sources = {
        str(p.relative_to(suite.ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
        for p in (suite.ROOT / "benchmarks").rglob("*")
        if p.suffix in (".rs", ".toml", ".c", ".idl")
    }
    source_record = output / "native-source-hashes.json"
    if source_record.exists() and json.loads(source_record.read_text()) != native_sources:
        raise RuntimeError("native sources changed; choose a fresh campaign")
    suite.atomic_json(source_record, native_sources)
    settings = {k: v for k, v in vars(arguments).items() if k not in ("resume", "output")}
    settings_record = output / "settings.json"
    if settings_record.exists() and json.loads(settings_record.read_text()) != settings:
        raise RuntimeError("campaign settings changed; choose a fresh output directory")
    suite.atomic_json(settings_record, settings)
    if not (output / "source.zip").exists():
        suite.snapshot_sources(output)
    registry = {m["name"]: m for m in json.loads(suite.REGISTRY.read_text())["methods"]}
    variants = {"control": METHODS, **EXPERIMENTS, **{p: METHODS for p in arguments.profiles}}
    builds = json.loads((output / "builds.json").read_text()) if arguments.resume else []
    import clang

    for variant, methods in variants.items():
        destination = output / "binaries" / variant
        if any(b["variant"] == variant and b["exit_code"] == 0 for b in builds) and all(
            (destination / (m + ".exe")).exists() for m in methods
        ):
            continue
        destination.mkdir(parents=True, exist_ok=arguments.resume)
        profile = variant if variant.startswith("release-") else "release"
        command = ["cargo", "build", "--locked", "--workspace", "--profile", profile]
        if variant in EXPERIMENTS:
            command += ["--features", "support/" + variant]
        environment = os.environ.copy()
        environment["LIBCLANG_PATH"] = str(Path(clang.__file__).parent / "native")
        if variant == "native":
            environment["RUSTFLAGS"] = (environment.get("RUSTFLAGS", "") + " -C target-cpu=native").strip()
        started = time.monotonic()
        build = subprocess.run(command, cwd=suite.ROOT, env=environment, capture_output=True, text=True)
        (destination / "build.stdout.txt").write_text(build.stdout, encoding="utf-8")
        (destination / "build.stderr.txt").write_text(build.stderr, encoding="utf-8")
        builds.append(
            {
                "variant": variant,
                "command": command,
                "rustflags": environment.get("RUSTFLAGS", ""),
                "elapsed_seconds": time.monotonic() - started,
                "exit_code": build.returncode,
            }
        )
        suite.atomic_json(output / "builds.json", builds)
        if build.returncode:
            raise RuntimeError(f"build failed: {variant}")
        for method in methods:
            shutil.copy2(suite.ROOT / "target" / profile / (method + ".exe"), destination / (method + ".exe"))
        print(f"built {variant}", flush=True)
    args = argparse.Namespace(
        action="run",
        profile="release",
        warmup=1000,
        trials=3,
        timeout=120,
        validation="sampled",
        measurement="batch",
        workload="round-trip",
        queue_depth=1,
        ring_capacity=64,
        stable_affinity=arguments.topology != "unpinned",
        cpu_pair=None,
        topology=arguments.topology,
        spin_budget=arguments.spin_budget,
        mailslot_count=None,
        features=[],
    )
    suite.atomic_json(output / "metadata.json", suite.metadata(args))
    cases = [
        (variant, method, size, launch)
        for variant, methods in variants.items()
        for method in methods
        for size in arguments.sizes
        for launch in range(arguments.launches)
    ]
    random.Random(arguments.seed).shuffle(cases)
    suite.atomic_json(output / "order.json", cases)
    # Correctness is a separate untimed gate for every binary/size before performance work.
    gate_args = copy.copy(args)
    gate_args.validation, gate_args.warmup, gate_args.trials = "full", 16, 1
    for variant, methods in variants.items():
        for method in methods:
            for size in arguments.sizes:
                entry = registry[method] | {"binary": str(output / "binaries" / variant / (method + ".exe"))}
                destination = output / "gates" / f"{variant}-{method}-{size}"
                destination.mkdir(parents=True, exist_ok=arguments.resume)
                result = suite.run_case(entry, size, 256, gate_args, destination)
                if result["status"] != "passed":
                    raise RuntimeError(f"full-validation gate failed: {variant} {method} {size}: {result}")
    results = (
        json.loads((output / "manifest.json").read_text())
        if arguments.resume and (output / "manifest.json").exists()
        else []
    )
    for index, (variant, method, size, launch) in enumerate(cases):
        if index < len(results):
            continue
        entry = registry[method] | {"binary": str(output / "binaries" / variant / (method + ".exe"))}
        destination = output / "cases" / f"{index:05d}"
        destination.mkdir(parents=True, exist_ok=arguments.resume)
        result = suite.run_case(entry, size, arguments.count, args, destination)
        results.append(result | {"variant": variant, "launch": launch, "case": index})
        suite.atomic_json(output / "manifest.json", results)
        print(f"{index + 1}/{len(cases)} {result['status']} {variant} {method} {size}", flush=True)
    groups = {}
    for result in results:
        if result["status"] != "passed":
            continue
        report = json.loads((output / "cases" / f"{result['case']:05d}" / "report.json").read_text())
        key = (result["variant"], result["method"], result["message_size"])
        groups.setdefault(key, []).append(report["summary"]["average_micros"])
    comparisons = []
    for (variant, method, size), values in sorted(groups.items()):
        if variant == "control":
            continue
        control = groups[("control", method, size)]
        comparisons.append(
            {
                "variant": variant,
                "method": method,
                "message_size": size,
                "control_launches": control,
                "variant_launches": values,
                "median_change_percent": 100 * (statistics.median(values) / statistics.median(control) - 1),
                "direction": "faster-with-disjoint-launch-ranges"
                if max(values) < min(control)
                else "slower-with-disjoint-launch-ranges"
                if min(values) > max(control)
                else "overlapping-launch-ranges",
            }
        )
    suite.atomic_json(output / "comparisons.json", comparisons)
    suite.atomic_json(
        output / "binary-hashes.json",
        {
            str(p.relative_to(output)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in (output / "binaries").rglob("*.exe")
        },
    )
    suite.publish(output)
    if any(r["status"] != "passed" for r in results):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
