"""Deterministic schema-v2 tables and SVG plots from validated raw measurements."""

from __future__ import annotations

import hashlib
import html
import json
import math
import statistics


def distribution(values):
    values = sorted(values)
    return {
        "values": values,
        "median": statistics.median(values),
        "p10": values[math.ceil(len(values) * 0.1) - 1],
        "p90": values[math.ceil(len(values) * 0.9) - 1],
    }


def publish_results(directory, validate, write_json):
    groups = {}
    manifest_path = directory / "manifest.json"
    manifest = json.loads(manifest_path.read_text()) if manifest_path.exists() else []
    variants = {entry.get("case", index): entry.get("variant") for index, entry in enumerate(manifest)}
    for source in sorted(directory.glob("cases/*/report.json")):
        report = json.loads(source.read_text(encoding="utf-8"))
        config = report["config"]
        validate(report, report["method"], config["message_size"], config["message_count"])
        result = json.loads(source.with_name("result.json").read_text())
        if result["status"] != "passed" or result.get("expected_failure"):
            raise ValueError(f"report is not a successful case: {source}")
        identity = {
            key: report.get(key)
            for key in (
                "method",
                "workload",
                "queue_depth",
                "ring_capacity",
                "validation_policy",
                "sampling_unit",
                "measurement_batch_size",
                "build_features",
                "effective_parent_affinity",
                "effective_child_affinity",
                "effective_processor_group",
            )
        }
        identity["config"] = {k: v for k, v in config.items() if k not in ("output_format", "role", "timeout_seconds")}
        identity["executable_sha256"] = result["executable_sha256"]
        identity["spin_budget"] = result.get("spin_budget", 256)
        identity["variant"] = variants.get(int(source.parent.name)) if source.parent.name.isdigit() else None
        key = json.dumps(identity, sort_keys=True)
        groups.setdefault(key, []).append((report, str(source.relative_to(directory))))
    rows = []
    for key, pairs in sorted(groups.items()):
        identity = json.loads(key)
        reports = [p[0] for p in pairs]
        throughput = reports[0]["workload"] in ("streaming", "windowed")
        elapsed = [
            sum(t["elapsed_seconds"] for t in r["trials"]) if throughput else r["summary"]["total_micros"] / 1e6
            for r in reports
        ]
        rates = [r["timed_operation_count"] / seconds for r, seconds in zip(reports, elapsed)]
        latencies = [
            p
            for r in reports
            for t in r["trials"]
            for p in (
                t.get("latency_samples_micros", [])
                if throughput
                else [s["average_micros"] for s in t["samples"]]
                if r["sampling_unit"] == "individual-round-trip"
                else []
            )
        ]
        cpu = [sum(r.get(field) or 0 for field in ("parent_cpu_seconds", "child_cpu_seconds")) for r in reports]
        row = {
            "group_id": hashlib.sha256(key.encode()).hexdigest()[:12],
            "identity": identity,
            "sources": [p[1] for p in pairs],
            "launches": len(reports),
            "rate_unit": "validated-deliveries/s" if throughput else "round-trips/s",
            "launch_rate": distribution(rates),
            "launch_process_cpu_seconds": distribution(cpu),
            "launch_timed_seconds": distribution(elapsed),
            "cpu_time_scope": "process-lifetime-through-shutdown;includes-startup-and-warmup",
            "minimum_trial_seconds": min(
                t["elapsed_seconds"] if throughput else t["total_micros"] / 1e6 for r in reports for t in r["trials"]
            ),
            "latency_sampling_policy": reports[0].get("latency_sampling_policy", reports[0].get("sampling_unit")),
        }
        if throughput:
            row["launch_payload_bytes_per_second"] = distribution(
                [v * identity["config"]["message_size"] for v in rates]
            )
            row["byte_count_direction"] = reports[0]["byte_count_direction"]
        else:
            row["launch_average_micros"] = distribution([r["summary"]["average_micros"] for r in reports])
            row["maximum_estimated_timer_fraction"] = max(
                r["timer_pair_micros"] * sum(len(t["samples"]) for t in r["trials"]) / r["summary"]["total_micros"]
                for r in reports
            )
        if latencies:
            ordered = sorted(latencies)
            row["pooled_observed_latency_percentiles_micros"] = [
                ordered[math.ceil(p * len(ordered)) - 1] for p in (0.5, 0.95, 0.99)
            ]
        rows.append(row)
    write_json(directory / "summary-v2.json", rows)
    write_json(directory / "throughput-v2.json", [r for r in rows if r["rate_unit"] == "validated-deliveries/s"])
    _experiment_comparisons(directory, rows, write_json)
    lines = [
        "# Retained schema-v2 measurements",
        "",
        "Each group has identical configuration, executable, features and effective affinity. Group IDs link to exact inputs in `summary-v2.json`.",
        "",
        "Ranges are launch p10-p90, not request-tail latency. CPU seconds include startup, warmup and shutdown. No universal transport winner is inferred.",
        "",
        "| Group | Method / workload | Payload | Depth / capacity | Launches | Rate/s (p10-p90) | RTT us | Observed p99 us | CPU seconds | Min trial s |",
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for row in rows:
        item, rate = row["identity"], row["launch_rate"]
        rtt = f"{row['launch_average_micros']['median']:.3f}" if "launch_average_micros" in row else "n/a"
        tails = row.get("pooled_observed_latency_percentiles_micros")
        p99 = f"{tails[2]:.3f}" if tails else "n/a"
        lines.append(
            f"| {row['group_id']} | {item['method']} / {item['workload']} | {item['config']['message_size']} | {item['queue_depth']} / {item.get('ring_capacity') or '-'} | {row['launches']} | {rate['median']:.0f} ({rate['p10']:.0f}-{rate['p90']:.0f}) | {rtt} | {p99} | {row['launch_process_cpu_seconds']['median']:.3f} | {row['minimum_trial_seconds']:.3f} |"
        )
    (directory / "comparison.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    _rate_chart(directory / "comparison.svg", rows)
    _saturation_chart(directory / "saturation.svg", rows)


def _experiment_comparisons(directory, rows, write_json):
    def controls(row):
        item = row["identity"]
        return json.dumps(
            {k: v for k, v in item.items() if k not in ("variant", "build_features", "executable_sha256")},
            sort_keys=True,
        )

    baselines = {controls(r): r for r in rows if r["identity"]["variant"] == "control"}
    if not baselines:
        return
    comparisons = []
    for row in rows:
        item = row["identity"]
        if item["variant"] in (None, "control"):
            continue
        baseline = baselines[controls(row)]
        original, changed = baseline["launch_average_micros"]["values"], row["launch_average_micros"]["values"]
        comparisons.append(
            {
                "variant": item["variant"],
                "method": item["method"],
                "message_size": item["config"]["message_size"],
                "control_launches": original,
                "variant_launches": changed,
                "median_change_percent": 100 * (statistics.median(changed) / statistics.median(original) - 1),
                "direction": "faster-with-disjoint-launch-ranges"
                if max(changed) < min(original)
                else "slower-with-disjoint-launch-ranges"
                if min(changed) > max(original)
                else "overlapping-launch-ranges",
            }
        )
    write_json(
        directory / "comparisons.json",
        sorted(comparisons, key=lambda r: (r["variant"], r["method"], r["message_size"])),
    )


def _rate_chart(path, rows):
    height = 100 + len(rows) * 28
    svg = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="1100" height="{height}" viewBox="0 0 1100 {height}">',
        '<rect width="100%" height="100%" fill="white"/>',
        '<g font-family="sans-serif" font-size="12" fill="#172b4d">',
        '<text x="20" y="25" font-size="18">Measured rate: median and launch p10-p90 (log scale)</text>',
        '<text x="20" y="46">Round trips and validated deliveries are separate workloads. See the table for CPU cost and latency.</text>',
    ]
    maximum = max([r["launch_rate"]["p90"] for r in rows] + [10])

    def scale(value):
        return 620 + 420 * math.log10(max(1, value)) / math.log10(maximum)

    for i, row in enumerate(rows):
        item, rate, y = row["identity"], row["launch_rate"], 82 + 28 * i
        label = f"{item['method']} {item['workload']} {item['config']['message_size']}B q{item['queue_depth']} [{row['group_id']}]"
        svg += [
            f'<text x="20" y="{y + 4}">{html.escape(label)}</text>',
            f'<line x1="{scale(rate["p10"]):.2f}" x2="{scale(rate["p90"]):.2f}" y1="{y}" y2="{y}" stroke="#147d92" stroke-width="3"/>',
            f'<circle cx="{scale(rate["median"]):.2f}" cy="{y}" r="4" fill="#172b4d"><title>{rate["median"]:.0f} {row["rate_unit"]}</title></circle>',
        ]
    svg.append("</g></svg>")
    path.write_text("\n".join(svg), encoding="utf-8")


def _saturation_chart(path, rows):
    series = {}
    for row in rows:
        if row["rate_unit"] != "validated-deliveries/s":
            continue
        identity = row["identity"]
        # Counts are duration-calibrated per depth; all other comparison controls remain identical.
        key = {k: v for k, v in identity.items() if k not in ("queue_depth", "config")}
        key["config"] = {k: v for k, v in identity["config"].items() if k not in ("queue_depth", "message_count")}
        series.setdefault(json.dumps(key, sort_keys=True), []).append(row)
    svg = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="{max(1, len(series)) * 270}" viewBox="0 0 1000 {max(1, len(series)) * 270}">',
        '<rect width="100%" height="100%" fill="white"/>',
        '<g font-family="sans-serif" font-size="13" fill="#172b4d">',
    ]
    for index, points in enumerate(series.values()):
        points.sort(key=lambda r: r["identity"]["queue_depth"])
        item, top = points[0]["identity"], index * 270
        maximum = max(p["launch_rate"]["p90"] for p in points) * 1.1
        svg.append(
            f'<text x="30" y="{top + 25}">{html.escape(item["method"])} / {item["workload"]} / {item["config"]["message_size"]} bytes / capacity {item.get("ring_capacity")}</text>'
        )
        svg.append(
            f'<text x="30" y="{top + 47}">Validated deliveries/s; median with launch p10-p90. Queue depth on log2 axis.</text>'
        )
        coords = []
        for p in points:
            rate, depth = p["launch_rate"], p["identity"]["queue_depth"]
            x = 130 + math.log2(depth) * 95

            def y(value):
                return top + 220 - value / maximum * 150

            coords.append(f"{x:.1f},{y(rate['median']):.1f}")
            svg += [
                f'<line x1="{x}" x2="{x}" y1="{y(rate["p10"]):.1f}" y2="{y(rate["p90"]):.1f}" stroke="#147d92" stroke-width="3"/>',
                f'<circle cx="{x}" cy="{y(rate["median"]):.1f}" r="4" fill="#172b4d"/>',
                f'<text x="{x - 10}" y="{top + 244}">{depth}</text>',
                f'<text x="{x - 25}" y="{y(rate["median"]) - 10:.1f}">{rate["median"] / 1000:.0f}k</text>',
            ]
        svg.append(f'<polyline points="{" ".join(coords)}" fill="none" stroke="#147d92"/>')
    svg.append("</g></svg>")
    path.write_text("\n".join(svg), encoding="utf-8")
