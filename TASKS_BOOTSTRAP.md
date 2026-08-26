# Bootstrap lattice — active work

Last pruned: 2026-08-25.

This file is the current bootstrap execution queue, not an architecture essay or
changelog. Standing decisions live in
[`wiki/architecture/bootstrap_lattice/decisions.md`](wiki/architecture/bootstrap_lattice/decisions.md),
the two remaining source-surface contracts live in
[`compiler_source_profile.md`](wiki/architecture/bootstrap_lattice/compiler_source_profile.md),
repository ownership lives in
[`repository_structure.md`](wiki/architecture/bootstrap_lattice/repository_structure.md),
and product-compiler implementation work lives in [`TASKS.md`](TASKS.md).
Completed work and exact historical gate counts remain in Git and beside their
own gates.

Before taking an item, fetch `main`, inspect the newest commits in that lane,
and avoid overlapping another active change. Commit and push coherent
milestones. Engineering difficulty is not a design blocker; mark an item
blocked only when a literal language/profile ruling is required and the
existing decision procedure cannot settle it.

## Fixed execution model

```text
language capability: Alpha → Beta → Gamma → Delta → Omega
build artifacts:     Alpha → Beta → Gamma → Delta → omega-bootstrap → omega [→ omega]
```

`omega-bootstrap` is a Delta-written compiler artifact, not a language rung.
It accepts the ordinary-Omega product-source profile `Ωself` and builds the
first production `omega`. That compiler already accepts full Omega and contains
the optimizer and advanced lowering even if its own executable was lowered
conservatively. The bracketed `omega` rebuild is optional executable
optimization, not bootstrap closure.

The queue has only two source contracts:

| Contract | Scope | Working rule |
| --- | --- | --- |
| Delta v1 | independent source language of the canonical Delta compiler and `omega-bootstrap` | robust deterministic C-class compiler host; specified failure and deterministic allocation backed by fixed, bump, or paged storage; Omega-like shape where cheap, with no subset requirement |
| `Ωself` | ordinary-Omega source profile of production `omega` | omit proof/dependent authoring forms by default; retain ordinary compiler facilities unless a measured refactor lowers total source, bridge, and assurance cost |

The exact feature procedure lives only in
[`compiler_source_profile.md`](wiki/architecture/bootstrap_lattice/compiler_source_profile.md);
Delta evidence lives only in
[`FEATURE_LEDGER.md`](bootstrap/rungs/delta/FEATURE_LEDGER.md). D0 and O0/O1 are
bounded implementation canaries, not languages, inventories, or steps toward a
hidden Omega0/Omega1 chain.

Queue invariants:

- Gamma supplies Delta's meaning route and happens to host one independent
  proof-kernel implementation. Proof checking remains cross-cutting assurance.
- Direct lower-rooted source-to-artifact refinement closes provenance. Do not
  create a DDC lane or make Rust agreement a bootstrap/release requirement.
- `compiler/{psi,omega}/` are permanent Omega-written product owners;
  external-language implementations live under suffixed `bootstrap/onramps/`.
- Only source transitively imported by the compiler belongs to the hosted
  closure. Standalone interpreters, REPLs, proof explorers, viewers, debuggers,
  and similar tools do not.
- The bridge may use a private checked IR and conservative backend. It need not
  execute the product optimizer or use Terminal Psi internally.

## Readiness and ownership

| Component | Closed evidence | Closure still required |
| --- | --- | --- |
| Delta | corpus, native path, self-host, growing Delta→Gamma route | one frozen v1 contract over both complete required Delta source closures |
| canonical Delta compiler | Delta-written self-host and bounded lower-rung runs | exact complete-source publication through Gamma joined to refinement |
| `omega-bootstrap` | deterministic custody and selected vertical compiler slices | general `Ωself` frontend/backend, complete Delta source closure, frozen acceptance |
| production `omega` source | checkpoint 000001 through the Psi lexer | remaining compiler phases, entrypoint closure, and final `Ωself` census |
| hosted build | bounded bridge canaries | one validated bridge-built full-Omega compiler |

Product source and source refactors belong to `OMEGA-PRODUCT-COMPILER-SOURCE` in
[`TASKS.md`](TASKS.md). This queue owns checkpoint census, bridge acceptance,
Delta closure, and the hosted build. A bridge-cost result may request a product
refactor, but the refactor lands through the product task and a new checkpoint.
No proof/generic/domain/tag/data-shape candidate becomes a standalone bootstrap
project before that measured disposition.

The required order is:

1. consume product checkpoints while growing general bridge capabilities;
2. freeze Delta v1 and `Ωself` at the completed Delta/bridge/product-source join;
3. publish the Delta compiler through Gamma and build `omega-bootstrap` with it;
4. perform the sole required hosted production build.

The package-evidence/accepted-lock custody contract can join when its product
owner publishes it; it blocks final authority, not the implementation work in
steps 1–2.

## Current language-design blockers

These are the only current literal language/profile blockers. Everything else
below is implementation or engineering work.

| Missing ruling | Work that remains valid meanwhile | Decision owner |
| --- | --- | --- |
| private visibility between distinct logical modules in one package | public cross-package and same-module private access | Omega language/product source |
| checkpoint lexer conflicts: Unicode XID versus ASCII-transparent identifiers, `\u{...}` versus its prohibition, unspecified raw strings, and direct `u32` cursors against `u64` collection interfaces | bridge every already-specified lexer form; exact explicit widening is settled, while implicit widening remains forbidden | `OMEGA-PRODUCT-COMPILER-SOURCE` in [`TASKS.md`](TASKS.md) |
| observable order of effectful or trapping named-record fields | pure nontrapping fields in declaration order, including `SourceId { value: source_id }` | Omega language |
| observable call-argument evaluation order | calls whose argument order cannot be observed | Omega language |
| zero initialization when an explicit sum discriminant moves the first case away from zero | sums without explicit discriminants; bridge-private compiler-controlled layout | Omega language |
| meaning and precedence of the product-only `Owner::provider_defaults` convention | opaque custody and bounded candidate discovery with no claimed selection | product owner: refactor to specified `Build::select_provider` or publish the legacy convention |

Completed-selection cohort closure is already settled; it does not define the
legacy provider-default declaration. Likewise, the Rust on-ramp's recognition
of that spelling is implementation evidence, not language authority.

## External contract dependency

The compilation-authority join is separately waiting on the package/security
owner, not on a bootstrap language ruling. Compiler-issued
`PackageAdmissionProjection` rows are permanently review-only. The settled
authority model instead rechecks exact source and artifact subjects against
canonical obligation semantics, reconstructed obligations, retained
certificates, transitive open obligations, and local admissions. What remains
unpublished is the bounded accepted-lock/closure wire, its acceptance root, and
the accepted-closure-to-`OMGCOMP` projection the bridge can independently
reconstruct. Continue fixture-driven resolution, checking, lowering, and
refinement while `RECHECKABLE-PACKAGE-EVIDENCE` and `ACCEPTED-LOCK-SCHEMA` in
[`TASKS_PACKAGE_MANAGER.md`](TASKS_PACKAGE_MANAGER.md) close that external
product contract; do not import compiler review rows, duplicate the compiler's
semantic projection, or invent a bridge-local verdict and call it authority.

### Rolling invariant — maintain the provisional Delta ledger

At every bridge milestone:

- identify each provisionally used Delta construct's concrete bridge need or
  explicit coherence/safety/robustness rationale;
- record the simpler rejected alternative, exact semantics, resource behavior,
  lower-rung meaning, positive coverage, and nearest excluded form;
- treat D0, samples, and Rust-producer acceptance as evidence only, never as
  admission to Delta v1;
- keep arithmetic, layout, call, allocation, trap, and sealed byte-I/O behavior
  aligned across native targets, the Delta self-host, and the Rust-free
  Delta-to-Gamma meaning route; and
- use the canonical deterministic source bundle without cross-unit token
  fusion or newline-concatenation semantics.

### 1. Consume product checkpoints and enforce provisional `Ωself`

- [ ] Consume refreshed checkpoint 000001 as the current coherent lexical
  product closure and reconcile bridge coverage against its source/profile
  changes. Its manifest, profile, Cargo/provider provenance, and extracted
  build prelude now pass together. Its repository-path dependency replay remains
  provisional; before the hosted join, consume a product checkpoint with
  canonical logical source placements rather than reproducing that compatibility
  scan in the bridge.
- [ ] For that checkpoint and every later coherent product checkpoint, verify
  the exact deterministic closure and update the one compositional
  feature/resource disposition table in
  [`compiler_source_profile.md`](wiki/architecture/bootstrap_lattice/compiler_source_profile.md).
  Record each used facility as provisionally retained, concretely refactored
  out, or unresolved with the missing evidence named. Absence from an incomplete
  compiler checkpoint is not a final exclusion.
- [ ] Enforce each provisional profile as general syntax, static semantics,
  resources, layout/ABI, diagnostics, and lowering rules. Gate the complete
  manifest and a phase-appropriate negative canary for every exclusion; never
  substitute file identities, statement counts, or enumerated AST shapes.
- [ ] Keep source-profile and product-language coverage visibly separate. Run
  full-Omega product tests as the compiler grows so that an `Ωself` exclusion is
  never mistaken for permission to omit that feature from production `omega`.

Acceptance for each checkpoint: its closure and separately versioned candidate
profile reproduce; admitted programs retain exact Omega meaning; unsupported
forms reject before publication; and every unresolved row names what will
settle it. The final profile remains unfrozen until the complete source/bridge
join.

### 2. Implement `omega-bootstrap` in Delta

Grow the bridge from source-profile needs through general capabilities. Exact
closed-slice history belongs in the
[`omega-bootstrap` status](bootstrap/omega-bootstrap/README.md), the
[`Ωself` evidence table](wiki/architecture/bootstrap_lattice/compiler_source_profile.md),
and versioned contracts beside their gates. A bounded slice measures cost; it
does not admit a facility to final `Ωself`.

- [ ] Close the remaining compiler-data/view forms retained by refreshed
  checkpoints: general fixed arrays and indexing, shared or mutable views,
  strings/bytes, and ordinary record/sum composition. Keep growable allocation
  separate, and do not bypass the unresolved `u32` cursor versus `u64`
  collection-interface ruling.
- [ ] Close the remaining unblocked compiler-control/scalar forms as general
  compositional relations: state parameters, mutation, calls, result fields,
  ranges, concrete trapping arithmetic, required casts, and any retained
  ranking form. The next known gap is the UTF-8 arithmetic whose subtraction
  results feed multiplication/addition. Continue to reject call arguments whose
  observable or trapping relative order depends on the unresolved language
  rule.
- [ ] Close source-graph and selected product-binding forms over resolver-owned
  logical placements: modules/import aliases, target-qualified and bodyless
  machines, `satisfies`, sealed compiler-intrinsic realizations, the retained
  boundary requirements, and static provider paths. Do not infer provider
  selection from the existing one-requirement candidate-resolution canary;
  wait for a normative product default-selection spelling before implementing
  that part.
- [ ] Join the already-closed generated-source custody route to the refreshed
  product checkpoint over the same generator/input/output tuple. Generated
  files remain ordinary source, never bridge exceptions.
- [ ] For every admitted bridge capability, ship parsing/resolution, checking,
  diagnostics, conservative lowering, resource and no-publication teeth,
  Rust-free meaning, and direct lower-rooted artifact reconstruction together.
  A frontend-only probe or a list of source permutations is not admission.
- [ ] Consume later product checkpoints by adding only newly observed or
  explicitly retained general capabilities. A later source need may reopen a
  provisional exclusion; it does not create another language or compiler
  generation.
- [ ] Publish separate complete deterministic Delta source manifests for the
  canonical Delta compiler and `omega-bootstrap`, including every transitive
  source and build input. Prove both valid under the provisional Delta ledger;
  final validity belongs to the Delta-v1 freeze. They share one Delta language
  contract rather than defining compiler-specific dialects.
- [ ] Once the package/security owner publishes the canonical bounded
  projection from recheckable package evidence and accepted lock state to one
  accepted source closure, independently reconstruct it and join the
  structurally checked multi-unit
  [compilation envelope](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_COMPILATION.md)
  to that closure's exact commitment and envelope SHA-256. The bounded
  Delta-written SHA-256 producer closes the hash computation only; the expected
  commitment and accepted source closure still come from this external
  projection. Compiler-issued review rows, stored verdicts, and structural
  validity alone are never compilation authority. This item is externally
  gated; it blocks final acceptance, not the implementation work above.

Acceptance: the bridge compiles the complete product-source manifest and every
program admitted by the general candidate profile with exact Omega semantics.
It rejects unsupported Omega before publication. It need not accept full Omega,
optimize its own output, use production allocation machinery, or host unrelated
product tools. The product optimizer and advanced lowering remain ordinary
modules in that manifest; they are compiled into the resulting production
compiler rather than reimplemented inside the bridge.

Implementation may advance independently across transport, authority,
resolution, checking/lowering, and artifact-refinement seams when their byte
contracts are fixed. Acceptance joins all of them; progress in one seam must
not be reported as authority for another.

### 3. Settle both source contracts at the completed required-source join

The join publishes two contracts with distinct scopes and versioning at the
same evidence milestone. This is a coordinated publication, not a subset or
versioning relationship: neither contract may be justified by a partial source
closure or by assumed costs in the other.

#### `Ωself`

- [ ] Reconcile the final deterministic product source closure, provisional
  compositional profile, and complete bridge implementation. For every
  disputed Omega source feature, retain it with its measured implementation and
  assurance cost discharged or refactor it out and preserve a negative canary.
  Freeze all transitive build inputs under the same rules.
- [ ] Resolve the high-leverage profile groups explicitly rather than letting
  them disappear inside the final census: presumptively exclude proof-program
  mathematics and dependent/proof-indexed typing (including linear-dependent
  forms); presumptively retain regular compiler data/control, ordinary
  ownership, basic generics, and concrete domains when used; and compare
  advanced generics/domains, numeric/schema tags, mixed record-plus-sum
  declarations, and aggregate transition payloads with simpler source
  encodings. A refactor wins only when it reduces total bridge/assurance cost
  without introducing duplication, invalid intermediate states, or
  compiler-file-shaped rules.
- [ ] Demonstrate in the hosted acceptance suite that source-profile exclusion
  is not product-language exclusion: production `omega` accepts representative
  full-Omega programs using each materially difficult facility omitted from
  `Ωself`, including the proof/dependent surface if it remains excluded.

Acceptance: every row is retained or excluded; every retained form has general
parsing, checking, meaning, lowering, resources, and negative-boundary coverage;
the profile binds a specific Omega specification revision, supported
target/configuration matrix, and deterministic union/maximum resource rule; and
the exact product source closes under those rules. Every exclusion has a valid
full-Omega canary that `omega-bootstrap` rejects at the intended phase without
partial publication and production `omega` accepts. Any product-source closure
change reopens the profile. This profile governs what `omega-bootstrap`
accepts, not which full-Omega features the resulting compiler implements.

#### Delta v1

- [ ] Publish the coherent C-class compiler-host baseline before optimizing for
  feature count: regular scalar/data/control and module facilities,
  deterministic bounded storage or allocation with specified exhaustion,
  explicit failure, and the sealed byte-input/artifact-output/diagnostic/exit
  boundary needed by both required Delta programs. This is a design floor, not
  automatic admission of every current D0 or Rust-producer construct.
- [ ] Classify every retained construct as required by the complete canonical
  Delta-compiler or `omega-bootstrap` closure, or justified by an explicit
  coherence, robustness, safety, or maintainability argument. Remove accidental
  D0, corpus, and Rust-producer behavior.
- [ ] Publish versioned normative grammar, static and dynamic semantics,
  representation/ABI, source-bundle, resource, and sealed-host-interface
  contracts under `bootstrap/rungs/delta/`.
- [ ] Reject every excluded source, type, module, boundary, and resource form
  explicitly.
- [ ] Publish a classified conformance corpus and feature manifest with positive
  observations, phase-isolated negatives, exhaustion teeth, cross-target
  layout/arithmetic edges, and native/self-host/lower-rung differentials.
  Differential agreement is bug-finding evidence, not artifact authority.
- [ ] Prove the complete deterministic Delta compiler and `omega-bootstrap`
  source closures valid under the same frozen contract.

Acceptance: Delta v1 is a coherent, robust, independently specified
compiler-host language sufficient for both required Delta programs—not a
whitelist of either program's current tokens. Within its published resource
bounds, the lattice-built Delta compiler accepts every conforming Delta-v1
program and rejects every nonconforming one according to the specified phase
and failure behavior. Later widening is an explicit versioned language change.

Joint acceptance: both publications bind the same completed
Delta-compiler/bridge/product join while remaining independent contracts.
`Ωself` does not define Delta, Delta does not define `Ωself`, and none of the
three exact source manifests substitutes for general language/profile rules.

### 4. Publish Delta through Gamma, then validate Delta → `omega-bootstrap`

- [ ] Execute the exact frozen Delta-written compiler through the canonical
  Beta-written Delta→Gamma elaborator and Gamma's Beta-written interpreter on
  its exact source to publish the native Delta compiler artifact. Join the
  compiler source, elaborated program, Gamma execution, produced artifact,
  canonical Delta meaning, resource/exhaustion behavior, and independently
  reconstructed source-to-artifact refinement. A Rust-built or Delta-self-built
  compiler may remain a differential and reproducibility control; neither is
  the required publisher.
- [ ] Use that exact lower-rung-published compiler to build the bridge artifact
  through the canonical lattice path and
  join its Delta source closure, produced artifact, canonical meaning, and
  independently reconstructed lower-rooted source-to-artifact refinement in one
  gate. Exercise profile-wide compositional positives, excluded-feature
  diagnostics, exhaustion, deterministic publication, conservative lowering,
  and relevant proof/translation-validation seams. Rust-built or self-host
  agreement may remain differential evidence but cannot substitute for the
  join.

Acceptance: the exact Delta compiler and exact bridge are each bound to their
exact source and meaning; the first compiler artifact is reproducibly published
through the lower-rung semantic route; the bridge needs no Rust producer or
ambient assembler/linker in the required path; and it correctly accepts and
rejects the frozen `Ωself` profile.

### 5. Perform the sole required hosted production build

- [ ] Run the validated Delta-built bridge on the exact frozen `Ωself`
  manifest. Validate the resulting compiler against canonical meaning, the
  versioned full-Omega conformance manifest, full compiler/language suites, and
  applicable artifact-refinement seams. Exercise representative facilities
  deliberately absent from `Ωself`, reach the optimizer and advanced lowering
  in executable tests, and compare specified artifacts/meaning with those passes
  enabled and disabled. All required acceptance runs use this first bridge-built
  compiler, never the optional self-rebuilt executable.

Acceptance: the produced compiler accepts full Omega and contains the product
optimizer and advanced lowering, although its own executable may have been
generated conservatively. This closes the required lattice. A later
`omega` → `omega` rebuild is optional product optimization and reproducibility
work.

## Gate and performance discipline

- Keep one focused gate per active capability; run the full lattice gate only
  at coherent milestones.
- Give transport decoding, semantic resolution, checked-IR validation, artifact
  reconstruction, and orchestration separate modules/checkers. Compose them by
  versioned artifacts and cross-pair tests; do not grow one verifier through a
  Cartesian product of source and artifact permutations.
- Put shared fixture generation and corpus registration in small harnesses.
  Keep positive, negative, resource, and target families in responsibility-
  specific files so adding a case does not recompile an unrelated monolith.
- A gate approaching tens of minutes must report subgate timings before more
  feature growth. Profile compiler, evaluator, and harness separately.
- Keep one compilation single-threaded until profiling justifies compiler
  concurrency. Parallelize independent fixture compiles first.
- Exhaustive native matrices may be paired with a small representative
  self-built/lower-rung matrix when the latter is semantically redundant and
  disproportionately slow; document the coverage split.
- Debug HTML, viewers, exhaustive dumps, and other human-only artifacts are
  opt-in. Default gates emit only checker-consumed evidence and concise failure
  diagnostics.
- Paged arenas, parallel lowering, advanced optimization, and incremental
  compilation are permitted performance work, not prerequisites for Delta or
  `omega-bootstrap`.
