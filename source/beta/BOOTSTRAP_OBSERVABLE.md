# Beta compiler bootstrap observable

This document fixes the whole-program relation that the `bc.beta` cold-start
edge is intended to preserve. A fixed point, a matching second compiler, or a
finite corpus does not establish this relation.

## Subject and observation

For every finite source byte stream `S` and supported finite resource profile
`B`, the exact persisted Alpha tape `A` should satisfy:

```text
observe_beta(bc.beta, S, B) = observe_alpha(A, S, B)
```

Malformed, truncated, oversized, and rejected source remains in the
quantification. The maximal observation is the complete ordered stdout byte
stream paired with exactly one terminal class:

```text
Halt(u32)
Trap(TrapKind)
Exhaust(ResourceKind, limit, requested)
Diverge
```

Output before a terminal outcome is retained. Checked exhaustion records the
resource identity and refused amount before any overlapping write. Divergence
means an infinite canonical small-step run, not a wall-clock timeout. Standard
error, duration, host addresses, process signals, and wrapper bytes are not
language observables. After the final input byte, Alpha `read` and Beta
`read_byte()` return the canonical all-ones sentinel.

## Supported profile `B_bc1`

The exact subjects are:

- `compiler/bc.beta`: 32,565 bytes, SHA-256
  `fe4b5af69f87163ce919d22e2aa662ad0b5f2a044a6904581c91e8638749aa25`;
- `compiler/artifacts/bc.tape`: 52,141 bytes, SHA-256
  `1b32401c4c8fb60598e97178d415136227c9aa2231e28d9eb44b30e7a2818a2f`.

The resource profile fixes:

- 64 MiB Alpha memory and an 8,192-entry hidden return stack;
- a 262,140-byte stamped tape payload;
- compiler data stack interval `[524288,1048576)`, initially at 1 MiB;
- source interval `[2097152,3145728)`, exactly 1,048,576 bytes;
- 1,024 paired local-name table entries per procedure;
- four live parameters or call arguments;
- 64 recursive `gen_expr` activations;
- 64 recursive `gen_stmts` activations;
- streamed output with no finite compiler-owned output buffer.

The checked resource identities and exact failed admissions are:

| failed admission | kind | limit | requested | process projection |
| --- | --- | ---: | ---: | ---: |
| source byte after a full source arena | `SourceBytes` | 1,048,576 | 1,048,577 | 253 |
| fifth actual or formal argument | `CallArity` | 4 | 5 | 252 |
| declaration after a full local table | `ProcedureLocalSlots` | 1,024 | 1,025 | 252 |
| `parse_proc` preflight over 1,024 slots | `ProcedureLocalSlots` | 1,024 | exact `nparams + count_lets()` | 252 |
| `gen_expr` activation at depth 64 | `ExpressionCodegenDepth` | 64 | 65 | 252 |
| `gen_stmts` activation at depth 64 | `BlockCodegenDepth` | 64 | 65 | 252 |

Origin, kind, limit, and requested amount are sticky through safe cleanup and
wrapper returns. Numeric status is only a one-way process projection; it cannot
reconstruct a typed resource outcome. `source-exhaustion.sh` pins adjacent
source/resource boundaries and retained output prefixes.

## Reconstruction boundary

An authority closing this edge must independently bind the exact source and
artifact and reconstruct:

1. the full input/resource quantification;
2. the complete output-stream relation;
3. every halt, trap, checked-exhaustion, and divergence case;
4. the Alpha small-step obligations for the artifact; and
5. the Beta source-meaning obligations for the compiler.

Producer-supplied summaries or witnesses may accelerate checking but may not
select the subjects, omit terminal cases, or define the proposition.

## Current evidence and open admission

The retained evidence is intentionally responsibility-specific:

- `cold-start/full-source.sh` reconstructs `bc.tape`, checks the fixed point,
  and runs the Beta corpus;
- `compiler/validation/admission/bc-artifact-structure.sh` checks reachable Alpha
  framing, direct targets, procedure regions, and call/return structure below
  `bc`;
- `compiler/validation/admission/bc-block-control.sh` reconstructs the canonical
  whole-source/artifact conjunction and its 82,172-byte ROOT maximal-observation
  checker (SHA-256
  `7e3675449da2af21bd43d5374f2f1c8300c87dfaa998fa9491dfd06b1e4c001b`),
  then applies four fail-closed format controls, one source-identity/event-PC
  mutation control, one same-key occurrence-order mutation control, and one
  same-block memory-identity swap control;
- optional `compiler/validation/stress/refinement.sh` checks proof-carrying
  equivalence for its stated symbolic program families with the below-Beta
  checker artifact;
- reference and differential gates exercise finite behavior but grant no
  authority.

The ROOT executable is strong, unique lower-rung evidence for the intended
maximal observation, including guarded greatest-fixed-point divergence. It is
not yet a derivation accepted by the universal proof checker: the repository
has no encoded Alpha/Beta simulation relation or settled coinduction rule in
that proof language. Complete source/artifact admission therefore remains
open, exactly as disclosed in the bootstrap chain manifest.
