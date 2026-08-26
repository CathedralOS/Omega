# Bootstrap lattice — active work

Last pruned: 2026-08-26.

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

The bootstrap has only two remaining source-surface selections. Full Omega is
already specified and is not a third bootstrap feature vote:

| Contract | Scope | Working rule |
| --- | --- | --- |
| Delta v1 | independent source language of the canonical Delta compiler and `omega-bootstrap` | robust deterministic C-class compiler host; specified failure and deterministic allocation backed by fixed, bump, or paged storage; Omega-like shape where cheap, with no subset requirement |
| Omega compiler-source profile (`Ωself`) | ordinary-Omega authoring profile of production `omega`; not another language or compiler generation | omit proof/dependent authoring forms by default; retain ordinary compiler facilities unless a measured refactor lowers total source, bridge, and assurance cost |

The exact feature procedure lives only in
[`compiler_source_profile.md`](wiki/architecture/bootstrap_lattice/compiler_source_profile.md);
Delta evidence lives only in
[`FEATURE_LEDGER.md`](bootstrap/delta/FEATURE_LEDGER.md). D0 and O0/O1 are
bounded implementation canaries, not languages, inventories, or steps toward a
hidden Omega0/Omega1 chain.

Keep the two decisions and the three compiler artifacts distinct:

| Question | Answered by |
| --- | --- |
| What language may the Delta compiler and bridge use? | Delta v1 |
| Which ordinary-Omega forms may production compiler source use? | `Ωself` |
| What must the resulting compiler accept? | the full Omega specification |

The canonical Delta compiler, `omega-bootstrap`, and production `omega` are
artifacts governed by those contracts; they are not three more language rungs.

Queue invariants:

- Gamma supplies Delta's meaning route and happens to host one independent
  proof-kernel implementation. Proof checking remains cross-cutting assurance.
- Direct lower-rooted, subject-qualified source-to-artifact refinement closes
  provenance. Bare kernel acceptance is insufficient; compiler agreement and
  Rust availability are never bootstrap or release requirements. The complete
  chain closes implementation-language dependency; its edge-by-edge direct
  refinement, not the mere existence of the chain, is what makes diverse
  double compilation unnecessary.
- `source/compiler/omega/{psi,omega}/` are permanent Omega-written product owners;
  the current external-language implementation lives at `source/compiler/rust/`.
- Only source transitively imported by the compiler executable belongs to the
  hosted closure. Full-Omega conformance requires every specified language
  facility, but does not require standalone interpreters, REPLs, proof
  explorers, viewers, debuggers, and similar adjacent tools.
- The bridge may use a private checked IR and conservative backend. It need not
  execute the product optimizer or use Terminal Psi internally.

## Delta → omega-bootstrap → production Omega readiness

Delta's compiler-host feasibility, self-host, and bounded bridge canaries exist.
The general `omega-bootstrap`, frozen Delta v1, frozen `Ωself`, and hosted
production build do not. Canaries and D0 are discovery evidence, not numbered
steps toward `Ωself` or definitions of Delta v1.

Current state, without extrapolating from bounded canaries:

| Component | What exists | What is still missing |
| --- | --- | --- |
| Delta language | executable corpus, native compiler path, self-host evidence, and a growing Delta→Gamma meaning route | a frozen v1 specification justified by both complete required Delta source closures, plus complete lower-rung coverage |
| canonical Delta compiler | a Delta-written self-hosting compiler and bounded lower-rung executions | publication of the exact final compiler artifact from its complete source through Gamma, joined to refinement |
| `omega-bootstrap` | multi-unit custody and selected vertical source→checked-IR→artifact→refinement slices, indexed by the bridge-local versioned contracts | the general compositional `Ωself` frontend, complete conservative backend, complete source closure, and frozen acceptance contract |
| production Omega source | checkpoint 000001 for the Psi source-to-token phase | the parser, checker, compiler-linked terminal-Psi representation/lowering, optimizer, backend, entrypoint closure, and final `Ωself` census; standalone interpreters and viewers are not implied |
| hosted production build | bounded bridge canaries only | the first validated build of full production `omega`; no optional self-rebuild is required to close bootstrap |

This table is the stopping-point summary. A row is not promoted by a nearby
fixture or format version: only its stated whole-source and acceptance join
closes it.

Five coupled workstreams co-evolve until their join:

| Workstream | Current evidence | Required closure |
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

Only unresolved Omega meaning may block a bridge slice. The current boundaries
are:

| Unresolved language point | Fail-closed bridge boundary | Work that remains unblocked |
| --- | --- | --- |
| private access between distinct logical modules in one package | reject that access | public cross-package access and same-module private access |
| Unicode XID identifiers, `\u{...}` escapes, and raw-string spelling | checkpoint 000001 records the conflict but does not claim full lexical conformance | bridge work over source forms whose meaning is already settled |
| evaluation order among effectful or trapping named-record fields | admit only combinations whose relative order is unobservable | pure, non-trapping fields such as `SourceId { value: source_id }` |
| call-argument evaluation order | admit at most one observably effectful or trapping argument | calls with pure/non-trapping siblings |
| explicit sum discriminants versus first-case/tag-zero initialization | exclude explicit discriminants from the bounded sum slice; never expose the bridge-private layout as a public ABI | declaration-order payload sums under compiler-controlled layout |

The former `u32` collection-index mismatch is closed: product source now uses
`u64` for collection coordinates and counts. The remaining lexical and
evaluation/layout rulings are also recorded under
`OMEGA-PRODUCT-COMPILER-SOURCE` in [`TASKS.md`](TASKS.md). Engineering cost,
missing implementation, slow gates, and incomplete assurance are not language-
design blockers.

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

The following are rolling acceptance obligations for each checkpoint, not five
independent tasks that can be permanently checked off:

- For every coherent product-source checkpoint published by
  `OMEGA-PRODUCT-COMPILER-SOURCE` in [`TASKS.md`](TASKS.md), verify its exact
  deterministic closure and derive or update the distinct compositional
  candidate feature/resource profile. Checkpoint 000001 supplied the first
  coherent published closure and normalized-syntax/resource profile. Its
  compiled sources, Cargo/provider provenance, extracted build prelude, and
  explicit typed Console-provider selection now match the refreshed evidence.
  Any future drift must refresh the complete manifest/profile/prelude set
  together; later compiler phases publish later checkpoints from that owner.
- Measure every used feature's source benefit against the cost of its
  general Delta-written bridge implementation. Record one provisional outcome:
  retain, refactor from product source and preserve a negative canary, or leave
  unresolved with the exact missing evidence. Absence from a partial checkpoint
  is not a final exclusion.
- Apply the standing asymmetry: provisionally retain ordinary compiler-building
  facilities after demonstrated use, and require a concrete source refactor
  before proposing exclusion. Keep proof/dependent facilities presumptively
  excluded unless the compiler source itself demonstrates a need.
- Publish and provisionally enforce compositional syntax, static semantics,
  resources, ABI/layout, and lowering rules. File identities, exact statement
  counts, and enumerated AST permutations are not profile rules.
- Update the single working feature-disposition table in
  [`compiler_source_profile.md`](wiki/architecture/bootstrap_lattice/compiler_source_profile.md)
  as evidence lands; do not copy that inventory into this task list.
- Gate the complete checkpoint manifest plus one phase-appropriate negative
  canary for every excluded capability. Separately run the full-Omega product
  suites so an omission from compiler source is never confused with an
  omission from the compiler it implements.

Checkpoint 000001's manifest, normalized census, provisional profile digest,
resource bounds, admission canaries, and mutation teeth are already gated.
Typed semantics, lowering, capacity, and general artifact coverage remain
section-2 work.

Its current resolver replay maps `use` components onto repository files and the
product sources generally omit explicit `module` items. That is exact evidence
about the provisional closure, not authority for legacy filename-derived name
resolution. Before the hosted join, the product owner must publish canonical
logical source placements and source valid under the normative visibility and
import rules. The bridge consumes those placements, requires any authored
module declaration to agree, and must not reproduce the compatibility scanner.

Acceptance for each checkpoint: its exact manifest and separately versioned
candidate-profile evidence reproduce; admitted programs retain exact Omega
meaning; unsupported forms reject; and every unresolved row names the evidence
needed to settle it. The final profile remains unfrozen until the complete
source and bridge join.

### 2. Implement `omega-bootstrap` in Delta

Grow the bridge from checkpoint needs through general capabilities. Do not
recognize the current compiler files, declaration counts, or syntax-tree
permutations.

Current bridge status is reported only at milestone granularity here; exact
formats, byte counts, mutation matrices, and responsibility-local evidence live
beside the linked contracts:

| Responsibility | Current closure | Canonical detail |
| --- | --- | --- |
| one-unit source/checking/artifact probe | closed for the finite, acyclic, returning `CKIR1`→limited-ELF tranche; not checkpoint closure | [`SOURCE_CUSTODY_FRONTEND_PROBE.md`](bootstrap/omega-bootstrap/compiler/SOURCE_CUSTODY_FRONTEND_PROBE.md), [`OMEGA_BOOTSTRAP_CHECKED_IR.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR.md) |
| multi-unit structural custody | closed for generic `OMGCOMP1`, the exact Linux-x86-64/native-provider configuration in `OMGCOMP2`, and exactly one explicit root-package build-source role in `OMGCOMP3`; bounded Delta SHA-256 closes exact raw-envelope hashing through the public ceiling, while source/provider spellings remain opaque and no expected commitment, resolver/lock, accepted-closure, or digest authority follows | [`OMEGA_BOOTSTRAP_COMPILATION.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_COMPILATION.md), [`OMEGA_BOOTSTRAP_COMPILATION_V2.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_COMPILATION_V2.md), [`OMEGA_BOOTSTRAP_COMPILATION_V3.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_COMPILATION_V3.md), [`OMEGA_BOOTSTRAP_SHA256.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_SHA256.md) |
| generated ordinary-source custody | closed for checkpoint 000001's exact Unicode tuple through a sealed locked/offline recipe, generic provenance roles, two-run reproduction, bounded/no-publication teeth, exact OMGCOMP1 extent, CKIR3/OMGRFN4 preflight composition, and the refreshed product-owned checkpoint join | [`OMEGA_BOOTSTRAP_GENERATED_SOURCE_CUSTODY.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_GENERATED_SOURCE_CUSTODY.md), [`source checkpoint status`](source/compiler/omega/source-checkpoints/README.md) |
| source resolution | bounded compositional relations are closed for the selected checkpoint facilities; OMGRSW9 additionally closes the product build's exact six-row `Console` selection and complete static `ProviderPlan`, with native/self identity and lower-rooted structural reconstruction; selection remains distinct from provider admission | [`OMEGA_BOOTSTRAP_RESOLUTION_V9.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V9.md) and preceding versioned contracts beside the [bridge compiler](bootstrap/omega-bootstrap/compiler/) |
| checked lowering and composition | bounded compositional relations are closed for selected data, control, scalar, and view facilities; OMGLOWI18/CKIR17 now carry the selected `Console` adapters and ranked helper through receiverless, platform-neutral execution with ordered abstract byte events, but no installed provider, syscall, or artifact | [`OMEGA_BOOTSTRAP_CHECKED_IR_V17.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V17.md), [`OMEGA_BOOTSTRAP_LOWERING_V18.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_LOWERING_V18.md), and preceding versioned contracts beside the [bridge compiler](bootstrap/omega-bootstrap/compiler/) |
| lower-rooted reconstruction | independent R1–R5 owners reconstruct the currently selected source/checked-IR/artifact relations; OMGRFN20 joins exact OMGCOMP3, OMGRSW9, and CKIR17 bytes to the abstract event observations while deliberately making no artifact or authority claim | [`omega-bootstrap` refinement status](source/assurance/refinement/omega-bootstrap/README.md) and its versioned witness contracts |
| compilation authority | externally gated: recheckable package evidence and accepted-lock schema are ruled, but their bounded accepted-closure projection plus exact envelope SHA-256 join is not yet published | compilation and witness contracts above |

None of these bounded closures admits a source family to final `Ωself` or
makes Terminal Psi part of the bridge. Terminal-Psi vocabulary and production
compiler implementation remain product work in `TASKS.md`.

The closed slices are bounded implementation-cost evidence, not general
language coverage or admission to final `Ωself`. Continue with capabilities
actually used by published product checkpoints; do not idle on the separately
gated compilation-authority join.

Checkpoint 000001 leaves the following implementation lanes. These are work
clusters, not final retain/exclude decisions and not an order mandate; take an
unblocked capability as one complete vertical milestone rather than widening
all members of a row at once. The single disposition inventory and exact
evidence stay in
[`compiler_source_profile.md`](wiki/architecture/bootstrap_lattice/compiler_source_profile.md).

| Open implementation lane | Checkpoint forms to carry generally | Known boundary |
| --- | --- | --- |
| compiler data and views | fixed arrays, checked runtime indexing, borrowed shared/mutable slices, byte/string literals, and remaining general named-record/payload-sum composition | growable allocation is separate; recurrent guarded head/tail over a runtime-capable direct `&[u8]`, with ordered direct-binder pass-through vectors, is closed through CKIR15/OMGRFN17. Its direct pure full-width `u64 < u64` prerequisite and true-edge range custody are closed through CKIR16/OMGRFN18; mutable views, dynamic indexing, computed/effectful siblings, and the remaining same-carrier `u64` collection operations remain open |
| compiler control and remaining scalar operations | state parameters, mutation, calls, explicit result fields, ranges, remaining concrete Trapping arithmetic/casts, and the observed ranking clause | exact widening, canonical leaf-plus-literal addition, one recursive pure full-width `u32 in Trapping` `+`/`-`/`*` tree, and direct pure full-width `u64 < u64` with true-edge range custody are closed through persisted lower-rooted evidence. CKIR17 additionally closes the exact authored `u8 as i32` used by the ranked `Console` helper; call-context widening remains forbidden. Call and CaseDispatch argument vectors are exact when they contain at most one potentially trapping argument and pure/nontrapping siblings; multiple observable trap sites still require the unresolved order ruling. Other `u64` operations, broader receivers, recursion, and packages remain separate |
| source graph and selected product bindings | modules/import aliases over resolver-owned logical placements; target-qualified and bodyless machines; `satisfies`; sealed compiler-intrinsic realizations; the boundary trait and static provider paths actually used | private cross-module visibility and final logical placements remain owner-gated; OMGCOMP3/OMGRSW9/OMGLOWI18/CKIR17/OMGRFN20 close the exact build role, complete six-requirement product `Console` plan, receiverless checked adapters, ranked helper, and ordered abstract byte-event execution. Provider admission and native effects remain open; do not infer target-default semantics or import general boundary traits into Delta |

The remaining actions are intentionally capability-shaped. Exact version identities,
fixture matrices, byte ceilings, and completed milestones remain with their
contracts, gates, and Git history rather than as checked-off tasks here.

- [ ] After an admitted provider installation and its target calling plan are
  available, carry that same checked relation through conservative native
  boundary execution and direct artifact reconstruction. Until then, keep the
  provider-effect milestone separate and fail closed rather than manufacturing
  authority from OMGRSW9's selected plan.

Implement each as a general relation rather than file-name checks, declaration
counts, compiler-source AST permutations, or a Cartesian matrix in one verifier.
Start a fresh focused assurance family when an existing owner is near its tape
or evaluation ceiling; keep responsibility-local mutations and one canonical
join.
- [ ] Consume each later provisional product checkpoint and add only its newly
  observed, directionally clear capability lanes under the same rules. A later
  source need may reopen a provisional exclusion; it does not create another
  bootstrap language or numbered compiler generation.
- [ ] Carry each admitted capability's compositional rules, negative boundary,
  resource teeth, Rust-free meaning, and direct artifact path in the same
  milestone. A bounded frontend-only cost probe is evidence, not bridge
  admission.
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
  Freeze all transitive build inputs under the same rules. Publish one final
  disposition ledger that records source use, the concrete alternative when
  excluded, general bridge/assurance cost, the retain-or-exclude result, and
  the paired bridge-rejection/product-acceptance evidence. Do not distribute
  these decisions among feature-specific task lists.
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
  contracts under `bootstrap/delta/`.
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
- Keep immutable R1–R5 responsibility owners thin. Shared parsing, fixture,
  semantic-word, and mutation machinery belongs in focused libraries; adding a
  new context must not append another permutation branch to a monolithic
  all-versions verifier. Split a responsibility owner before it becomes the
  dominant compile/evaluation cost.
- Put shared fixture generation and corpus registration in small harnesses.
  Keep positive, negative, resource, and target families in responsibility-
  specific files so adding a case does not recompile an unrelated monolith.
- A gate approaching tens of minutes must report subgate timings before more
  feature growth. Profile compiler, evaluator, and harness separately.
- The checkpoint 000001 native compiler phase measured about seven minutes at
  sustained one-core CPU on macOS arm64 on 2026-08-26. Treat that compiler
  phase, not the focused 1–9 second CKIR17/OMGLOWI18/OMGRFN20 gates, as the
  current performance target; capture stage timings before arena or compiler-
  concurrency changes.
- Do not put OMGRSW9's monolithic Gamma replay in the default lattice. Its
  681,067-byte elaborated program took 209.34 seconds for the canonical case on
  the 2026-08-26 measurement and then produced an invalid observation element.
  Preserve the focused native/self and modular lower-rooted gates; revisit a
  lower-rung replay only after phase splitting or evaluator/compiler profiling
  makes it both bounded and diagnostically sound.
- Keep one compilation single-threaded until profiling justifies compiler
  concurrency. Parallelize independent fixture compiles first.
- Exhaustive native matrices may be paired with a small representative
  self-built/lower-rung matrix when the latter is semantically redundant and
  disproportionately slow; document the coverage split.
- Put version-specific matrix policy beside its gate. The default lattice step
  should run exhaustive cheap owner-local checks plus a representative expensive
  native/self or lower-rung join; reserve historical Cartesian audits for an
  explicit opt-in mode. Keep phase timings and precise cache inputs live.
- Debug HTML, viewers, exhaustive dumps, and other human-only artifacts are
  opt-in. Default gates emit only checker-consumed evidence and concise failure
  diagnostics.
- Paged arenas, parallel lowering, advanced optimization, and incremental
  compilation are permitted performance work, not prerequisites for Delta or
  `omega-bootstrap`.
