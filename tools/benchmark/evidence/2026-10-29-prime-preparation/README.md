# Prime-counting preparation

The [previous run](../2026-10-29-windows/README.md) stopped package preparation
after twenty minutes without reaching a timed compile or native execution.
This assignment fixes that route and retains its separate setup measurement.

## Cause and repair

The late [noninvasive stack snapshot](late-stack.txt), taken after 200 seconds,
places the old producer inside `infer_synchronous_invocations`. Early snapshots
were in structural proof checking; that was an earlier phase, not evidence that
proof normalization caused the eventual nontermination.

Realized invocation targets represent a set. Each fixed-point pass rebuilt its
vector in discovery order, then compared vectors for equality. In a recursive
graph the membership could stabilize while order alternated. Ordinary invocation
targets already had canonical ordering. Realized targets now use the same rule:
requirements, positional abstract targets, and abstract services each retain
their identity and stable ordering. Target membership is unchanged.

The six-node, three-target graph retained by the concurrent
`contract-query-publication/build/preparation-convergence` investigation supplied
the small regression and canonicalization candidate. This session independently
replayed prime counting, captured its late stack, and exercised the real merge
function: [before](regression-before.txt) fails on order oscillation; [after](regression-after.txt)
converges with all three targets present. The test uses no Omega source input;
it is the bounded graph-algorithm regression. Customer and rejection evidence
use ordinary Omega programs.

The retained old producer is release opt-level 3, hash
`7e8f008562203f0788e1bc276c436493e64ae0e6fde00ec6f46303ae5297a321`,
from `8badab9ee404808dbc2024bbc3534f4d08de8e49`.
Its [240-second replay](baseline-replay.json) was diagnostic and includes debugger
observation time, so it is not a clean timing sample. A fresh producer from
[the fixed current-main base](main-producer.json),
`ad7c6890a8971c1609006e1965ba4e4626f30591`, also exceeded the ten-second probe.
The inference loop and original prime/library sources were unchanged between
those bases. [Candidate identity](candidate-producer.json) and the
[Cargo effective-profile receipt](candidate-profile.json) bind the repaired
Rust inputs and executable.

## Workload compatibility

Once preparation could finish, the original sample produced eighteen
[missing-block-acknowledgement diagnostics](acknowledgement-diagnostics.txt).
The [current acknowledgement contract](../../../../wiki/spec/language/effects.md#call-site-acknowledgements)
applies to terminal transition calls too. The sample migration adds `block` to
those eighteen transfers. Prime bounds, guards, arithmetic, console operations,
and the expected exit of eight stay unchanged. The producer's Rust inputs are
at `12e446dac0026ed89e85cf7d2517e3b7311fdb42`; the migrated workload is at
`f8f4b83177c969d3bdc95256f8ab28be98bdefad`. This is not a matched-source speedup.

## Replay

```text
mbx nextest run --release -p validation --lib -E test(realized_invocation_cycle_converges_without_losing_targets)
python tools/benchmark/benchmark.py measure --root samples/cli/arithmetic/prime_counter/main.omg --target windows_x86_64 --omega <verified-producer> --compile-samples 3 --run-samples 9 --expected-exit 8
omega run --both --keep samples/cli/arithmetic/prime_counter/main.omg
```

Use one build-slot hold for the local chain. A missing project lock measures
update, project-review acceptance and resume under `package_preparation_ms`.
Existing host source and file caches remain uncontrolled. Preparation is one
observation; compiles use fresh output directories; native timing includes
startup. Project LOC excludes the external standard library. Never substitute
the old timeout for a successful timing or present it as a median speedup.

For execution evidence, `omega run --both` uses host mode without `--target`.
An explicit target selects compile-only operation, even when it names the host.
The recorder instead executes its emitted EXE directly.

## Results

| Phase | Observation |
| --- | --- |
| Package preparation | 129.221 s; one missing-lock observation with existing host caches |
| Compile | 0.961 s median; 0.935–5.107 s across three fresh output directories |
| Native process | 57.684 ms median; 55.944–74.988 ms across nine runs |
| Runtime oracle | All nine native exits are 8 |
| Image | 9,728 bytes in all three compile samples |

The [normal benchmark record](../../records/prime_counter__windows_x86_64__default.json)
keeps all samples, verified producer settings and lock/source identity. The
[separate host-mode run](prime-both.txt) reports `native exit 8` and
`interp exit: 8`. Its [invocation record](prime-both.json) retains the producer
hash and CLI status. This establishes execution agreement, not a stdout diff.

The [control records](quality/results.json) retain two valid selected-provider
programs running in both engines and two actual synchronous-cycle rejects,
replayed with both baseline and candidate producers. All eight cases pass their
oracles. Compile-only provisional probes are excluded from this count.

The loop defect is repaired; preparation is still expensive. The dashboard
shows preparation separately and retains the 129-second cost above the charts.
No other host or sustained runtime throughput was measured.

## Research log

2026-10-29: initial searches for `rustc queries memoization key context` and
`Lean isDefEq cache context transparency` returned compiler-query and cache-key
material. Only search excerpts were read; no proof-cache change was adopted.
The late stack instead selected fixed-point convergence as the repair.

Query: `site.doc.rust-lang.org/nightly/nightly-rustc rustc_mir_dataflow JoinSemiLattice join changed fixed point`.
Read the Rust compiler's [dataflow convergence description](https://rustc-dev-guide.rust-lang.org/mir/dataflow.html)
and [lattice implementation](https://doc.rust-lang.org/nightly/nightly-rustc/src/rustc_mir_dataflow/framework/lattice.rs.html).
These support comparing the semantic set state, with change driven by set union;
they do not establish an Omega timing result. The local regression and original
customer replay supply that evidence.
