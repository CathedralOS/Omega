# Delta and the Omega self-hosting profile

[Lattice overview](bootstrap_lattice.md) | [Standing decisions](decisions.md) |
[Delta rung](rungs/delta.md) | [Psi/Omega toolchain](omega_toolchain.md)

The final bootstrap has two deliberately different source surfaces and one
product language specification:

```text
Delta source              Omega source constrained to Ωself
     │                                  │
     ▼                                  ▼
omega-bootstrap  ─────────────────▶  production omega
 accepts Ωself only                    implements full Ω
```

Delta and `Ωself` are the only bootstrap feature choices left once the full
Omega specification is fixed.

- **Delta** is an independent, robust compiler-host language. It may resemble
  Omega in spelling and shape, but it is not required to be an Omega subset.
- **`Ωself`** is the Omega self-hosting source profile: a strict selection of
  ordinary Omega programs and dependencies accepted by `omega-bootstrap`.
  It introduces no syntax or semantics of its own.
- **Full Omega** is the language implemented by the resulting production
  compiler. A compiler can implement a feature without using that feature in
  its own source.

The bootstrap closure condition is therefore:

```text
main.delta ∈ Delta
main.omg and every transitive compiler dependency ∈ Ωself
omega-bootstrap correctly compiles Ωself
production omega correctly implements full Ω
```

It is not necessary for `omega-bootstrap` to accept every Omega program. It
must reject unsupported constructs rather than approximate them, and every
construct it does accept has exactly its normal Omega meaning, ABI, layout, and
artifact contract.

## Delta design budget

Delta should have C-like systems power without inheriting C's undefined and
ambient behavior. Its literal specification should favor:

- fixed-width scalars, bytes, predictable aggregates, arrays, slices, and
  explicit representation;
- procedures, modules or deterministic source bundling, loops, recursion,
  state-machine control, and payload-bearing sum data;
- explicit references or integer arena handles, checked indexing, and stable
  calling/layout conventions;
- deterministic trapping or checked arithmetic and explicit boundary I/O;
- runtime-sized allocation from fixed backing, typed/indexed arenas, and bulk
  reclamation, with specified exhaustion;
- conservative lowering and auditable code generation.

Omega-like lexical and structural conventions reduce cognitive and tooling
distance, but similarity is not a semantic subset promise. Delta-only bootstrap
facilities are acceptable when they reduce the lower-rung implementation and
assurance burden and remain explicit in Delta's specification.

## Working `Ωself` policy

The exact profile cannot be frozen before the production compiler source and
deterministic dependency manifest exist. The working policy is nevertheless
specific enough to guide that source.

Presumptively excluded from compiler source:

- the mathematical proof/program surface;
- linear and dependent types;
- terminal-Psi-only declarations, interpreters, REPLs, and product tooling not
  imported by the compiler build;
- numeric/schema field tags such as `0:` when ordinary fields suffice;
- transitions carrying complex values when a simple discriminant plus an
  explicit context object expresses the same compiler state.

Presumptively retained because removing them is likely to make the compiler
larger or less robust:

- ordinary named record fields;
- payload-bearing enums/sum data for syntax trees and IR;
- basic generics needed by collections, results, arena IDs, and compiler data
  structures.

Candidates to decide from the actual source and a bootstrap-cost measurement:

- concrete domains versus explicit compiler contexts;
- domain polymorphism and arithmetic;
- advanced generic constraints, specialization, or reflection;
- complex transition payloads and schema-tagged data.

For a retained feature, `omega-bootstrap` need implement only the valid Omega
cases exercised by `Ωself`; the production compiler implements the complete
feature. Simplified cases must preserve full Omega semantics. No bootstrap-only
Omega dialect or private extension is permitted.

The selection rule is total cost, not the smallest feature count:

```text
benefit and robustness in main.omg
──────────────────────────────────
implementation + assurance cost in main.delta
```

Basic generics and payload enums are likely favorable. Proof syntax and
dependent typing are not. Profile growth is an architectural change and must
update the allowlist, compiler, meaning route, diagnostics, and negative gates
together.

## One required hosted compile

`omega-bootstrap` may itself be a slow binary and may lower `main.omg`
conservatively. It must understand enough `Ωself` to compile the source that
*implements* the production optimizer and advanced lowering; it does not need
to run those product passes during this build. The required hosted result is a
full optimizing compiler, although that compiler's own machine code may still
be conservatively generated.

```text
Delta compiler ──▶ omega-bootstrap (slow binary, Ωself input)
omega-bootstrap ──▶ production omega (full optimizing compiler; binary may be conservative)
```

A later production-Omega rebuild can optimize the compiler binary itself and
provide fixed-point or reproducibility evidence. It is optional: neither full
language functionality nor bootstrap dependency closure waits for it.

## Mechanical enforcement

The production compiler task must publish one deterministic source/dependency
manifest. The bootstrap gate must compile that exact closure under an explicit
`Ωself` allowlist and reject an excluded-feature canary for every exclusion.
The manifest includes compiler modules, compile-time code, build/module
behavior, and runtime/library dependencies; hiding a feature in a library does
not remove it from the bootstrap surface.

The current Rust Psi/Omega compiler remains a maintained reference and
differential producer while useful. It is neither a bootstrap dependency nor an
authority source: lower-rooted refinement and canonical meaning decide
acceptance. Cross-compiler diagnostics, normalized IR, artifacts, and execution
observations remain valuable bug-finding evidence.
