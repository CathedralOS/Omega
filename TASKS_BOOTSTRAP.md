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

## Settled boundary

```text
Alpha → Beta → Gamma → Delta
Delta bridge source ──[lattice-built Delta compiler]──▶ omega-bootstrap
Ωself product source ──[omega-bootstrap]──────────────▶ omega (full Ω; conservative binary)
Ωself product source ──[optional omega rebuild]───────▶ omega (same compiler; optimized binary)
```

In artifact shorthand this is `Alpha → Beta → Gamma → Delta → omega-bootstrap
→ omega [→ omega]`. Language growth stops at Delta. Everything to its right is
a compiler artifact or a build edge, not another language rung.

Only two source contracts remain open:

| Surface | Kind | Required closure |
| --- | --- | --- |
| Delta v1 | independent robust compiler-host language, C-like in power and Omega-shaped where cheap | the complete Delta source of `omega-bootstrap` plus explicit coherence, robustness, safety, and maintainability arguments |
| `Ωself` | compositional subset of already-valid Omega, with no private meaning | the complete Omega source of production `omega` |

`omega-bootstrap` is written in Delta and need only accept `Ωself`. The
production source is written in `Ωself` but must define a compiler that accepts
full Omega and contains the production optimizer and advanced lowering. A
compiler does not need to use a language feature in order to implement that
feature for its users.

Those two source choices discharge three artifact obligations:

| Artifact | Must accept | Must contain or produce |
| --- | --- | --- |
| lattice-built Delta compiler | Delta v1 | a correct `omega-bootstrap` executable from the exact Delta bridge closure |
| `omega-bootstrap` | frozen `Ωself` | a semantically exact, possibly conservative production-compiler executable |
| production `omega` | full Omega | the full optimizer, advanced lowering, and specified artifact behavior |

The first production binary may be slow because the bridge lowered it
conservatively; its accepted language and the compiler implementation it
contains are still complete. Only the optional bracketed edge is an Omega
self-rebuild. Detailed rationale and the feature-disposition procedure live in
[`compiler_source_profile.md`](wiki/architecture/bootstrap_lattice/compiler_source_profile.md).

Guardrails for this queue:

- The proof kernel is cross-cutting assurance, with Beta and Gamma
  implementations; Gamma is not the proof-checker rung.
- Compiler authority follows direct lower-rooted source-to-artifact refinement;
  cross-compiler agreement is optional bug-finding evidence. See
  [D5](wiki/architecture/bootstrap_lattice/decisions.md#d5--direct-checked-refinement-closes-compiler-provenance).
- Do not create a diverse-double-compilation lane. The complete lower-rooted
  chain checks each compiler edge directly; another producer may find bugs but
  cannot add authority or become a release dependency merely by agreeing.
- The current Rust Psi/Omega compiler stays under the explicitly suffixed
  `bootstrap/onramps/omega-rust/` owner as an optional differential producer.
  `compiler/{psi,omega}/` owns Omega-written product source.
- Standalone Terminal-Psi interpreters, verifiers, REPLs, proof explorers,
  viewers, and debuggers are not in the hosted compiler closure unless the
  compiler executable imports them. Product Terminal-Psi representation and
  lowering modules that it does import remain ordinary source dependencies.
- `omega-bootstrap` may use a direct checked-IR lowering. It need not use
  Terminal Psi internally merely because it compiles product modules that do.

## Delta → omega-bootstrap → production Omega readiness

Delta's compiler-host feasibility, self-host, and bounded bridge canaries exist.
The general `omega-bootstrap`, frozen Delta v1, frozen `Ωself`, and hosted
production build do not. Canaries and D0 are discovery evidence, not numbered
steps toward `Ωself` or definitions of Delta v1.

Two lanes co-evolve until their join:

| Lane | Owner | Bootstrap responsibility |
| --- | --- | --- |
| production compiler source | `OMEGA-PRODUCT-COMPILER-SOURCE` in [`TASKS.md`](TASKS.md) | consume each deterministic checkpoint; derive and measure provisional `Ωself` |
| bridge and language closure | this file | implement general profile rules in Delta; maintain the Delta ledger; freeze both contracts at their completed closures |

The immediate executable order is:

1. continue general checkpoint-000001 capabilities, one compositional vertical
   slice at a time;
2. consume later product-source checkpoints as they are published; and
3. perform the profile/language freezes and hosted builds only after both
   complete source closures exist.

The recheckable package-evidence/accepted-lock custody join may land whenever
its external contract is published. It does not reorder or block these items.

Product Psi/Omega implementation and any chosen source refactor stay in
`TASKS.md`. This queue must not turn possible `Ωself` exclusions into separate
proof, generics, domain, field-tag, data-shape, or transition projects. Those
choices stay in the single disposition table in
[`compiler_source_profile.md`](wiki/architecture/bootstrap_lattice/compiler_source_profile.md).
Likewise, this queue does not decide whether full Omega has those features: the
language specification already does. Bootstrap work only prices and implements
the ordinary-Omega forms retained in the compiler's own source profile.

The required execution order is:

1. consume product checkpoints while growing the general bridge;
2. freeze `Ωself` at the complete product-source/bridge join;
3. freeze Delta v1 from the complete bridge source and explicit language
   arguments;
4. build and validate `omega-bootstrap` through the lattice; and
5. perform the one required hosted production build.

The optional product self-rebuild is not part of this queue. Fixed or paged
backing, typed/indexed arenas, bulk reclamation, and conservative lowering are
available bridge implementation choices when they reduce total cost. They do
not become Delta features without specified behavior, lower-rung meaning, and
explicit failure. Maintain that evidence only in
[`bootstrap/rungs/delta/FEATURE_LEDGER.md`](bootstrap/rungs/delta/FEATURE_LEDGER.md).

## Language rulings required

The visibility rule for private access between distinct logical modules in one
package is unspecified. Until it is ruled, the bridge rejects that case. Public
cross-package access and same-module private access remain unblocked, including
the current two-package nominal-data artifact. The selected constant-aggregate,
runtime-record, and direct-field-receiver slices are deliberately same-module
and do not depend on this ruling.

Checkpoint 000001's product lexer also conflicts with the current language
guide: Unicode XID identifiers contradict its ASCII-transparent wording,
`\u{...}` escapes contradict its explicit prohibition, raw-string semantics
are absent, and `u32` cursors compare directly with the specified-`u64` slice
length without a settled cross-carrier rule. Those are product-language ruling
blockers recorded under `OMEGA-PRODUCT-COMPILER-SOURCE` in
[`TASKS.md`](TASKS.md). They do not block bridge implementation for source
forms whose meaning is already settled, as the closed same-module runtime-
record tranche demonstrates. No implementation or engineering difficulty below
is otherwise a design blocker.

Omega also does not yet specify observable evaluation order among effectful or
trapping fields of a runtime named-record literal. CKIR4 therefore admits only
pure, non-trapping leaf fields and canonicalizes them by declaration ordinal;
broader constructor fields remain design-blocked until the language owner rules
their order. The exact `SourceId { value: source_id }` dependency does not need
that ruling.

Call-argument evaluation order is likewise still advisory rather than
normative in the language guide, while the current bridge lowering evaluates
arguments left-to-right. Until the language owner rules it, admitted bridge
calls must have argument expressions whose relative order cannot be observed;
effectful or trapping argument combinations remain blocked.

The sum specification has one unresolved interaction outside the first bridge
slice: explicit discriminants can move the first case away from zero despite
the zero-initialization rule. Default aggregate layout is compiler-controlled,
so declaration-order case identity must not be described as a unique public
byte ABI. The first payload-sum tranche remains unblocked by excluding explicit
discriminants and deriving one bridge-private layout from the checked
declaration graph.

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

- [ ] For every coherent product-source checkpoint published by
  `OMEGA-PRODUCT-COMPILER-SOURCE` in [`TASKS.md`](TASKS.md), verify its exact
  deterministic closure and derive or update the distinct compositional
  candidate feature/resource profile. Checkpoint 000001 already supplies the
  first closure and normalized-syntax/resource profile; later compiler phases
  publish later checkpoints from their product owner.
- [ ] Measure every used feature's source benefit against the cost of its
  general Delta-written bridge implementation. Record one provisional outcome:
  retain, refactor from product source and preserve a negative canary, or leave
  unresolved with the exact missing evidence. Absence from a partial checkpoint
  is not a final exclusion.
- [ ] Publish and provisionally enforce compositional syntax, static semantics,
  resources, ABI/layout, and lowering rules. File identities, exact statement
  counts, and enumerated AST permutations are not profile rules.
- [ ] Update the single working feature-disposition table in
  [`compiler_source_profile.md`](wiki/architecture/bootstrap_lattice/compiler_source_profile.md)
  as evidence lands; do not copy that inventory into this task list.
- [ ] Gate the complete checkpoint manifest plus one phase-appropriate negative
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
| multi-unit structural custody | closed for exact `OMGCOMP`; no resolver/lock or digest authority | [`OMEGA_BOOTSTRAP_COMPILATION.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_COMPILATION.md) |
| source resolution | closed through same-module direct-self and direct-field receivers in `OMGRSW2` | [`OMEGA_BOOTSTRAP_RESOLUTION.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION.md), [`OMEGA_BOOTSTRAP_RESOLUTION_V2.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V2.md) |
| checked lowering and composition | closed through CKIR4 for selected calls, constant aggregates, runtime named records, and direct-field receivers, with native, self-built, mixed, and representative Rust-free paths | [`OMEGA_BOOTSTRAP_CHECKED_IR.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR.md), [`OMEGA_BOOTSTRAP_CHECKED_IR_V2.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V2.md), [`OMEGA_BOOTSTRAP_CHECKED_IR_V3.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V3.md), [`OMEGA_BOOTSTRAP_CHECKED_IR_V4.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V4.md), [`OMEGA_BOOTSTRAP_RESOLVED_TO_CKIR4_V2.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLVED_TO_CKIR4_V2.md) |
| lower-rooted artifact reconstruction | closed through the CKIR4 direct-field successor while retaining five distinct responsibility owners and shared bounded decoders | [`OMGCOMP_REFINEMENT_WITNESS.md`](bootstrap/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS.md), [`OMGCOMP_REFINEMENT_WITNESS_V3.md`](bootstrap/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V3.md), [`OMGCOMP_REFINEMENT_WITNESS_V4.md`](bootstrap/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V4.md), [`OMGCOMP_REFINEMENT_WITNESS_V5.md`](bootstrap/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V5.md), [`OMGCOMP_REFINEMENT_WITNESS_V6.md`](bootstrap/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V6.md) |
| compilation authority | externally gated: recheckable package evidence and accepted-lock schema are ruled, but their bounded accepted-closure projection plus exact envelope SHA-256 join is not yet published | compilation and witness contracts above |

None of these bounded closures admits a source family to final `Ωself` or
makes Terminal Psi part of the bridge. Terminal-Psi vocabulary and production
compiler implementation remain product work in `TASKS.md`.

The closed slices are bounded implementation-cost evidence, not general
language coverage or admission to final `Ωself`. Continue with capabilities
actually used by published product checkpoints; do not idle on the separately
gated compilation-authority join.

- [ ] Price a closed payload-bearing-sum tranche as the next checked-IR
  facility unless a newer product checkpoint changes the measured order. The
  first slice may use unnumbered pure sums, declaration-order case identity with
  the first case at tag zero, payload-free and payload-bearing cases,
  recursively copyable payloads through the provisional four-field ceiling,
  construction, Copy/Call arguments, tag dispatch, and payload binding. Its
  checker must derive a bridge-private layout from declarations; it must not
  claim a unique public/default Omega byte ABI. Keep generic sums, mixed
  record/sum declarations, explicit schema/discriminant tags, structural
  returns, aggregate transition literals, effectful or trapping payload
  expressions, and calls with observably order-dependent arguments outside the
  first slice. This is bridge-cost evidence for a provisional `Ωself`
  candidate, not a final profile ruling.
- [ ] Continue through the remaining general capabilities used by checkpoint
  000001, then later provisional checkpoints, until the bridge generally parses,
  resolves, checks, diagnoses, and conservatively lowers every program admitted
  by candidate `Ωself`. The closed versioned-call tranches own their admitted
  call forms; transport work alone does not imply a source widening.
- [ ] Carry each admitted capability's compositional rules, negative boundary,
  resource teeth, Rust-free meaning, and direct artifact path in the same
  milestone. A bounded frontend-only cost probe is evidence, not bridge
  admission.
- [ ] Publish the complete deterministic Delta source closure of
  `omega-bootstrap`, including every transitive source and build input. Prove
  it valid under the provisional Delta ledger; final validity belongs to the
  Delta-v1 freeze.
- [ ] Once the package/security owner publishes the canonical bounded
  projection from recheckable package evidence and accepted lock state to one
  accepted source closure, independently reconstruct it and join the
  structurally checked multi-unit
  [compilation envelope](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_COMPILATION.md)
  to that closure's exact commitment and envelope SHA-256. Compiler-issued
  review rows, stored verdicts, and structural validity alone are never
  compilation authority. This item is externally gated; it blocks final
  acceptance, not the implementation work above.

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

### 3. Freeze `Ωself` at the completed bridge join

- [ ] Reconcile the final deterministic product source closure, provisional
  compositional profile, and complete bridge implementation. For every
  disputed Omega source feature, retain it with its measured implementation and
  assurance cost discharged or refactor it out and preserve a negative canary.
  Freeze all transitive build inputs under the same rules.

Acceptance: every row is retained or excluded; every retained form has general
parsing, checking, meaning, lowering, resources, and negative-boundary coverage;
the profile binds a specific Omega specification revision, supported
target/configuration matrix, and deterministic union/maximum resource rule; and
the exact product source closes under those rules. Every exclusion has a valid
full-Omega canary that `omega-bootstrap` rejects at the intended phase without
partial publication and production `omega` accepts. Any product-source closure
change reopens the profile. This profile governs what `omega-bootstrap`
accepts, not which full-Omega features the resulting compiler implements.

### 4. Freeze Delta v1

- [ ] Classify every retained construct as required by the complete bridge
  closure or justified by an explicit coherence, robustness, safety, or
  maintainability argument. Remove accidental D0, corpus, and Rust-producer
  behavior.
- [ ] Publish versioned normative grammar, static and dynamic semantics,
  representation/ABI, source-bundle, resource, and sealed-host-interface
  contracts under `bootstrap/rungs/delta/`.
- [ ] Reject every excluded source, type, module, boundary, and resource form
  explicitly.
- [ ] Publish a classified conformance corpus and feature manifest with positive
  observations, phase-isolated negatives, exhaustion teeth, cross-target
  layout/arithmetic edges, and native/self-host/lower-rung differentials.
  Differential agreement is bug-finding evidence, not artifact authority.
- [ ] Prove the complete deterministic `omega-bootstrap` source closure valid
  under the frozen contract.

Acceptance: Delta v1 is a coherent, robust, independently specified
compiler-host language sufficient for the complete bridge—not a whitelist of
that bridge's current tokens. Within its published resource bounds, the
lattice-built Delta compiler accepts every conforming Delta-v1 program and
rejects every nonconforming one according to the specified phase and failure
behavior. Later widening is an explicit versioned language change.

### 5. Validate Delta → `omega-bootstrap`

- [ ] Build the exact bridge artifact through the canonical lattice path and
  join its Delta source closure, produced artifact, canonical meaning, and
  independently reconstructed lower-rooted source-to-artifact refinement in one
  gate. Exercise profile-wide compositional positives, excluded-feature
  diagnostics, exhaustion, deterministic publication, conservative lowering,
  and relevant proof/translation-validation seams. Rust-built or self-host
  agreement may remain differential evidence but cannot substitute for the
  join.

Acceptance: the exact lattice-built bridge is bound to its exact source and
meaning, needs no Rust producer or ambient assembler/linker in the required
path, and correctly accepts and rejects the frozen `Ωself` profile.

### 6. Perform the sole required hosted production build

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
