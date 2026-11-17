# Rust parser layout and capacity comparison — 2026-11-16

The parser now starts at `parser.rs` and exposes grammar domains directly.
The paired benchmarks found no reliable speed change from this reorganization.
An experiment reserving eight expression frames also showed no reliable gain,
so it was removed. Token and syntax-tree storage are unchanged.

## Timing

Negative paired change means less time. These are parse-only nanoseconds per
input byte: tokens are prepared before timing, while syntax-tree creation and
destruction remain inside it. Each comparison uses 16 alternating AB/BA pairs,
one warmup per executable, and a paired-median bootstrap interval (4,000 resamples).
The percentage is the median of paired changes, not the ratio of the two medians.

| Input | Candidate | Before ns/byte | After ns/byte | Paired change | 95% interval |
| --- | --- | ---: | ---: | ---: | --- |
| authored | layout | 72.79 | 74.66 | -0.26% | [-1.81%, +1.47%] |
| authored | reserved | 72.98 | 73.52 | -0.82% | [-2.96%, +1.67%] |
| small | layout | 3.11 | 3.11 | -0.26% | [-1.42%, +1.91%] |
| small | reserved | 3.10 | 3.10 | -0.31% | [-2.46%, +3.08%] |
| numeric | layout | 77.46 | 76.47 | -1.42% | [-3.21%, +0.95%] |
| numeric | reserved | 80.20 | 80.45 | +0.01% | [-4.21%, +2.07%] |

`layout` compares the original parser against the reorganization. `reserved`
compares that reorganization against `Machine { frames: Vec::with_capacity(8),
..Machine::default() }` at expression entry. All intervals include zero; none
establishes a speedup or slowdown. This small trial does not show that parser
allocation is cheap, only that this reservation did not earn its complexity.

Peak resident memory is measured for the whole process, including untimed lexing
and snapshot checks, not isolated parser scratch. Median peak MiB:

| Input | Candidate | Before | After |
| --- | --- | ---: | ---: |
| authored | layout | 11.33 | 11.34 |
| authored | reserved | 11.29 | 11.22 |
| small | layout | 5.56 | 5.56 |
| small | reserved | 5.56 | 5.57 |
| numeric | layout | 9.80 | 9.81 |
| numeric | reserved | 9.81 | 9.61 |

## Inputs and correctness

The six authored files are the frozen copies used in the lexer investigation;
they total 151,835 bytes. `small` is the standalone sample; `numeric` is the two
float/numeric-conversion library files. Calibration selected 21, 76,220 and 39
repeats respectively, targeting roughly 250 ms per measured invocation.

| Authored path | Bytes | Frozen SHA-256 |
| --- | ---: | --- |
| `samples/cli/basics/print_number/main.omg` | 4596 | `a837df1162fb49a9049996e36068cee1ddfe729f29f8952b13139106ad2c4c5f` |
| `samples/cli/basics/standalone/main.omg` | 764 | `e176221071f912c2c7f1478b8f880aa49efefa3763c7708b2503aaa8fb86d847` |
| `source/library/core/math.omg` | 16717 | `ebecd9c90cc7dd2b2ea7b08929bf6d61850fd5dc24d15259f2e09381db5517fa` |
| `source/library/core/compact_binary.omg` | 49671 | `93816adddbab610f55be8504ddc085a7f248540d1a34acb00cb62c9aca7e8529` |
| `source/library/core/float_operations.omg` | 46619 | `4c7b08f98871c2ce097565838201cec27b47a77f0a97a00ba9c6363a01671b34` |
| `source/library/core/numeric_conversion.omg` | 33468 | `c79adc2f5e26688601575fd4a5abf1f1346b7b67f85b97e3f661152915561a9a` |

Every invocation checked the same fingerprint of source text, token kinds,
spans, decoded token bytes, root counts, import ranges and existing syntax
snapshots. Snapshot generation runs outside the timer. The snapshot covers root
syntax, not every internal table; it supplements rather than replaces the tests.
All 306 existing parser tests passed before and after extraction. A separate
comparison preserved their names and exact literal inputs and expectations.

## Source and host

Baseline: `ac37cf3c3e62080c0a2a4ce0014de85de4468202`, with only the same
`benchmarks/parser_timing.rs` harness and its Cargo registration added.
The layout executable uses the production source in this change. The rejected
reservation differs only at expression-machine construction. Build commands:

```text
mbx bench -p tokens-to-syntax-trees --bench parser_timing --no-run
omega-bench.exe compare <plan.json> <new-result-directory>
```

All builds used `nightly-2026-09-04`, release/bench defaults, mbx 1.29.2,
`CARGO_BUILD_JOBS=18`, `MBX_SCHEDULER_CPUS=18`, Idle priority and affinity
`0xFFFFC0`. Measurements ran at Idle priority on logical CPU 8 (`0x100`) on
Windows x86_64, Intel Core i9-12900K, 24 logical CPUs. No builds from this session
ran during timing. This was a shared workstation without exclusive CPU access;
outliers remain in the results. Within-run intervals do not cover host drift,
contention, other architectures or workload selection.

| Executable | SHA-256 |
| --- | --- |
| `parser-baseline.exe` | `34c3453600103864f3c427316cf53054f96ed4a20a9d45c888431cb247ebc033` |
| `parser-layout.exe` | `a0126f6ea421ad68dbd9788b02ff4488b1d97881fb08b93d31e289aba45d26d2` |
| `parser-reserved.exe` | `d75baeea72a7803d23e4de146e990b76a372cfc07f9aadbb1e8dbe1f4ed3dc57` |

Raw journals, per-process resources, plans, source hashes, immutable executables
and inputs are retained locally under
`C:/Users/zacha/.codex/artifacts/rust-parser-2026-11-16/`.
The source-hash manifest identifies the measured layout independently of commit
metadata. The benchmark harness remains in the parser's `benchmarks/` directory;
[the parser map](../../../omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/README.md)
explains its timing boundary and the upstream compiler comparison.
