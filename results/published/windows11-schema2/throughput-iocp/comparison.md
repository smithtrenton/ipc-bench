# Retained schema-v2 measurements

Each group has identical configuration, executable, features and effective affinity. Group IDs link to exact inputs in `summary-v2.json`.

Ranges are launch p10-p90, not request-tail latency. CPU seconds include startup, warmup and shutdown. No universal transport winner is inferred.

| Group | Method / workload | Payload | Depth / capacity | Launches | Rate/s (p10-p90) | RTT us | Observed p99 us | CPU seconds | Min trial s |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 63ca2e3d5a92 | named-pipe-iocp / windowed | 64 | 2 / 64 | 5 | 198521 (126154-288024) | n/a | 56.900 | 0.781 | 0.544 |
| 5ef05886e206 | named-pipe-iocp / windowed | 64 | 8 / 64 | 5 | 300984 (232712-345875) | n/a | 75.300 | 1.422 | 0.936 |
| a96e43534b15 | named-pipe-iocp / windowed | 64 | 64 / 64 | 5 | 346991 (299283-356353) | n/a | 288.000 | 1.953 | 1.213 |
| 02e57aad27a9 | named-pipe-iocp / windowed | 64 | 256 / 64 | 5 | 361086 (338191-369820) | n/a | 988.300 | 1.922 | 1.188 |
| e6a35930debf | named-pipe-iocp / windowed | 64 | 1 / 64 | 5 | 79223 (46752-87592) | n/a | 59.800 | 0.500 | 0.691 |
| fee9b25f33ce | named-pipe-iocp / windowed | 65536 | 1 / 64 | 5 | 7140 (6290-7518) | n/a | 262.700 | 0.875 | 0.892 |
| 574eca911d87 | named-pipe-iocp / windowed | 65536 | 2 / 64 | 5 | 8043 (7938-8532) | n/a | 405.600 | 1.234 | 1.091 |
| 3db97fdaf2c2 | named-pipe-iocp / windowed | 65536 | 256 / 64 | 5 | 8124 (7180-8463) | n/a | 34594.800 | 1.328 | 1.147 |
| f4ecdb6b9dab | named-pipe-iocp / windowed | 65536 | 8 / 64 | 5 | 7959 (7807-8286) | n/a | 2591.400 | 1.344 | 1.185 |
| 0defb70f13b8 | named-pipe-iocp / windowed | 65536 | 64 / 64 | 5 | 8163 (7888-8573) | n/a | 9253.400 | 1.312 | 1.159 |
