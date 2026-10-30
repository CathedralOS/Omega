# Prime preparation: repeated captured-result fact scans

Cold package preparation falls from **89.750 s to 52.420 s median (41.6%)**
in the controlled comparison. The ordinary recorder's final observation is
**49.982 s preparation, 1.813 s median compilation, and 142.354 ms median
native process time**. Preparation remains substantial; this does not claim
subsecond package review or faster prime-counting instructions.

## Fixed inputs and review scope

The fixed main base is `e91e31365edf6e760f6199a04d444e727c902cc0`.
The baseline adds only profiler scopes in
`3e0d10796805c66c16ca0a2499c65d399de89522`; the repaired compiler inputs are
`9faf48f97ab27e2be5dce33d3901826be50c066c`. Both preserved producers are
release opt-level 3, with binary-bound Cargo receipts under [producers](producers).
Prime's source, bound, Build declaration, dependency and exit oracle are unchanged.
The complete bundled-library Git tree is identical between producers.
Raw trial `source_revision` identifies the project checkout; each producer's
separate receipt identifies the compiler source actually built.

This work continues the [previous resolution repair](../2026-10-29-prime-resolution/README.md).
Full dependency checking here belongs to explicit package review/acceptance.
Ordinary accepted compilation already loads the consumer's imports, as required
by [package acceptance](../../../../wiki/spec/packages/acceptance.md) and
[package review](../../../../wiki/spec/packages/review.md). An absent lock grants
no accepted dependency program-entry binding. The retained
[historical unlocked probe](diagnostics/unlocked-1793331186322593000/result.json.gz)
uses the previous `7a0b63d1ff9` producer and refuses that binding; it is a route
diagnostic, excluded from the current timing comparison. No package policy,
source-output limit, proof obligation or acceptance finding was removed.
Recorded benchmark acceptance is not a human source-code audit.

## Attribution and repair

The new baseline profile separates nominal field checks from contextual cleanup.
It observes **143,619 captured-result membership queries** repeatedly scanning
the same captured contexts. The worker stacks corroborate that scan. These are
inclusive sums across workers, not elapsed preparation times:

| Scope | Baseline | Repaired |
| --- | ---: | ---: |
| Captured-result membership | 317,372 ms | 639 ms |
| Write chronology within those queries | 98 ms | 38 ms |
| Nominal input checks | 321,795 ms | 4,093 ms |

The repaired run still asks all 143,619 questions, preparing only **272** source
promise rosters. `CapturedResultMemberships` borrows one immutable facts owner
and lives inside a call's input checks or an exit's result checks. Its key retains
the recorded operand's arena index and generation. Unrecorded scratch operands
use an uncached roster; foreign owners refuse. Every promise retains dependencies
from its own original context, never a union across contexts. Each query still
compares the exact projection and domain instance, authenticates observation
chronology, and checks the complete intervening write footprint. Empty dependency
rosters establish nothing. No proof verdict or caller authority is cached.

The profiler rules out write-history construction and nominal-domain proof as
the dominant cost in this region. No predicate-order or broader proof-cache
experiment was retained. The repair stops at removing the measured rescan and
verifying the complete review, execution and refusal controls. Remaining cold
review includes existing numeric-prefix observation, typed preparation and flow
construction; their costs are not hidden in the compilation column.

## Controlled comparison and final benchmark

The [comparison](comparison.json) runs **baseline, repaired, repaired, baseline**.
Every trial has a newly created private `LOCALAPPDATA` package/source cache and
starts without the project's lock or generated review directory. Existing state
is preserved and restored. OS file caches and other host activity are uncontrolled.
No valid slow trial was discarded. Debugger/profiler runs are separate.

| Producer | Cold preparation, seconds | Median |
| --- | --- | ---: |
| Baseline | 93.697, 85.804 | 89.750 |
| Repaired | 50.886, 53.954 | 52.420 |

All four clean trials, plus both diagnostic profiles, execute prime in real host
mode with `run --both --keep`: native and Terminal Psi both return **8**. Their
native image and Terminal artifact hashes are identical. Accepted locks also
match after replacing only the compiler-identity hash lines for comparison;
raw locks and hashes remain preserved. Normalized locks are never compiler inputs.

The [ordinary benchmark record](benchmark-record.json) uses the supported recorder,
a separate fresh cache, three compilation samples and nine native process samples.
Compilation samples are **8.817, 1.728, 1.813 s**; the first sample remains included.
Native samples span **90.604–480.011 ms**, with **9/9 exits of 8**. Runtime includes
startup. Project LOC excludes std, and package preparation is reported separately.
The native image remains **9,728 bytes**. The [raw recorder output](final-raw-record.json.gz)
and [transcript](final-benchmark.log.gz) retain all measurements; the submitted record
adds only a visible warning explaining cold review versus prepared compilation.

```text
python tools/benchmark/benchmark.py measure --root samples/cli/arithmetic/prime_counter/main.omg --target windows_x86_64 --omega <verified-producer> --compile-samples 3 --run-samples 9 --expected-exit 8
omega run --both --keep samples/cli/arithmetic/prime_counter/main.omg
```

## Correctness and evidence custody

- [56 source-bound cases](quality-results.json.gz): 26 native/Terminal execution
  pairs and 30 intended refusals across both producers. Coverage includes all nine
  borrowed-result field places, indexed sibling writes at four places, an owned
  copy preceding a later repair, and mismatched indexed domain instances.
- Every positive uses the authored source and Build when provided, executes its
  runtime oracle, and has identical native and Terminal artifacts across producers
  at the same unchanged source path. Every negative checks its authored diagnostic.
- [Pinned qualification](qualification-state.json.gz) records the complete compiler,
  library, toolchain, fixture and harness closure, verified producer profiles,
  raw streams and immutable observation receipt, with exit 0 and no invalidations.
- [16 crate unit tests](unit-tests.log.gz) passed. [Clippy](clippy.log.gz) completed;
  existing warnings remain. Other hosts and the full corpus were not rerun.

An initial candidate build overlapped restoration of two unrelated recursive
rustfmt edits. mbx detected changing inputs; that producer was rejected and a
stable clean build replaced it before measurement. Its raw build log is retained
compressed. Harness pilots also caught a missing Python name, stale acceptance
after rewriting a reused test project, and differing Terminal artifacts from
different project namespaces. Fresh matched projects, unchanged source files,
and separate logs fixed the harness; no pilot is counted among the 56 cases.

The [scripts](scripts) are snapshots of helpers executed from
`build/prime-preparation-3`, not standalone tools at their archived location.
Preserved executables and generated products remain in that ignored worktree
directory. Archived receipts/streams preserve their original bytes and paths;
the supported benchmark commands above remain the normal refresh entrance.

## Research

2026-10-29: query `site.postgresql.org "order_qual_clauses" "cost"`.
Read PostgreSQL's [actual clause-ordering implementation](https://github.com/postgres/postgres/blob/master/src/backend/optimizer/plan/createplan.c)
and its restrictions on reordering security-sensitive predicates. This suggested
checking selective filtering before expensive proof work, not permission to
change Omega proof order. The new profile instead located repeated result-fact
scans, so predicate reordering was not selected. The invocation-local evidence
plan follows the existing Omega observation-owner pattern; the measured comparison
and unchanged outputs decide retention, not the external example.

Raw receipts, streams, locks, producer profiles and helper snapshots are gzip-compressed so Git cannot rewrite their line endings. Decompress before checking original byte digests or restoring a helper. Presentation JSON remains directly readable.
