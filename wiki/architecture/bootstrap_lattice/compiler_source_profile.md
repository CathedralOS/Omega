# Delta and Omega compiler source contracts

[Lattice overview](bootstrap_lattice.md) | [Decisions](decisions.md) |
[Omega toolchain](omega_toolchain.md)

Three independent facts govern the top of the lattice:

| Subject | Language | Purpose |
| --- | --- | --- |
| Delta v1 | independently specified Delta | language accepted by the Gamma-written Delta compiler |
| `D` | Delta source closure | first complete Omega compiler implementation |
| `C` | ordinary Omega source closure | optimized self-hosting Omega compiler implementation |

`D` and `C` implement the same full Omega specification. They are not the same
source closure and neither defines the product language.

## Required artifacts

```text
Gamma-written Delta compiler source
  └─ Gamma compiler ───────────────▶ delta_compiler.tape

Delta-written Omega source D
  └─ delta_compiler.tape ──────────▶ omega₀.tape

Omega-written Omega source C
  └─ omega₀.tape ──────────────────▶ omega.tape
```

Every output above is canonical Alpha tape. A host-specific VM seed may execute
or package it, but native container bytes do not replace the tape identity.

## Delta v1

Delta is a small deterministic systems/compiler-host language. It may share
spelling with Omega, but its grammar, static rules, execution, resources, and
observations are self-contained. Its compiler is written in Gamma and lowers
Delta directly to Alpha tape. Neither the superseded Beta translator nor the
sample corpus defines Delta.

Delta needs a coherent C-like floor: structured state-machine control, scalar
and aggregate data, fixed storage or explicit allocation failure, sealed byte
I/O, deterministic traps, and sufficient modularity to maintain `D`. It does
not inherit Omega proofs, dependent types, packages, or optimizer machinery.

Every bound is source-visible semantics, an explicit profile parameter, or a
private budget whose exhaustion returns `Incomplete` and publishes no tape.

## Delta-written Omega implementation `D`

`D` is allowed to be conservative and operationally plain. It must nevertheless
implement the complete Omega language required of a product compiler. In
particular, omitting advanced language features from Delta itself does not
permit `D` to omit them from the Omega compiler it implements.

`D` may avoid optimizer sophistication in the code generated for `omega₀`.
That makes the first compiler artifact slow or large; it does not weaken the
Omega programs `omega₀` accepts. The optimizer implemented by `D` runs when
`omega₀` compiles `C` and may therefore produce a materially better `omega`
tape than the Delta compiler produced for `omega₀`.

## Omega-written implementation `C`

`C` is ordinary Omega deliberately authored with a conservative incidental
feature profile. That profile is not a named language or dialect. For every
candidate facility, record separately:

1. whether `C` uses it;
2. whether `omega₀` accepts that use with exact Omega meaning;
3. whether the resulting compiler implements it for users; and
4. whether an adjacent tool using it actually belongs to `C`.

The initial likely omissions from `C` include mathematical proof authoring and
linear dependent types. Domains, generics, named fields, mixed data/case forms,
and rich transitions remain candidates only if concrete implementation work
shows they materially obstruct the first self-build. Rejection by `omega₀` must
be structural and compositional, never a file or AST allowlist.

## Closure rules

- `D` and `C` are independently package-resolved exact closures.
- The first edge compiles Delta; the second compiles Omega. No compiler pretends
  the two source languages are interchangeable.
- Unsupported source rejects loudly; there is no approximate meaning.
- Resource ceilings are explicit inputs or semantic bounds, not hidden host
  limits.
- Both compiler artifacts are Alpha tapes and use the same Alpha observation
  model.
- Target-specific product dependencies remain symbolic until Omega target
  realization. They do not leak into Delta compilation.
- Repo-owned bootstrap machinery that does not implement an edge, enforce one
  of these invariants, or provide explicitly consumed execution scaffolding is
  negative value: adapt it into this structure or delete it. Historical,
  neutral, and merely potentially useful are not retention categories.

## What scripts may do

Shell or host-language runners may invoke a compiler, stamp a tape, compare
outputs, and report failures. They may not discover `D` or `C`, parse or lower
accepted source, manufacture certificates, or define an edge. If deleting a
runner changes chain meaning, it has become an undeclared compiler stage.

The execution queue is
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md); this document defines
contracts, not a parallel task list.
