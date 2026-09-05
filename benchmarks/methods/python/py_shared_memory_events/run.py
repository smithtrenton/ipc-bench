"""Python shared-memory benchmark using multiprocessing events."""

from __future__ import annotations

import multiprocessing as mp
from multiprocessing import shared_memory

from benchmarks.methods.python.benchmark_adapter import (
    make_payload,
    parse_config,
    print_report,
    run_benchmark,
    stabilize_process_pair,
    update_payload,
)
from benchmarks.methods.python.runtime import (
    finish_worker,
    owned_worker,
    transform_response,
    worker_finished,
    worker_started,
)


def _worker(
    name: str,
    message_size: int,
    signals: tuple[mp.Event, mp.Event, mp.Event, mp.Event],
) -> None:
    worker_started()
    request, response, stop, ready = signals
    shm = shared_memory.SharedMemory(name=name)
    try:
        request_buffer = shm.buf[:message_size]
        response_buffer = shm.buf[message_size : message_size * 2]
        scratch = bytearray(message_size)
        ready.set()
        while True:
            request.wait()
            request.clear()
            if stop.is_set():
                worker_finished()
                return
            scratch[:] = request_buffer
            if scratch:
                transform_response(scratch)
            response_buffer[:] = scratch
            response.set()
    finally:
        del request_buffer
        del response_buffer
        shm.close()


def _main() -> None:
    config = parse_config()
    shm = shared_memory.SharedMemory(create=True, size=config.wire_size * 2)
    request = mp.Event()
    response = mp.Event()
    stop = mp.Event()
    ready = mp.Event()
    process = mp.Process(
        target=_worker,
        args=(shm.name, config.wire_size, (request, response, stop, ready)),
    )
    request_buffer = None
    response_buffer = None
    operation = None
    try:
        with owned_worker(process):
            stabilize_process_pair(process)
            if not ready.wait(5):
                message = "py-shared-memory-events worker failed to signal readiness"
                raise TimeoutError(message)

            outbound = make_payload(config.wire_size)
            inbound = bytearray(config.wire_size)
            request_buffer = shm.buf[: config.wire_size]
            response_buffer = shm.buf[config.wire_size : config.wire_size * 2]

            def operation() -> None:
                request_buffer[:] = outbound
                request.set()
                if not response.wait(5):
                    message = "shared-memory response deadline exceeded"
                    raise TimeoutError(message)
                response.clear()
                inbound[:] = response_buffer
                update_payload(outbound, inbound)

            report = run_benchmark("py-shared-memory-events", config, operation, child_ready=True)
            stop.set()
            request.set()
            report.update(finish_worker(process))
            print_report(report, config.output_format)
    finally:
        operation = None
        request_buffer = None
        response_buffer = None
        shm.close()
        shm.unlink()


if __name__ == "__main__":
    mp.freeze_support()
    _main()
