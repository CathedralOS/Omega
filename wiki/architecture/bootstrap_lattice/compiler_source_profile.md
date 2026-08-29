# Delta and Omega compiler source contracts

[Lattice overview](bootstrap_lattice.md) | [Decisions](decisions.md) |
[Omega toolchain](omega_toolchain.md)

Two source facts govern the top of the lattice:

| Contract | Kind | Selected from |
| --- | --- | --- |
| Delta v1 | independent language specification | the complete canonical Delta compiler source plus compiler-host safety and maintainability requirements |
| compiler-source usage | incidental compositional subset of ordinary Omega | the complete Omega product compiler source closure `C` |

Full Omega is already the product language specification. It is not a third
bootstrap source profile.

## Required artifacts

```text
canonical Delta compiler source ∈ Delta v1
  └─ Delta→Gamma/Gamma publication ─▶ Delta-produced compiler

exact ordinary-Omega product source closure C
  └─ Delta-produced compiler ────────▶ omega₀

the exact same C
  └─ omega₀ ─────────────────────────▶ omega
```

The Delta-produced compiler must accept every ordinary-Omega form exercised by
`C` with exact Omega meaning and reject unsupported source. `omega₀` and
`omega` are two artifacts of the same product compiler source, not two compiler
designs.

## Delta v1

Delta is a small deterministic systems/compiler-host language. It may share
spelling with Omega, but its grammar, static rules, elaboration, resources, and
observations are defined without consulting Omega. A source file accepted by
both languages receives only the meaning of the selected route. Delta is not
restricted to valid Omega and does not inherit Omega complexity merely for
spelling consistency. Its contract is justified by the source and safety needs
of the canonical compiler, not by whatever a temporary implementation happens
to accept.

Candidate facilities must earn their place by reducing total implementation
and assurance cost or by materially improving correctness and maintainability.
Omission is not a goal by itself. Delta still needs a coherent compiler-host
floor: structured control, scalar and aggregate data, modules, deterministic
storage/allocation with explicit exhaustion, and sealed byte/artifact/
diagnostic/exit boundaries.

The contract fixes sealed input bytes and the admissible input profile before
stating refinement. It classifies each fixed capacity while defining its
resource section: source-visible bounds and explicit profile parameters are
semantic; private implementation budgets are not. Exhausting the latter yields
`Incomplete`, never a Delta rejection, termination result, or artifact claim.

## Compiler-source usage

`C` may deliberately omit advanced Omega features from its own implementation
source. That authoring choice does not constrain what the resulting compiler
implements for users and does not define a named dialect.

For every candidate Omega facility, record separately:

1. whether `C` uses it;
2. whether the Delta-produced compiler admits the form used by `C`;
3. whether production `omega` implements it for users; and
4. whether any adjacent tool using it belongs to the actual compiler closure.

Those facts must not collapse into one “supported” bit.

## Closure rules

- `C` is determined by package resolution and the accepted source graph, not a
  hand-maintained file list.
- The accepted surface is structural and compositional. A compiler must not recognize
  only the particular trees present in the current source.
- Unsupported source rejects loudly; there is no approximate bootstrap
  meaning.
- Resource ceilings are explicit inputs or contract bounds, not hidden host
  limits.
- Target realization dependencies stay symbolic until target closure and enter
  artifact compatibility identity.
- The first and second product builds use the same exact `C`.

## What conservative generation permits

`omega₀` may be large, slow, or poorly optimized. That says nothing about the
language implemented by its source. The optimizer and advanced lowering are
ordinary modules in `C`; they run when `omega₀` compiles later programs even if
the Delta compiler did not optimize `omega₀` itself.

## What scripts may do

Shell or host-language runners may invoke stages, compare outputs, and report
failures. They are replaceable conveniences. They may not discover the source
closure, parse or lower accepted source, manufacture certificates, or define
the semantics of a compiler edge. If deleting a runner changes the meaning of
the chain rather than merely how it is invoked, the runner has become an
undeclared compiler stage.

The current execution queue is
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md); this document defines the
contracts, not a second task list.
