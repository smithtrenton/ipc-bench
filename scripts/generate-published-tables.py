from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Final

MESSAGE_SIZES: Final[list[int]] = [64, 1024, 4096, 16384, 32704]


@dataclass(frozen=True)
class MethodInfo:
    tier: str
    method: str


METHODS: Final[list[MethodInfo]] = [
    MethodInfo("Native baseline", "copy-roundtrip"),
    MethodInfo("Core native", "anon-pipe"),
    MethodInfo("Core native", "named-pipe-byte-sync"),
    MethodInfo("Core native", "named-pipe-message-sync"),
    MethodInfo("Core native", "named-pipe-overlapped"),
    MethodInfo("Core native", "tcp-loopback"),
    MethodInfo("Core native", "shm-events"),
    MethodInfo("Core native", "shm-semaphores"),
    MethodInfo("Core native", "shm-mailbox-spin"),
    MethodInfo("Core native", "shm-mailbox-hybrid"),
    MethodInfo("Core native", "shm-ring-spin"),
    MethodInfo("Core native", "shm-ring-hybrid"),
    MethodInfo("Extensions", "shm-raw-sync-event"),
    MethodInfo("Extensions", "shm-raw-sync-busy"),
    MethodInfo("Extensions", "iceoryx2-request-response-loan"),
    MethodInfo("Extensions", "iceoryx2-publish-subscribe-loan"),
    MethodInfo("Extensions", "af-unix"),
    MethodInfo("Extensions", "udp-loopback"),
    MethodInfo("Extensions", "mailslot"),
    MethodInfo("Extensions", "rpc"),
    MethodInfo("Experimental", "alpc"),
    MethodInfo("Python baselines", "py-multiprocessing-pipe"),
    MethodInfo("Python baselines", "py-multiprocessing-queue"),
    MethodInfo("Python baselines", "py-socket-tcp-loopback"),
    MethodInfo("Python baselines", "py-shared-memory-events"),
    MethodInfo("Python baselines", "py-shared-memory-queue"),
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Convert a published summary.json file into the markdown comparison table used by the docs."
    )
    parser.add_argument("summary_path", type=Path, help="Path to a published summary.json file.")
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        help="Optional output path for the generated markdown table. Defaults to stdout.",
    )
    return parser.parse_args()


def load_completed_rows(summary_path: Path) -> dict[tuple[str, int], dict[str, float]]:
    rows = json.loads(summary_path.read_text(encoding="utf-8"))
    data: dict[tuple[str, int], dict[str, float]] = {}

    for row in rows:
        if row["status"] != "completed":
            continue

        key = (row["method"], int(row["message_size"]))
        data[key] = {
            "average_micros": float(row["average_micros"]),
            "message_rate": float(row["message_rate"]),
        }

    return data


def compact_rate(value: float) -> str:
    if value >= 1_000_000:
        return f"{value / 1_000_000:.1f}M/s"
    if value >= 1_000:
        return f"{value / 1_000:.1f}K/s"
    return f"{value:.0f}/s"


def format_size(size: int) -> str:
    return f"{size} B"


def require_complete_matrix(data: dict[tuple[str, int], dict[str, float]]) -> None:
    missing: list[str] = []

    for info in METHODS:
        for size in MESSAGE_SIZES:
            if (info.method, size) not in data:
                missing.append(f"{info.method} @ {size}")

    if missing:
        details = ", ".join(missing[:10])
        if len(missing) > 10:
            details += ", ..."
        raise ValueError(f"summary is missing expected rows: {details}")


def compute_highlights(data: dict[tuple[str, int], dict[str, float]]) -> dict[str, str]:
    winning_sizes: dict[str, list[int]] = {info.method: [] for info in METHODS}

    for size in MESSAGE_SIZES:
        best_method = min(
            (
                info.method
                for info in METHODS
                if info.method != "copy-roundtrip"
            ),
            key=lambda method: data[(method, size)]["average_micros"],
        )
        winning_sizes[best_method].append(size)

    highlights: dict[str, str] = {"copy-roundtrip": "**Baseline floor**"}
    for method, sizes in winning_sizes.items():
        if method == "copy-roundtrip":
            continue
        if not sizes:
            highlights[method] = "—"
            continue
        labels = ", ".join(format_size(size) for size in sizes)
        highlights[method] = f"**Leader: {labels}**"

    return highlights


def render_table(data: dict[tuple[str, int], dict[str, float]]) -> str:
    require_complete_matrix(data)
    highlights = compute_highlights(data)

    lines = [
        "| Highlight | Tier | Method | 64 B | 1024 B | 4096 B | 16384 B | 32704 B |",
        "| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |",
    ]

    for info in METHODS:
        cells: list[str] = []
        for size in MESSAGE_SIZES:
            row = data[(info.method, size)]
            cells.append(f"{row['average_micros']:.3f} us<br>{compact_rate(row['message_rate'])}")
        lines.append(
            f"| {highlights[info.method]} | {info.tier} | `{info.method}` | " + " | ".join(cells) + " |"
        )

    return "\n".join(lines) + "\n"


def main() -> None:
    args = parse_args()
    table = render_table(load_completed_rows(args.summary_path))

    if args.output is not None:
        args.output.write_text(table, encoding="utf-8")
        print(args.output)
        return

    print(table, end="")


if __name__ == "__main__":
    main()
