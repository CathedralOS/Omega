# `source/gamma/` - safe definitional computation

Gamma is the typed, pure functional rung above Beta. It supplies nominal
algebraic data, exhaustive pattern matching, checked signed integers, immutable
bytes, forward and mutual recursion, and proper tail calls. It is deliberately
small but sufficient to implement the Delta compiler.

The normative contract is [`LANGUAGE.md`](LANGUAGE.md). The canonical edge is:

```text
Gamma source --gamma_compiler_bytecode.tape--> Alpha tape
```

The compiler accepting Gamma is implemented in Beta and owned by `compiler/`.
The program that consumes that compiler is the Gamma-written Delta compiler.
It exposes a pure `main : Bytes -> DeltaCompileOutcome`; a generated Alpha
adapter alone reads sealed input, writes either exact tape or one failure frame,
and owns private resource failures.

## Current implementation state

`interp.beta` and `typeck.beta` predate the unified contract:

- `interp.beta` is an untyped, bounded evaluation oracle;
- `typeck.beta` is a typed, nonexecuting static-semantics oracle; and
- neither is an accepted compiler artifact or an alternate Gamma language.

They may be reorganized or absorbed into the direct compiler where economical.
Once the compiler and its checked edge subsume a component's unique diagnostic
role, that component or gate is deleted. The direct compiler must type-check
Gamma and emit Alpha tape; it may not publish an interpreter plus serialized
syntax.

The current pair retains one correlated blind spot: the checker does not test
match exhaustiveness. The interpreter now traps rather than fabricating integer
zero when no arm matches, but that runtime hardening is not the missing static
judgment. The canonical compiler must reject nonexhaustive source statically;
differential agreement does not prove a rule that compared programs omit.

The evaluator's 4 MiB source ceiling, 16 MiB arena, 4 KiB argument scratch, and
50,000,000-call fuel are oracle resource bounds, not Gamma semantics. Exhaustion
is fail-closed and publishes no partial value. Gamma's compact `Bytes` primitive
prevents compiler input from requiring one algebraic node per byte.

Principal artifacts:

- `LANGUAGE.md` - the normative typed executable Gamma contract;
- `compiler/` - owner of `gamma_compiler.beta`, its tape, and exact edge;
- `interp.beta` - temporary bounded semantic oracle and candidate component;
- `typeck.beta` - temporary static-semantics oracle and candidate component;
- `reference/` - temporary Python differential scaffolding.

Run the currently retained diagnostic gates from the repository root:

```sh
sh source/gamma/test-interp.sh
sh source/gamma/test-interp-arena.sh
sh source/gamma/test-typeck.sh
sh source/gamma/reference/gamma-diamond-py.sh
```

Python, Rust, shell, and host tools are not Gamma implementations in the
completed lattice. The checked direct edge must leave the repository buildable
from the audited Alpha seed and repository-owned bytes on an otherwise blank,
offline machine.

## Retention inventory

| Retained child | Canonical role | Deletion condition |
| --- | --- | --- |
| `LANGUAGE.md` | Normative Gamma source and execution contract. | Replace only through an explicit language ruling with synchronized compiler and contract tests. |
| `compiler/` | Sole owner of the Beta-written compiler accepting Gamma and its exact Alpha-tape edge. | Replace only atomically with an explicitly ruled lattice change. |
| `interp.beta`, `test-interp.sh`, `test-interp-arena.sh` | Candidate compiler material plus bounded semantic and resource discriminators. | Absorb or delete after the direct compiler subsumes each retained failure surface. |
| `typeck.beta`, `test-typeck.sh` | Candidate compiler material plus static-semantics discriminators. | Absorb or delete after the direct compiler subsumes each retained failure surface. |
| `reference/` | Temporary independent meaning comparison and differential gate. | Delete when the checked direct edge subsumes every named diagnostic role. |

The older imperative experiment, generic canonical-byte prototype, and terminal
codec spike are retired to Git history. Being written in Gamma never made them
part of Gamma meaning or the canonical compiler chain.
