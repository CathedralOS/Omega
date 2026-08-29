# Rung: Delta — compiler-host systems language

[Lattice overview](../bootstrap_lattice.md) | Prev: [Gamma](gamma.md) | Next:
[Omega product toolchain](../omega_toolchain.md)

Delta is the robust C-like implementation language used to write the first full
Omega compiler. It is independently specified, not an Omega subset and not a
Gamma macro surface.

## Direct responsibility

```text
Gamma-written Delta compiler source
  └─ gamma_compiler.tape ─▶ delta_compiler_bytecode.tape

Delta-written Omega compiler source D
  └─ delta_compiler.tape ─▶ omega0_compiler_bytecode.tape
```

The Delta compiler accepts Delta and emits Alpha tape. The Delta-written program
`D` accepts Omega and is the first full Omega compiler. Calling both artifacts
“the Delta compiler” obscures this boundary and is forbidden in current docs.

## Language boundary

Delta provides deterministic state-machine control, checked scalar arithmetic,
finite aggregates, fixed storage or explicit allocation failure, sealed byte
I/O, and enough modularity to maintain `D`. It does not inherit Omega's proof
surface, dependent types, packages, optimizer, or target model merely because
`D` implements those facilities for Omega users.

Every source-visible bound, resource-profile parameter, and private
implementation budget is distinguished. Private exhaustion returns
`Incomplete` and publishes no tape.

## Implementation owners

- `source/delta/LANGUAGE.md` owns Delta syntax and semantics;
- the target compiler source is `source/delta/compiler/delta_compiler.gamma`;
- `source/delta/compiler/artifacts/delta_compiler_bytecode.tape` is the future
  canonical artifact; and
- adjacent validation owns Gamma-source/Alpha-tape refinement.

The former `source/delta/meaning/delta2gamma.beta` route and the restricted
Delta-written Darwin compiler prototype are deleted. Neither implemented the
Gamma-written Delta edge or a full Omega `D`; their historical source remains
available in Git without occupying a live compiler owner.

## Closure criteria

Delta closes when:

1. its independent language contract is complete;
2. a Gamma-written compiler accepts that language and emits exact Alpha tape;
3. the tape directly refines the Gamma compiler source under Gamma and Alpha
   semantics;
4. the compiler accepts the exact Delta source closure `D`; and
5. compiling `D` yields an `omega₀` tape refining the full Omega compiler it
   implements.

## Owner escalation

Escalate rather than locally redesign when `D` compilation has terrible
performance or tape size, Alpha seems too verbose, a special native accelerator
appears necessary, proof checking becomes prohibitive, or Delta compilation
requires any external older-rung semantic tool.

The exact work order lives in
[`TASKS_BOOTSTRAP.md`](../../../../TASKS_BOOTSTRAP.md).
