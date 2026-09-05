"""Bounded ownership of benchmark workers and their Windows process trees."""

from __future__ import annotations

import ctypes
import os
import sys
import threading
from contextlib import contextmanager
from typing import TYPE_CHECKING, ClassVar

if TYPE_CHECKING:
    import multiprocessing as mp
    from collections.abc import Iterator


class _BasicLimits(ctypes.Structure):
    _fields_: ClassVar = [
        ("process_time", ctypes.c_int64),
        ("job_time", ctypes.c_int64),
        ("flags", ctypes.c_uint32),
        ("minimum", ctypes.c_size_t),
        ("maximum", ctypes.c_size_t),
        ("active", ctypes.c_uint32),
        ("affinity", ctypes.c_size_t),
        ("priority", ctypes.c_uint32),
        ("scheduling", ctypes.c_uint32),
    ]


class _ExtendedLimits(ctypes.Structure):
    _fields_: ClassVar = [
        ("basic", _BasicLimits),
        ("io", ctypes.c_uint64 * 6),
        ("process_memory", ctypes.c_size_t),
        ("job_memory", ctypes.c_size_t),
        ("peak_process", ctypes.c_size_t),
        ("peak_job", ctypes.c_size_t),
    ]


def supervise(seconds: int) -> None:
    """Join a kill-on-close job before spawning; enforce the deadline off the timed thread."""
    job = None
    kernel = None
    if sys.platform == "win32":
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.CreateJobObjectW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p]
        kernel.CreateJobObjectW.restype = ctypes.c_void_p
        kernel.SetInformationJobObject.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p, ctypes.c_uint32]
        kernel.AssignProcessToJobObject.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
        kernel.GetCurrentProcess.restype = ctypes.c_void_p
        kernel.TerminateJobObject.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
        kernel.CloseHandle.argtypes = [ctypes.c_void_p]
        job = kernel.CreateJobObjectW(None, None)
        if not job:
            raise ctypes.WinError(ctypes.get_last_error())
        limits = _ExtendedLimits()
        limits.basic.flags = 0x2000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        if not kernel.SetInformationJobObject(
            job,
            9,
            ctypes.byref(limits),
            ctypes.sizeof(limits),
        ) or not kernel.AssignProcessToJobObject(job, kernel.GetCurrentProcess()):
            error = ctypes.WinError(ctypes.get_last_error())
            kernel.CloseHandle(job)
            raise error

    def expire() -> None:
        sys.stderr.write("process-tree supervisor deadline exceeded\n")
        sys.stderr.flush()
        if job and kernel:
            kernel.TerminateJobObject(job, 124)
        os._exit(124)

    timer = threading.Timer(seconds, expire)
    timer.daemon = True
    timer.start()


@contextmanager
def owned_worker(process: mp.Process) -> Iterator[mp.Process]:
    """Own startup failures and reap workers after exceptions or normal shutdown."""
    try:
        process.start()
        yield process
    finally:
        if process.pid is not None:
            if process.is_alive():
                process.terminate()
            process.join(timeout=5)
            if process.is_alive():
                process.kill()
                process.join(timeout=5)
            process.close()


def finish_worker(process: mp.Process) -> dict[str, float | int | None]:
    """Require successful termination before allowing a report to escape."""
    process.join(timeout=5)
    if process.is_alive():
        message = "worker shutdown deadline exceeded"
        raise TimeoutError(message)
    if process.exitcode != 0:
        message = f"worker exited with code {process.exitcode}"
        raise RuntimeError(message)

    if sys.platform != "win32":
        return {"child_cpu_seconds": None, "effective_child_affinity": None}
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.OpenProcess.argtypes = [ctypes.c_uint32, ctypes.c_int, ctypes.c_uint32]
    kernel.OpenProcess.restype = ctypes.c_void_p
    kernel.GetProcessTimes.argtypes = [ctypes.c_void_p] + [ctypes.POINTER(ctypes.c_uint64)] * 4
    kernel.GetProcessAffinityMask.argtypes = [ctypes.c_void_p] + [ctypes.POINTER(ctypes.c_size_t)] * 2
    kernel.CloseHandle.argtypes = [ctypes.c_void_p]
    handle = kernel.OpenProcess(0x1000, 0, process.pid)
    if not handle:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        values = [ctypes.c_uint64() for _ in range(4)]
        if not kernel.GetProcessTimes(handle, *(ctypes.byref(v) for v in values)):
            raise ctypes.WinError(ctypes.get_last_error())
        process_mask, system_mask = ctypes.c_size_t(), ctypes.c_size_t()
        if not kernel.GetProcessAffinityMask(handle, ctypes.byref(process_mask), ctypes.byref(system_mask)):
            raise ctypes.WinError(ctypes.get_last_error())
        return {
            "child_cpu_seconds": (values[2].value + values[3].value) / 10_000_000,
            "effective_child_affinity": process_mask.value,
        }
    finally:
        kernel.CloseHandle(handle)


def close_queue(queue: object) -> None:
    """Release queue handles without waiting forever on a feeder after peer death."""
    queue.cancel_join_thread()
    queue.close()


def worker_started() -> None:
    """Apply opt-in peer-death injection before readiness or during a long request run."""
    fault = os.environ.get("IPC_BENCH_TEST_FAULT")
    if fault == "before-ready":
        os._exit(42)
    if fault == "mid-request":
        timer = threading.Timer(0.15, lambda: os._exit(42))
        timer.daemon = True
        timer.start()


def worker_finished() -> None:
    """Simulate a peer that cannot finish shutdown until its owner kills the tree."""
    if os.environ.get("IPC_BENCH_TEST_FAULT") == "shutdown":
        threading.Event().wait()


def transform_response(payload: bytearray) -> None:
    """Apply the response transform and optional transport corruption fault."""
    payload[0] = (payload[0] + 1) % 256
    if os.environ.get("IPC_BENCH_TEST_FAULT") == "corrupt":
        payload[-1] ^= 1
