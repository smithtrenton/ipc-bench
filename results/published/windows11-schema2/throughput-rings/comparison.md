# Retained schema-v2 measurements

Each group has identical configuration, executable, features and effective affinity. Group IDs link to exact inputs in `summary-v2.json`.

Ranges are launch p10-p90, not request-tail latency. CPU seconds include startup, warmup and shutdown. No universal transport winner is inferred.

| Group | Method / workload | Payload | Depth / capacity | Launches | Rate/s (p10-p90) | RTT us | Observed p99 us | CPU seconds | Min trial s |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| b742c3323fa6 | shm-ring-spin / windowed | 64 | 8 / 64 | 5 | 8737848 (8434738-8996379) | n/a | 1.700 | 2.312 | 1.132 |
| 9e9baaa6d980 | shm-ring-hybrid / windowed | 65536 | 1 / 64 | 5 | 11783 (10642-11899) | n/a | 228.300 | 1.016 | 0.889 |
| 3159746ef3ac | shm-ring-hybrid / windowed | 65536 | 2 / 64 | 5 | 14215 (13025-14331) | n/a | 346.600 | 0.844 | 0.779 |
| 96fd7a247eec | shm-ring-spin / streaming | 64 | 256 / 64 | 5 | 11679302 (10536058-11802362) | n/a | 8.100 | 2.344 | 1.163 |
| 1113f6209529 | shm-ring-spin / streaming | 64 | 2 / 64 | 5 | 12415091 (12364306-12526595) | n/a | 0.300 | 2.297 | 1.139 |
| e0f69013627e | shm-ring-hybrid / windowed | 65536 | 8 / 64 | 5 | 14261 (12034-14468) | n/a | 842.200 | 1.172 | 1.029 |
| 8d4161c7e401 | shm-ring-spin / streaming | 64 | 8 / 64 | 5 | 15078280 (14890042-15139020) | n/a | 0.700 | 2.000 | 0.998 |
| 7d0a37066a10 | shm-ring-spin / streaming | 64 | 64 / 64 | 5 | 15175762 (15074240-15212161) | n/a | 6.600 | 2.078 | 1.042 |
| a5d6f884a13d | shm-ring-spin / windowed | 65536 | 1 / 64 | 5 | 14289 (14205-14687) | n/a | 110.600 | 2.391 | 1.145 |
| f6037688815a | shm-ring-spin / windowed | 65536 | 64 / 64 | 5 | 14970 (14721-15066) | n/a | 317.000 | 2.344 | 1.147 |
| e7ff2ac8648a | shm-ring-spin / windowed | 65536 | 8 / 64 | 5 | 15042 (14417-15425) | n/a | 517.300 | 2.344 | 1.128 |
| 73c18ecfc1b4 | shm-ring-hybrid / windowed | 65536 | 64 / 64 | 5 | 14937 (14415-15053) | n/a | 5786.000 | 1.344 | 1.159 |
| 6f15f463589e | shm-ring-hybrid / windowed | 65536 | 256 / 64 | 5 | 14955 (14659-15108) | n/a | 4976.000 | 1.359 | 1.163 |
| 6787db447c86 | shm-ring-spin / windowed | 65536 | 256 / 64 | 5 | 15099 (14618-15126) | n/a | 314.300 | 2.406 | 1.177 |
| 480ec2ef1142 | shm-ring-spin / windowed | 65536 | 2 / 64 | 5 | 15181 (14696-15515) | n/a | 216.000 | 2.438 | 1.173 |
| 4efef51f596a | shm-ring-hybrid / streaming | 64 | 8 / 64 | 5 | 3562765 (3298413-3627646) | n/a | 0.400 | 1.125 | 0.559 |
| 338ac6951485 | shm-ring-hybrid / windowed | 64 | 64 / 64 | 5 | 2594663 (2549336-2597460) | n/a | 34.800 | 1.734 | 0.865 |
| f4e955df4b44 | shm-ring-hybrid / windowed | 64 | 256 / 64 | 5 | 2619779 (2534371-2663749) | n/a | 37.100 | 1.734 | 0.847 |
| ea4ebf8344a6 | shm-ring-spin / streaming | 65536 | 1 / 64 | 5 | 21662 (20568-21733) | n/a | 67.000 | 2.359 | 1.168 |
| 93079d82636a | shm-ring-hybrid / streaming | 65536 | 1 / 64 | 5 | 21456 (20119-21653) | n/a | 89.500 | 2.391 | 1.174 |
| 225b492f7427 | shm-ring-hybrid / streaming | 65536 | 2 / 64 | 5 | 22446 (22228-22467) | n/a | 122.300 | 2.406 | 1.182 |
| dd72a7a2facd | shm-ring-hybrid / streaming | 65536 | 64 / 64 | 5 | 22620 (21586-22724) | n/a | 3426.800 | 2.422 | 1.184 |
| 14ee379e3bda | shm-ring-spin / streaming | 65536 | 2 / 64 | 5 | 22489 (22303-22576) | n/a | 101.400 | 2.438 | 1.194 |
| 7d305e55eee0 | shm-ring-hybrid / streaming | 65536 | 256 / 64 | 5 | 22548 (22132-22618) | n/a | 3148.700 | 2.438 | 1.193 |
| 5847889b0954 | shm-ring-hybrid / streaming | 65536 | 8 / 64 | 5 | 22521 (22200-22629) | n/a | 399.100 | 2.438 | 1.197 |
| ec3313e89c9b | shm-ring-spin / streaming | 65536 | 8 / 64 | 5 | 22558 (22262-22594) | n/a | 388.100 | 2.438 | 1.200 |
| 1257fa34338f | shm-ring-spin / streaming | 65536 | 256 / 64 | 5 | 22586 (22543-22629) | n/a | 2970.900 | 2.422 | 1.200 |
| 1b9e53dbca33 | shm-ring-spin / streaming | 65536 | 64 / 64 | 5 | 22517 (22450-22599) | n/a | 2980.300 | 2.438 | 1.201 |
| f41701dd0419 | shm-ring-hybrid / windowed | 64 | 2 / 64 | 5 | 2561103 (2501180-2587209) | n/a | 1.400 | 2.234 | 1.104 |
| 7d6733c71fc6 | shm-ring-hybrid / windowed | 64 | 8 / 64 | 5 | 2638565 (2620065-2658899) | n/a | 5.500 | 2.188 | 1.086 |
| 621f4a7a9524 | shm-ring-hybrid / windowed | 64 | 1 / 64 | 5 | 2550246 (2504953-2583969) | n/a | 0.600 | 2.328 | 1.155 |
| 216ff8bb7d70 | shm-ring-hybrid / streaming | 64 | 256 / 64 | 5 | 3592446 (3576154-3683668) | n/a | 0.500 | 2.250 | 1.106 |
| c2fe9a052c55 | shm-ring-hybrid / streaming | 64 | 2 / 64 | 5 | 3628188 (3557899-3649927) | n/a | 0.400 | 2.266 | 1.132 |
| c58a4b3ea2e9 | shm-ring-hybrid / streaming | 64 | 1 / 64 | 5 | 3594796 (3519067-3633170) | n/a | 0.400 | 2.438 | 1.209 |
| ed9a331bb950 | shm-ring-hybrid / streaming | 64 | 64 / 64 | 5 | 3633163 (3532563-3666353) | n/a | 0.500 | 2.453 | 1.204 |
| 64d2e9e5055c | shm-ring-spin / windowed | 64 | 1 / 64 | 5 | 4119711 (4009667-4160481) | n/a | 0.400 | 2.500 | 1.249 |
| 92525cb9e5d8 | shm-ring-spin / streaming | 64 | 1 / 64 | 5 | 6313185 (6283467-6329860) | n/a | 0.200 | 2.141 | 1.064 |
| c080205f507b | shm-ring-spin / windowed | 64 | 2 / 64 | 5 | 7879881 (7706996-7965781) | n/a | 0.400 | 2.234 | 1.100 |
| 35b51b95e3f0 | shm-ring-spin / windowed | 64 | 256 / 64 | 5 | 8366313 (8311902-8719221) | n/a | 14.400 | 2.188 | 1.057 |
| 53bff462816a | shm-ring-spin / windowed | 64 | 64 / 64 | 5 | 8568070 (7543201-8994135) | n/a | 14.600 | 2.250 | 1.070 |
