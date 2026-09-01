# `source/delta/` - safe definitional computation

Delta is the typed, pure functional rung above Gamma. It supplies nominal
algebraic data, exhaustive pattern matching, checked signed integers, immutable
bytes, forward and mutual recursion, and proper tail calls. It is deliberately
small but sufficient to implement the Epsilon compiler.

D21 makes every valid `Bytes` logical length a nonnegative `Int`. Concatenation
traps on an unrepresentable exact sum before allocation; physical storage
failure remains profile-owned `Incomplete`, and malformed private descriptors
remain `InternalFailure`.

The normative contract is [`LANGUAGE.md`](LANGUAGE.md). The canonical edge is:

```text
Delta source --delta_compiler_bytecode.tape--> Alpha tape
```

The compiler accepting Delta is implemented in Gamma and owned by `compiler/`.
Every canonical invocation supplies one D19 sealed application-profile ID.
`ConformanceBytesV1` selects pure `main : Bytes -> Bytes` for general
language conformance; `EpsilonCompilerV1` selects the Delta-written Epsilon
compiler's source-owned `main : Bytes -> EpsilonCompileOutcome` and its checked
`ECOUT` schema. A generated Alpha adapter alone reads sealed input, writes the
selected profile's exact success output, owns private resource failures, and
validates D31/D34's sole source-authored application-static-storage refusal.
D20 fixes deterministic source identity: grammar position selects separate
type, constructor, function, and local-value namespaces; globals are unique
within their namespace; and an active local binding may not be shadowed.
Disjoint scopes may reuse names.

## Current implementation state

The Gamma-written compiler source now lives at
`compiler/delta_compiler.gamma`. Its retained strict frontend and direct Alpha
emitter/runtime-containment substrate include executed checked-`Int` and
compact immutable-`Bytes` helpers, resolved expression lowering, the
arbitrary-arity/proper-tail-call frame and algebraic-value ABIs, profile-neutral
whole-function label/body emission, and a dormant profile-parameterized
sealed-input reader. D19 fixes its two possible application contracts and D20's
resolver is implemented. The compiler now validates both exact D19 entry
schemas, D31/D34's attributed/aggregate bounded-witness storage outcomes, and
the declaration-order-independent 26-code Epsilon rejection bijection. D30 fixes `DCREQ`, both
profile IDs and maxima, the generated-runtime observation block, and the exact
`DCOUT`/`ECOUT` identities and tables. D33 fixes the bounded request order,
schema-category priority, absence coordinates, and per-profile code legality.
The retained compiler now enforces canonical `DCREQ` admission and records
request/source rejection codes 3 through 18, all twelve compiler resources,
and present emitter-internal classes 2 through 4 with exact quantitative
fields. Schema and remaining internal DCOUT judgments, adapter completion,
remaining lowering, and publication remain open. Complete
fixed-up payloads are structurally
replayed against Alpha's closed instruction shapes and direct-target starts
before publication. The source remains incomplete compiler material, not an
accepted compiler artifact.
`tests/delta/interpreter/interp.gamma` remains an untyped, bounded evaluation oracle; it is not an
alternate Delta language or a runtime to be serialized into emitted tapes.

The compiler frontend and interpreter close their former correlated match blind
spot in different ways: the frontend rejects incomplete or duplicate coverage,
while the interpreter traps rather than fabricating integer zero when no arm
matches. Their historical shared omission remains the warning that differential
agreement cannot prove a rule both compared programs omit; the completed
canonical compiler must own the static judgment.

The evaluator's 4 MiB source ceiling, 16 MiB arena, 4 KiB argument scratch, and
50,000,000-call fuel are oracle resource bounds, not Delta semantics. Exhaustion
is fail-closed and publishes no partial value. Delta's compact `Bytes` primitive
prevents compiler input from requiring one algebraic node per byte.

Principal artifacts:

- `LANGUAGE.md` - the normative typed executable Delta contract;
- `compiler/` - owner of `delta_compiler.gamma`, its closed tables, its future
  tape, and the exact edge;
- `tests/delta/` - compiler tests, the temporary bounded semantic oracle, and
  independent differential scaffolding.

Run the currently retained diagnostic gates from the repository root:

```sh
sh tests/delta/interpreter/test-interp.sh
sh tests/delta/interpreter/test-interp-arena.sh
sh tests/delta/compiler/test-frontend.sh
sh tests/delta/reference/delta-diamond-py.sh
```

Python, Rust, shell, and host tools are not Delta implementations in the
completed bootstrap chain. The checked direct edge must leave the repository buildable
from the audited Alpha seed and repository-owned bytes on an otherwise blank,
offline machine.

## Retention inventory

| Retained child | Canonical role | Deletion condition |
| --- | --- | --- |
| `LANGUAGE.md` | Normative Delta source and execution contract. | Replace only through an explicit language ruling with synchronized compiler and contract tests. |
| `compiler/` | Sole owner of the Gamma-written compiler accepting Delta, its closed tables, and its exact Alpha-tape edge. | Replace only atomically with an explicitly ruled bootstrap-chain change. |

The older imperative experiment, generic canonical-byte prototype, and terminal
codec spike are retired to Git history. Being written in Delta never made them
part of Delta meaning or the canonical compiler chain.
