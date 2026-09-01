# Compiler lattice repository structure

[Lattice overview](bootstrap_lattice.md) | [Standing decisions](decisions.md) |
[Product repository layout](../repository_layout.md)

The repository groups source by the language a compiler accepts. The source
suffix records the language in which that compiler is implemented. Bootstrap is
a build-graph property, not a generic folder or source owner.

## Target layout

```text
source/
  alpha/                         Alpha semantics and audited native VM seeds
    assembler/                   Alpha assembler and its tape
    checker/                     universal derivation checker

  beta/                          Beta language
    compiler/
      beta_compiler.alpha        canonical compiler implementation
      beta_compiler_bytecode.tape
      validation/                Alpha-source/Beta-compiler tape refinement

  gamma/                         Gamma language
    compiler/
      gamma_compiler.beta        canonical compiler/reference implementation
      gamma_compiler_bytecode.tape
      validation/                Beta-source/Gamma-compiler tape refinement
    reference/                   temporary differential implementations only

  delta/                         Delta language
    compiler/
      delta_compiler.gamma       canonical compiler implementation
      delta_compiler_bytecode.tape
      validation/                Gamma-source/Delta-compiler tape refinement

  psi/                           target-neutral product compiler packages
  omega/                         Omega language and both Omega implementations
    omega_compiler.delta         Delta-written source closure D
    main.omg / build.omg         Omega-written source closure C
    omega0_compiler_bytecode.tape
    omega_compiler_bytecode.tape
    validation/                  D→omega₀ and C→omega refinement

  library/                       core, allocation, and standard-library source
  omega-rust/                    maintained Rust product/comparator

tests/omega/                      Omega acceptance/rejection cases
tools/lattice/                    replaceable invocation and tape stamping
```

Names identify accepted language and implementation format without inventing
`bootstrap/`, `on-ramp/`, `assurance/`, `canaries/`, or generation-owned source
trees. `omega₀` and `omega` are artifacts, not directories or languages.

## Current gaps

The committed tree still has two important gaps:

- `source/gamma/compiler/gamma_compiler.beta` now owns a real strict frontend,
  direct Alpha emitter substrate, resolved expression lowering, and
  profile-neutral whole-function emission. Both D19 source schemas are
  validated, D30 fixes the physical application profiles, and D33 fixes
  bounded request/schema failure selection; the generated adapters, canonical
  tape, and refinement are still missing.
- the former Beta-written Delta-to-Gamma and Darwin-native publication trees
  were deleted because they implemented a superseded cross-rung route. The
  Gamma-written replacement now owns complete syntax, D22/D24 identity census,
  D36's receiver parser and now-superseded case/machine collision census, D31
  structural type formation, a source-backed resolution catalog, ordered local
  resolution, scalar/aggregate value-place facts, one generalized callable
  ledger with direct-qualified, settled grouped/unqualified, named-data
  receiver, and sealed-boundary receiver results plus postfix-statement
  category admission, separate resolved/complete explicit-state custody and
  state/machine collision rejection, transition subject/resolved-case/complete-
  binder custody and retained sum coverage, the superseded special
  receiver-scoped `self` carrier,
  settled field/index/slice projection failures, D37 scalar and
  argument-`never` category joins, let/assignment/assert and explicit-return
  relations, first-following-statement terminal flow, D38's source fact
  relation, and symbolic Alpha encoding.
  D50 fixes state-transfer spelling, D51 fixes receiver normalization,
  static-qualified removal, and disjoint case/method namespaces, D52 fixes
  resultless-argument anchoring, and D53 fixes local block exits without
  reachability analysis; D56 fixes entry diagnostics and D57 fixes transition-
  pattern/coverage diagnostics. Their branches, D37 remaining terminal closure,
  D38 lowering/executable controls, body/control
  checking, D34 physical storage refusal, lowering, tape publication, and
  refinement are still open.
  The restricted Delta-written native compiler prototype was also deleted: it
  was neither that compiler nor the full Omega closure `D` and had no
  economical unit-level adaptation into either owner.

These gaps are implementation work, not alternate accepted architectures. A
legacy file stays only when this document or `TASKS_BOOTSTRAP.md` names its
direct adaptation into a canonical edge, canonical owner, and deletion
condition. Otherwise it has negative value: it enlarges the audit surface,
creates false architectural choices, and consumes maintenance and test time.
Delete it; Git history is the archive.

The same rule applies to Python and other host-language references. They may
temporarily diagnose an incomplete direct edge, but they are not eligible for
permanent membership in the self-contained chain and are deleted when their
named differential role is subsumed.

## Artifact rule

Every required compiler artifact is a descriptive `.tape` file governed by
Alpha semantics. Target containers are disposable realizations:

```text
canonical compiler identity = exact Alpha tape
host execution              = selected Alpha VM seed + exact tape
optional acceleration       = checked general Alpha-to-native realization
```

Mach-O, ELF, PE, code signatures, installation inventories, and elapsed-time
records do not become rung-specific compiler identities. Product Omega may emit
native artifacts for users; that target work remains inside the product
compiler rather than Beta, Gamma, or Delta.

## Ownership rules

- `source/<language>/compiler/` owns the compiler accepting that language, even
  though its source is written in the immediate predecessor language.
- The source suffix must match the implementation language: `.alpha` for the
  Beta compiler, `.beta` for Gamma, `.gamma` for Delta, `.delta` for `omega₀`,
  and `.omg` for self-hosted `omega`.
- A lower rung must not parse a language beyond its immediate successor.
- A compiler artifact must consume its own language and emit the next runnable
  Alpha tape without invoking an older compiler or semantic host script.
- The artifact being admitted owns its validation. Evidence stays adjacent to
  that artifact; there is no generic evidence archive.
- Optional comparators, fuzzers, and corpora must name the exact edge property
  and failure class they exercise. They are deleted when they duplicate a
  cheaper gate or cease to exercise the canonical subject. “Diagnostic” is not
  a permanent ownership category.
- `source/alpha/checker/` owns the universal derivation checker. It is beside
  compiler edges, not another rung.
- `source/psi/` owns target-neutral processing inside the Omega product
  compiler. Psi is not a bootstrap language rung.
- `source/omega-rust/` remains a comparator and migration aid without canonical
  bootstrap authority.
- `tools/lattice/` may invoke compilers and stamp tapes. It may not discover a
  source closure, parse, lower, manufacture evidence, or decide trust.

## File naming

`.alpha`, `.beta`, `.gamma`, `.delta`, `.omg`, and `.psi` identify source
languages. `.proof` identifies proof-source input to untrusted elaboration.
`.tape` identifies canonical Alpha VM bytecode. Artifact base names describe
their role, such as `delta_compiler_bytecode.tape`; opaque rung abbreviations
are not canonical names.

## Canonical ownership map

| Responsibility | Canonical owner |
| --- | --- |
| Alpha execution and tape semantics | `source/alpha/` |
| universal proof checking | `source/alpha/checker/` |
| Beta compiler and admission | `source/beta/compiler/` |
| Gamma compiler and admission | `source/gamma/compiler/` |
| Delta compiler and admission | `source/delta/compiler/` |
| first and self-hosted Omega compilers | `source/omega/` |
| product target-neutral phases | `source/psi/` |
| language libraries | `source/library/` |
| optional Rust implementation | `source/omega-rust/` |
| non-authoritative invocation | `tools/lattice/` |

Cross-owner paths are checked by
[`tools/lattice/check-path-hygiene.sh`](../../../tools/lattice/check-path-hygiene.sh).
