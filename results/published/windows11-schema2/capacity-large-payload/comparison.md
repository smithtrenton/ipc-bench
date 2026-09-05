# Retained schema-v2 measurements

Each group has identical configuration, executable, features and effective affinity. Group IDs link to exact inputs in `summary-v2.json`.

Ranges are launch p10-p90, not request-tail latency. CPU seconds include startup, warmup and shutdown. No universal transport winner is inferred.

| Group | Method / workload | Payload | Depth / capacity | Launches | Rate/s (p10-p90) | RTT us | Observed p99 us | CPU seconds | Min trial s |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| f55befeb638d | shm-ring-hybrid / streaming | 65536 | 1 / 8 | 5 | 20852 (20271-21013) | n/a | 98.100 | 1.219 | 0.603 |
| c89a4de9b4cb | shm-ring-spin / streaming | 65536 | 1 / 8 | 5 | 21164 (20995-21296) | n/a | 57.100 | 1.234 | 0.596 |
| 71c91d746ebd | shm-ring-hybrid / streaming | 65536 | 256 / 8 | 5 | 22461 (22434-22551) | n/a | 441.800 | 1.234 | 0.596 |
| 58a373dcfd4c | shm-ring-spin / streaming | 65536 | 8 / 8 | 5 | 22470 (22342-22628) | n/a | 384.800 | 1.219 | 0.595 |
| 1afb9d95c6b5 | shm-ring-hybrid / streaming | 65536 | 8 / 8 | 5 | 22486 (22418-22505) | n/a | 397.100 | 1.234 | 0.603 |
| 1c9fe53c2c7d | shm-ring-spin / streaming | 65536 | 256 / 8 | 5 | 22581 (22245-22849) | n/a | 446.400 | 1.219 | 0.594 |
| 261cc27ace04 | shm-ring-hybrid / streaming | 1048576 | 1 / 8 | 5 | 1321 (1213-1345) | n/a | 1377.300 | 6.484 | 3.046 |
| f28ca4d0059f | shm-ring-spin / streaming | 1048576 | 1 / 8 | 5 | 1357 (1321-1378) | n/a | 833.300 | 6.438 | 2.973 |
| a4ff2aea265c | shm-ring-hybrid / windowed | 1048576 | 1 / 8 | 5 | 859 (648-940) | n/a | 4504.800 | 5.266 | 4.359 |
| 7cb7f0781d33 | shm-ring-spin / windowed | 1048576 | 1 / 8 | 5 | 911 (905-950) | n/a | 1308.000 | 9.547 | 4.312 |
| 48586010f16a | shm-ring-hybrid / streaming | 1048576 | 256 / 8 | 5 | 1408 (1390-1418) | n/a | 6604.900 | 6.172 | 2.889 |
| 058f249e886c | shm-ring-spin / streaming | 1048576 | 256 / 8 | 5 | 1408 (1384-1423) | n/a | 6618.300 | 6.156 | 2.878 |
| 65242632c95b | shm-ring-hybrid / windowed | 1048576 | 256 / 8 | 5 | 956 (834-990) | n/a | 13635.300 | 4.812 | 4.139 |
| 668429d29f42 | shm-ring-spin / windowed | 1048576 | 256 / 8 | 5 | 957 (948-995) | n/a | 4660.100 | 9.047 | 4.118 |
| 5944aa96368a | shm-ring-hybrid / streaming | 1048576 | 8 / 8 | 5 | 1409 (1406-1429) | n/a | 5797.000 | 6.172 | 2.867 |
| 2c086fa48c68 | shm-ring-spin / streaming | 1048576 | 8 / 8 | 5 | 1415 (1394-1419) | n/a | 5888.800 | 6.172 | 2.886 |
| e9e021af43ba | shm-ring-hybrid / windowed | 1048576 | 8 / 8 | 5 | 979 (864-993) | n/a | 15246.500 | 4.703 | 4.125 |
| a2ba66de3a27 | shm-ring-spin / windowed | 1048576 | 8 / 8 | 5 | 990 (942-1002) | n/a | 4558.300 | 8.797 | 4.089 |
| bf2ccb5f64c4 | shm-ring-hybrid / windowed | 65536 | 1 / 8 | 5 | 11611 (9311-13208) | n/a | 199.100 | 0.703 | 0.536 |
| 70cd9561e1b9 | shm-ring-hybrid / windowed | 65536 | 8 / 8 | 5 | 14229 (13263-14603) | n/a | 1735.800 | 0.609 | 0.503 |
| 7ca94159f54f | shm-ring-spin / windowed | 65536 | 256 / 8 | 5 | 15699 (15237-15778) | n/a | 263.300 | 1.078 | 0.506 |
| 5f5b884c95e2 | shm-ring-spin / windowed | 65536 | 1 / 8 | 5 | 14299 (13915-14615) | n/a | 108.600 | 1.172 | 0.555 |
| 9635b1d0f368 | shm-ring-hybrid / windowed | 65536 | 256 / 8 | 5 | 14414 (13898-14734) | n/a | 833.500 | 0.688 | 0.592 |
| bbdd532e6347 | shm-ring-spin / windowed | 65536 | 8 / 8 | 5 | 14898 (14795-14972) | n/a | 275.700 | 1.219 | 0.593 |
