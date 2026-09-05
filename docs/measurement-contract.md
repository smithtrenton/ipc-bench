# Measurement contract (schema 2)

The supported native target is **x86_64-pc-windows-msvc**. Python comparisons use the pinned 64-bit Python in `.python-version`. See [the registry](../benchmarks/methods/registry.json) for available methods, workloads and payload limits. The two original published directories and their numerical results remain historical snapshots; schema 2 starts a separate series.

## Frames and successful work

`message_size` counts application payload bytes. The wire frame contains eight additional little-endian sequence bytes, so `wire_size = message_size + 8`. Payload bytes are the independently generated pattern `index % 251`. A request contains its monotonically increasing sequence; a response increments the first header byte modulo 256 without carrying into the remaining header bytes. The parent checks against its own sequence and payload oracle, then advances its sequence independently. This detects short, corrupt, duplicate and stale responses even at payload size one. No empty frame is counted as application work.

Payloads are 1 through 1,048,576 bytes, except UDP (65,499) and ALPC (32,719); these preserve their original wire limits after accounting for the header. Zero counts/trials, overflow, more than one billion total operations, more than one million retained round-trip samples, and deadlines outside 1–86,400 seconds are rejected. Shared mappings have a checked 256 MiB limit, so some maximum-capacity/maximum-payload combinations are rejected. Queue depth is 1–256; ring capacity is independently selected from powers of two in 1–256.

Full validation is the default and checks every frame. Sampled round-trip performance mode checks the complete sequence every operation, the full frame every 1,024 sequences, every operation in the last measurement batch of each trial, and untimed preflight/final exchanges. It retains a response-to-next-request copy in the control implementation. The suite runs a separate full-validation gate before sampled performance launches. Sampled checks establish less than full per-operation payload validation; the report names that policy explicitly.

One successful preflight exchange precedes warmup and synchronizes both endpoints; one final exchange follows all trials. Frames, mappings and fixed operation pools are touched before measured use. Errors contain the method and phase, with trial/iteration or sequence where applicable. An operation failure invalidates the whole report; failed work is never averaged into a successful result.

The parent joins a kill-on-close Windows job before creating workers. A separate watchdog enforces the total invocation deadline. Readiness and normal shutdown have five-second bounds; error cleanup kills and reaps owned children. Explicit kernel waits have bounds; blocking transport and spin paths also have the outer job deadline. Python owns workers across startup failures and closes queues, connections and shared-memory views. Pending overlapped I/O is cancelled and drained before its storage is released. IOCP uses stable operation allocations and drains cancellation completions; an unrecoverable cancellation drain aborts the job rather than freeing kernel-owned buffers. RPC exceptions cross the C boundary as errors.

## Round-trip statistics

`workload: round-trip` has one outstanding request. `round_trip_rate` counts completed request/response pairs per second. The legacy `message_rate` field is an exact alias; it is **not** saturated one-way message throughput. `copy-roundtrip` is a synthetic sequence/validation/copy workload, not an IPC implementation or a universal hardware copy floor. `placeholder` is explicitly `harness-overhead` and has no transport validation.

Batch mode retains each batch's `average_micros` and `operations`. Batch size is `min(100, ceil(message_count / 100))`, at least one. The final partial batch carries its actual weight. Trial and aggregate averages and population standard deviations are operation-weighted; their minimum, maximum and spread describe **batch averages**. Storage is allocated for the actual number of samples before timing.

`--measurement latency` times and retains individual round trips. Only this mode reports per-operation p50/p95/p99, using nearest-rank percentiles. It includes timer overhead and is a distinct comparison group. Across-process launch p10/p90 describes run-to-run spread. The publisher never relabels that spread as request tails.

`timer_pair_micros` is a separate 1,000-iteration timer-pair/loop calibration; it is never subtracted. Publication reports its estimated fraction of the measured total. `--min-trial-seconds` retains a full-validation gate and a timing pilot using the requested validation policy, then chooses one fixed operation count for all subsequent launches of that configuration, with a 20% duration margin and the retention cap. Full-validation runs reuse their gate as the timing pilot. Check actual minimum trial duration and timer fraction in the generated summary: a pilot is an estimate, not a guarantee. Increase duration/count when the report shows insufficient measurement time. Empty harness work is separately available via `placeholder`.

## Delivery throughput and latency under load

`shm-ring-spin` and `shm-ring-hybrid` support `streaming` and `windowed`; `named-pipe-iocp` supports `windowed` only. The ring endpoints make independent send/receive progress under backpressure. Streaming completion advances only after child validation; slot reuse alone does not prove delivery. Windowed responses validate sequence and contents before returning credit. IOCP has concurrent read and write operation pools, supports completion reordering, and counts a response once. The synchronous/one-outstanding overlapped pipe remains separately named.

Throughput reports have **no round-trip summary or `message_rate` alias**. They contain exact validated delivery counts, zero delivery errors on success, elapsed time, delivered messages/s and payload bytes/s. Streaming counts request bytes; windowed counts response bytes. Header bytes, acknowledgments and offered-but-undelivered traffic are excluded. Queue depth and ring capacity are independent report fields. Buffers and credits are bounded by those limits rather than run length.

Latency under load measures send to observation of validated delivery for every 16th sequence, retaining the first 65,536 samples per trial. Streaming acknowledgments can reveal several completed deliveries at once, so this is observation latency. Percentiles cover the retained sample window and include sampling overhead; they do not claim unsampled or whole-run tails. The raw samples and policy remain in each trial.

## Experimental controls

Native Cargo features are off by default. Enable each separately as `support/<feature>`:

| Feature | Change from control |
| --- | --- |
| `padded-layout` | 64-byte-aligned control fields, payload starts and ring slot stride |
| `copy-elision` | Removes the parent's response-to-next-request copy; validation still consumes the response |
| `borrowed-response` | Mailboxes and iceoryx2 respond directly from borrowed requests into response storage, removing child scratch copies |
| `conditional-wake` | Hybrid notification occurs only when the receiver has armed its sleeping state |
| `cached-cursors` | Each SPSC endpoint retains its owned cursor and reloads peer progress only when empty/full |

The fixed copy-only control has four guarded intermediary frame copies and the final response-to-request copy, each moving `wire_size` bytes. Copy elision removes the final one; direct mailbox/loan responses remove one intermediary copy where enabled. The release assembly evidence verifies the guarded intermediary copies. C RPC stubs use `/O2` in optimized profiles and `/Od` in debug, plus `/MD` or `/MT` matching Rust's `crt-static`; the build records exact commands and SDK/MSVC paths.

For the SPSC ring, only the producer writes available slots. Release publication of the write cursor happens before consumer acquire/read; consumer release of the read cursor happens before producer reuse. References remain within those ownership windows. Wrapping subtraction handles cursors near the integer maximum; validated power-of-two capacity permits mask indexing. Cached cursors retain the release/acquire publication edges.

For conditional wake, the receiver arms with a sequentially consistent read-modify-write, returns to recheck publication/stop, and only then waits. The producer publishes before exchanging the armed bit to false and signaling if it was set. Either the receiver observes publication or the producer observes its armed state. Stop always signals. The state-transition model enumerates sequentially consistent interleavings; real forced-yield cross-process tests separately exercise the implementation. It is not an exhaustive weak-memory proof. `--spin-budget` (or direct `IPC_BENCH_SPIN_BUDGET`) sweeps CPU versus kernel waiting; default 256. Signal/wait counters cover explicit parent Win32 calls, not hidden library syscalls or the child's calls.

`--stable-affinity` selects separate physical cores sharing the last-level cache; `--cpu-pair 0,2` selects explicit logical CPUs. `--topology smt|separate-core|separate-cache|unpinned` resolves actual Windows core/cache relationships. Reports read back both process masks and the parent thread's processor group. Controlled placement currently supports single-group hosts only; multi-group hosts can run unpinned. Masks do not prove permanent residence of an unpinned thread on one CPU. Separate-core selection searches cache membership rather than assuming that the first two enumerated cores share cache.

`parent_cpu_seconds` and `child_cpu_seconds` cover process lifetime through shutdown, including setup and warmup. They are not timed-loop CPU utilization. Interpret short-run resource comparisons cautiously; use long measured runs to reduce setup's contribution. Busy-spin and sleeping implementations should be compared using both CPU cost and latency.

## Running and regenerating

From an x64 Windows developer environment with MSVC C++ tools and a Windows SDK:

```powershell
uv sync --locked --all-groups
$env:LIBCLANG_PATH = uv run --locked --group build python -c "from pathlib import Path; import clang; print(Path(clang.__file__).parent / 'native')"
cargo build --locked --release --workspace
uv run --locked --group build python scripts/benchmark_suite.py smoke --skip-build --output target/boundaries
uv run --locked python scripts/verify_transports.py --output target/faults.json
uv run --locked python scripts/verify_affinity.py --output target/affinity
uv run --locked --group build python scripts/benchmark_suite.py run --output results/round-trip-v2 --validation sampled --stable-affinity --launches 5 --min-trial-seconds 1
uv run --locked --group build python scripts/benchmark_suite.py run --output results/latency-v2 --measurement latency --stable-affinity --count 10000 --methods shm-ring-spin named-pipe-overlapped py-multiprocessing-pipe
uv run --locked --group build python scripts/experiment_suite.py --output results/ab-v2 --profiles release-thin release-lto native
uv run --locked --group build python scripts/throughput_suite.py --output results/throughput-v2
uv run --locked python scripts/benchmark_suite.py publish --output results/throughput-v2
```

Every run creates a fresh directory; existing output is refused. Native binaries are built once and invoked directly. Launch order is shuffled with a retained seed. Metadata includes toolchains, locks, source archive/hashes, binary hashes, power plan, host and C compiler provenance. stdout, stderr, configuration, timestamps, failures and raw measurements are retained independently; JSON writes are atomic. Use `--skip-build` only with binaries you deliberately prepared; actual feature flags and executable hashes still distinguish them.

`publish` validates retained measurements, recomputes rates/spreads/percentiles and writes `summary-v2.json`, `throughput-v2.json`, `comparison.md`, `comparison.svg` and `saturation.svg`. It separates executable hashes, features, measurement configuration, spin budget and effective placement into distinct groups. Generated group IDs identify their exact inputs. A/B results include all launch averages and mark overlapping launch ranges instead of promoting a tiny median difference to a confident winner. Machine-native executables have host-specific compatibility; their profile and build-time cost are separately recorded. PGO, hardware cache counters and broader host profiling remain optional follow-up experiments, not assumed speedups.
