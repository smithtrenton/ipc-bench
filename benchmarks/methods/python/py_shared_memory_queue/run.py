"""Python shared-memory benchmark using multiprocessing queues."""

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
    close_queue,
    finish_worker,
    owned_worker,
    transform_response,
    worker_finished,
    worker_started,
)


def _worker(
    name: str,
    message_size: int,
    requests: mp.Queue[int | None],
    responses: mp.Queue[int],
    ready: mp.Event,
) -> None:
    worker_started()
    shm = shared_memory.SharedMemory(name=name)
    try:
        request_buffer = shm.buf[:message_size]
        response_buffer = shm.buf[message_size : message_size * 2]
        scratch = bytearray(message_size)
        ready.set()
        while True:
            token = requests.get()
            if token is None:
                worker_finished()
                return
            scratch[:] = request_buffer
            if scratch:
                transform_response(scratch)
            response_buffer[:] = scratch
            responses.put(token)
    finally:
        del request_buffer
        del response_buffer
        shm.close()


def _main() -> None:
    config = parse_config()
    shm = shared_memory.SharedMemory(create=True, size=config.wire_size * 2)
    requests: mp.Queue[int | None] = mp.Queue(maxsize=1)
    responses: mp.Queue[int] = mp.Queue(maxsize=1)
    ready = mp.Event()
    process = mp.Process(
        target=_worker,
        args=(shm.name, config.wire_size, requests, responses, ready),
    )
    request_buffer = None
    response_buffer = None
    operation = None
    try:
        with owned_worker(process):
            stabilize_process_pair(process)
            if not ready.wait(5):
                message = "py-shared-memory-queue worker failed to signal readiness"
                raise TimeoutError(message)

            outbound = make_payload(config.wire_size)
            inbound = bytearray(config.wire_size)
            request_buffer = shm.buf[: config.wire_size]
            response_buffer = shm.buf[config.wire_size : config.wire_size * 2]
            sequence = 0

            def operation() -> None:
                nonlocal sequence
                sequence += 1
                request_buffer[:] = outbound
                requests.put(sequence, timeout=5)
                if responses.get(timeout=5) != sequence:
                    message = "stale shared-memory response token"
                    raise ValueError(message)
                inbound[:] = response_buffer
                update_payload(outbound, inbound)

            report = run_benchmark("py-shared-memory-queue", config, operation, child_ready=True)
            requests.put(None, timeout=5)
            report.update(finish_worker(process))
            print_report(report, config.output_format)
    finally:
        operation = None
        request_buffer = None
        response_buffer = None
        shm.close()
        shm.unlink()
        close_queue(requests)
        close_queue(responses)


if __name__ == "__main__":
    mp.freeze_support()
    _main()
