"""Shared adapter helpers for Python benchmark methods."""

from __future__ import annotations

import argparse
import ctypes
import json
import os
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING, ClassVar, NoReturn

if TYPE_CHECKING:
    from collections.abc import Callable

MESSAGE_BYTE_MODULUS = 251
FIRST_BYTE_MODULUS = 256
MICROS_PER_MILLISECOND = 1_000.0
MICROS_PER_SECOND = 1_000_000.0
TARGET_BATCHES_PER_TRIAL = 100
MAX_BATCH_SIZE = 100
STABLE_AFFINITY_ENV = "IPC_BENCH_STABLE_AFFINITY"
RELATION_PROCESSOR_CORE = 0
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
    value = os.environ.get(STABLE_AFFINITY_ENV)
    if value is None:
        return False
    return value.strip().lower() not in {"", "0", "false", "no", "off"}


def _resolve_stable_affinity_pair() -> tuple[int, int]:
    """Pick one logical processor from each of the first two physical CPU cores."""
    if sys.platform != "win32":
        message = "stable affinity is only supported on Windows"
        raise OSError(message)

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    returned_length = ctypes.c_uint32(0)
    kernel32.GetLogicalProcessorInformation.argtypes = [
        ctypes.POINTER(_SystemLogicalProcessorInformation),
        ctypes.POINTER(ctypes.c_uint32),
    ]
    kernel32.GetLogicalProcessorInformation.restype = ctypes.c_int

    kernel32.GetLogicalProcessorInformation(None, ctypes.byref(returned_length))
    if returned_length.value == 0:
        raise ctypes.WinError(ctypes.get_last_error())

    entry_size = ctypes.sizeof(_SystemLogicalProcessorInformation)
    entry_count = returned_length.value // entry_size
    buffer = (_SystemLogicalProcessorInformation * entry_count)()
    if not kernel32.GetLogicalProcessorInformation(buffer, ctypes.byref(returned_length)):
        raise ctypes.WinError(ctypes.get_last_error())

    core_masks = [
        int(entry.processor_mask)
        for entry in buffer[: returned_length.value // entry_size]
        if entry.relationship == RELATION_PROCESSOR_CORE and entry.processor_mask
    ]
    if len(core_masks) < STABLE_AFFINITY_CORE_COUNT:
        message = "stable affinity requires at least two physical CPU cores with addressable logical processors"
        raise OSError(message)

    return (
        core_masks[0] & -core_masks[0],
        core_masks[1] & -core_masks[1],
    )


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

    def to_report(self) -> dict[str, object]:
        """Return a JSON-serializable representation of the configuration."""
        return {
            "message_count": self.message_count,
            "message_size": self.message_size,
            "warmup_count": self.warmup_count,
            "trials": self.trials,
            "output_format": self.output_format,
            "role": self.role,
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
    args = parser.parse_args()

    if args.message_count <= 0:
        _raise_config_error("message count must be greater than zero")
    if args.trials <= 0:
        _raise_config_error("trials must be greater than zero")
    if args.message_size < 0:
        _raise_config_error("message size must not be negative")
    if args.warmup_count < 0:
        _raise_config_error("warmup count must not be negative")

    return BenchmarkConfig(
        message_count=args.message_count,
        message_size=args.message_size,
        warmup_count=args.warmup_count,
        trials=args.trials,
        output_format=args.format,
        role=args.role,
    )


def make_payload(size: int) -> bytearray:
    """Create the deterministic payload used by benchmark rounds."""
    return bytearray(index % MESSAGE_BYTE_MODULUS for index in range(size))


def update_payload(outbound: bytearray, inbound: bytes | bytearray) -> None:
    """Update the outbound payload using the most recent response bytes."""
    if not outbound:
        return
    outbound[:] = inbound
    outbound[0] = (outbound[0] + 1) % FIRST_BYTE_MODULUS


def measurement_batch_size(message_count: int) -> int:
    """Pick a batch size that reduces timer overhead without collapsing each trial to one sample."""
    return max(
        1,
        min(MAX_BATCH_SIZE, (message_count + TARGET_BATCHES_PER_TRIAL - 1) // TARGET_BATCHES_PER_TRIAL),
    )


def run_benchmark(
    method: str,
    config: BenchmarkConfig,
    operation: Callable[[], None],
    *,
    child_ready: bool,
) -> dict[str, object]:
    """Run warmups and timed trials for a benchmark method."""
    for _ in range(config.warmup_count):
        operation()

    trials: list[dict[str, float | int]] = []
    batch_size = measurement_batch_size(config.message_count)
    for trial_index in range(1, config.trials + 1):
        batches: list[tuple[float, int]] = []
        remaining = config.message_count
        while remaining > 0:
            current_batch = min(batch_size, remaining)
            start = time.perf_counter_ns()
            for _ in range(current_batch):
                operation()
            elapsed_micros = (time.perf_counter_ns() - start) / MICROS_PER_MILLISECOND
            batches.append((elapsed_micros / current_batch, current_batch))
            remaining -= current_batch

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
        trials.append(
            {
                "trial_index": trial_index,
                "total_micros": total_micros,
                "average_micros": average_micros,
                "min_micros": min_micros,
                "max_micros": max_micros,
                "stddev_micros": stddev_micros,
                "message_rate": message_rate,
            },
        )

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

    return {
        "method": method,
        "child_ready": child_ready,
        "config": config.to_report(),
        "trials": trials,
        "summary": summary,
    }


def render_report(report: dict[str, object], output_format: str) -> str:
    """Render a benchmark report in either text or JSON form."""
    if output_format == "json":
        return json.dumps(report, indent=2)

    summary = report["summary"]
    config = report["config"]
    lines = [
        "============ RESULTS ================",
        f"Method:             {report['method']}",
        f"Child bootstrap:    {'ok' if report['child_ready'] else 'not used'}",
        f"Message size:       {config['message_size']}",
        f"Message count:      {config['message_count']}",
        f"Warmup count:       {config['warmup_count']}",
        f"Trial count:        {config['trials']}",
        f"Total duration:     {summary['total_micros'] / MICROS_PER_MILLISECOND:.3f}\tms",
        f"Average duration:   {summary['average_micros']:.3f}\tus",
        f"Minimum duration:   {summary['min_micros']:.3f}\tus",
        f"Maximum duration:   {summary['max_micros']:.3f}\tus",
        f"Standard deviation: {summary['stddev_micros']:.3f}\tus",
        f"Message rate:       {summary['message_rate']:.0f}\tmsg/s",
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
    sys.stdout.write(f"{render_report(report, output_format)}\n")
