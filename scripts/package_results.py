"""Package a local campaign with validated tables, plots and compressed raw evidence."""

from __future__ import annotations
import argparse
import hashlib
import json
import shutil
import tempfile
import zipfile
from pathlib import Path
import benchmark_suite as suite


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--exclude-methods", nargs="*", default=[])
    args = parser.parse_args()
    args.destination.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(dir=suite.ROOT / "target", prefix="publication-") as temporary:
        staging = Path(temporary)
        manifest = json.loads((args.source / "manifest.json").read_text())
        selected = []
        for index, entry in enumerate(manifest):
            if entry["method"] in args.exclude_methods:
                continue
            selected.append(entry | {"case": entry.get("case", index)})
            source = args.source / "cases" / f"{index:05d}"
            shutil.copytree(source, staging / "cases" / source.name)
        suite.atomic_json(staging / "manifest.json", selected)
        suite.publish(staging)
        for source in args.source.iterdir():
            if source.is_file() and source.name not in (
                "manifest.json",
                "summary-v2.json",
                "throughput-v2.json",
                "comparison.md",
                "comparison.svg",
                "saturation.svg",
            ):
                shutil.copy2(source, args.destination / source.name)
        for source in staging.iterdir():
            if source.is_file():
                shutil.copy2(source, args.destination / source.name)
        with zipfile.ZipFile(args.destination / "raw.zip", "w", zipfile.ZIP_DEFLATED) as archive:
            for base in (staging / "cases", args.source / "gates"):
                for source in sorted(base.rglob("*")):
                    if source.is_file():
                        archive.write(source, Path(base.name) / source.relative_to(base))
        suite.atomic_json(
            args.destination / "package.json",
            {
                "source_directory": str(args.source),
                "excluded_methods": args.exclude_methods,
                "retained_cases": len(selected),
                "failed_cases": sum(r["status"] != "passed" for r in selected),
                "files": {
                    p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                    for p in args.destination.iterdir()
                    if p.is_file()
                },
                "regeneration": "Extract raw.zip here, then run scripts/benchmark_suite.py publish --output <this directory>. Manifest and raw measurements are retained; binaries are identified by hash, not distributed.",
            },
        )
        print(f"Packaged {len(selected)} retained cases in {args.destination}")


if __name__ == "__main__":
    main()
