"""Python socket TCP loopback benchmark."""

from __future__ import annotations

import multiprocessing as mp
import socket
from queue import Empty

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


def _recv_exact_into(stream: socket.socket, buffer: bytearray) -> None:
    view = memoryview(buffer)
    received = 0
    while received < len(buffer):
        chunk = stream.recv_into(view[received:])
        if chunk == 0:
            message = "socket closed"
            raise EOFError(message)
        received += chunk


def _worker(ports: mp.Queue[int], message_size: int) -> None:
    worker_started()
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as server:
        server.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        server.bind(("127.0.0.1", 0))
        server.listen(1)
        ports.put(server.getsockname()[1])
        conn, _ = server.accept()
        with conn:
            conn.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            scratch = bytearray(message_size)
            while True:
                try:
                    _recv_exact_into(conn, scratch)
                except EOFError:
                    worker_finished()
                    return
                if scratch:
                    transform_response(scratch)
                conn.sendall(scratch)


def _main() -> None:
    config = parse_config()
    ports: mp.Queue[int] = mp.Queue(maxsize=1)
    process = mp.Process(target=_worker, args=(ports, config.wire_size))
    stream = None
    try:
        with owned_worker(process):
            stabilize_process_pair(process)
            try:
                port = ports.get(timeout=5)
            except Empty as error:
                message = "py-socket-tcp-loopback worker failed to publish its port"
                raise TimeoutError(message) from error

            stream = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            stream.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            stream.settimeout(5)
            stream.connect(("127.0.0.1", port))

            outbound = make_payload(config.wire_size)
            inbound = bytearray(config.wire_size)

            def operation() -> None:
                stream.sendall(outbound)
                _recv_exact_into(stream, inbound)
                update_payload(outbound, inbound)

            report = run_benchmark("py-socket-tcp-loopback", config, operation, child_ready=True)
            stream.close()
            report.update(finish_worker(process))
            print_report(report, config.output_format)
    finally:
        if stream is not None:
            stream.close()
        close_queue(ports)


if __name__ == "__main__":
    mp.freeze_support()
    _main()
