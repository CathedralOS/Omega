# Bootstrap lattice — active work

Last pruned: 2026-08-24.

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

## Settled build shape

```text
Alpha → Beta → Gamma → Delta
Delta bridge source ──[lattice-built Delta compiler]──▶ omega-bootstrap
Ωself product source ──[omega-bootstrap]──────────────▶ omega (full Ω; conservative binary)
Ωself product source ──[optional omega rebuild]───────▶ omega (same compiler; optimized binary)
```

In artifact shorthand this is `Alpha → Beta → Gamma → Delta → omega-bootstrap
→ omega [→ omega]`, where the bracketed self-rebuild is optional. The bridge is
an artifact between language rungs, not a language generation of its own.

The languages become increasingly capable through Delta. Delta is the final
small-language rung: an independent, robust compiler-host language with C-like
power and Omega-shaped conventions where cheap. It is not required to be valid
Omega or a subset of Omega.

`omega-bootstrap` is a compiler artifact written in Delta. It accepts the
ordinary-Omega product-source profile `Ωself`, rejects unsupported Omega, and
compiles the production compiler conservatively. The product compiler source
is normal Omega constrained to `Ωself`; the compiler it defines implements
the complete Omega specification and contains the optimizer and advanced
lowering pipeline.
The bridge compiles those passes as source but does not duplicate or run them.

Only the optional final `omega` → `omega` edge is strict self-hosting. It may
improve the compiler executable and add reproducibility evidence, but it is not
a bootstrap dependency, language generation, or second implementation. There
is no omega0, omega1, or Epsilon rung. O0 and O1 are bounded regression canaries
only.

Exactly two source inventories remain to be discovered and frozen:

| Contract | Meaning | Freeze evidence |
| --- | --- | --- |
| Delta v1 | the literal independent language used to write `omega-bootstrap` | the complete bridge source closure plus explicit coherence, safety, robustness, and maintainability arguments |
| `Ωself` | a compositional profile of valid Omega used by the product compiler source | the complete product source closure plus measured bridge and assurance cost for every retain/refactor choice |

The bridge artifacts do not create a third feature inventory. A compiler can
implement a full-Omega feature without using that feature in its own source.
Generated-code quality is an artifact property, not a language-surface choice.

These standing rulings are not tasks:

- The proof kernel is cross-cutting assurance, with Beta and Gamma
  implementations; Gamma is not the proof-checker rung.
- Direct lower-rooted source-to-artifact refinement grants compiler authority.
  Compiler multiplicity, cross-build agreement, and byte agreement are not
  trust requirements.
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

Current status: Delta is demonstrably compiler-capable, and the O0/O1 plus
bounded scalar-call bridge canaries close through native, lower-rung meaning,
and direct artifact paths. The complete general bridge, frozen Delta v1,
frozen `Ωself`, and hosted production build remain open.

The current `lowermachine.alp` proves substantial compiler-host feasibility and
self-compilation, but it does not define Delta v1 or implement
`omega-bootstrap`. The current bridge canaries prove bounded infrastructure,
not a numbered path to `Ωself`. Further accepted-source growth must trace a
measured product-source checkpoint need.

Delta may use fixed backing, deterministic bump or paged reservation,
typed/indexed arenas, bulk reclamation, a byte-preserving source bundle, and
conservative lowering when those choices reduce total implementation and
assurance cost. It does not need the product allocator, optimizer, parallel
compiler architecture, or general host abstractions. Every retained behavior
must nevertheless be specified, deterministic, lower-rung meaningful, and
fail explicitly rather than truncate or depend on ambient authority. Maintain
the candidate inventory only in
[`bootstrap/rungs/delta/FEATURE_LEDGER.md`](bootstrap/rungs/delta/FEATURE_LEDGER.md).

Product source is an external input to this queue.
`OMEGA-PRODUCT-COMPILER-SOURCE` in [`TASKS.md`](TASKS.md) owns implementation
under `compiler/{psi,omega}/` and publishes deterministic source checkpoints.
Do not duplicate product Psi/Omega implementation tasks here.

Likewise, do not turn candidate `Ωself` exclusions into standalone bootstrap
projects. Proof syntax, linear/dependent types, generics, domains, field tags,
data shapes, and transition payloads remain rows in the one working disposition
table until checkpoint evidence and measured bridge cost settle them. This queue
implements and assures the resulting general profile; `TASKS.md` owns any
chosen product-source refactor.

The two work streams meet at explicit joins rather than forming a circular
task dependency:

1. Product work publishes a coherent Omega-written compiler-source checkpoint.
2. Bootstrap work derives a provisional compositional `Ωself` profile from
   that closure and implements its general rules in `omega-bootstrap`.
3. Measured bridge cost feeds back into retain-versus-refactor decisions for the
   next product checkpoint; it does not authorize bridge-shaped special cases.
4. The complete product closure and complete bridge freeze `Ωself`; the
   complete Delta bridge closure separately freezes Delta v1.
5. The lattice-built Delta compiler builds the validated bridge, which performs
   the one required production build.

Step 1 and any product-source refactor chosen in step 3 remain in `TASKS.md`.
Profile derivation, bridge implementation and measurement, both freezes, and
hosted validation in steps 2 through 5 are bootstrap work here. The optional
production `omega` self-rebuild remains product optimization/reproducibility
work after the required build closes.

Items 1 and 2 therefore co-evolve; they are not sequential phases. The current
concrete bootstrap edge is the remaining lower-rooted refinement item under
section 2. Sections 3 through 6 are downstream joins, not parallel language-
design projects.

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
- [ ] Measure every used feature's product-source benefit against the cost of
  implementing and assuring its general form in the Delta-written bridge.
  Absence from a partial checkpoint is provisional evidence, not a final
  exclusion.
- [ ] Give every disputed facility an explicit provisional outcome: retain a
  general compositional candidate, refactor it out and preserve a negative
  canary, or leave it unresolved with the exact missing evidence named.
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

Acceptance for each checkpoint: the product-owned exact manifest and the
separately versioned candidate-profile evidence are reproducible artifacts;
every admitted program is ordinary Omega with exact Omega meaning; unsupported
forms reject; and every unresolved row names the source and bridge evidence
needed to settle it. The final profile remains unfrozen until the complete
source and bridge join.

### 2. Implement `omega-bootstrap` in Delta

Grow the bridge from checkpoint needs through general capabilities. Do not
recognize the current compiler files, declaration counts, or syntax-tree
permutations.

The first checkpoint-driven frontend probe, selected private `CKIR1` direct
artifact route, and exact independent CKIR-to-ELF reconstruction are
established. Their completed evidence and remaining artifact obligations live in
[`SOURCE_CUSTODY_FRONTEND_PROBE.md`](bootstrap/omega-bootstrap/compiler/SOURCE_CUSTODY_FRONTEND_PROBE.md)
and
[`OMEGA_BOOTSTRAP_CHECKED_IR.md`](bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_CHECKED_IR.md).
That evidence measures the route; it does not by itself admit the source
families to `Ωself`.

- [ ] Close the corresponding first artifact tranche through the selected
  versioned private checked-IR handoff and a direct conservative backend; do
  not widen Terminal Psi for bridge-only structural operations. Keep
  representative self-built
  publication fast enough to remain a real gate; do not hide per-byte boundary
  overhead behind a longer timeout or weaken the compositional renamed/
  reordered case. This handoff choice settles only the tranche's implementation
  route, not the final `Ωself` disposition of any source feature. Any future
  Terminal-Psi vocabulary work remains product work in `TASKS.md`.
  - [ ] Land lower-rooted source→CKIR→limited-ELF refinement with perturbed
    source/result/operation/offset/branch/artifact negatives (§10.6).
    Exact envelope custody, one-unit source lexing, and independent CKIR1
    relation/result checking are established below Delta. The remaining claim
    is source semantic-table reconstruction to canonical CKIR plus exact
    limited-ELF reconstruction/execution and their composed negative teeth.
- [ ] Continue against later provisional checkpoints until the bridge generally
  parses, resolves, checks, diagnoses, and conservatively lowers every program
  admitted by the candidate `Ωself` profile while rejecting everything else
  before publication.
- [ ] Carry each admitted capability's compositional rules, negative boundary,
  resource teeth, Rust-free meaning, and direct artifact path in the same
  milestone. A bounded frontend-only cost probe is evidence, not bridge
  admission.
- [ ] Publish the complete deterministic Delta source closure of
  `omega-bootstrap`, including every transitive source and build input. Prove
  it valid under the provisional Delta ledger; final validity belongs to the
  Delta-v1 freeze.
- [ ] Compile, rather than duplicate, the product optimizer and advanced
  lowering source.

Acceptance: the bridge compiles the complete product-source manifest and every
program admitted by the general candidate profile with exact Omega semantics.
It rejects unsupported Omega before publication. It need not accept full Omega,
optimize its own output, use production allocation machinery, or host unrelated
product tools.

### 3. Freeze `Ωself` at the completed bridge join

- [ ] Reconcile the final deterministic product source closure, provisional
  compositional profile, and complete bridge implementation. For every
  disputed Omega source feature, retain it with its measured implementation and
  assurance cost discharged or refactor it out and preserve a negative canary.
  Freeze all transitive build inputs under the same rules.

Acceptance: every row is retained or excluded; every retained form has general
parsing, checking, meaning, lowering, resources, and negative-boundary coverage;
and the exact product source closes under those rules. This profile governs
what `omega-bootstrap` accepts, not which full-Omega features the resulting
compiler implements.

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
that bridge's current tokens. Later widening is an explicit versioned language
change.

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
  full compiler and language suites, and the applicable artifact-refinement
  seams.

Acceptance: the produced compiler accepts full Omega and contains the product
optimizer and advanced lowering, although its own executable may have been
generated conservatively. This closes the required lattice. A later
`omega` → `omega` rebuild is optional product optimization and reproducibility
work.

## Gate and performance discipline

- Keep one focused gate per active capability; run the full lattice gate only
  at coherent milestones.
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
