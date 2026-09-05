"""Python multiprocessing queue benchmark."""

from __future__ import annotations

import multiprocessing as mp

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
    requests: mp.Queue[bytearray | None],
    responses: mp.Queue[bytearray],
    ready: mp.Event,
) -> None:
    worker_started()
    ready.set()
    while True:
        payload = requests.get()
        if payload is None:
            worker_finished()
            return
        if payload:
            transform_response(payload)
        responses.put(payload)


def _main() -> None:
    config = parse_config()
    requests: mp.Queue[bytearray | None] = mp.Queue(maxsize=1)
    responses: mp.Queue[bytearray] = mp.Queue(maxsize=1)
    ready = mp.Event()
    process = mp.Process(target=_worker, args=(requests, responses, ready))
    try:
        with owned_worker(process):
            stabilize_process_pair(process)
            if not ready.wait(5):
                message = "py-multiprocessing-queue worker failed to signal readiness"
                raise TimeoutError(message)

            outbound = make_payload(config.wire_size)
            inbound = bytearray(config.wire_size)

            def operation() -> None:
                requests.put(outbound.copy(), timeout=5)
                inbound[:] = responses.get(timeout=5)
                update_payload(outbound, inbound)

            report = run_benchmark("py-multiprocessing-queue", config, operation, child_ready=True)
            requests.put(None, timeout=5)
            report.update(finish_worker(process))
            print_report(report, config.output_format)
    finally:
        close_queue(requests)
        close_queue(responses)


if __name__ == "__main__":
    mp.freeze_support()
    _main()
