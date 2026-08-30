# Rung: Beta — structured compiler construction

[Lattice overview](../bootstrap_lattice.md) | Prev: [Alpha](alpha.md) | Next:
[Gamma](gamma.md)

Beta turns raw Alpha construction into a small language suitable for writing
the Gamma compiler. It resembles Omega control flow without importing higher
types, effects, ownership, or proofs.

## Adds

- named procedures, parameters, locals, calls, and recursion;
- explicit stack frames and a fixed calling convention;
- recursively organized state blocks that flatten to one procedure-wide CFG,
  guarded transitions, and fixed-point definite initialization;
- one fixed-width scalar plus raw byte/word memory; and
- deterministic byte I/O and fixed-text emission.

Beta's meaning is the written small-step
[`SEMANTICS.md`](../../../../source/beta/SEMANTICS.md). The canonical Beta
compiler is written in Alpha and emits Alpha tape. Its output is the standalone
compiler used to consume the Beta-written Gamma compiler source.

Nested state braces are an authoring aid for substantial compiler CFGs, not a
second control hierarchy. Labels and locals remain procedure-wide; exact
depth-first lexical flattening and every-path initialization keep the Alpha
compiler and its proof obligations finite and explicit.

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

Its construction certificate derives one exact source-to-payload root equality
from bounded checked assembler lemmas. Pass-one and pass-two partitions compose
through one checked frozen-label-map joint; certificate-selected cuts carry no
authority. Source and payload byte counts are derived artifact observations, so
any edit or size reduction rebuilds and rechecks this certificate rather than
changing the architecture.

The compiler boundary is a closed `Complete` / `Reject` / `Incomplete` /
`InternalFailure` result. Alpha halt values 0/1/2/3 carry only that case tag.
Successful stdout remains the exact runnable payload; failures carry the
versioned diagnostic frame defined by the compiler owner and can never publish
an artifact. `Reject` means a Beta rule was observed to fail. `Incomplete`
means only that this compiler's private profile was insufficient. Generated
program statuses 250 and 251 remain runtime containment outcomes and are not
compiler failures.

## Implementation frontiers

- complete the Alpha-written compiler for the Beta surface required by the
  Gamma compiler;
- guard explicit data/return stacks and expose fail-closed resources;
- check the exact Alpha-assembly-source-to-tape encoding of the compiler
  artifact, then separately prove that compiler correct for arbitrary admitted
  Beta source; and
- escalate rather than extend Alpha locally if realistic Gamma compiler source
  creates unacceptable tape verbosity or performance.
