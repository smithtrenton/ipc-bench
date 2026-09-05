# Retained schema-v2 measurements

Each group has identical configuration, executable, features and effective affinity. Group IDs link to exact inputs in `summary-v2.json`.

Ranges are launch p10-p90, not request-tail latency. CPU seconds include startup, warmup and shutdown. No universal transport winner is inferred.

| Group | Method / workload | Payload | Depth / capacity | Launches | Rate/s (p10-p90) | RTT us | Observed p99 us | CPU seconds | Min trial s |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| de517c9a8d72 | py-multiprocessing-pipe / round-trip | 32704 | 1 / - | 5 | 20128 (10691-24110) | 49.682 | n/a | 1.500 | 0.407 |
| 438461510bed | py-multiprocessing-queue / round-trip | 32704 | 1 / - | 5 | 7781 (6071-10421) | 128.525 | n/a | 3.422 | 0.923 |
| a003d5f90cf2 | py-shared-memory-events / round-trip | 32704 | 1 / - | 5 | 13609 (11729-14322) | 73.479 | n/a | 1.875 | 0.619 |
| 8183515e3f9c | py-shared-memory-queue / round-trip | 32704 | 1 / - | 5 | 14408 (11245-14940) | 69.408 | n/a | 2.109 | 0.661 |
| cb7a96bbd59c | py-socket-tcp-loopback / round-trip | 32704 | 1 / - | 5 | 24658 (18422-25565) | 40.555 | n/a | 1.484 | 0.385 |
| 63cbd43f4f2a | py-multiprocessing-pipe / round-trip | 4096 | 1 / - | 5 | 16066 (15008-18577) | 62.242 | n/a | 1.812 | 0.468 |
| 077fdc07352a | py-multiprocessing-queue / round-trip | 4096 | 1 / - | 5 | 9352 (7316-10591) | 106.933 | n/a | 2.672 | 0.854 |
| 895c657793c2 | py-shared-memory-events / round-trip | 4096 | 1 / - | 5 | 14869 (9072-17056) | 67.253 | n/a | 1.812 | 0.571 |
| efa4d0e2f74a | py-shared-memory-queue / round-trip | 4096 | 1 / - | 5 | 13905 (8321-17789) | 71.918 | n/a | 2.156 | 0.554 |
| 24c6959cdd3a | py-socket-tcp-loopback / round-trip | 4096 | 1 / - | 5 | 28386 (23345-29133) | 35.228 | n/a | 1.219 | 0.317 |
| efd6fab845cc | py-multiprocessing-pipe / round-trip | 64 | 1 / - | 5 | 45147 (29751-49725) | 22.150 | n/a | 0.797 | 0.185 |
| 6af3cb761d37 | py-multiprocessing-queue / round-trip | 64 | 1 / - | 5 | 14501 (12013-17650) | 68.963 | n/a | 1.891 | 0.547 |
| 9516e8a9cc7f | py-shared-memory-events / round-trip | 64 | 1 / - | 5 | 12919 (10612-18222) | 77.402 | n/a | 1.578 | 0.509 |
| 2b9299acbf9f | py-shared-memory-queue / round-trip | 64 | 1 / - | 5 | 13826 (8610-17371) | 72.328 | n/a | 2.188 | 0.565 |
| ceaaa950fda4 | py-socket-tcp-loopback / round-trip | 64 | 1 / - | 5 | 31682 (24719-34624) | 31.563 | n/a | 1.219 | 0.279 |
| 60b9961682d8 | udp-loopback / round-trip | 32704 | 1 / - | 5 | 30196 (25881-41978) | 33.117 | n/a | 0.781 | 0.237 |
| 3d53bcd9d5fd | anon-pipe / round-trip | 32704 | 1 / - | 5 | 42512 (34161-50175) | 23.523 | n/a | 0.734 | 0.198 |
| 3d125b9b1930 | shm-events / round-trip | 32704 | 1 / - | 5 | 41211 (40121-73231) | 24.266 | n/a | 0.359 | 0.136 |
| 33485de4bfed | shm-semaphores / round-trip | 32704 | 1 / - | 5 | 58258 (51115-76827) | 17.165 | n/a | 0.328 | 0.127 |
| 86642a8b302e | alpc / round-trip | 32704 | 1 / - | 5 | 28802 (17348-42550) | 34.720 | n/a | 0.797 | 0.222 |
| 45f067d7caae | named-pipe-overlapped / round-trip | 32704 | 1 / - | 5 | 54952 (45783-56204) | 18.198 | n/a | 0.531 | 0.163 |
| 4992d57c1898 | tcp-loopback / round-trip | 32704 | 1 / - | 5 | 28910 (18179-33866) | 34.590 | n/a | 1.156 | 0.288 |
| cf070a2dc06c | mailslot / round-trip | 32704 | 1 / - | 5 | 58494 (27354-68318) | 17.096 | n/a | 0.500 | 0.145 |
| e24c3ecee6ee | shm-raw-sync-event / round-trip | 32704 | 1 / - | 5 | 33970 (30422-56323) | 29.438 | n/a | 0.344 | 0.154 |
| cd8d3f93f0dd | named-pipe-message-sync / round-trip | 32704 | 1 / - | 5 | 51825 (39944-53212) | 19.296 | n/a | 0.375 | 0.165 |
| 21de02a16e9f | af-unix / round-trip | 32704 | 1 / - | 5 | 45067 (25551-63302) | 22.189 | n/a | 0.609 | 0.154 |
| 992c54244aac | named-pipe-byte-sync / round-trip | 32704 | 1 / - | 5 | 44618 (31392-53752) | 22.412 | n/a | 0.453 | 0.174 |
| d25211cf607d | rpc / round-trip | 32704 | 1 / - | 5 | 9261 (6591-10709) | 107.977 | n/a | 2.531 | 0.848 |
| 3adb64363359 | udp-loopback / round-trip | 4096 | 1 / - | 5 | 27033 (25557-51285) | 36.991 | n/a | 0.844 | 0.193 |
| 31353c1b40bd | anon-pipe / round-trip | 4096 | 1 / - | 5 | 44043 (37500-60481) | 22.705 | n/a | 0.469 | 0.162 |
| 0f5aa0d630ba | shm-events / round-trip | 4096 | 1 / - | 5 | 45136 (28506-90471) | 22.155 | n/a | 0.172 | 0.105 |
| 129fb031e89b | shm-semaphores / round-trip | 4096 | 1 / - | 5 | 61120 (43246-96531) | 16.361 | n/a | 0.203 | 0.096 |
| ff07d3840535 | alpc / round-trip | 4096 | 1 / - | 5 | 48192 (23158-91892) | 20.750 | n/a | 0.234 | 0.106 |
| 2606b144014f | named-pipe-overlapped / round-trip | 4096 | 1 / - | 5 | 61236 (43288-77184) | 16.330 | n/a | 0.375 | 0.125 |
| df4ac6ecfaef | tcp-loopback / round-trip | 4096 | 1 / - | 5 | 36700 (29058-46515) | 27.248 | n/a | 0.844 | 0.208 |
| 4baea5a9e304 | mailslot / round-trip | 4096 | 1 / - | 5 | 70130 (68068-86829) | 14.259 | n/a | 0.375 | 0.110 |
| dfe0e98adb37 | shm-raw-sync-event / round-trip | 4096 | 1 / - | 5 | 73641 (38122-77920) | 13.579 | n/a | 0.172 | 0.109 |
| 06378b6b3c52 | named-pipe-message-sync / round-trip | 4096 | 1 / - | 5 | 50126 (41648-83329) | 19.950 | n/a | 0.312 | 0.119 |
| 5bee102027f4 | af-unix / round-trip | 4096 | 1 / - | 5 | 62920 (44947-83838) | 15.893 | n/a | 0.344 | 0.119 |
| b2ecb9fb904a | named-pipe-byte-sync / round-trip | 4096 | 1 / - | 5 | 51316 (35472-82333) | 19.487 | n/a | 0.328 | 0.115 |
| 5469e1d28ed4 | rpc / round-trip | 4096 | 1 / - | 5 | 17743 (10324-21761) | 56.361 | n/a | 1.344 | 0.449 |
| 90cc3ccda48c | udp-loopback / round-trip | 64 | 1 / - | 5 | 28049 (18982-42207) | 35.652 | n/a | 0.625 | 0.233 |
| 77c22b563dcb | anon-pipe / round-trip | 64 | 1 / - | 5 | 62160 (54860-65825) | 16.087 | n/a | 0.438 | 0.143 |
| 831877e89a46 | shm-events / round-trip | 64 | 1 / - | 5 | 75659 (26402-86737) | 13.217 | n/a | 0.125 | 0.107 |
| 1a4b4fdfd63b | shm-semaphores / round-trip | 64 | 1 / - | 5 | 73274 (35454-97324) | 13.647 | n/a | 0.172 | 0.088 |
| eaa2a13b812c | named-pipe-overlapped / round-trip | 64 | 1 / - | 5 | 90886 (50814-101529) | 11.003 | n/a | 0.312 | 0.097 |
| 4576fa592d21 | tcp-loopback / round-trip | 64 | 1 / - | 5 | 23476 (21722-42039) | 42.597 | n/a | 1.078 | 0.227 |
| 743f825f6206 | rpc / round-trip | 64 | 1 / - | 5 | 35061 (28765-48760) | 28.521 | n/a | 0.406 | 0.177 |
| 849e66996a12 | named-pipe-byte-sync / round-trip | 64 | 1 / - | 5 | 88562 (78947-94352) | 11.291 | n/a | 0.219 | 0.101 |
| 315bbdf0a0b2 | shm-raw-sync-event / round-trip | 64 | 1 / - | 5 | 59562 (49971-102181) | 16.789 | n/a | 0.219 | 0.092 |
| 792a007e30ce | af-unix / round-trip | 64 | 1 / - | 5 | 67429 (51548-91433) | 14.830 | n/a | 0.391 | 0.111 |
| 3f5829044eb5 | named-pipe-message-sync / round-trip | 64 | 1 / - | 5 | 85850 (53255-96117) | 11.648 | n/a | 0.250 | 0.115 |
| a5c0275812a8 | alpc / round-trip | 64 | 1 / - | 5 | 61820 (43644-104014) | 16.176 | n/a | 0.234 | 0.108 |
| 0ca789cff948 | mailslot / round-trip | 64 | 1 / - | 5 | 87323 (74424-103834) | 11.452 | n/a | 0.312 | 0.112 |
| c04b5735f6ed | shm-raw-sync-busy / round-trip | 64 | 1 / - | 5 | 9929808 (9589593-10551711) | 0.101 | n/a | 0.734 | 0.110 |
| 7b045b166f66 | iceoryx2-request-response-loan / round-trip | 64 | 1 / - | 5 | 1153104 (1139118-1173258) | 0.867 | n/a | 0.812 | 0.117 |
| eeba32c6c531 | shm-mailbox-hybrid / round-trip | 4096 | 1 / - | 5 | 1511908 (1462942-1514107) | 0.661 | n/a | 0.641 | 0.105 |
| 8f480241df47 | shm-ring-hybrid / round-trip | 4096 | 1 / - | 5 | 1451856 (1401690-1487747) | 0.689 | n/a | 0.703 | 0.109 |
| 58ada42332ad | shm-mailbox-spin / round-trip | 4096 | 1 / - | 5 | 1521363 (1482062-1546311) | 0.657 | n/a | 0.688 | 0.109 |
| 783952a1aed9 | shm-raw-sync-busy / round-trip | 4096 | 1 / - | 5 | 1555126 (1522587-1597769) | 0.643 | n/a | 0.703 | 0.110 |
| b1cae66a85dd | iceoryx2-publish-subscribe-loan / round-trip | 64 | 1 / - | 5 | 1488036 (1429733-1511413) | 0.672 | n/a | 0.828 | 0.119 |
| bcc09f2ef0e9 | shm-ring-spin / round-trip | 4096 | 1 / - | 5 | 1598440 (1533481-1642141) | 0.626 | n/a | 0.719 | 0.113 |
| 05aa9d741c5e | shm-mailbox-hybrid / round-trip | 32704 | 1 / - | 5 | 171124 (117110-204260) | 5.844 | n/a | 0.688 | 0.101 |
| cf88368ef2bd | iceoryx2-request-response-loan / round-trip | 32704 | 1 / - | 5 | 184266 (182597-185529) | 5.427 | n/a | 0.875 | 0.115 |
| 454016efa160 | iceoryx2-publish-subscribe-loan / round-trip | 32704 | 1 / - | 5 | 191611 (188455-192766) | 5.219 | n/a | 0.844 | 0.114 |
| b07e8b3eef4b | shm-mailbox-spin / round-trip | 32704 | 1 / - | 5 | 210424 (210270-214729) | 4.752 | n/a | 0.703 | 0.103 |
| 5270c907ff1a | shm-raw-sync-busy / round-trip | 32704 | 1 / - | 5 | 245210 (207496-247529) | 4.078 | n/a | 0.734 | 0.106 |
| 7811cd5490e9 | shm-ring-hybrid / round-trip | 32704 | 1 / - | 5 | 271659 (257944-294027) | 3.681 | n/a | 0.703 | 0.108 |
| f64ac24f2f2d | shm-ring-spin / round-trip | 32704 | 1 / - | 5 | 310188 (251839-320371) | 3.224 | n/a | 0.734 | 0.107 |
| 797378ab723e | shm-ring-hybrid / round-trip | 64 | 1 / - | 5 | 3364510 (3271747-3401894) | 0.297 | n/a | 0.703 | 0.116 |
| 6358d0d71dd2 | shm-mailbox-hybrid / round-trip | 64 | 1 / - | 5 | 3681634 (3646766-3759680) | 0.272 | n/a | 0.656 | 0.108 |
| 751d421dd0bd | copy-roundtrip / round-trip | 64 | 1 / - | 5 | 38988784 (35077895-39689964) | 0.026 | n/a | 0.375 | 0.104 |
| 3f503ef497eb | copy-roundtrip / round-trip | 32704 | 1 / - | 5 | 410218 (407771-415417) | 2.438 | n/a | 0.359 | 0.106 |
| 0dc8ee6d3b68 | copy-roundtrip / round-trip | 4096 | 1 / - | 5 | 5876688 (5773617-6550877) | 0.170 | n/a | 0.281 | 0.083 |
| d13c588a6bed | shm-ring-spin / round-trip | 64 | 1 / - | 5 | 5677603 (5602013-5847368) | 0.176 | n/a | 0.734 | 0.117 |
| 43b56dd02218 | iceoryx2-request-response-loan / round-trip | 4096 | 1 / - | 5 | 718696 (702345-731082) | 1.391 | n/a | 0.859 | 0.118 |
| 0dc0facd5d1c | shm-mailbox-spin / round-trip | 64 | 1 / - | 5 | 7851639 (7498095-8224523) | 0.127 | n/a | 0.719 | 0.110 |
| 2948f5863f97 | iceoryx2-publish-subscribe-loan / round-trip | 4096 | 1 / - | 5 | 823630 (813722-840577) | 1.214 | n/a | 0.812 | 0.113 |
