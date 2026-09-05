"""Fault injection, owned-process cleanup, and cross-process ring stress gate."""

from __future__ import annotations
import argparse
import ctypes
import json
import os
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class ProcessEntry(ctypes.Structure):
    _fields_ = [
        ("size", ctypes.c_uint32),
        ("usage", ctypes.c_uint32),
        ("pid", ctypes.c_uint32),
        ("heap", ctypes.c_size_t),
        ("module", ctypes.c_uint32),
        ("threads", ctypes.c_uint32),
        ("parent", ctypes.c_uint32),
        ("priority", ctypes.c_long),
        ("flags", ctypes.c_uint32),
        ("exe", ctypes.c_wchar * 260),
    ]


def processes():
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.CreateToolhelp32Snapshot.argtypes = [ctypes.c_uint32, ctypes.c_uint32]
    kernel.CreateToolhelp32Snapshot.restype = ctypes.c_void_p
    kernel.Process32FirstW.argtypes = [ctypes.c_void_p, ctypes.POINTER(ProcessEntry)]
    kernel.Process32NextW.argtypes = [ctypes.c_void_p, ctypes.POINTER(ProcessEntry)]
    kernel.CloseHandle.argtypes = [ctypes.c_void_p]
    snapshot = kernel.CreateToolhelp32Snapshot(2, 0)
    if snapshot == ctypes.c_void_p(-1).value:
        raise ctypes.WinError(ctypes.get_last_error())
    result = {}
    try:
        entry = ProcessEntry()
        entry.size = ctypes.sizeof(entry)
        valid = kernel.Process32FirstW(snapshot, ctypes.byref(entry))
        while valid:
            result[entry.pid] = entry.parent
            valid = kernel.Process32NextW(snapshot, ctypes.byref(entry))
    finally:
        kernel.CloseHandle(snapshot)
    return result


def descendants(root, tree):
    owned = {root}
    while True:
        expanded = owned | {pid for pid, parent in tree.items() if parent in owned}
        if expanded == owned:
            return owned
        owned = expanded


def run_owned(command, env, failure, timeout=12):
    existing = set(processes())
    process = subprocess.Popen(command, env=env, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    observed = {process.pid}
    start = time.monotonic()
    # Drain pipes in a separate thread so large retained reports cannot block exit.
    import concurrent.futures

    with concurrent.futures.ThreadPoolExecutor(max_workers=1) as executor:
        output = executor.submit(process.communicate)
        while not output.done() and time.monotonic() - start < timeout:
            observed |= descendants(process.pid, processes()) - existing
            time.sleep(0.01)
        if not output.done():
            subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"], capture_output=True, check=False)
            raise TimeoutError("owned launch exceeded outer deadline")
        stdout, stderr = output.result()
    # Job termination may still be completing when parent exit is observed.
    for _ in range(100):
        survivors = observed & processes().keys()
        if not survivors:
            break
        time.sleep(0.01)
    if survivors:
        raise AssertionError(f"surviving owned workers: {survivors}")
    if failure:
        if process.returncode == 0 or stdout.strip():
            raise AssertionError(f"fault emitted successful report: {stdout[:200]}")
    elif process.returncode:
        raise AssertionError(stderr)
    return {
        "exit_code": process.returncode,
        "elapsed_seconds": time.monotonic() - start,
        "observed_pids": sorted(observed),
        "stderr": stderr,
        "report": json.loads(stdout) if stdout.strip() else None,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("debug", "release"), default="release")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--skip-faults", action="store_true")
    parser.add_argument("--force-yield", action="store_true")
    parser.add_argument("--methods", nargs="+", help="Limit fault/stress checks to these registry methods")
    args = parser.parse_args()
    methods = json.loads((ROOT / "benchmarks/methods/registry.json").read_text())["methods"]
    results = []
    cases = []
    if not args.skip_faults:
        for method in methods:
            if method["name"] in ("copy-roundtrip", "placeholder"):
                continue
            command = (
                [sys.executable, "-m", method["module"]]
                if method["kind"] == "python"
                else [str(ROOT / "target" / args.profile / (method["name"] + ".exe"))]
            )
            if method["name"] == "named-pipe-iocp":
                command += ["--workload", "windowed", "--queue-depth", "8"]
            for fault in ("before-ready", "mid-request", "shutdown", "corrupt"):
                count = "100000000" if fault == "mid-request" else "16"
                cases.append(
                    (
                        method["name"],
                        fault,
                        command
                        + [
                            "--message-size",
                            "65",
                            "--message-count",
                            count,
                            "--warmup-count",
                            "0",
                            "--trials",
                            "1",
                            "--format",
                            "json",
                            "--timeout-seconds",
                            "3",
                        ],
                        True,
                    )
                )
    for method in ("shm-ring-spin", "shm-ring-hybrid"):
        for workload in ("streaming", "windowed"):
            for capacity in (1, 2, 64, 256):
                for depth in (1, 2, 8, 64, 256):
                    cases.append(
                        (
                            method,
                            f"{workload}-capacity{capacity}-depth{depth}",
                            [
                                str(ROOT / "target" / args.profile / (method + ".exe")),
                                "--workload",
                                workload,
                                "--queue-depth",
                                str(depth),
                                "--ring-capacity",
                                str(capacity),
                                "--message-size",
                                "65",
                                "--message-count",
                                "4097",
                                "--warmup-count",
                                "65",
                                "--trials",
                                "2",
                                "--format",
                                "json",
                                "--timeout-seconds",
                                "10",
                            ],
                            False,
                        )
                    )
    for depth in (1, 2, 8, 64, 256):
        cases.append(
            (
                "named-pipe-iocp",
                f"windowed-depth{depth}",
                [
                    str(ROOT / "target" / args.profile / "named-pipe-iocp.exe"),
                    "--workload",
                    "windowed",
                    "--queue-depth",
                    str(depth),
                    "--message-size",
                    "65",
                    "--message-count",
                    "4097",
                    "--warmup-count",
                    "65",
                    "--trials",
                    "2",
                    "--format",
                    "json",
                    "--timeout-seconds",
                    "10",
                ],
                False,
            )
        )
    if args.methods:
        cases = [case for case in cases if case[0] in args.methods]
        if not cases:
            parser.error("no cases match the selected methods")
    for method, label, command, failure in cases:
        environment = os.environ.copy()
        if failure:
            environment["IPC_BENCH_TEST_FAULT"] = label
        else:
            environment.pop("IPC_BENCH_TEST_FAULT", None)
        if args.force_yield:
            environment["IPC_BENCH_TEST_YIELD"] = "1"
            environment["IPC_BENCH_SPIN_BUDGET"] = "0"
        try:
            evidence = run_owned(command, environment, failure)
            if not failure:
                report = evidence["report"]
                assert report["timed_operation_count"] == 8194
                assert report["delivery_errors"] == 0
                assert all(
                    t["delivered_messages"] == 4097 and t["delivered_payload_bytes"] == 4097 * 65
                    for t in report["trials"]
                )
            results.append({"method": method, "case": label, "status": "passed", **evidence})
        except Exception as error:
            results.append({"method": method, "case": label, "status": "failed", "error": str(error)})
        print(f"{len(results)}/{len(cases)} {results[-1]['status']} {method} {label}", flush=True)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(results, indent=2), encoding="utf-8")
    if any(r["status"] != "passed" for r in results):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
