# Rung: Gamma — structured compiler construction

[Lattice overview](../bootstrap_lattice.md) | Prev: [Beta](beta.md) | Next:
[Delta](delta.md)

Gamma turns Beta-level tape construction into a small language suitable for writing
the Delta compiler. It resembles Omega control flow without importing higher
types, effects, ownership, or proofs.

## Adds

- named procedures, parameters, locals, calls, and recursion;
- explicit stack frames and a fixed calling convention;
- recursively organized state blocks that flatten to one procedure-wide CFG,
  guarded transitions, and fixed-point definite initialization;
- one fixed-width scalar plus raw byte/word memory; and
- deterministic byte I/O and fixed-text emission.

Gamma's meaning is the written small-step
[`SEMANTICS.md`](../../../../source/gamma/SEMANTICS.md). The canonical Gamma
compiler is written in Beta and emits Alpha tape. Its output is the standalone
compiler used to consume the Gamma-written Delta compiler source.

Nested state braces are an authoring aid for substantial compiler CFGs, not a
second control hierarchy. Labels and locals remain procedure-wide; exact
depth-first lexical flattening and every-path initialization keep the Alpha
compiler and its proof obligations finite and explicit.

## Current implementation

The repository promotes `compiler/gamma_compiler.beta` directly as the Gamma
compiler. Its persisted artifact is the one-step output of the Beta-written
assembler. The historical Gamma self-host was deleted after the direct artifact
and focused compiler gate subsumed its useful migration role.

Evidence tied specifically to “Gamma source admits the Gamma compiler” does not
apply to this edge and cannot be reused as authority.

## Must not contain

No algebraic data types, pattern matching, safe ownership, regions, effects,
generics, or proof language. Gamma does not parse Epsilon or manufacture Epsilon
semantics. Its only upward compiler responsibility is Delta.

## Canonical artifact

```text
Beta-written Gamma compiler source
  └─ audited Alpha construction/refinement ─▶ gamma_compiler_bytecode.tape
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
an artifact. `Reject` means a Gamma rule was observed to fail. `Incomplete`
means only that this compiler's private profile was insufficient. Generated
program statuses remain runtime observations rather than compiler failures.
D30 preserves Gamma's 250 StackExhausted and 251
MemoryContainmentViolation meanings inside the common 248-through-254
generated-program block; Alpha's VM trap remains 132 and 255 is noncanonical.

## Implementation frontiers

- complete the Beta-written compiler for the Gamma surface required by the
  Delta compiler;
- guard explicit data/return stacks and expose fail-closed resources;
- check the exact Beta-assembly-source-to-tape encoding of the compiler
  artifact, then separately prove that compiler correct for arbitrary admitted
  Gamma source; and
- escalate rather than extend Alpha locally if realistic Delta compiler source
  creates unacceptable tape verbosity or performance.
