"""Shared adapter helpers for Python benchmark methods."""

from __future__ import annotations

import argparse
import ctypes
import json
import math
import os
import struct
import sys
import time
from contextvars import ContextVar
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
from typing import TYPE_CHECKING, ClassVar, NoReturn

from benchmarks.methods.python.runtime import supervise

if TYPE_CHECKING:
    from collections.abc import Callable

MESSAGE_BYTE_MODULUS = 251
FIRST_BYTE_MODULUS = 256
MICROS_PER_MILLISECOND = 1_000.0
MICROS_PER_SECOND = 1_000_000.0
TARGET_BATCHES_PER_TRIAL = 100
MAX_BATCH_SIZE = 100
FRAME_SIZE = 8
VALIDATION_INTERVAL = 1024
MAX_PAYLOAD = 1024 * 1024
MAX_OPERATIONS = 1_000_000_000
MAX_SAMPLES = 1_000_000
MAX_TIMEOUT = 86400
_FULL_VALIDATION = ContextVar("full_validation", default=True)
STABLE_AFFINITY_ENV = "IPC_BENCH_STABLE_AFFINITY"
RELATION_PROCESSOR_CORE = 0
RELATION_CACHE = 2
AFFINITY_BITS = 64
PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
PROCESS_SET_INFORMATION = 0x0200
STABLE_AFFINITY_CORE_COUNT = 2


if sys.platform == "win32":

    class _SystemLogicalProcessorInformationUnion(ctypes.Union):
        _fields_: ClassVar[list[tuple[str, object]]] = [
            ("flags", ctypes.c_byte),
            ("node_number", ctypes.c_uint32),
            ("reserved", ctypes.c_ulonglong * 2),
        ]

    class _SystemLogicalProcessorInformation(ctypes.Structure):
        _fields_: ClassVar[list[tuple[str, object]]] = [
            ("processor_mask", ctypes.c_size_t),
            ("relationship", ctypes.c_int),
            ("anonymous", _SystemLogicalProcessorInformationUnion),
        ]


def _stable_affinity_enabled() -> bool:
    """Return whether stable affinity is enabled for this process."""
    if os.environ.get("IPC_BENCH_TOPOLOGY") == "unpinned" and not os.environ.get("IPC_BENCH_CPU_PAIR"):
        return False
    if os.environ.get("IPC_BENCH_CPU_PAIR") or os.environ.get("IPC_BENCH_TOPOLOGY", "unpinned") != "unpinned":
        return True
    value = os.environ.get(STABLE_AFFINITY_ENV)
    if value is None:
        return False
    return value.strip().lower() not in {"", "0", "false", "no", "off"}


def _resolve_stable_affinity_pair() -> tuple[int, int]:  # noqa: C901 - explicit topology cases
    """Resolve the requested group-0 CPUs from core/cache relationships."""
    if sys.platform != "win32":
        message = "stable affinity is only supported on Windows"
        raise OSError(message)

    topology = processor_topology()
    if any(entry["group"] != 0 for entry in topology):
        message = "controlled process affinity currently supports processor group 0 only"
        raise OSError(message)
    cores = [entry["mask"] for entry in topology if entry["kind"] == "core"]
    requested = os.environ.get("IPC_BENCH_CPU_PAIR")
    if requested:
        bits = [int(bit) for bit in requested.split(",")]
        if (
            len(bits) != STABLE_AFFINITY_CORE_COUNT
            or bits[0] == bits[1]
            or any(not 0 <= bit < AFFINITY_BITS for bit in bits)
        ):
            message = "invalid CPU pair"
            raise ValueError(message)
        masks = (1 << bits[0], 1 << bits[1])
        if any(not any(mask & core for core in cores) for mask in masks):
            message = "requested CPU pair is unavailable"
            raise ValueError(message)
        return masks
    placement = os.environ.get("IPC_BENCH_TOPOLOGY", "separate-core")
    if placement == "smt":
        for core in cores:
            first = core & -core
            rest = core & ~first
            if rest:
                return first, rest & -rest
    elif placement == "separate-cache":
        caches = [entry for entry in topology if entry["cache_level"] is not None]
        level = max(entry["cache_level"] for entry in caches)
        masks = [entry["mask"] for entry in caches if entry["cache_level"] == level]
        for second in masks[1:]:
            if not second & masks[0]:
                return masks[0] & -masks[0], second & -second
    elif placement == "separate-core":
        return _shared_cache_pair(cores, topology)
    message = "requested CPU topology is unavailable"
    raise ValueError(message)


def _shared_cache_pair(cores: list[int], topology: list[dict[str, object]]) -> tuple[int, int]:
    """Find separate physical cores that share the last-level cache."""
    caches = [entry for entry in topology if entry["cache_level"] is not None]
    level = max((entry["cache_level"] for entry in caches), default=0)
    for cache in caches:
        if cache["cache_level"] == level:
            sharing = [core & cache["mask"] for core in cores if core & cache["mask"]]
            if len(sharing) >= STABLE_AFFINITY_CORE_COUNT:
                return sharing[0] & -sharing[0], sharing[1] & -sharing[1]
    message = "separate physical cores sharing the last-level cache are unavailable"
    raise ValueError(message)


def _set_current_process_affinity(mask: int) -> None:
    """Pin the current process to a single logical processor mask."""
    if sys.platform != "win32":
        return

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.GetCurrentProcess.argtypes = []
    kernel32.GetCurrentProcess.restype = ctypes.c_void_p
    kernel32.SetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    kernel32.SetProcessAffinityMask.restype = ctypes.c_int

    handle = kernel32.GetCurrentProcess()
    if not kernel32.SetProcessAffinityMask(handle, mask):
        raise ctypes.WinError(ctypes.get_last_error())


def _set_process_affinity_by_pid(pid: int, mask: int) -> None:
    """Pin another process to a single logical processor mask."""
    if sys.platform != "win32":
        return

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.OpenProcess.argtypes = [ctypes.c_uint32, ctypes.c_int, ctypes.c_uint32]
    kernel32.OpenProcess.restype = ctypes.c_void_p
    kernel32.SetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    kernel32.SetProcessAffinityMask.restype = ctypes.c_int
    kernel32.CloseHandle.argtypes = [ctypes.c_void_p]
    kernel32.CloseHandle.restype = ctypes.c_int

    inherit_handle = 0
    handle = kernel32.OpenProcess(
        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SET_INFORMATION,
        inherit_handle,
        pid,
    )
    if not handle:
        raise ctypes.WinError(ctypes.get_last_error())

    try:
        if not kernel32.SetProcessAffinityMask(handle, mask):
            raise ctypes.WinError(ctypes.get_last_error())
    finally:
        kernel32.CloseHandle(handle)


def stabilize_process_pair(process: object) -> None:
    """Pin the current process and its child process to separate physical cores."""
    if not _stable_affinity_enabled() or sys.platform != "win32":
        return

    pid = getattr(process, "pid", None)
    if pid is None:
        message = "child process must have a PID before affinity can be applied"
        raise ValueError(message)

    parent_mask, child_mask = _resolve_stable_affinity_pair()
    _set_process_affinity_by_pid(int(pid), child_mask)
    _set_current_process_affinity(parent_mask)


@dataclass
class BenchmarkConfig:
    """Configuration shared across Python benchmark methods."""

    message_count: int = 1000
    message_size: int = 1000
    warmup_count: int = 100
    trials: int = 3
    output_format: str = "text"
    role: str = "parent"
    validation: str = "full"
    measurement: str = "batch"
    timeout_seconds: int = 120

    @property
    def wire_size(self) -> int:
        """Return payload plus explicit sequence framing."""
        return self.message_size + FRAME_SIZE

    def to_report(self) -> dict[str, object]:
        """Return a JSON-serializable representation of the configuration."""
        return {
            "message_count": self.message_count,
            "message_size": self.message_size,
            "warmup_count": self.warmup_count,
            "trials": self.trials,
            "output_format": self.output_format,
            "role": self.role,
            "validation": self.validation,
            "measurement": self.measurement,
            "timeout_seconds": self.timeout_seconds,
        }


def _raise_config_error(message: str) -> NoReturn:
    raise SystemExit(message)


def parse_config() -> BenchmarkConfig:
    """Parse command-line flags into a benchmark configuration."""
    parser = argparse.ArgumentParser(prog=Path(sys.argv[0]).stem)
    parser.add_argument("-c", "--message-count", type=int, default=1000)
    parser.add_argument("-s", "--message-size", type=int, default=1000)
    parser.add_argument("-w", "--warmup-count", type=int, default=100)
    parser.add_argument("-t", "--trials", type=int, default=3)
    parser.add_argument("--format", choices=("text", "json"), default="text")
    parser.add_argument("--role", choices=("parent", "child"), default="parent")
    parser.add_argument("--validation", choices=("full", "sampled"), default="full")
    parser.add_argument("--measurement", choices=("batch", "latency"), default="batch")
    parser.add_argument("--timeout-seconds", type=int, default=120)
    args = parser.parse_args()

    if args.message_count <= 0:
        _raise_config_error("message count must be greater than zero")
    if args.trials <= 0:
        _raise_config_error("trials must be greater than zero")
    if not 1 <= args.message_size <= MAX_PAYLOAD:
        _raise_config_error("message size must be between 1 and 1048576 payload bytes")
    if args.warmup_count < 0:
        _raise_config_error("warmup count must not be negative")

    if args.message_count * args.trials + args.warmup_count > MAX_OPERATIONS:
        _raise_config_error("total operation count exceeds 1000000000")
    batch = 1 if args.measurement == "latency" else measurement_batch_size(args.message_count)
    if math.ceil(args.message_count / batch) * args.trials > MAX_SAMPLES:
        _raise_config_error("retained sample count exceeds 1000000")
    if not 1 <= args.timeout_seconds <= MAX_TIMEOUT:
        _raise_config_error("timeout must be between 1 and 86400 seconds")
    supervise(args.timeout_seconds)
    return BenchmarkConfig(
        message_count=args.message_count,
        message_size=args.message_size,
        warmup_count=args.warmup_count,
        trials=args.trials,
        output_format=args.format,
        role=args.role,
        validation=args.validation,
        measurement=args.measurement,
        timeout_seconds=args.timeout_seconds,
    )


@lru_cache(maxsize=8)
def payload_pattern(size: int) -> bytes:
    """Compute the payload oracle independently of received data."""
    return bytes(index % MESSAGE_BYTE_MODULUS for index in range(size))


def make_payload(size: int) -> bytearray:
    """Create the deterministic payload used by benchmark rounds."""
    return bytearray(FRAME_SIZE) + bytearray(payload_pattern(size - FRAME_SIZE))


def update_payload(outbound: bytearray, inbound: bytes | bytearray) -> None:
    """Check the response against independent request state and advance the sequence."""
    if len(inbound) != len(outbound) or len(inbound) <= FRAME_SIZE:
        message = "incorrect response length"
        raise ValueError(message)
    sequence = int.from_bytes(outbound[:FRAME_SIZE], "little")
    full = _FULL_VALIDATION.get() or sequence % VALIDATION_INTERVAL == 0
    checked = len(inbound) if full else FRAME_SIZE
    if (
        inbound[0] != (outbound[0] + 1) % FIRST_BYTE_MODULUS
        or inbound[1:FRAME_SIZE] != outbound[1:FRAME_SIZE]
        or (full and inbound[FRAME_SIZE:checked] != payload_pattern(len(inbound) - FRAME_SIZE))
    ):
        message = f"corrupt/stale response at sequence={sequence}"
        raise ValueError(message)
    outbound[:] = inbound
    outbound[:FRAME_SIZE] = (sequence + 1).to_bytes(FRAME_SIZE, "little")


def measurement_batch_size(message_count: int) -> int:
    """Pick a batch size that reduces timer overhead without collapsing each trial to one sample."""
    return max(
        1,
        min(MAX_BATCH_SIZE, (message_count + TARGET_BATCHES_PER_TRIAL - 1) // TARGET_BATCHES_PER_TRIAL),
    )


def run_benchmark(  # noqa: C901, PLR0915 - phase-aware measurement orchestration
    method: str,
    config: BenchmarkConfig,
    operation: Callable[[], None],
    *,
    child_ready: bool,
) -> dict[str, object]:
    """Run warmups and timed trials for a benchmark method."""
    _FULL_VALIDATION.set(True)
    invoke = operation
    phase = "preflight"
    trial_index = 0
    iteration = 0

    def checked_operation() -> None:
        nonlocal iteration
        iteration += 1
        try:
            invoke()
        except Exception as error:
            message = f"method={method} phase={phase} trial={trial_index} iteration={iteration}: {error}"
            raise RuntimeError(message) from error

    operation = checked_operation
    operation()
    phase = "warmup"
    iteration = 0
    for _ in range(config.warmup_count):
        operation()
    calibration = time.perf_counter_ns()
    for _ in range(1000):
        time.perf_counter_ns()
        time.perf_counter_ns()
    timer_pair_micros = (time.perf_counter_ns() - calibration) / 1_000_000
    _FULL_VALIDATION.set(config.validation == "full")
    phase = "timed"

    trials: list[dict[str, float | int]] = []
    batch_size = 1 if config.measurement == "latency" else measurement_batch_size(config.message_count)
    for trial_index in range(1, config.trials + 1):
        _FULL_VALIDATION.set(config.validation == "full")
        iteration = 0
        batches = [None] * math.ceil(config.message_count / batch_size)
        sample_index = 0
        remaining = config.message_count
        while remaining > 0:
            current_batch = min(batch_size, remaining)
            if remaining <= batch_size:
                _FULL_VALIDATION.set(True)
            start = time.perf_counter_ns()
            for _ in range(current_batch):
                operation()
            elapsed_micros = (time.perf_counter_ns() - start) / MICROS_PER_MILLISECOND
            if elapsed_micros <= 0:
                message = "timer resolution insufficient for measurement batch"
                raise ValueError(message)
            batches[sample_index] = (elapsed_micros / current_batch, current_batch)
            sample_index += 1
            remaining -= current_batch

        trials.append(summarize_batches(trial_index, batches, latency=config.measurement == "latency"))

    phase = "final"
    trial_index = 0
    iteration = 0
    _FULL_VALIDATION.set(True)
    operation()
    total_messages = config.message_count * len(trials)
    total_micros = sum(float(trial["total_micros"]) for trial in trials)
    average_micros = total_micros / total_messages
    variance = (
        sum(
            config.message_count
            * (float(trial["stddev_micros"]) ** 2 + (float(trial["average_micros"]) - average_micros) ** 2)
            for trial in trials
        )
        / total_messages
    )
    summary = {
        "total_micros": total_micros,
        "average_micros": average_micros,
        "min_micros": min(trial["min_micros"] for trial in trials),
        "max_micros": max(trial["max_micros"] for trial in trials),
        "stddev_micros": variance**0.5,
        "message_rate": float("inf") if total_micros == 0 else total_messages / (total_micros / MICROS_PER_SECOND),
    }

    summary["round_trip_rate"] = summary["message_rate"]
    if not all(math.isfinite(value) for value in summary.values()):
        message = "non-finite measurement invalidates report"
        raise ValueError(message)
    return {
        "schema_version": 2,
        "workload": "round-trip",
        "queue_depth": 1,
        "wire_size": config.wire_size,
        "validation_policy": "full-payload-every-operation"
        if config.validation == "full"
        else "sequence-every-operation;full-every-1024;full-preflight-and-final;full-last-batch-each-trial",
        "sampling_unit": "individual-round-trip" if config.measurement == "latency" else "batch-average-round-trip",
        "measurement_batch_size": batch_size,
        "timed_operation_count": config.message_count * config.trials,
        "preflight_operations": 1,
        "final_check_operations": 1,
        "timer_pair_micros": timer_pair_micros,
        "effective_parent_affinity": effective_affinity(),
        "method": method,
        "child_ready": child_ready,
        "config": config.to_report(),
        "trials": trials,
        "summary": summary,
    }


def render_report(report: dict[str, object], output_format: str) -> str:
    """Render a benchmark report in either text or JSON form."""
    if output_format == "json":
        return json.dumps(report, indent=2, allow_nan=False)

    summary = report["summary"]
    config = report["config"]
    lines = [
        "============ RESULTS ================",
        f"Method:             {report['method']}",
        f"Sampling unit:      {report['sampling_unit']}",
        f"Child bootstrap:    {'ok' if report['child_ready'] else 'not used'}",
        f"Message size:       {config['message_size']}",
        f"Message count:      {config['message_count']}",
        f"Warmup count:       {config['warmup_count']}",
        f"Trial count:        {config['trials']}",
        f"Total duration:     {summary['total_micros'] / MICROS_PER_MILLISECOND:.3f}\tms",
        f"Average duration:   {summary['average_micros']:.3f}\tus",
        f"Minimum sample:     {summary['min_micros']:.3f}\tus",
        f"Maximum sample:     {summary['max_micros']:.3f}\tus",
        f"Sample stddev:      {summary['stddev_micros']:.3f}\tus",
        f"Round-trip rate:       {summary['message_rate']:.0f}\tmsg/s",
    ]
    lines.extend(
        (
            "Trial {trial_index:>2}: total {total_micros:.3f} us | avg "
            "{average_micros:.3f} us | rate {message_rate:.0f} msg/s"
        ).format(**trial)
        for trial in report["trials"]
    )
    lines.append("=====================================")
    return "\n".join(lines)


def print_report(report: dict[str, object], output_format: str) -> None:
    """Write a rendered benchmark report to standard output."""
    report["processor_topology"] = processor_topology()
    report["effective_processor_group"] = effective_processor_group()
    report["parent_cpu_seconds"] = time.process_time()
    report["cpu_time_scope"] = "process-lifetime-through-shutdown"
    sys.stdout.write(f"{render_report(report, output_format)}\n")


def effective_affinity() -> int | None:
    """Read the effective process affinity rather than recording the request."""
    if sys.platform != "win32":
        return None
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.GetCurrentProcess.restype = ctypes.c_void_p
    kernel.GetProcessAffinityMask.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(ctypes.c_size_t),
        ctypes.POINTER(ctypes.c_size_t),
    ]
    process = ctypes.c_size_t()
    system = ctypes.c_size_t()
    if not kernel.GetProcessAffinityMask(kernel.GetCurrentProcess(), ctypes.byref(process), ctypes.byref(system)):
        raise ctypes.WinError(ctypes.get_last_error())
    return process.value


def effective_processor_group() -> int | None:
    """Read the calling thread's processor group, including unpinned hosts."""
    if sys.platform != "win32":
        return None
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.GetCurrentThread.restype = ctypes.c_void_p
    kernel.GetThreadGroupAffinity.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    affinity = ctypes.create_string_buffer(16)
    if not kernel.GetThreadGroupAffinity(kernel.GetCurrentThread(), affinity):
        raise ctypes.WinError(ctypes.get_last_error())
    return struct.unpack_from("<H", affinity.raw, 8)[0]


def summarize_batches(
    trial_index: int,
    batches: list[tuple[float, int]],
    *,
    latency: bool = False,
) -> dict[str, object]:
    """Summarize retained batches with operation-weighted statistics shared with Rust."""
    total_messages = sum(count for _, count in batches)
    total_micros = sum(batch_average_micros * count for batch_average_micros, count in batches)
    average_micros = total_micros / total_messages
    min_micros = min(batch_average_micros for batch_average_micros, _ in batches)
    max_micros = max(batch_average_micros for batch_average_micros, _ in batches)
    variance = (
        sum(count * (batch_average_micros - average_micros) ** 2 for batch_average_micros, count in batches)
        / total_messages
    )
    stddev_micros = variance**0.5
    message_rate = float("inf") if total_micros == 0 else total_messages / (total_micros / MICROS_PER_SECOND)
    return {
        "trial_index": trial_index,
        "total_micros": total_micros,
        "average_micros": average_micros,
        "min_micros": min_micros,
        "max_micros": max_micros,
        "stddev_micros": stddev_micros,
        "message_rate": message_rate,
        "round_trip_rate": message_rate,
        "samples": [{"average_micros": average, "operations": count} for average, count in batches],
        "latency_percentiles_micros": [
            sorted(value for value, _ in batches)[math.ceil(p * len(batches)) - 1] for p in (0.5, 0.95, 0.99)
        ]
        if latency
        else None,
    }


def processor_topology() -> list[dict[str, object]]:
    """Read Windows x64 processor-group/core/cache relationships through the extended API."""
    if sys.platform != "win32":
        return []
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.GetLogicalProcessorInformationEx.argtypes = [ctypes.c_int, ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint32)]
    records = []
    for relation in (RELATION_PROCESSOR_CORE, RELATION_CACHE):
        length = ctypes.c_uint32()
        kernel.GetLogicalProcessorInformationEx(relation, None, ctypes.byref(length))
        if not length.value:
            raise ctypes.WinError(ctypes.get_last_error())
        buffer = ctypes.create_string_buffer(length.value)
        if not kernel.GetLogicalProcessorInformationEx(relation, buffer, ctypes.byref(length)):
            raise ctypes.WinError(ctypes.get_last_error())
        data = buffer.raw
        offset = 0
        while offset < length.value:
            _, size = struct.unpack_from("<II", data, offset)
            count_offset, mask_offset = (30, 32) if relation == 0 else (38, 40)
            if size < mask_offset or offset + size > length.value:
                message = "invalid Windows topology record"
                raise ValueError(message)
            count = struct.unpack_from("<H", data, offset + count_offset)[0]
            if mask_offset + count * 16 > size:
                message = "invalid Windows topology group count"
                raise ValueError(message)
            for index in range(count):
                mask, group = struct.unpack_from("<QH", data, offset + mask_offset + index * 16)
                records.append(
                    {
                        "kind": "core" if relation == 0 else "cache",
                        "group": group,
                        "mask": mask,
                        "cache_level": data[offset + 8] if relation == RELATION_CACHE else None,
                    },
                )
            offset += size
    return records
