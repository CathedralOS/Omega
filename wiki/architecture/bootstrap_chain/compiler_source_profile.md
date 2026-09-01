# Bootstrap compiler source contracts

[Chain overview](bootstrap_chain.md) | [Decisions](decisions.md) |
[Omega toolchain](omega_toolchain.md)

Three independent facts govern the top of the chain:

| Subject | Language | Purpose |
| --- | --- | --- |
| Epsilon v1 | independently specified Epsilon | language accepted by the Delta-written Epsilon compiler |
| `D` | Epsilon source closure | first complete Omega compiler implementation |
| `C` | ordinary Omega source closure | optimized self-hosting Omega compiler implementation |

`D` and `C` implement the same full Omega specification. They are not the same
source closure and neither defines the product language.

## Shared implementation-source envelope

Beta assembly, Gamma, Delta, and Epsilon are repository-controlled bootstrap
implementation languages. Their source is not decoded text: it is a finite byte
sequence containing only HT, LF, CR, and printable ASCII. NUL, DEL, bytes above
`0x7F`, BOMs, and every other control byte reject before tokenization. Each
language uses explicit ASCII identifier and digit classes, exactly space/tab/
CR/LF whitespace, CR/LF/source-end comment termination, and printable raw
literal contents plus its closed escapes.

This envelope governs implementation source, including comments; it does not
restrict the byte data a compiler may consume or emit. Exact source closures,
not filename suffixes, select enforcement. The full rule and offline-completion
requirement are fixed by [D15](decisions.md#d15--bootstrap-implementation-source-is-closed-textual-ascii).

## Required artifacts

```text
Delta-written Epsilon compiler source
  └─ Delta compiler + sealed EpsilonCompilerV1 ─▶ epsilon_compiler_bytecode.tape

Epsilon-written Omega source D
  └─ epsilon_compiler_bytecode.tape ─▶ omega0_compiler_bytecode.tape

Omega-written Omega source C
  └─ omega0_compiler_bytecode.tape ─▶ omega_compiler_bytecode.tape
```

Every output above is canonical Alpha tape. A host-specific VM seed may execute
or package it, but native container bytes do not replace the tape identity.
The first edge includes D19's sealed profile ID in its exact compilation
question and checks the source-owned outcome/reason schema before emitting the
`ECOUT` adapter; source names do not select that boundary. D30 gives that
question its exact `DCREQ` byte envelope, profile IDs and maxima, generated
runtime observations, and `DCOUT`/`ECOUT` tables. D33 fixes the bounded request
suborder and total DCOUT schema diagnosis before either implementation may
publish that boundary.

## Epsilon v1

Epsilon is the closed deterministic compiler-host language fixed by D17 and
`source/epsilon/LANGUAGE.md`. It may share spelling with Omega, but its grammar,
checking, execution, resources, and observations are self-contained. Its
compiler is written in Delta and lowers Epsilon directly to Alpha tape. Neither
the superseded Gamma translator nor a sample corpus defines Epsilon.

V1 provides finite records and sums, fixed arrays, bounded views, checked
scalars, state-machine control, recursion, and one sealed `Console` boundary.
It has no package model, heap, or recursive value type. `D` therefore represents
dynamic structures in source-declared arrays with integer indexes. Those
capacities are program semantics; a compiler's parser, arena, stack, and output
ceilings are private budgets whose exhaustion returns outer `Incomplete` and
publishes no tape.

## Epsilon-written Omega implementation `D`

`D` is allowed to be conservative and operationally plain. It must nevertheless
implement the complete Omega language required of a product compiler. In
particular, omitting advanced language features from Epsilon itself does not
permit `D` to omit them from the Omega compiler it implements.

`D` may avoid optimizer sophistication in the code generated for `omega₀`.
That makes the first compiler artifact slow or large; it does not weaken the
Omega programs `omega₀` accepts. The optimizer implemented by `D` runs when
`omega₀` compiles `C` and may therefore produce a materially better `omega`
tape than the Epsilon compiler produced for `omega₀`.

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
- The first edge compiles Epsilon; the second compiles Omega. No compiler pretends
  the two source languages are interchangeable.
- Unsupported source rejects loudly; there is no approximate meaning.
- Resource ceilings are explicit inputs or semantic bounds, not hidden host
  limits.
- Both compiler artifacts are Alpha tapes and use the same Alpha observation
  model.
- Target-specific product dependencies remain symbolic until Omega target
  realization. They do not leak into Epsilon compilation.
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
