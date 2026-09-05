# Implementation verification

This document records the initial schema-2 implementation campaign. The [September 4 refresh](../results/published/windows11-20260904/README.md) reruns all six campaigns from source `794d9fb`, including the fresh-host iceoryx2 fix, and records a successful GitHub CI run.

Implemented on 2026-09-04 on Windows 11 build 26200, AMD Ryzen 9 7950X3D, Rust 1.98.1 and Python 3.14.7. This is a dirty working-tree implementation following the dependency refresh; retained source archives, native source hashes, executable hashes and compiler records identify the measured revisions. The original schema 1 published directories are unchanged.

## Delivered scope

| Plan milestone | Implementation and evidence |
| --- | --- |
| Dependencies/reproducibility | Locked Rust/Python tools, explicit libclang setup, optimized RPC C flags/runtime, weekly dependency PR configuration |
| Input/shutdown/bounds | Explicit eight-byte sequence framing, size-one controls, method limits, checked mappings/FFI sizes and validated attachment headers |
| Supervision/data integrity | Full and sampled independent validation, contextual failures, owned process trees, bounded readiness/shutdown, cancellation drains and RPC error conversion |
| Comparable measurement | Schema 2, retained weighted batches and individual latency samples, timer calibration, duration pilots, effective affinity/topology, CPU cost, seeded interleaved launches and atomic output |
| Isolated optimizations | Five opt-in features, control builds, full-validation gates, 780 retained A/B launches, LTO/native profile sweep and build costs |
| Delivery throughput | Streaming/windowed rings, concurrent IOCP, independent credits/backpressure, exact validated delivery and payload-byte accounting, sampled latency under load, depth/capacity/large-payload sweeps |
| Publication/CI | Registry-driven boundaries, fault and synchronization stress, shared statistics/tamper fixtures, generated summaries/tables/SVGs and compressed raw evidence |

The [measurement contract](measurement-contract.md) describes the exact semantics and commands. The [schema 2 publication](../results/published/windows11-schema2/README.md) contains the regenerated evidence, including its scope and limitations.

## Correctness checks

The final locked workspace checks passed for both default and all-feature builds: 13 Rust unit tests per build, Clippy with warnings denied, and release compilation. All eleven Python contract tests, Python/tooling lint and formatting, Rust formatting, both PowerShell parser checks, and `git diff --check` passed. The final default build passed all 392 boundary cases; the final feature build passed all 85 forced-yield ring/IOCP stress cases. Default release binaries were restored after feature verification. The final cache-membership resolver also passed Clippy/workspace tests and has a fixture with deliberately interleaved core/cache enumeration.

The release feature build passed 392/392 registry boundary cases: 28 methods, each at payloads 1, 2, 63, 64, 65, 4,095, 4,096, 4,097 and its limit, plus zero, limit+1, integer maximum, zero count and zero trials. Unit tests separately exercise short/corrupt/stale frames, arithmetic overflow, one-slot rings, actual slot publication across integer wraparound, invalid mapping descriptors, and sleep/wake state interleavings.

The fault/stress campaign passed 189/189 cases: 104 injected peer failures/corruption checks, 80 ring workload/capacity/depth combinations and five IOCP depths. It observes owned descendants and checks that none survive. The feature run forces yields around wake arming and sets the spin budget to zero. A final IOCP review made sequence numbers monotonic across phase boundaries; its nine fault/depth cases were rerun successfully, with all five depths using uneven counts and multiple trials. Final IOCP performance measurements supersede the earlier IOCP throughput samples.

The reusable affinity driver passed all 20 host checks: copy-only, anonymous pipe, spinning ring and Python pipe under SMT, shared-cache separate cores, separate cache domains, unpinned scheduling and explicit CPU pairs. Both parent and worker masks were read back. CI records unsupported topology cases on hosts that cannot provide a requested relationship.

Python contract tests also verify that summary/percentile tampering is rejected, incompatible executable groups remain separate, publication regenerates deterministically, invalid inputs fail before supervision, and spawn failures retain diagnostics. Native and Python statistics share the same weighted fixture. GitHub-hosted CI itself has not been executed here; its workflow performs these checks and uploads evidence on the hosted runner.

## Performance evidence and limits

The A/B campaign contains 780 successful launches over 156 binary/method/payload configurations, with five launches per configuration and a separate full-validation gate for each. It compares the control to each feature independently, thin LTO, fat LTO, and a machine-native target. Of 132 comparisons, 101 have overlapping launch ranges, 28 are faster with disjoint ranges and three are slower with disjoint ranges. These short exploratory launches do not justify enabling one feature universally. All optimizations remain opt-in. Build times are incremental host observations, not clean-build guarantees.

The corrected round-trip comparison contains 390 successful launches: 26 comparison methods, three payloads, five launches and three measured trials. Sampled validation follows full-validation gates and policy-matched timing pilots; duration-calibrated counts are fixed across launches of each configuration. Python launches were refreshed after the cache-membership audit; the merged series records separate source archives and actual timestamps for its native/Python components. Individual-latency evidence contains 30 launches for a spinning ring, overlapped pipe and Python multiprocessing pipe, at 64 and 65,536 payload bytes, with 10,000 individual samples each. Full validation is enabled for this latency series.

The throughput evidence separately measures ring streaming/windowed and IOCP windowed delivery at queue depths 1, 2, 8, 64 and 256, with five launches per configuration. Main trials target one second using retained pilots. A separate capacity-eight sweep includes 65,536-byte and 1 MiB payloads at depths 1, 8 and 256. Actual minimum durations, sample policies and CPU seconds are in each generated summary. The main ring curves flatten or decline at high depths on this host; this establishes an observed saturation region for these implementations and controls, not a platform-wide maximum.

Controlled runs used effective parent/child masks 1 and 4 in processor group 0. The Windows topology and power plan are retained. This was a local host session, not an isolated performance laboratory: background machine activity was not eliminated. No hardware cache/coherence counters or PGO profile were collected. Multi-group controlled affinity is explicitly unsupported; unpinned execution remains available. CPU measurements cover process lifetime and therefore include startup/warmup. Throughput latency samples cover only the documented bounded sampling window. Use longer runs and topology/spin-budget sweeps before making deployment-specific performance decisions.

## Assembly and C build audit

`cargo rustc --locked -p support --release -- --emit=asm` produced the optimized copy operation. Its four intermediary frame copies remain as four `memcpy` calls separated by compiler barriers, followed by the independent response validator. The final fixed-copy stage is in the harness validator; the source and separately emitted harness assembly identify that stage. See the retained assembly excerpts in the publication's `assembly` directory. This evidence supports these pinned release builds; `black_box` remains a best-effort compiler aid for future toolchains.

RPC release C compilation records `/O2` and `/MD` for this host. Debug uses `/Od`; a Rust `crt-static` target selects `/MT`. The refreshed release workspace builds no longer emit the original LNK4098 runtime-linkage warning. Exact MSVC, SDK and C command records are retained with the series metadata.

## Reproduction

Follow [the documented commands](measurement-contract.md#running-and-regenerating). Each packaged campaign includes its metadata, launch order, manifest, source provenance, validated summary and `raw.zip`. Extract `raw.zip` inside a copy of the campaign directory, then run `scripts/benchmark_suite.py publish --output <directory>` to regenerate its numerical tables and charts. Publication artifacts contain no executable binaries; executable hashes and build settings are retained. The experiment campaign's native source archive is reconstructed against its saved hashes, because publication tooling was completed after that campaign ran.

One-off implementation scripts and duplicate local benchmark campaigns were removed after archival. The repository publication preserves the reviewable evidence and can regenerate its tables without those temporary files. Reusable build caches remain under ignored `target/`.
