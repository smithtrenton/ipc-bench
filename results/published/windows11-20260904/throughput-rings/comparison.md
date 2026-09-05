# Retained schema-v2 measurements

Each group has identical configuration, executable, features and effective affinity. Group IDs link to exact inputs in `summary-v2.json`.

Ranges are launch p10-p90, not request-tail latency. CPU seconds include startup, warmup and shutdown. No universal transport winner is inferred.

| Group | Method / workload | Payload | Depth / capacity | Launches | Rate/s (p10-p90) | RTT us | Observed p99 us | CPU seconds | Min trial s |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 5e33575f222f | shm-ring-spin / windowed | 64 | 64 / 64 | 5 | 9054561 (8917310-9262878) | n/a | 12.600 | 2.250 | 1.090 |
| 914cca7ef142 | shm-ring-spin / windowed | 64 | 256 / 64 | 5 | 8553454 (7811531-8902712) | n/a | 13.300 | 2.359 | 1.138 |
| 96ca4b2051cf | shm-ring-spin / windowed | 64 | 8 / 64 | 5 | 9025487 (7730234-9128102) | n/a | 1.700 | 2.266 | 1.125 |
| db22a09eca9f | shm-ring-spin / streaming | 64 | 256 / 64 | 5 | 11339818 (11131259-11768494) | n/a | 8.900 | 2.469 | 1.192 |
| c138c9b34408 | shm-ring-hybrid / windowed | 65536 | 1 / 64 | 5 | 11929 (11436-13373) | n/a | 218.700 | 1.359 | 1.062 |
| 3f5f41bb4bbf | shm-ring-spin / streaming | 64 | 2 / 64 | 5 | 12293474 (12174496-12453490) | n/a | 0.300 | 2.297 | 1.155 |
| 34ee0129d21f | shm-ring-spin / streaming | 64 | 64 / 64 | 5 | 14902771 (13844940-15088443) | n/a | 7.000 | 2.141 | 1.061 |
| c64acdcd0741 | shm-ring-spin / streaming | 64 | 8 / 64 | 5 | 14569638 (12533282-15101900) | n/a | 0.900 | 2.203 | 1.075 |
| cd03a7dfba5c | shm-ring-hybrid / windowed | 65536 | 8 / 64 | 5 | 13776 (12458-14748) | n/a | 1839.500 | 1.344 | 1.123 |
| 209eb529c324 | shm-ring-hybrid / windowed | 65536 | 2 / 64 | 5 | 13767 (11265-14335) | n/a | 350.000 | 1.422 | 1.200 |
| 34a36fccde62 | shm-ring-hybrid / windowed | 65536 | 64 / 64 | 5 | 12973 (9652-14814) | n/a | 17733.600 | 1.453 | 1.172 |
| b26959ae97a5 | shm-ring-hybrid / windowed | 65536 | 256 / 64 | 5 | 14888 (13228-14986) | n/a | 5147.400 | 1.375 | 1.158 |
| 4a761d12951f | shm-ring-spin / windowed | 65536 | 1 / 64 | 5 | 15269 (15149-15304) | n/a | 111.700 | 2.344 | 1.145 |
| 76bd5ff5c113 | shm-ring-spin / windowed | 65536 | 256 / 64 | 5 | 15819 (15248-15963) | n/a | 305.400 | 2.328 | 1.138 |
| d37e38542c7a | shm-ring-spin / windowed | 65536 | 8 / 64 | 5 | 15821 (15233-16049) | n/a | 319.800 | 2.344 | 1.134 |
| af554650c5e6 | shm-ring-spin / windowed | 65536 | 2 / 64 | 5 | 15602 (13798-15775) | n/a | 252.500 | 2.359 | 1.155 |
| 9a327df41f49 | shm-ring-spin / windowed | 65536 | 64 / 64 | 5 | 15677 (15225-15854) | n/a | 404.700 | 2.328 | 1.151 |
| 2f956552de66 | shm-ring-hybrid / streaming | 65536 | 1 / 64 | 5 | 21629 (20872-21830) | n/a | 60.300 | 2.203 | 1.093 |
| a859abe27f2a | shm-ring-spin / streaming | 65536 | 1 / 64 | 5 | 21728 (21237-21999) | n/a | 52.400 | 2.422 | 1.181 |
| 36f09c4dea77 | shm-ring-spin / streaming | 65536 | 2 / 64 | 5 | 22557 (21802-22644) | n/a | 104.600 | 2.375 | 1.161 |
| 87974bcec4be | shm-ring-hybrid / streaming | 65536 | 64 / 64 | 5 | 22406 (21751-22580) | n/a | 3243.300 | 2.391 | 1.171 |
| 4bd39640d5c0 | shm-ring-hybrid / streaming | 65536 | 8 / 64 | 5 | 22388 (22143-22530) | n/a | 411.600 | 2.406 | 1.177 |
| 1f325d5c4e2c | shm-ring-hybrid / streaming | 65536 | 256 / 64 | 5 | 22568 (22326-22681) | n/a | 3067.600 | 2.422 | 1.189 |
| 6e047b2f3536 | shm-ring-hybrid / streaming | 65536 | 2 / 64 | 5 | 22541 (22475-22682) | n/a | 110.000 | 2.422 | 1.189 |
| a4d00bd7119a | shm-ring-spin / streaming | 65536 | 8 / 64 | 5 | 22474 (22393-22529) | n/a | 382.900 | 2.438 | 1.203 |
| 619c5b475972 | shm-ring-spin / streaming | 65536 | 64 / 64 | 5 | 22484 (22432-22870) | n/a | 2938.100 | 2.406 | 1.189 |
| 5546451c582e | shm-ring-spin / streaming | 65536 | 256 / 64 | 5 | 22557 (22401-22718) | n/a | 3013.700 | 2.438 | 1.199 |
| 92996e5962dc | shm-ring-hybrid / windowed | 64 | 2 / 64 | 5 | 2618429 (2430462-2658839) | n/a | 1.100 | 2.188 | 1.093 |
| 11b97d85ded8 | shm-ring-hybrid / windowed | 64 | 1 / 64 | 5 | 2604145 (2539278-2635488) | n/a | 0.600 | 2.234 | 1.110 |
| 866ac4a97745 | shm-ring-hybrid / windowed | 64 | 64 / 64 | 5 | 2597865 (2556026-2645857) | n/a | 37.800 | 2.297 | 1.137 |
| 30039833fcff | shm-ring-hybrid / windowed | 64 | 8 / 64 | 5 | 2620173 (2468672-2666370) | n/a | 5.400 | 2.375 | 1.184 |
| d182975ada07 | shm-ring-hybrid / windowed | 64 | 256 / 64 | 5 | 2642432 (2625694-2670645) | n/a | 38.100 | 2.391 | 1.188 |
| 18f2d045e487 | shm-ring-hybrid / streaming | 64 | 1 / 64 | 5 | 3551007 (3156272-3587379) | n/a | 0.400 | 2.141 | 1.054 |
| 05ddf233eafb | shm-ring-hybrid / streaming | 64 | 256 / 64 | 5 | 3603868 (3533924-3623546) | n/a | 0.500 | 2.312 | 1.153 |
| e8aa240720f9 | shm-ring-hybrid / streaming | 64 | 8 / 64 | 5 | 3589690 (2626949-3671506) | n/a | 0.400 | 2.328 | 1.158 |
| e95ead1c33f3 | shm-ring-hybrid / streaming | 64 | 64 / 64 | 5 | 3638660 (3528690-3740367) | n/a | 0.500 | 2.359 | 1.160 |
| 1727fbf6ea9b | shm-ring-hybrid / streaming | 64 | 2 / 64 | 5 | 3555119 (3488456-3651265) | n/a | 0.400 | 2.438 | 1.197 |
| 18f44989c270 | shm-ring-spin / windowed | 64 | 1 / 64 | 5 | 4150435 (4071933-4252811) | n/a | 0.400 | 2.125 | 1.060 |
| f0dbcbfb55ed | shm-ring-spin / streaming | 64 | 1 / 64 | 5 | 6202883 (6151587-6374042) | n/a | 0.200 | 2.344 | 1.145 |
| 5862cd9037db | shm-ring-spin / windowed | 64 | 2 / 64 | 5 | 8013664 (7081067-8318884) | n/a | 0.400 | 2.312 | 1.130 |
