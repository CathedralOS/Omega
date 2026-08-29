# Rung: Beta — structured compiler construction

[Lattice overview](../bootstrap_lattice.md) | Prev: [Alpha](alpha.md) | Next:
[Gamma](gamma.md)

Beta turns raw Alpha construction into a small language suitable for writing
the Gamma compiler. It resembles Omega control flow without importing higher
types, effects, ownership, or proofs.

## Adds

- named procedures, parameters, locals, calls, and recursion;
- explicit stack frames and a fixed calling convention;
- CFG-shaped state blocks and guarded transitions;
- one fixed-width scalar plus raw byte/word memory; and
- deterministic byte I/O and fixed-text emission.

Beta's meaning is the written small-step
[`SEMANTICS.md`](../../../../source/beta/SEMANTICS.md). The canonical Beta
compiler is written in Alpha and emits Alpha tape. Its output is the standalone
compiler used to consume the Beta-written Gamma compiler source.

## Current migration

The repository currently promotes the self-hosted `bc.beta` fixed point as the
Beta compiler and keeps the Alpha-written implementation under
`compiler/cold-start/bc-alpha.alpha`. D11 reverses that authority relationship:
the Alpha-written compiler must become or construct the complete canonical Beta
compiler edge. The `bc.beta` fixed point may remain valuable differential and
self-host evidence, but it is not an extra required rung.

Existing source/artifact refinement work against the persisted Beta tape
remains useful only where its exact source proposition matches the new canonical
Alpha-written compiler edge. Evidence tied specifically to “Beta source admits
the Beta compiler” must be reclassified rather than silently reused.

## Must not contain

No algebraic data types, pattern matching, safe ownership, regions, effects,
generics, or proof language. Beta does not parse Delta or manufacture Delta
semantics. Its only upward compiler responsibility is Gamma.

## Canonical artifact

```text
Alpha-written Beta compiler source
  └─ audited Alpha construction/refinement ─▶ beta_compiler_bytecode.tape
```

The tape is platform-independent. Native seeds merely execute it.

## Implementation frontiers

- complete the Alpha-written compiler for the Beta surface required by the
  Gamma compiler;
- guard explicit data/return stacks and expose fail-closed resources;
- retain useful existing Beta/Alpha refinement lemmas under the corrected
  source subject; and
- escalate rather than extend Alpha locally if realistic Gamma compiler source
  creates unacceptable tape verbosity or performance.
