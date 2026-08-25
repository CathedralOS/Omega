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
Delta compiler source ──[Delta→Gamma + Gamma execution]──▶ delta compiler
Delta bridge source ──[delta compiler]───────────────────▶ omega-bootstrap
Ωself product source ──[omega-bootstrap]──────────────▶ omega (full Ω; conservative binary)
Ωself product source ──[optional omega rebuild]───────▶ omega (same compiler; optimized binary)
```

In artifact shorthand this is `Alpha → Beta → Gamma → Delta → omega-bootstrap
→ omega [→ omega]`. Selection of new bootstrap-language rungs stops at Delta.
Everything to its right is a compiler artifact or a build edge; the `omega`
artifacts implement the already-specified full Omega language rather than
introducing another bootstrap dialect.

The corresponding language-capability view is simply
`Alpha → Beta → Gamma → Delta → Omega`. `omega-bootstrap` appears only in the
artifact view because it is a Delta-written compiler, not a language. The
bracketed second `omega` is the same product source rebuilt for executable
quality, not another feature surface or compiler generation.

Only two source contracts remain open:

| Surface | Kind | Required closure |
| --- | --- | --- |
| Delta v1 | independent robust compiler-host language, C-like in power and Omega-shaped where cheap | the complete Delta source of the canonical Delta compiler and `omega-bootstrap`, plus explicit coherence, robustness, safety, and maintainability arguments |
| `Ωself` | compositional subset of already-valid Omega, with no private meaning | the complete Omega source of production `omega` |

Use this working direction until complete-source measurements overturn it:

| Surface | Default | Deliberate pressure points |
| --- | --- | --- |
| Delta v1 | a coherent C-class compiler host with regular data/control, modules, deterministic bounded storage or allocation, explicit exhaustion/failure, and sealed byte/artifact/diagnostic/exit I/O | exact arithmetic, aggregate, call, arena, representation, and module inventory still follows the two complete Delta source closures plus robustness arguments |
| `Ωself` | retain ordinary compiler-building Omega facilities once real source uses them | presumptively omit proof-program mathematics and dependent/proof-indexed forms; measure advanced generics/domains, numeric schema tags, mixed record-plus-sum declarations, and aggregate transition payloads |

For ordinary `Ωself` facilities, a concrete cheaper source refactor is
required to justify exclusion. Feature-count reduction by itself is not a win.

`omega-bootstrap` is written in Delta and need only accept `Ωself`. The
production source is written in `Ωself` but must define a compiler that accepts
full Omega and contains the production optimizer and advanced lowering. A
compiler does not need to use a language feature in order to implement that
feature for its users.

Those two source choices discharge three artifact obligations:

| Artifact | Must accept | Must contain or produce |
| --- | --- | --- |
| lower-rung-published Delta compiler | Delta v1 | a correct `omega-bootstrap` executable from the exact Delta bridge closure |
| `omega-bootstrap` | frozen `Ωself` | a semantically exact, possibly conservative production-compiler executable |
| production `omega` | full Omega | the full optimizer, advanced lowering, and specified artifact behavior |

The first production binary may be slow because the bridge lowered it
conservatively; its accepted language and the compiler implementation it
contains are still complete. Only the optional bracketed edge is an Omega
self-rebuild. Detailed rationale and the feature-disposition procedure live in
[`compiler_source_profile.md`](wiki/architecture/bootstrap_lattice/compiler_source_profile.md).

Use the role names precisely in tasks and status reports. `omega-bootstrap` is
the deliberately input-incomplete bridge compiler, not an “Omega 0” generation.
The first `omega` it produces is already the full-spec production compiler; a
later rebuild changes the quality of that compiler's executable, not its source
language, implementation obligations, or generation number.

Guardrails for this queue:

- The proof kernel is cross-cutting assurance, with Beta and Gamma
  implementations; Gamma is not the proof-checker rung.
- Artifact authority is subject-qualified operational refinement, never bare
  kernel acceptance. Bootstrap gates reconstruct the source/artifact subjects,
  observation profile, semantics versions, checked bridge graph, and disclosed
  admissions; a physical-target claim remains a deployment admission.
- Compiler authority follows direct lower-rooted source-to-artifact refinement;
  cross-compiler agreement is optional bug-finding evidence. See
  [D5](wiki/architecture/bootstrap_lattice/decisions.md#d5--direct-checked-refinement-closes-compiler-provenance).
- Do not create a diverse-double-compilation lane. The complete lower-rooted
  chain checks source correspondence and semantic refinement at every compiler
  edge directly, which subsumes the relevant DDC provenance question. Another
  producer may find bugs but cannot add authority or become a release
  dependency merely by agreeing.
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

Current state, without extrapolating from bounded canaries:

| Component | What exists | What is still missing |
| --- | --- | --- |
| Delta language | executable corpus, native compiler path, self-host evidence, and a growing Delta→Gamma meaning route | a frozen v1 specification justified by both complete required Delta source closures, plus complete lower-rung coverage |
| canonical Delta compiler | a Delta-written self-hosting compiler and bounded lower-rung executions | publication of the exact final compiler artifact from its complete source through Gamma, joined to refinement |
| `omega-bootstrap` | multi-unit custody and selected vertical source→checked-IR→ELF→refinement slices through CKIR11/OMGRFN13 | the general compositional `Ωself` frontend, complete conservative backend, complete source closure, and frozen acceptance contract |
| production Omega source | checkpoint 000001 for the Psi source-to-token phase | the parser, checker, terminal-Psi path, optimizer, backend, entrypoint closure, and final `Ωself` census |
| hosted production build | bounded bridge canaries only | the first validated build of full production `omega`; no optional self-rebuild is required to close bootstrap |

This table is the stopping-point summary. A row is not promoted by a nearby
fixture or format version: only its stated whole-source and acceptance join
closes it.

Two lanes co-evolve until their join:

| Lane | Owner | Bootstrap responsibility |
| --- | --- | --- |
| production compiler source | `OMEGA-PRODUCT-COMPILER-SOURCE` in [`TASKS.md`](TASKS.md) | consume each deterministic checkpoint; derive and measure provisional `Ωself` |
| Delta compiler, bridge, and language closure | this file | close both required Delta source manifests; implement general profile rules in the bridge; maintain the Delta ledger; publish both frozen contracts at the completed source/bridge join |

The execution order is:

1. continue general checkpoint-000001 capabilities one compositional vertical
   slice at a time, consuming later product-source checkpoints as published;
2. at the completed Delta-compiler/bridge/product-source join, publish `Ωself`
   from the complete production source plus measured bridge cost and Delta v1
   from both complete required Delta closures plus its compiler-host arguments;
3. publish the Delta compiler through Gamma, then build and validate
   `omega-bootstrap` through that artifact; and
4. perform the one required hosted production build.

The two contracts published in step 2 remain separately scoped and versioned.
Neither is an upstream language rung for the other, and there is no third
bootstrap source inventory or circular build dependency.

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

Use this scope test before adding work here:

| Proposed work | Owner |
| --- | --- |
| author or refactor production Psi/Omega modules | `OMEGA-PRODUCT-COMPILER-SOURCE` in `TASKS.md` |
| implement a full-Omega user-facing feature in production `omega` | the relevant product/language task, not this queue |
| census a product checkpoint, maintain `Ωself`, or implement its general bridge acceptance | this queue |
| specify or implement a facility used by the canonical Delta compiler or bridge source | this queue and the Delta v1 ledger |
| build an interpreter, viewer, REPL, proof explorer, debugger, or similar adjacent tool | product tooling unless the hosted compiler imports it |
| optimize the first production compiler executable by rebuilding it with `omega` | optional product work after required bootstrap closure |

In particular, this queue may consume and validate product checkpoints but must
not edit product Psi/Omega source merely to make a bridge milestone convenient.
If measured bridge cost motivates a source refactor, record the evidence here
and route the implementation to `TASKS.md`; the next checkpoint then records
the result.

The optional product self-rebuild is not part of this queue. Fixed or paged
backing, typed/indexed arenas, bulk reclamation, and conservative lowering are
available bridge implementation choices when they reduce total cost. They do
not become Delta features without specified behavior, lower-rung meaning, and
explicit failure. Maintain that evidence only in
[`bootstrap/rungs/delta/FEATURE_LEDGER.md`](bootstrap/rungs/delta/FEATURE_LEDGER.md).

## Current language-design blockers

The visibility rule for private access between distinct logical modules in one
package is unspecified. Until it is ruled, the bridge rejects that case. Public
cross-package access and same-module private access remain unblocked, including
the current two-package nominal-data artifact. The selected constant-aggregate,
runtime-record, and direct-field-receiver slices are deliberately same-module
and do not depend on this ruling.

Checkpoint 000001's product lexer also conflicts with the current language
guide: Unicode XID identifiers contradict its ASCII-transparent wording,
`\u{...}` escapes contradict its explicit prohibition, raw-string semantics
are absent, and `u32` cursors are used directly where the specified `Array` and
`Slice` indexing/count interfaces require `u64`. Explicit exact `as` widening is
settled and implicit widening is forbidden; the current direct uses still need
a product-source refactor or a distinct heterogeneous conversion/comparison
ruling. Those are
product-language ruling blockers recorded under
`OMEGA-PRODUCT-COMPILER-SOURCE` in
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

The following are rolling acceptance obligations for each checkpoint, not five
independent tasks that can be permanently checked off:

- For every coherent product-source checkpoint published by
  `OMEGA-PRODUCT-COMPILER-SOURCE` in [`TASKS.md`](TASKS.md), verify its exact
  deterministic closure and derive or update the distinct compositional
  candidate feature/resource profile. Checkpoint 000001 already supplies the
  first closure and normalized-syntax/resource profile; later compiler phases
  publish later checkpoints from their product owner.
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
| multi-unit structural custody | closed for exact `OMGCOMP`; no resolver/lock or digest authority | [`OMEGA_BOOTSTRAP_COMPILATION.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_COMPILATION.md) |
| source resolution | closed through same-module direct receivers and the first pure-sum ownership relation; OMGRSW3 native/self publication, least-version behavior, canonical types, and 251/252 boundaries are gated | [`OMEGA_BOOTSTRAP_RESOLUTION.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION.md), [`OMEGA_BOOTSTRAP_RESOLUTION_V2.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V2.md), [`OMEGA_BOOTSTRAP_RESOLUTION_V3.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_RESOLUTION_V3.md) |
| checked lowering and composition | closed through CKIR11 for selected canonical `u32 in Trapping` leaf-plus-literal addition in assignment, guard, call, and transition arguments; OMGLOWC retains least OMGRSW1/2/3, inherited widening, runtime overflow, and conservative carry/range traps | [`OMEGA_BOOTSTRAP_CHECKED_IR.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR.md), [`OMEGA_BOOTSTRAP_CHECKED_IR_V11.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V11.md), [`OMEGA_BOOTSTRAP_CHECKED_IR_V11_BACKEND.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR_V11_BACKEND.md) |
| lower-rooted artifact reconstruction | closed through OMGRFN13: independent R1–R5 owners consume one immutable result-70 frame, reconstruct four authored additions plus inherited CKIR10 widening, and pin exact Add/carry/range/store bytes in the conservative ELF | [`omega-bootstrap refinement status`](bootstrap/assurance/refinement/omega-bootstrap/README.md), [`OMGCOMP_REFINEMENT_WITNESS_V13.md`](bootstrap/assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS_V13.md) |
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
| compiler data and views | fixed arrays, checked runtime indexing, borrowed shared/mutable slices, byte/string literals, and remaining general named-record/payload-sum composition | growable allocation is separate; the program-static shared-byte-view window (`&[u8]`, nonempty guard, `[0]`, `[1..]`) is unblocked, while authored `u32` indexes/cursors versus the `u64` `Array`/`Slice` contracts remain language-blocked |
| compiler control and remaining scalar operations | state parameters, mutation, calls, explicit result fields, ranges, concrete Trapping arithmetic, the remaining proof-gated narrowing/other casts, and the observed ranking clause | exact widening and canonical `u32 in Trapping` leaf-plus-literal addition are closed; only argument combinations with multiple potentially observable/trapping computations need the unresolved call-order ruling; broader receivers, recursion, and packages remain separate |
| source graph and selected product bindings | modules/import aliases over resolver-owned logical placements; target-qualified and bodyless machines; `satisfies`; sealed compiler-intrinsic realizations; the boundary trait and static provider paths actually used | private cross-module visibility and final logical placements remain owner-gated; do not import general boundary traits into Delta |
| generated closure and resource behavior | generated ordinary-Omega Unicode data, pinned generator/external inputs, rounded profile ceilings, exhaustion, and no-partial-publication behavior | generated files are ordinary source, not hard-coded bridge exceptions |

- [ ] Close the compiler-data/view lane through general parsing, resolution,
  checking, diagnostics, conservative lowering, and artifact reconstruction.
  The next bounded cost slice is the program-static shared-byte-view window
  already used by `console_write_bytes`: exact literal bytes, immutable
  `{ptr,len}` transport, a nonempty fact, head access, and one-element tail
  subslicing. It must not claim general indexing, mutable slices, allocation,
  UTF-8 meaning, or resolution of the authored `u32`/`u64` mismatch.
- [ ] Close the remaining unblocked compiler-control/scalar forms one general
  vertical relation at a time. Preserve the CKIR11 rule that a call may contain
  at most one potentially trapping argument while every sibling is
  pure/total/nontrapping; do not describe the still-unruled observable-order
  combinations as generally supported.
- [ ] Close the unblocked source-graph/provider forms without waiting on private
  cross-module visibility. Start from the exact sealed static provider and
  `Console::exit_process` path used by the hosted entrypoint; this is product
  binding support, not admission of general boundary traits to Delta.
- [ ] Close generated-source custody and resource behavior by binding ordinary
  generated Omega source, its generator and external inputs, rounded ceilings,
  exhaustion, and no-partial-publication behavior. Reuse already-closed
  constant-aggregate/Unicode artifact evidence rather than creating a second
  generated-data lowering path.
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
