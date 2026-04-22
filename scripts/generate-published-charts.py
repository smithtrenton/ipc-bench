from __future__ import annotations

import json
import math
from dataclasses import dataclass
from html import escape
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PUBLISHED_DIR = ROOT / "results" / "published"
CHARTS_DIR = PUBLISHED_DIR / "charts"
MESSAGE_SIZES = [64, 1024, 4096, 16384, 32704]


@dataclass(frozen=True)
class MethodStyle:
    method: str
    label: str
    color: str
    dash: str | None = None


@dataclass(frozen=True)
class ChartConfig:
    result_set: str
    metric: str
    title: str
    output_name: str
    lower_metric: str | None = None
    upper_metric: str | None = None


METHOD_STYLES = [
    MethodStyle("copy-roundtrip", "copy-roundtrip (floor)", "#6b7280", "8 5"),
    MethodStyle("shm-mailbox-spin", "shm-mailbox-spin", "#dc2626"),
    MethodStyle("shm-mailbox-hybrid", "shm-mailbox-hybrid", "#f97316"),
    MethodStyle("shm-ring-spin", "shm-ring-spin", "#2563eb"),
    MethodStyle("shm-ring-hybrid", "shm-ring-hybrid", "#0891b2"),
    MethodStyle("iceoryx2-publish-subscribe-loan", "iceoryx2-pubsub-loan", "#16a34a"),
    MethodStyle("named-pipe-message-sync", "named-pipe-message-sync", "#7c3aed"),
    MethodStyle("shm-events", "shm-events", "#a16207"),
    MethodStyle("py-multiprocessing-pipe", "py-multiprocessing-pipe", "#db2777"),
]

CHARTS = [
    ChartConfig(
        result_set="windows11-initial",
        metric="average_micros",
        title="Initial published run: selected latency curves",
        output_name="windows11-initial-headline-latency.svg",
    ),
    ChartConfig(
        result_set="windows11-initial",
        metric="average_micros",
        title="Initial published run: median latency with min/max run range",
        output_name="windows11-initial-headline-latency-range.svg",
        lower_metric="min_average_micros",
        upper_metric="max_average_micros",
    ),
    ChartConfig(
        result_set="windows11-initial",
        metric="message_rate",
        title="Initial published run: selected throughput curves",
        output_name="windows11-initial-headline-throughput.svg",
    ),
    ChartConfig(
        result_set="windows11-high-iterations",
        metric="average_micros",
        title="High-iteration run: selected latency curves",
        output_name="windows11-high-iterations-headline-latency.svg",
    ),
    ChartConfig(
        result_set="windows11-high-iterations",
        metric="average_micros",
        title="High-iteration run: median latency with min/max run range",
        output_name="windows11-high-iterations-headline-latency-range.svg",
        lower_metric="min_average_micros",
        upper_metric="max_average_micros",
    ),
    ChartConfig(
        result_set="windows11-high-iterations",
        metric="message_rate",
        title="High-iteration run: selected throughput curves",
        output_name="windows11-high-iterations-headline-throughput.svg",
    ),
]

SUMMARY_FILES = {
    "windows11-initial": PUBLISHED_DIR / "windows11-initial" / "summary.json",
    "windows11-high-iterations": PUBLISHED_DIR / "windows11-high-iterations" / "summary.json",
}


def load_summary(path: Path) -> dict[tuple[str, int], dict[str, float]]:
    rows = json.loads(path.read_text(encoding="utf-8"))
    data: dict[tuple[str, int], dict[str, float]] = {}

    for row in rows:
        if row["status"] != "completed":
            continue

        key = (row["method"], row["message_size"])
        data[key] = {
            "average_micros": float(row["average_micros"]),
            "message_rate": float(row["message_rate"]),
            "min_average_micros": float(row["min_average_micros"]),
            "max_average_micros": float(row["max_average_micros"]),
        }

    return data


def log_ticks(min_value: float, max_value: float) -> list[float]:
    ticks: list[float] = []
    min_exp = math.floor(math.log10(min_value)) - 1
    max_exp = math.ceil(math.log10(max_value)) + 1

    for exponent in range(min_exp, max_exp + 1):
        base = 10**exponent
        for multiplier in (1, 2, 5):
            tick = multiplier * base
            if min_value <= tick <= max_value:
                ticks.append(tick)

    return ticks


def padded_log_range(values: list[float]) -> tuple[float, float]:
    min_value = min(values)
    max_value = max(values)
    min_log = math.log10(min_value)
    max_log = math.log10(max_value)
    pad = 0.08 * max(max_log - min_log, 1.0)
    return 10 ** (min_log - pad), 10 ** (max_log + pad)


def map_log(value: float, domain_min: float, domain_max: float, range_min: float, range_max: float) -> float:
    position = (math.log10(value) - math.log10(domain_min)) / (math.log10(domain_max) - math.log10(domain_min))
    return range_min + position * (range_max - range_min)


def format_latency_tick(value: float) -> str:
    if value >= 10:
        return f"{value:.0f} us"
    if value >= 1:
        return f"{value:.1f} us"
    if value >= 0.1:
        return f"{value:.2f} us"
    return f"{value:.3f} us"


def format_rate_tick(value: float) -> str:
    if value >= 1_000_000:
        return f"{value / 1_000_000:.1f}M/s"
    if value >= 1_000:
        return f"{value / 1_000:.0f}K/s"
    return f"{value:.0f}/s"


def build_chart_svg(config: ChartConfig, data: dict[tuple[str, int], dict[str, float]]) -> str:
    width = 1320
    height = 760
    plot_left = 100
    plot_top = 90
    plot_right = 940
    plot_bottom = 660
    plot_width = plot_right - plot_left
    plot_height = plot_bottom - plot_top

    x_min = min(MESSAGE_SIZES)
    x_max = max(MESSAGE_SIZES)

    plotted_metrics = [config.metric]
    if config.lower_metric is not None:
        plotted_metrics.append(config.lower_metric)
    if config.upper_metric is not None:
        plotted_metrics.append(config.upper_metric)

    series_values = [
        data[(style.method, size)][metric]
        for style in METHOD_STYLES
        for size in MESSAGE_SIZES
        for metric in plotted_metrics
        if (style.method, size) in data
    ]
    y_min, y_max = padded_log_range(series_values)
    y_ticks = log_ticks(y_min, y_max)

    if config.metric == "average_micros" and config.lower_metric is None:
        y_tick_formatter = format_latency_tick
        y_axis_label = "Median launch-average latency (log scale)"
        subtitle = "Selected cross-tier methods; lower is better; copy-roundtrip is the baseline floor."
    elif config.metric == "average_micros":
        y_tick_formatter = format_latency_tick
        y_axis_label = "Launch-average latency (log scale)"
        subtitle = "Lines show the median; shaded bands span the min/max launch-average latency across fresh runs."
    else:
        y_tick_formatter = format_rate_tick
        y_axis_label = "Messages per second (log scale)"
        subtitle = "Selected cross-tier methods; higher is better; copy-roundtrip is the baseline floor."

    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
        f'viewBox="0 0 {width} {height}" role="img" aria-labelledby="title desc">',
        "<title id=\"title\">"
        + escape(config.title)
        + "</title>",
        "<desc id=\"desc\">"
        + escape(subtitle)
        + "</desc>",
        "<rect width=\"100%\" height=\"100%\" fill=\"#ffffff\" />",
        f'<text x="{plot_left}" y="42" font-family="Segoe UI, Arial, sans-serif" font-size="28" '
        'font-weight="700" fill="#111827">'
        + escape(config.title)
        + "</text>",
        f'<text x="{plot_left}" y="68" font-family="Segoe UI, Arial, sans-serif" font-size="15" fill="#4b5563">'
        + escape(subtitle)
        + "</text>",
    ]

    for tick in y_ticks:
        y = plot_bottom - map_log(tick, y_min, y_max, 0, plot_height)
        parts.append(
            f'<line x1="{plot_left}" y1="{y:.2f}" x2="{plot_right}" y2="{y:.2f}" stroke="#e5e7eb" stroke-width="1" />'
        )
        parts.append(
            f'<text x="{plot_left - 12}" y="{y + 5:.2f}" text-anchor="end" '
            'font-family="Segoe UI, Arial, sans-serif" font-size="13" fill="#374151">'
            + escape(y_tick_formatter(tick))
            + "</text>"
        )

    for size in MESSAGE_SIZES:
        x = map_log(size, x_min, x_max, plot_left, plot_right)
        parts.append(
            f'<line x1="{x:.2f}" y1="{plot_top}" x2="{x:.2f}" y2="{plot_bottom}" stroke="#f3f4f6" stroke-width="1" />'
        )
        parts.append(
            f'<text x="{x:.2f}" y="{plot_bottom + 28}" text-anchor="middle" '
            'font-family="Segoe UI, Arial, sans-serif" font-size="13" fill="#374151">'
            + escape(f"{size} B")
            + "</text>"
        )

    parts.extend(
        [
            f'<line x1="{plot_left}" y1="{plot_bottom}" x2="{plot_right}" y2="{plot_bottom}" stroke="#111827" stroke-width="1.5" />',
            f'<line x1="{plot_left}" y1="{plot_top}" x2="{plot_left}" y2="{plot_bottom}" stroke="#111827" stroke-width="1.5" />',
            f'<text x="{(plot_left + plot_right) / 2:.2f}" y="{height - 28}" text-anchor="middle" '
            'font-family="Segoe UI, Arial, sans-serif" font-size="14" fill="#374151">Payload size (bytes, log spacing)</text>',
            f'<text x="26" y="{(plot_top + plot_bottom) / 2:.2f}" text-anchor="middle" '
            'font-family="Segoe UI, Arial, sans-serif" font-size="14" fill="#374151" '
            'transform="rotate(-90 26 '
            f'{(plot_top + plot_bottom) / 2:.2f})">'
            + escape(y_axis_label)
            + "</text>",
        ]
    )

    for style in METHOD_STYLES:
        points: list[tuple[float, float]] = []
        lower_points: list[tuple[float, float]] = []
        upper_points: list[tuple[float, float]] = []
        for size in MESSAGE_SIZES:
            row = data[(style.method, size)]
            metric_value = row[config.metric]
            x = map_log(size, x_min, x_max, plot_left, plot_right)
            y = plot_bottom - map_log(metric_value, y_min, y_max, 0, plot_height)
            points.append((x, y))

            if config.lower_metric is not None and config.upper_metric is not None:
                lower_value = row[config.lower_metric]
                upper_value = row[config.upper_metric]
                lower_y = plot_bottom - map_log(lower_value, y_min, y_max, 0, plot_height)
                upper_y = plot_bottom - map_log(upper_value, y_min, y_max, 0, plot_height)
                lower_points.append((x, lower_y))
                upper_points.append((x, upper_y))

        if lower_points and upper_points:
            band_points = upper_points + list(reversed(lower_points))
            band_point_string = " ".join(f"{x:.2f},{y:.2f}" for x, y in band_points)
            parts.append(
                f'<polygon points="{band_point_string}" fill="{style.color}" fill-opacity="0.12" stroke="none" />'
            )

        point_string = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
        dash_attr = f' stroke-dasharray="{style.dash}"' if style.dash else ""
        parts.append(
            f'<polyline fill="none" stroke="{style.color}" stroke-width="3.2" stroke-linecap="round" '
            f'stroke-linejoin="round"{dash_attr} points="{point_string}" />'
        )

        for x, y in points:
            parts.append(
                f'<circle cx="{x:.2f}" cy="{y:.2f}" r="4.4" fill="#ffffff" stroke="{style.color}" stroke-width="2.2" />'
            )

    legend_x = 980
    legend_y = 110
    legend_width = 290
    legend_height = 36 + len(METHOD_STYLES) * 32
    parts.append(
        f'<rect x="{legend_x}" y="{legend_y}" width="{legend_width}" height="{legend_height}" rx="12" '
        'fill="#f9fafb" stroke="#e5e7eb" />'
    )
    parts.append(
        f'<text x="{legend_x + 18}" y="{legend_y + 26}" font-family="Segoe UI, Arial, sans-serif" '
        'font-size="16" font-weight="700" fill="#111827">Legend</text>'
    )

    for index, style in enumerate(METHOD_STYLES):
        row_y = legend_y + 52 + index * 32
        dash_attr = f' stroke-dasharray="{style.dash}"' if style.dash else ""
        parts.append(
            f'<line x1="{legend_x + 18}" y1="{row_y}" x2="{legend_x + 52}" y2="{row_y}" stroke="{style.color}" '
            f'stroke-width="3.2" stroke-linecap="round"{dash_attr} />'
        )
        parts.append(
            f'<circle cx="{legend_x + 35}" cy="{row_y}" r="4.2" fill="#ffffff" stroke="{style.color}" stroke-width="2.1" />'
        )
        parts.append(
            f'<text x="{legend_x + 64}" y="{row_y + 5}" font-family="Segoe UI, Arial, sans-serif" '
            'font-size="13.5" fill="#111827">'
            + escape(style.label)
            + "</text>"
        )

    parts.append(
        f'<text x="{plot_left}" y="{height - 8}" font-family="Segoe UI, Arial, sans-serif" font-size="12" fill="#6b7280">'
        + escape("Generated from published summary.json files by scripts/generate-published-charts.py")
        + "</text>"
    )
    parts.append("</svg>")

    return "\n".join(parts)


def main() -> None:
    CHARTS_DIR.mkdir(parents=True, exist_ok=True)
    all_data = {result_set: load_summary(path) for result_set, path in SUMMARY_FILES.items()}

    for chart in CHARTS:
        svg = build_chart_svg(chart, all_data[chart.result_set])
        output_path = CHARTS_DIR / chart.output_name
        output_path.write_text(svg, encoding="utf-8")
        print(f"wrote {output_path.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
