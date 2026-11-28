# R2.6 Player swapchain and frame pacing

[English](README.md) | [Русский](README.ru.md) | [Benchmark contract](../../bench.md#not-worse)

The [paired Bench result](compare.json) accepted S1 and S2 on the Windows reference host. Five paired Release rounds followed the two unscored prewarm passes. Both builds used the RTX 4080 SUPER with driver 617.42, Immediate presentation, physical 1920×1080 windows, and frame profiling off. S1 entered Playing with a fresh profile. S2 remained in Playing with 20 enemies. Each process contributed 239 UI-observed intervals. Source and executable hashes matched before and after the series.

| Identity | R0.6 baseline | R2.6 candidate |
| --- | --- | --- |
| Source HEAD | `731661c47e181847450e00480e60c0645abd4753` | `692394558fa482377dcf458523be2cafc882516e` |
| Source digest | `2ea46137f7fd19ed5d4a739fa367b634f3a5426ca7d0b1404acfeca07be1d873` | `0293be5d3cb6b6fcf49d93f5e2f38092208d3a7e32b04b41d4e3530d14fb2486` |
| Editor EXE SHA-256 | `29e673db9e632ca491e7d8b2ac91e8c44f6116ee1c0361e43f4a0626edf694b1` | `8d6251cdf7a7d3f275bdacadc0565e2815282cd98ace5b7bd01aee4b6c67fc45` |
| Window backend | WGPU / DX12 | Ash / Vulkan |

| Blocking result | R0.6 median | R2.6 median | Change | Tolerance | Verdict |
| --- | ---: | ---: | ---: | ---: | --- |
| S1 UI-observed frame p95 | 9.486 ms | 9.679 ms | +2.03% | 12.76% | Same |
| S2 UI-observed frame p95 | 9.416 ms | 9.823 ms | +4.33% | 12.05% | Same |
| S2 GameHost step p95 | 4.033 ms | 2.973 ms | −26.30% | 56.28% | Same |
| S1 peak working set | 259.9 MiB | 220.8 MiB | −15.06% | 2.44% | Better |
| S2 peak working set | 228.6 MiB | 191.8 MiB | −16.09% | 2.23% | Better |

The candidate recorded 704–720 successful window `queue_present` calls per S1 run and 685–723 per S2 run. The median run p95 for distinct game-frame intervals at those calls was 9.769 ms for S1 and 9.812 ms for S2. The call CPU time p95 medians were 0.345 ms and 0.330 ms. These rows measure the swapchain submission on the host. Texture rendering, display scanout, and physical input-to-pixel latency are separate measurements.

Both sides recorded runs without input focus. A separate Win32 probe confirmed visible presentation in an unfocused candidate S2 window. The scored session completed all requested cases without a borderline rerun. The verification controller first stopped after prewarm because of a script error and resumed with the same input hashes, EXEs and checkout digest. The comparison wrote `accepted=true` before its redirected CP1251 console rejected a glyph when printing the table. The verifier recomputed the verdict from the saved reports using the same comparator with UTF-8 output and obtained the same result.

## Matched game inputs

[The archive](r06-game-fixtures.zip) contains the exact R0.6 source documents for S1 and S2. The four documents reconstructed from the two executables reproduced their originally recorded prewarm hashes. Their differences covered the SDK data API, the additional available `physics.rapier` module and its version, and the derived Engine profile and lock hashes. Rapier is absent from the active workloads. The candidate replay checks the exact input and the accepted Host document for every scored process. The [Bench procedure](../../bench.md#not-worse) describes the input archive and comparison command.

The source-bound local evidence is under `outputs/bench/2026-11-28/043226-201900-compare/`. It includes the Release build events, exact executables, manifests, per-process logs, and native reports. The separate verifier's receipt and foreground probe are under `outputs/verify-queue/r26-69239455/` on the verification host.
