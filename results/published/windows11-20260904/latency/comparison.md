# Retained schema-v2 measurements

Each group has identical configuration, executable, features and effective affinity. Group IDs link to exact inputs in `summary-v2.json`.

Ranges are launch p10-p90, not request-tail latency. CPU seconds include startup, warmup and shutdown. No universal transport winner is inferred.

| Group | Method / workload | Payload | Depth / capacity | Launches | Rate/s (p10-p90) | RTT us | Observed p99 us | CPU seconds | Min trial s |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| e3cd0c37fac5 | py-multiprocessing-pipe / round-trip | 64 | 1 / - | 5 | 29572 (21129-31449) | 33.815 | 91.600 | 0.484 | 0.318 |
| ca960a7e07b4 | py-multiprocessing-pipe / round-trip | 65536 | 1 / - | 5 | 9979 (9360-11262) | 100.208 | 250.500 | 1.078 | 0.888 |
| 7d6c83fa1fba | named-pipe-overlapped / round-trip | 64 | 1 / - | 5 | 54945 (50068-59536) | 18.200 | 68.100 | 0.109 | 0.168 |
| 885333bcca00 | shm-ring-spin / round-trip | 64 | 1 / - | 5 | 3768465 (2949070-3909151) | 0.265 | 0.400 | 0.016 | 0.003 |
| 321e00e821d1 | named-pipe-overlapped / round-trip | 65536 | 1 / - | 5 | 9551 (8802-9692) | 104.697 | 196.400 | 1.031 | 1.032 |
| d8852cea5c15 | shm-ring-spin / round-trip | 65536 | 1 / - | 5 | 14557 (14532-15042) | 68.694 | 128.300 | 1.500 | 0.665 |
