"""Pure contract and retained-measurement verification; no benchmark rankings."""

import copy
import argparse
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from benchmarks.methods.python import benchmark_adapter as adapter

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("benchmark_suite", ROOT / "scripts/benchmark_suite.py")
suite = importlib.util.module_from_spec(spec)
spec.loader.exec_module(suite)


class ContractTests(unittest.TestCase):
    def test_shared_cache_selection_does_not_assume_core_order(self):
        caches = [{"cache_level": 3, "mask": 15}, {"cache_level": 3, "mask": 48}]
        self.assertEqual(adapter._shared_cache_pair([3, 48, 12], caches), (1, 4))
        with self.assertRaises(ValueError):
            adapter._shared_cache_pair([3, 48], caches)

    def test_shared_statistics_fixture(self):
        fixture = json.loads((ROOT / "tests/statistics.json").read_text())
        batches = [(b["average_micros"], b["operations"]) for b in fixture["batches"]]
        trial = adapter.summarize_batches(1, batches)
        for key, value in fixture.items():
            if key != "batches":
                self.assertAlmostEqual(trial[key], value)
        single = adapter.summarize_batches(1, [(2.0, 1)], latency=True)
        self.assertEqual(single["stddev_micros"], 0.0)
        self.assertEqual(single["latency_percentiles_micros"], [2.0] * 3)

    def test_full_payload_short_corruption_and_stale(self):
        for size in (1, 2, 63, 64, 65, 4095, 4096, 4097):
            request = adapter.make_payload(size + 8)
            reply = request.copy()
            reply[0] += 1
            corrupt = reply.copy()
            corrupt[-1] ^= 1
            with self.assertRaises(ValueError):
                adapter.update_payload(request, corrupt)
            with self.assertRaises(ValueError):
                adapter.update_payload(request, reply[:-1])
            adapter.update_payload(request, reply)
            with self.assertRaises(ValueError):
                adapter.update_payload(request, reply)

    def test_report_regenerates_and_rejects_tampering(self):
        config = adapter.BenchmarkConfig(message_count=205, message_size=1, trials=1, warmup_count=0)
        report = adapter.run_benchmark("test", config, lambda: None, child_ready=False)
        self.assertEqual(len(report["trials"][0]["samples"]), 69)
        self.assertEqual(report["trials"][0]["samples"][-1]["operations"], 1)
        suite.validate_report(report, "test", 1, 205)
        damaged = copy.deepcopy(report)
        damaged["summary"]["total_micros"] += 100
        with self.assertRaises(ValueError):
            suite.validate_report(damaged, "test", 1, 205)

    def test_failed_operation_has_context(self):
        def fail():
            raise EOFError("peer died")

        with self.assertRaisesRegex(RuntimeError, "method=test phase=preflight trial=0 iteration=1"):
            adapter.run_benchmark("test", adapter.BenchmarkConfig(), fail, child_ready=False)

    def test_invalid_input_does_not_start_supervisor(self):
        for arguments in (
            ("--message-size", "0"),
            ("--message-size", str(2**64)),
            ("--message-count", "0"),
            ("--trials", "0"),
            ("--warmup-count", "-1"),
        ):
            with patch("sys.argv", ["test", *arguments]), patch.object(adapter, "supervise") as supervisor:
                with self.assertRaises(SystemExit):
                    adapter.parse_config()
                supervisor.assert_not_called()

    def test_latency_retention_limit_rejects_before_startup(self):
        with (
            patch("sys.argv", ["test", "--measurement", "latency", "--message-count", "1000001"]),
            patch.object(adapter, "supervise") as supervisor,
        ):
            with self.assertRaises(SystemExit):
                adapter.parse_config()
            supervisor.assert_not_called()

    def test_percentile_and_summary_spread_tampering(self):
        config = adapter.BenchmarkConfig(
            message_count=16, message_size=1, trials=2, warmup_count=0, measurement="latency"
        )
        report = adapter.run_benchmark("test", config, lambda: None, child_ready=False)
        for field in ("stddev_micros", "min_micros", "message_rate"):
            damaged = copy.deepcopy(report)
            damaged["summary"][field] += 1
            with self.assertRaises(ValueError):
                suite.validate_report(damaged, "test", 1, 16)
        report["trials"][0]["latency_percentiles_micros"][2] += 1
        with self.assertRaises(ValueError):
            suite.validate_report(report, "test", 1, 16)

    def test_publication_separates_executables_and_regenerates(self):
        config = adapter.BenchmarkConfig(message_count=16, message_size=1, trials=1, warmup_count=0)
        report = adapter.run_benchmark("test", config, lambda: None, child_ready=False)
        with tempfile.TemporaryDirectory() as temporary, patch.object(sys, "path", [str(ROOT / "scripts"), *sys.path]):
            directory = Path(temporary)
            for i, digest in enumerate(("binary-a", "binary-a", "binary-b")):
                case = directory / "cases" / str(i)
                case.mkdir(parents=True)
                suite.atomic_json(case / "report.json", report)
                suite.atomic_json(case / "result.json", {"status": "passed", "executable_sha256": digest})
            suite.publish(directory)
            first = (directory / "summary-v2.json").read_bytes()
            self.assertEqual(sorted(r["launches"] for r in json.loads(first)), [1, 2])
            suite.publish(directory)
            self.assertEqual(first, (directory / "summary-v2.json").read_bytes())
            self.assertTrue((directory / "comparison.svg").exists())

    def test_runner_retains_spawn_failure(self):
        args = argparse.Namespace(
            warmup=0,
            trials=1,
            action="run",
            mailslot_count=None,
            profile="release",
            timeout=1,
            validation="full",
            measurement="batch",
            workload="round-trip",
            stable_affinity=False,
            cpu_pair=None,
            topology=None,
            queue_depth=1,
            ring_capacity=64,
        )
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary)
            method = {
                "name": "missing",
                "kind": "native",
                "workloads": ["round-trip"],
                "binary": str(destination / "missing.exe"),
            }
            result = suite.run_case(method, 1, 1, args, destination)
            self.assertEqual(result["status"], "failed")
            self.assertTrue((destination / "result.json").exists())
            self.assertFalse((destination / "report.json").exists())

    def test_calibration_uses_requested_policy_after_full_gate(self):
        policies = []

        def fake_run(method, size, count, args, destination):
            policies.append(args.validation)
            suite.atomic_json(
                destination / "report.json",
                {"summary": {"total_micros": 1_000_000 if args.validation == "full" else 100_000}},
            )
            return {"status": "passed"}

        args = argparse.Namespace(validation="sampled", measurement="batch", trials=1)
        with tempfile.TemporaryDirectory() as temporary, patch.object(suite, "run_case", side_effect=fake_run):
            count = suite.calibrate({}, 1, 100, args, Path(temporary) / "pilot", 1)
        self.assertEqual(policies, ["full", "sampled"])
        self.assertEqual(count, 1200)


if __name__ == "__main__":
    unittest.main()
