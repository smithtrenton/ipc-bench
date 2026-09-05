"""Reproducible schema-v2 runner, correctness matrix, and publication generator.

The legacy published snapshots are immutable inputs to the legacy generators.
This runner creates a new directory, builds once, and never mixes stderr with JSON.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import math
import os
import platform
import random
import subprocess
import sys
import zipfile
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REGISTRY = ROOT / "benchmarks/methods/registry.json"


def atomic_json(path, data):
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(data, indent=2, allow_nan=False), encoding="utf-8")
    temporary.replace(path)


def capture(command):
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=30)
    return {
        "command": command,
        "exit_code": result.returncode,
        "stdout": result.stdout.strip(),
        "stderr": result.stderr.strip(),
    }


def metadata(args):
    return {
        "schema_version": 2,
        "started_at": datetime.now(timezone.utc).isoformat(),
        "arguments": {k: str(v) if isinstance(v, Path) else v for k, v in vars(args).items()},
        "platform": platform.platform(),
        "python": sys.version,
        "commit": capture(["git", "rev-parse", "HEAD"]),
        "dirty_state": capture(["git", "status", "--porcelain"]),
        "rustc": capture(["rustc", "-vV"]),
        "cargo": capture(["cargo", "-V"]),
        "lock_hashes": {
            name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in ("Cargo.lock", "uv.lock")
        },
        "rustflags": os.environ.get("RUSTFLAGS", ""),
        "profile": args.profile,
        "power_plan": capture(["powercfg", "/getactivescheme"]),
        "host": capture(
            [
                "powershell",
                "-NoProfile",
                "-Command",
                "Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors | ConvertTo-Json",
            ]
        ),
        "toolchain_environment": {
            k: os.environ.get(k) for k in ("VCToolsVersion", "WindowsSDKVersion", "LIBCLANG_PATH")
        },
        "rpc_c_build_records": {
            str(p.relative_to(ROOT)): p.read_text(encoding="utf-8")
            for p in (ROOT / "target" / args.profile / "build").glob("rpc-*/out/c-build-info.txt")
        },
    }


def validate_report(report, method, size, count):
    def finite(value):
        if isinstance(value, float) and not math.isfinite(value):
            raise ValueError("non-finite report value")
        if isinstance(value, dict):
            for item in value.values():
                finite(item)
        elif isinstance(value, list):
            for item in value:
                finite(item)

    finite(report)
    if report["schema_version"] != 2 or report["method"] != method:
        raise ValueError("report identity/schema mismatch")
    config = report["config"]
    if count <= 0 or config["trials"] <= 0 or size <= 0:
        raise ValueError("successful report contains an invalid count or size")
    if config["message_size"] != size or config["message_count"] != count:
        raise ValueError("report configuration mismatch")
    if report["wire_size"] != size + 8 or report["timed_operation_count"] != count * config["trials"]:
        raise ValueError("incorrect byte/operation accounting")
    if len(report["trials"]) != config["trials"]:
        raise ValueError("incorrect trial accounting")

    def equal(a, b):
        if not math.isfinite(a) or not math.isclose(a, b, rel_tol=1e-8, abs_tol=1e-8):
            raise ValueError(f"summary does not match retained measurements: {a} != {b}")

    def percentiles(samples, actual):
        if not samples:
            if actual is not None:
                raise ValueError("percentiles without samples")
            return
        if actual is None or len(actual) != 3 or any(s < 0 for s in samples):
            raise ValueError("invalid latency samples/percentiles")
        ordered = sorted(samples)
        for p, value in zip((0.5, 0.95, 0.99), actual):
            equal(value, ordered[math.ceil(p * len(ordered)) - 1])

    if report["workload"] in ("streaming", "windowed"):
        if (
            config["validation"] != "full"
            or report["validation_policy"] != "full-payload-every-delivery;exact-sequence"
        ):
            raise ValueError("throughput requires full delivery validation")
        if report["delivery_errors"] != 0 or report["queue_depth"] != config["queue_depth"]:
            raise ValueError("incorrect throughput accounting")
        for trial in report["trials"]:
            if trial["delivered_messages"] != count or trial["delivered_payload_bytes"] != count * size:
                raise ValueError("incorrect delivery count")
            elapsed = trial["elapsed_seconds"]
            if not math.isfinite(elapsed) or elapsed <= 0:
                raise ValueError("invalid throughput duration")
            equal(trial["delivered_messages_per_second"], count / elapsed)
            equal(trial["payload_bytes_per_second"], count * size / elapsed)
            percentiles(trial["latency_samples_micros"], trial["latency_percentiles_micros"])
        return
    if report["workload"] not in ("round-trip", "harness-overhead") or report["queue_depth"] != 1:
        raise ValueError("invalid round-trip workload/depth")
    latency = config["measurement"] == "latency"
    expected_batch = 1 if latency else min(100, max(1, math.ceil(count / 100)))
    expected_unit = "individual-round-trip" if latency else "batch-average-round-trip"
    if report["measurement_batch_size"] != expected_batch or report["sampling_unit"] != expected_unit:
        raise ValueError("incorrect measurement batch size or sampling unit")
    for trial in report["trials"]:
        samples = trial["samples"]
        if [s["operations"] for s in samples] != [
            min(expected_batch, count - start) for start in range(0, count, expected_batch)
        ]:
            raise ValueError("retained batches do not match their declared sampling unit")
        if sum(s["operations"] for s in samples) != count or any(
            s["operations"] <= 0 or s["average_micros"] < 0 for s in samples
        ):
            raise ValueError("invalid batch operation counts")
        total = sum(s["operations"] * s["average_micros"] for s in samples)
        equal(trial["total_micros"], total)
        average = total / count
        equal(trial["average_micros"], average)
        equal(trial["min_micros"], min(s["average_micros"] for s in samples))
        equal(trial["max_micros"], max(s["average_micros"] for s in samples))
        equal(
            trial["stddev_micros"],
            math.sqrt(sum(s["operations"] * (s["average_micros"] - average) ** 2 for s in samples) / count),
        )
        equal(trial["round_trip_rate"], count * 1e6 / total)
        equal(trial["message_rate"], trial["round_trip_rate"])
        if report["sampling_unit"] == "individual-round-trip":
            if any(s["operations"] != 1 for s in samples):
                raise ValueError("individual latency contains a batch average")
            percentiles([s["average_micros"] for s in samples], trial["latency_percentiles_micros"])
        elif trial["latency_percentiles_micros"] is not None:
            raise ValueError("batch averages cannot have request-tail percentiles")
    total = sum(t["total_micros"] for t in report["trials"])
    equal(report["summary"]["total_micros"], total)
    equal(report["summary"]["average_micros"], total / report["timed_operation_count"])
    equal(report["summary"]["round_trip_rate"], report["timed_operation_count"] * 1e6 / total)
    equal(report["summary"]["message_rate"], report["summary"]["round_trip_rate"])
    equal(report["summary"]["min_micros"], min(t["min_micros"] for t in report["trials"]))
    equal(report["summary"]["max_micros"], max(t["max_micros"] for t in report["trials"]))
    average = report["summary"]["average_micros"]
    equal(
        report["summary"]["stddev_micros"],
        math.sqrt(
            sum(t["stddev_micros"] ** 2 + (t["average_micros"] - average) ** 2 for t in report["trials"])
            / len(report["trials"])
        ),
    )


def build(profile, features=()):
    environment = os.environ.copy()
    if "LIBCLANG_PATH" not in environment:
        import clang

        environment["LIBCLANG_PATH"] = str(Path(clang.__file__).parent / "native")
    command = ["cargo", "build", "--locked", "--workspace"]
    if profile != "debug":
        command += ["--profile", profile]
    if features:
        command += ["--features", ",".join("support/" + feature for feature in features)]
    subprocess.run(command, cwd=ROOT, env=environment, check=True)


def run_case(method, size, count, args, destination, extra=(), expected_failure=False):
    warmup, trials = args.warmup, args.trials
    if method["name"] == "mailslot" and args.action == "run" and args.mailslot_count is not None:
        count, warmup, trials = args.mailslot_count, args.mailslot_warmup, args.mailslot_trials
    command = (
        [sys.executable, "-m", method["module"]]
        if method["kind"] == "python"
        else [method.get("binary", str(ROOT / "target" / args.profile / (method["name"] + ".exe")))]
    )
    command += [
        "--message-size",
        str(size),
        "--message-count",
        str(count),
        "--warmup-count",
        str(warmup),
        "--trials",
        str(trials),
        "--format",
        "json",
        "--timeout-seconds",
        str(args.timeout),
        "--validation",
        args.validation,
        "--measurement",
        args.measurement,
        *extra,
    ]
    workload = method["workloads"][0] if args.action == "smoke" else args.workload
    if method["kind"] == "native":
        command += [
            "--workload",
            workload,
            "--queue-depth",
            str(args.queue_depth),
            "--ring-capacity",
            str(args.ring_capacity),
        ]
    environment = os.environ.copy()
    environment["IPC_BENCH_STABLE_AFFINITY"] = "1" if args.stable_affinity else "0"
    environment.pop("IPC_BENCH_TEST_FAULT", None)
    environment.pop("IPC_BENCH_TEST_YIELD", None)
    environment.pop("IPC_BENCH_CPU_PAIR", None)
    environment.pop("IPC_BENCH_TOPOLOGY", None)
    environment["IPC_BENCH_SPIN_BUDGET"] = str(getattr(args, "spin_budget", 256))
    if args.cpu_pair:
        environment["IPC_BENCH_CPU_PAIR"] = args.cpu_pair
    elif args.topology:
        environment["IPC_BENCH_TOPOLOGY"] = args.topology
    started = datetime.now(timezone.utc).isoformat()
    result = {
        "method": method["name"],
        "message_size": size,
        "command": command,
        "started_at": started,
        "expected_failure": expected_failure,
        "status": "failed",
        "spin_budget": int(environment["IPC_BENCH_SPIN_BUDGET"]),
    }
    process = None
    stdout, stderr = "", ""
    try:
        result["executable_sha256"] = hashlib.sha256(Path(command[0]).read_bytes()).hexdigest()
        process = subprocess.Popen(
            command, cwd=ROOT, env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True
        )
        stdout, stderr = process.communicate(timeout=args.timeout + 10)
        (destination / "stdout.txt").write_text(stdout, encoding="utf-8")
        (destination / "stderr.txt").write_text(stderr, encoding="utf-8")
        result["exit_code"] = process.returncode
        if expected_failure:
            if process.returncode == 0 or stdout.strip():
                raise ValueError("invalid input produced a successful report")
        else:
            if process.returncode:
                raise RuntimeError(f"exit {process.returncode}: {stderr.strip()}")
            report = json.loads(stdout)
            validate_report(report, method["name"], size, count)
            if any(
                report["config"][key] != value
                for key, value in (
                    ("warmup_count", warmup),
                    ("trials", trials),
                    ("validation", args.validation),
                    ("measurement", args.measurement),
                )
            ):
                raise ValueError("report does not match requested measurement controls")
            if method["name"] != "placeholder" and report["workload"] != workload:
                raise ValueError("report workload mismatch")
            atomic_json(destination / "report.json", report)
        result["status"] = "passed"
    except Exception as error:
        result["error"] = str(error)
        if process is not None and process.poll() is None:
            subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"], capture_output=True, check=False)
            try:
                stdout, stderr = process.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                result["cleanup_error"] = "process output did not close after tree termination"
    (destination / "stdout.txt").write_text(stdout, encoding="utf-8")
    (destination / "stderr.txt").write_text(stderr, encoding="utf-8")
    result["finished_at"] = datetime.now(timezone.utc).isoformat()
    atomic_json(destination / "result.json", result)
    return result


def publish(directory):
    """Regenerate summaries and a comparison table solely from validated retained reports."""
    from publication import publish_results

    publish_results(directory, validate_report, atomic_json)


def snapshot_sources(output):
    sources = [
        p
        for base in (ROOT / "benchmarks", ROOT / "scripts", ROOT / "tests")
        for p in base.rglob("*")
        if p.suffix in (".rs", ".py", ".toml", ".json", ".c", ".idl", ".ps1") and "__pycache__" not in p.parts
    ]
    sources += [
        ROOT / name
        for name in ("Cargo.toml", "Cargo.lock", "pyproject.toml", "uv.lock", "rust-toolchain.toml", ".python-version")
    ]
    with zipfile.ZipFile(output / "source.zip", "w", zipfile.ZIP_DEFLATED) as archive:
        for source in sources:
            archive.write(source, source.relative_to(ROOT))
    atomic_json(
        output / "source-hashes.json",
        {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sources},
    )


def calibrate(method, size, count, args, destination, seconds):
    """Use a retained fresh-process pilot to choose a fixed count for all launches."""
    pilot = copy.copy(args)
    pilot.trials, pilot.validation = 1, "full"
    destination.mkdir(parents=True)
    result = run_case(method, size, count, pilot, destination)
    if result["status"] != "passed":
        raise RuntimeError(f"calibration/full-validation gate failed: {result}")
    report = json.loads((destination / "report.json").read_text())
    if args.validation != "full" and seconds > 0:
        # Full validation is the correctness gate, but its cost cannot size a sampled run.
        pilot.validation = args.validation
        timing = destination / "timing-pilot"
        timing.mkdir()
        result = run_case(method, size, count, pilot, timing)
        if result["status"] != "passed":
            raise RuntimeError(f"measurement-policy timing pilot failed: {result}")
        report = json.loads((timing / "report.json").read_text())
    elapsed = report["summary"]["total_micros"] / 1e6 if "summary" in report else report["trials"][0]["elapsed_seconds"]
    limit = (1_000_000 if args.measurement == "latency" else 100_000_000) // args.trials
    calibrated = min(limit, max(count, math.ceil(count * seconds / elapsed * 1.2)))
    atomic_json(
        destination / "calibration.json",
        {
            "pilot_seconds": elapsed,
            "timing_validation": pilot.validation,
            "requested_trial_seconds": seconds,
            "selected_count": calibrated,
            "retention_limit_reached": calibrated == limit,
        },
    )
    return calibrated


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("run", "smoke", "publish"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--profile", choices=("debug", "release", "release-thin", "release-lto"), default="release")
    parser.add_argument("--sizes", type=int, nargs="+", default=[64, 1024, 4096, 16384, 32704])
    parser.add_argument("--count", type=int, default=1000)
    parser.add_argument(
        "--min-trial-seconds", type=float, default=0, help="Retain a pilot and increase count to target this duration"
    )
    parser.add_argument("--warmup", type=int, default=100)
    parser.add_argument("--trials", type=int, default=3)
    parser.add_argument("--launches", type=int, default=5)
    parser.add_argument("--seed", type=int, default=20260904)
    parser.add_argument("--timeout", type=int, default=120)
    parser.add_argument("--validation", choices=("full", "sampled"), default="full")
    parser.add_argument("--measurement", choices=("batch", "latency"), default="batch")
    parser.add_argument("--stable-affinity", action="store_true")
    parser.add_argument("--skip-python", action="store_true")
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--methods", nargs="+")
    parser.add_argument(
        "--features",
        nargs="*",
        choices=("padded-layout", "copy-elision", "borrowed-response", "conditional-wake", "cached-cursors"),
        default=[],
    )
    parser.add_argument("--workload", choices=("round-trip", "streaming", "windowed"), default="round-trip")
    parser.add_argument("--queue-depth", type=int, default=1)
    parser.add_argument("--ring-capacity", type=int, default=64)
    parser.add_argument("--spin-budget", type=int, default=256)
    parser.add_argument("--cpu-pair", help="Explicit group-0 logical CPUs, e.g. 0,2")
    parser.add_argument("--topology", choices=("smt", "separate-core", "separate-cache", "unpinned"))
    parser.add_argument("--mailslot-count", type=int)
    parser.add_argument("--mailslot-warmup", type=int, default=200)
    parser.add_argument("--mailslot-trials", type=int, default=5)
    args = parser.parse_args()
    if args.action == "publish":
        publish(args.output)
        return
    if args.output.exists():
        parser.error("result directory already exists; choose a fresh series directory")
    if min(args.count, args.trials, args.launches, args.timeout) <= 0 or args.warmup < 0:
        parser.error("counts, trials, launches and timeout must be positive; warmup must be nonnegative")
    if not math.isfinite(args.min_trial_seconds) or args.min_trial_seconds < 0:
        parser.error("minimum trial duration must be finite and nonnegative")
    if not 0 <= args.spin_budget <= 1_000_000:
        parser.error("spin budget must be between zero and 1000000")
    methods = json.loads(REGISTRY.read_text(encoding="utf-8"))["methods"]
    if args.skip_python:
        methods = [m for m in methods if m["kind"] != "python"]
    if args.methods:
        unknown = set(args.methods) - {m["name"] for m in methods}
        if unknown:
            parser.error(f"unknown methods: {sorted(unknown)}")
        methods = [m for m in methods if m["name"] in args.methods]
    if args.action == "run":
        methods = [m for m in methods if args.workload in m["workloads"]]
        if not methods:
            parser.error("no selected method supports the requested workload")
    else:
        args.warmup, args.trials, args.count, args.validation = 2, 1, 16, "full"
    if not args.skip_build:
        build(args.profile, args.features)
    args.output.mkdir(parents=True)
    atomic_json(args.output / "metadata.json", metadata(args))
    snapshot_sources(args.output)
    cases = []
    if args.action == "smoke":
        args.warmup, args.trials, args.count, args.validation = 2, 1, 16, "full"
        for method in methods:
            for size in [1, 2, 63, 64, 65, 4095, 4096, 4097, method["max_payload"]]:
                cases.append((method, size, 16, [], False))
            for size in [0, method["max_payload"] + 1, 2**64 - 1]:
                cases.append((method, size, 16, [], True))
            cases.append((method, 64, 0, [], True))
            cases.append((method, 64, 16, ["--trials", "0"], True))
    else:
        counts = {}
        for method in methods:
            if method["name"] == "placeholder":
                continue
            for size in args.sizes:
                if not 1 <= size <= method["max_payload"]:
                    parser.error(f"unsupported payload for {method['name']}: {size}")
                count = args.count
                if args.validation == "sampled" or args.min_trial_seconds > 0:
                    count = calibrate(
                        method,
                        size,
                        count,
                        args,
                        args.output / "gates" / f"{method['name']}-{size}",
                        args.min_trial_seconds,
                    )
                counts[method["name"], size] = count
        for _ in range(args.launches):
            for method in methods:
                if method["name"] == "placeholder":
                    continue
                for size in args.sizes:
                    if not 1 <= size <= method["max_payload"]:
                        parser.error(f"unsupported payload for {method['name']}: {size}")
                    cases.append((method, size, counts[method["name"], size], [], False))
    random.Random(args.seed).shuffle(cases)
    atomic_json(
        args.output / "order.json",
        [{"method": m["name"], "size": s, "count": c, "extra": e, "expected_failure": f} for m, s, c, e, f in cases],
    )
    results = []
    for index, (method, size, count, extra, expected_failure) in enumerate(cases):
        destination = args.output / "cases" / f"{index:05d}"
        destination.mkdir(parents=True)
        result = run_case(method, size, count, args, destination, extra, expected_failure)
        results.append(result)
        print(
            f"{index + 1}/{len(cases)} {result['status']} {method['name']} size={size}"
            + (f" {result['error']}" if "error" in result else ""),
            flush=True,
        )
        atomic_json(args.output / "manifest.json", results)
    publish(args.output)
    if any(r["status"] != "passed" for r in results):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
