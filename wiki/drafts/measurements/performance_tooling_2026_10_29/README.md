# Rust benchmark comparison tooling - 2026-10-29

Outcome: a usable first Rust comparison runner in `tools/benchmark`, alongside
the existing Python optimizer-record workflow. It compares identified commands
with retained warmups and alternating A/B order, requires authored result
oracles, distinguishes process latency from child-reported timed work, and
retains failed observations instead of publishing their timings as success.
This is instrument validation, not a compiler or generated-code speedup claim.

Fixed implementation base: `0add3ad35224b3b037f3952d652453236310673c`.
Branch: `codex/performance-tooling`.
Host: Windows x86_64, shared with other work; no contention isolation.
Toolchain: `rustc 1.100.0-nightly (a69a63265 2026-09-03)`.
[Source identities](source-identities.json) bind the final Rust, configuration,
plans, Cargo files, and guide. Each journal independently binds its actual
runner, subject binaries, plan, and declared source/library inputs.

## Focused validation

| Check | Result |
| --- | --- |
| `mbx nextest run -p omega-bench --features fixtures` | 14 passed; run `b3426223-74ea-46f3-83e7-080e1587ce59` |
| `mbx clippy -p omega-bench --all-targets --features fixtures --no-deps -- -D warnings` | Passed |
| `mbx build -p omega-bench --features fixtures` | Passed |
| `cargo fmt -p omega-bench -- --check` | Passed |
| `python tools/tests/test_benchmark.py -q` | 46 passed; Python implementation and records unchanged |
| `git diff --check` | Passed |

Build/test chains used `tools/build_slot.py`. The nextest override serializes
the process instrument tests even in an ordinary focused invocation. The
existing `bounded-process` dependency warns about deprecated `fetch_update` on
Windows; the new crate passes warnings-as-errors Clippy without changing it.
No whole-workspace or corpus run was needed: compiler semantics, libraries,
corpus fixtures, and the old measurement implementation were not edited.

Tests include actual child processes for exact output and checksum checks,
native compiler/artifact transport, preserved warmups and ABBA ordering,
overwrite refusal, input mutation/deletion, failed compilation, wrong exit,
timeout, output overflow, unoptimized producer refusal, and stale published
artifact-path refusal. Argument controls prevent a compiler subcommand or
second source from replacing the authored product. Statistical checks recover
a known paired difference under simulated host drift and reject malformed
samples. These do not prove the bootstrap's assumptions hold on a busy host.

The first instrument test run had five failures: a 200 ms child-cleanup budget
was too small under concurrent Windows test execution. A 2 s cleanup allowance
within the same overall deadline, adequate test deadlines, and serialized
process cases resolved that cause. Refusal tests then required the intended
oracle/timeout/overflow reason, rather than accepting any nonzero harness exit.

## Executable observations

- [Compiler A/A journal](compiler-aa/journal.jsonl) and
  [report](compiler-aa/report.json): one warmup per role and two measured pairs
  on `samples/cli/facts/length/local/main.omg`. All six images have SHA-256
  `fcd8851bc939f80fbbf33fc476299334f5999603647fec53e4c668860a9ea016`;
  all six executed with exit 0 and empty stdout/stderr. This is a positive
  instrument check with insufficient pairs for inference. The plan forces the
  same explicitly hashed library root on both roles.
- The retained producer has SHA-256
  `a8aa73f50cc8b24ff2a7c25fc96592247d2d3812d9d2ca4d8843d302acc1fcfa`
  and reports effective opt-level 3. Its `--version` says `unrecorded`; its
  source revision is therefore not inferred from the working checkout or the
  directory from which it was copied. The positive result establishes that
  retained binary's behavior on these exact inputs, not a freshly built main.
- [Runtime A/A journal](runtime-aa/journal.jsonl) and
  [report](runtime-aa/report.json): one warmup per role and twelve measured
  pairs of the checked wrapping-square-sum fixture. Every observation agreed
  on one million work units and checksum `333332833333500017`. The within-run
  interval crosses zero. This is a Rust instrument fixture, not generated Omega
  code or evidence of a runtime optimization.
- [Older-producer refusal](old-producer-refusal/journal.jsonl) retains a real
  compiler exit 1 and the [syntax diagnostic](old-producer-refusal/compiler.stderr).
  Binary `76c1a318b806fa4eaf2390b10b5f829426fe3a923d5b399f90b5508cb6b2cd6c`
  did not accept the nested-machine sample. No success report was written.
  The source/oracle was preserved and a compatible retained producer supplied
  the positive observation; this tooling assignment did not rewrite the fact
  to accommodate the old compiler.

The final compiler smoke uses runner SHA-256
`07cc8002724592b20b558f6284312f1228f9c011f4746e924d62d76853d10f61`.
The retained runtime A/A uses earlier runner SHA-256
`8e50b89d8b00725c8a29c3e5cbfec6d71a5bb6685aecc9f062f6ecac748fa4d2`:
subsequent production changes only restrict compiler arguments and preserve
compiler profile/version logs. They do not change the runtime branch, timed-work
oracle, process runner, or statistics. Its original identity is preserved;
final-source runtime checks also pass in the fourteen-test run.

The two completed journals' terminal report digests were checked against their
retained report bytes. Native exit and empty-stream hashes were read back from
every compiler sample. Full local run directories, including native images,
remain under ignored `build/performance/validation/`; the committed journals
preserve their identities without checking generated executables into Git.

## Remaining limits

Peak memory is explicitly unavailable. Hardware counters, allocation profiling,
automatic dependency discovery, historical regression scheduling, and broader
representative runtime workloads remain open. Declared inputs are hashed around
each observation; transient changes restored between checks cannot be ruled out.
Process timing includes launch/capture/cleanup overhead. Child-region timing
depends on correct child instrumentation and checked observable computation.
Cross-host native coverage was not run. No remote publication was requested.
