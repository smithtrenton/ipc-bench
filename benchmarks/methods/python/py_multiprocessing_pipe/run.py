"""Python multiprocessing pipe benchmark."""

from __future__ import annotations

import multiprocessing as mp
from typing import TYPE_CHECKING

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

if TYPE_CHECKING:
    from multiprocessing.connection import Connection


def _worker(connection: Connection, ready: mp.Event) -> None:
    worker_started()
    ready.set()
    while True:
        try:
            payload = connection.recv_bytes()
        except EOFError, BrokenPipeError:
            worker_finished()
            return
        response = bytearray(payload)
        if response:
            transform_response(response)
        connection.send_bytes(response)


def _main() -> None:
    config = parse_config()
    parent, child = mp.Pipe(duplex=True)
    ready = mp.Event()
    process = mp.Process(target=_worker, args=(child, ready))
    try:
        with owned_worker(process):
            stabilize_process_pair(process)
            child.close()
            if not ready.wait(5):
                message = "py-multiprocessing-pipe worker failed to signal readiness"
                raise TimeoutError(message)

            outbound = make_payload(config.wire_size)
            inbound = bytearray(config.wire_size)

            def operation() -> None:
                parent.send_bytes(outbound)
                response = parent.recv_bytes()
                inbound[:] = response
                update_payload(outbound, inbound)

            report = run_benchmark("py-multiprocessing-pipe", config, operation, child_ready=True)
            parent.close()
            report.update(finish_worker(process))
            print_report(report, config.output_format)
    finally:
        parent.close()
        child.close()


if __name__ == "__main__":
    mp.freeze_support()
    _main()
