# Retained schema-v2 measurements

Each group has identical configuration, executable, features and effective affinity. Group IDs link to exact inputs in `summary-v2.json`.

Ranges are launch p10-p90, not request-tail latency. CPU seconds include startup, warmup and shutdown. No universal transport winner is inferred.

| Group | Method / workload | Payload | Depth / capacity | Launches | Rate/s (p10-p90) | RTT us | Observed p99 us | CPU seconds | Min trial s |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 8d08c1c3376b | named-pipe-iocp / windowed | 65536 | 64 / 64 | 5 | 8472 (7610-8530) | n/a | 8424.400 | 1.344 | 1.199 |
| f4a0482b93b7 | named-pipe-iocp / windowed | 64 | 8 / 64 | 5 | 272749 (269474-297564) | n/a | 116.900 | 1.703 | 1.334 |
| 4be7e83bdf2a | named-pipe-iocp / windowed | 64 | 256 / 64 | 5 | 375523 (366579-377743) | n/a | 1076.100 | 1.812 | 1.182 |
| cc4fa8485047 | named-pipe-iocp / windowed | 64 | 64 / 64 | 5 | 362441 (359553-368927) | n/a | 300.300 | 2.047 | 1.309 |
| 59e511a745d5 | named-pipe-iocp / windowed | 64 | 1 / 64 | 5 | 41916 (39295-43187) | n/a | 124.800 | 0.641 | 1.514 |
| 5b23cad4dd95 | named-pipe-iocp / windowed | 65536 | 1 / 64 | 5 | 5697 (5478-5989) | n/a | 608.700 | 1.078 | 1.208 |
| edf0a69fd053 | named-pipe-iocp / windowed | 64 | 2 / 64 | 5 | 106105 (95306-116331) | n/a | 102.900 | 0.453 | 0.655 |
| 0ea5515ed38b | named-pipe-iocp / windowed | 65536 | 2 / 64 | 5 | 7866 (7132-8056) | n/a | 574.700 | 1.219 | 1.157 |
| d71a393129fc | named-pipe-iocp / windowed | 65536 | 8 / 64 | 5 | 8270 (8128-8396) | n/a | 1640.000 | 1.312 | 1.176 |
| f1fd589e4f03 | named-pipe-iocp / windowed | 65536 | 256 / 64 | 5 | 8387 (8317-8403) | n/a | 33050.000 | 1.297 | 1.182 |
