# Retained schema-v2 measurements

Each group has identical configuration, executable, features and effective affinity. Group IDs link to exact inputs in `summary-v2.json`.

Ranges are launch p10-p90, not request-tail latency. CPU seconds include startup, warmup and shutdown. No universal transport winner is inferred.

| Group | Method / workload | Payload | Depth / capacity | Launches | Rate/s (p10-p90) | RTT us | Observed p99 us | CPU seconds | Min trial s |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| e3cd0c37fac5 | py-multiprocessing-pipe / round-trip | 64 | 1 / - | 5 | 22635 (21147-23557) | 44.179 | 115.400 | 0.547 | 0.424 |
| ca960a7e07b4 | py-multiprocessing-pipe / round-trip | 65536 | 1 / - | 5 | 9362 (8776-9568) | 106.812 | 236.900 | 1.281 | 1.045 |
| e2fa02b2e74e | named-pipe-overlapped / round-trip | 64 | 1 / - | 5 | 42066 (36955-45997) | 23.772 | 83.700 | 0.156 | 0.217 |
| 33927b68cfd6 | shm-ring-spin / round-trip | 64 | 1 / - | 5 | 3758268 (3659117-3809524) | 0.266 | 0.400 | 0.016 | 0.003 |
| 16c986c5c152 | named-pipe-overlapped / round-trip | 65536 | 1 / - | 5 | 8488 (8069-8547) | 117.815 | 264.300 | 1.047 | 1.170 |
| 9b7c808817e2 | shm-ring-spin / round-trip | 65536 | 1 / - | 5 | 14617 (14425-14705) | 68.414 | 111.100 | 1.531 | 0.680 |
