# Retained schema-v2 measurements

Each group has identical configuration, executable, features and effective affinity. Group IDs link to exact inputs in `summary-v2.json`.

Ranges are launch p10-p90, not request-tail latency. CPU seconds include startup, warmup and shutdown. No universal transport winner is inferred.

| Group | Method / workload | Payload | Depth / capacity | Launches | Rate/s (p10-p90) | RTT us | Observed p99 us | CPU seconds | Min trial s |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| d8dc340f370f | shm-ring-hybrid / streaming | 65536 | 1 / 8 | 5 | 21026 (20474-21253) | n/a | 73.800 | 1.234 | 0.593 |
| 3a2a6682d478 | shm-ring-spin / streaming | 65536 | 1 / 8 | 5 | 21190 (21008-21320) | n/a | 54.800 | 1.219 | 0.592 |
| 823d339fb35b | shm-ring-hybrid / streaming | 65536 | 8 / 8 | 5 | 22652 (22606-22845) | n/a | 381.200 | 1.203 | 0.587 |
| 25bcd713a59a | shm-ring-hybrid / streaming | 65536 | 256 / 8 | 5 | 22560 (22388-22719) | n/a | 431.100 | 1.219 | 0.592 |
| 36aed55f4ec3 | shm-ring-spin / streaming | 65536 | 256 / 8 | 5 | 22617 (22477-22671) | n/a | 430.300 | 1.234 | 0.594 |
| 1daee1bf2909 | shm-ring-spin / streaming | 65536 | 8 / 8 | 5 | 22601 (22516-22652) | n/a | 389.900 | 1.219 | 0.597 |
| 2c5054d02a47 | shm-ring-hybrid / streaming | 1048576 | 1 / 8 | 5 | 1330 (1311-1348) | n/a | 1002.300 | 6.375 | 3.038 |
| fa0a71874d38 | shm-ring-spin / streaming | 1048576 | 1 / 8 | 5 | 1381 (1370-1383) | n/a | 787.900 | 6.312 | 2.961 |
| 08d7833170f5 | shm-ring-hybrid / windowed | 1048576 | 1 / 8 | 5 | 735 (642-863) | n/a | 4369.800 | 6.141 | 4.747 |
| 107361d9c99b | shm-ring-spin / windowed | 1048576 | 1 / 8 | 5 | 908 (754-922) | n/a | 2089.600 | 9.547 | 4.442 |
| 1052b09c2e2f | shm-ring-hybrid / streaming | 1048576 | 256 / 8 | 5 | 1422 (1418-1425) | n/a | 6571.200 | 6.125 | 2.874 |
| df755a805181 | shm-ring-spin / streaming | 1048576 | 256 / 8 | 5 | 1416 (1400-1425) | n/a | 6614.300 | 6.141 | 2.875 |
| 7af0eb34fc1a | shm-ring-hybrid / windowed | 1048576 | 256 / 8 | 5 | 938 (921-974) | n/a | 14211.200 | 4.719 | 4.204 |
| b6f391faacd9 | shm-ring-spin / windowed | 1048576 | 256 / 8 | 5 | 998 (968-1009) | n/a | 4465.100 | 8.719 | 4.060 |
| bb459e8ac38e | shm-ring-hybrid / streaming | 1048576 | 8 / 8 | 5 | 1420 (1391-1425) | n/a | 6039.600 | 6.141 | 2.874 |
| 7cd80491c95b | shm-ring-spin / streaming | 1048576 | 8 / 8 | 5 | 1394 (1329-1413) | n/a | 9461.900 | 6.234 | 2.899 |
| feb385ac8ff1 | shm-ring-hybrid / windowed | 1048576 | 8 / 8 | 5 | 963 (831-987) | n/a | 14984.300 | 4.688 | 4.150 |
| 053c30cd52b9 | shm-ring-spin / windowed | 1048576 | 8 / 8 | 5 | 997 (963-1009) | n/a | 3750.000 | 8.734 | 4.058 |
| 70fbd8f4ad81 | shm-ring-hybrid / windowed | 65536 | 1 / 8 | 5 | 11636 (11423-13196) | n/a | 209.100 | 0.766 | 0.592 |
| 23f7c05537ee | shm-ring-hybrid / windowed | 65536 | 8 / 8 | 5 | 14538 (11516-14938) | n/a | 1851.700 | 0.688 | 0.574 |
| e1f132852e1c | shm-ring-hybrid / windowed | 65536 | 256 / 8 | 5 | 14491 (13648-14887) | n/a | 845.700 | 0.719 | 0.587 |
| a6a9d944ef99 | shm-ring-spin / windowed | 65536 | 1 / 8 | 5 | 14658 (14204-15030) | n/a | 110.100 | 1.266 | 0.592 |
| 1d5747da320e | shm-ring-spin / windowed | 65536 | 256 / 8 | 5 | 15809 (15004-15960) | n/a | 289.700 | 1.250 | 0.593 |
| dca8b3667876 | shm-ring-spin / windowed | 65536 | 8 / 8 | 5 | 15020 (14892-15268) | n/a | 299.400 | 1.312 | 0.622 |
