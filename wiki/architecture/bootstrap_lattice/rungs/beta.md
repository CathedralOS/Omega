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

## Current implementation

The repository promotes `compiler/beta_compiler.alpha` directly as the Beta
compiler. Its persisted artifact is the one-step output of the Alpha-written
assembler. The historical Beta self-host was deleted after the direct artifact
and focused compiler gate subsumed its useful migration role.

Evidence tied specifically to “Beta source admits the Beta compiler” does not
apply to this edge and cannot be reused as authority.

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
- check the exact Alpha-assembly-source-to-tape encoding of the compiler
  artifact, then separately prove that compiler correct for arbitrary admitted
  Beta source; and
- escalate rather than extend Alpha locally if realistic Gamma compiler source
  creates unacceptable tape verbosity or performance.
