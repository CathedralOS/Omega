# Bootstrap lattice — active work

Last pruned: 2026-08-27.

This is the live bootstrap execution queue, not an architecture essay or a
history of completed gates. Standing decisions live in
[`decisions.md`](wiki/architecture/bootstrap_lattice/decisions.md), the two
remaining source-surface contracts live in
[`compiler_source_profile.md`](wiki/architecture/bootstrap_lattice/compiler_source_profile.md),
repository ownership lives in
[`repository_structure.md`](wiki/architecture/bootstrap_lattice/repository_structure.md),
and production-compiler implementation lives in [`TASKS.md`](TASKS.md).

Before taking an item, fetch `main`, inspect the newest work in that lane, and
avoid overlapping another active change. Commit and push coherent milestones.
Engineering difficulty, incomplete code, and slow gates are not language-design
blockers.

## Fixed model

```text
language capability: Alpha → Beta → Gamma → Delta → Omega

build artifacts:     Alpha → Beta → Gamma → Delta → omega-bootstrap
                     → omega (full Ω; conservatively generated binary)
                     [→ omega (same source and compiler; optimized binary)]
```

There are two remaining source-surface decisions, not a hidden sequence of
Omega0/Omega1/Epsilon languages:

| Contract | What it decides | Standing bias |
| --- | --- | --- |
| Delta v1 | the independent language used by the canonical Delta compiler and `omega-bootstrap` | robust deterministic C-class compiler power, explicit failure, deterministic fixed/bump/paged storage or allocation, and Omega-like spelling only where cheap |
| `Ωself` | the ordinary-Omega forms used by the production compiler's own source | omit proof/dependent authoring forms by default; retain regular compiler facilities unless a concrete source refactor lowers total bridge and assurance cost |

The artifacts have different obligations:

| Artifact | Written in | Must accept |
| --- | --- | --- |
| canonical Delta compiler | Delta v1 | all Delta v1 |
| `omega-bootstrap` | Delta v1 | the compositional `Ωself` profile, with exact ordinary-Omega meaning |
| production `omega` | ordinary Omega constrained to `Ωself` | full Omega |

The first bridge-built `omega` already contains the full parser, checker,
optimizer, advanced lowering, and backend required by the product specification.
Its own binary may be slow because `omega-bootstrap` lowers conservatively. The
optional final rebuild recompiles the same source to improve that executable; it
does not add language functionality or close a missing bootstrap dependency.

Standing invariants:

- Delta is independent of Omega. Shared spelling does not make it a subset.
- `Ωself` is a true compositional subset of ordinary Omega: no private syntax,
  changed meaning, file allowlist, or compiler-AST permutation matching.
- Full-Omega conformance and the `Ωself` authoring census are separate facts. A
  product compiler can implement proofs, dependent types, or any other feature
  without using those features in its own source.
- Only transitive dependencies of the compiler executable enter the hosted
  closure. Standalone interpreters, REPLs, proof explorers, viewers, debuggers,
  and similar tools are excluded unless the compiler imports them.
- Gamma supplies Delta's canonical execution route. Its independent proof-kernel
  implementation is a cross-cutting assurance service, not a language stage.
- Every compiler edge gains authority from direct lower-rooted reconstruction
  of exact source, canonical meaning, and exact artifact. Compiler agreement,
  fixed points, and implementation diversity are regression evidence only; DDC
  has no bootstrap or release role.
- `source/{psi,omega}/` are the permanent Omega-written product owners.
  `source/on-ramp/rust/` contains only the temporary Psi/Omega product
  implementation and development CLI. There are no external Alpha, Beta, or
  Delta producers.
- The bridge may use a private checked IR and conservative backend. It need not
  use Terminal Psi internally merely because it compiles product modules that
  implement Terminal Psi.

## Readiness

| Component | Present | Remaining closure |
| --- | --- | --- |
| Delta | executable corpus, Delta-written self-hosting compiler experiment, checked-in provisional artifacts, and a growing Delta→Gamma meaning route | a canonical lower-rooted compiler publication, complete required-source coverage, and frozen v1 semantics/resources |
| `omega-bootstrap` | multi-unit custody plus bounded compositional source→checked-IR→artifact→refinement slices | general `Ωself` frontend, complete conservative backend, complete source closure, and frozen acceptance/resources |
| production Omega source | Psi source-to-token processing | parser, checker, compiler-linked Terminal-Psi path, optimizer, backend, entrypoint closure, and final `Ωself` census |
| hosted build | bounded bridge canaries | one validated `omega-bootstrap` build of full production `omega` |

Bounded canaries establish implementation cost only. They do not define Delta
v1, admit a feature to final `Ωself`, or grant package/compilation authority.

## Current language-design blockers

Only unresolved Omega meaning can block an otherwise selected bridge slice:

| Open ruling | Fail-closed boundary | Work still available |
| --- | --- | --- |
| private access between distinct logical modules in one package | reject that access | public cross-package and same-module private access |
| Unicode XID identifiers, `\u{...}` escapes, and raw-string spelling | do not claim full lexical conformance | settled lexical forms |
| evaluation order among effectful or trapping named-record fields | admit only combinations whose relative order is unobservable | pure, nontrapping fields |
| call-argument evaluation order | admit at most one observably effectful or trapping argument | calls with pure/nontrapping siblings |
| explicit sum discriminants versus first-case/tag-zero initialization | exclude explicit discriminants from the bounded sum slice | declaration-order payload sums under compiler-controlled layout |

The earlier `u32` collection-index mismatch is closed: product source uses
`u64` for collection coordinates and counts. Missing implementation, provider
artifacts, performance, and assurance work are engineering or external-contract
dependencies, not language rulings.

## External contract dependency

Compilation authority waits on the package/security owner to publish the
bounded accepted-lock/source-closure projection and its acceptance root.
Compiler-issued `PackageAdmissionProjection` rows remain review-only. Once the
canonical projection exists, the bridge must independently reconstruct it and
join the accepted closure to exact `OMGCOMP` bytes and their expected SHA-256.

This dependency blocks final authority, not source resolution, checking,
lowering, resource work, artifacts, refinement, or source-closure machinery.
Track the owner work under `RECHECKABLE-PACKAGE-EVIDENCE` and
`ACCEPTED-LOCK-SCHEMA` in [`TASKS_PACKAGE_MANAGER.md`](TASKS_PACKAGE_MANAGER.md).

## Execution queue

The order is:

1. grow general bridge capabilities against the live resolved product source;
2. settle Delta v1 and `Ωself` at the completed required-source join;
3. publish the exact Delta compiler through Gamma and use it to build the
   bridge;
4. perform the one required hosted production build.

### 0. `LOWER-ROOTED-DELTA-PUBLICATION` — replace the retired external producer

The Rust Alpha assembler, Beta compiler, and Delta compiler producers were
retired on 2026-08-26. Alpha and Beta already have canonical lower-rooted
construction paths, so they need no replacement producer. Delta does not yet
have a complete published host artifact; that missing edge is now visible
instead of being disguised by a Rust fallback.

- [ ] Execute the exact canonical Delta compiler source through the
  Beta-written Delta→Gamma elaborator and Gamma interpreter, producing the
  compiler artifact for every required build host.
- [ ] Join exact source identity, Delta semantics/resources, target identity,
  emitted artifact identity, and direct lower-rooted refinement. Checked-in
  provisional binaries are inputs to reconstruction, never authorities.
- [ ] Restore the useful compiler-facing gates formerly driven by the external
  producer: Delta compilation, native execution, self-reproduction,
  source-to-artifact comparison, backend checks, contract discharge, and
  certifier checks. They must consume the canonical published artifact through
  one role, not build an implementation as a side effect.
- [x] Rewire or remove every suspended gate and cache profile that still names
  the retired Rust Delta location or role. Retain a gate only when it specifies
  a property that the canonical artifact must satisfy.
- [ ] Restore the downstream Delta/refinement portion of
  `tools/bootstrap/verify-lattice.sh` only after those gates are lower-rooted.

Source-only lower-rung evidence restored on 2026-08-27: the default lattice now
runs complete marker-free elaboration of the canonical compiler source, bounded
exact compiler-fixture execution through Gamma, state/tree/source/argument
resource teeth, the proof-carrying certifier loop, and the path-independent
source-closure snapshot. The obsolete native differential was removed rather
than redirected to a hidden producer.

The measured compiler-on-self transport blockers were then removed generally:
canonical invocations use an ordinary depth-17 `Chunks`/`Node` carrier with four
exact input bytes per immediate `u32`, and Gamma now reclaims evaluation values
with a stable-address, representation-aware collector while pinning parsed
syntax. The transport is interpreter-checked at Delta's exact 524,288-byte
ceiling, the adjacent byte fails closed, more than 40 MiB of persistent-update
garbage is reclaimed with aliases and temporary roots intact, and irreducibly
live growth still exits `254` without output. A pre-publication helper also
reconstructs the exact 168,560-byte canonical LF image and validates the bounded
Darwin-arm64 assembly dialect without assigning authority to an unobserved
artifact. The next step is the exact full-source execution and repeatable
assembly observation; this remains engineering work, not a Delta or Omega
language-design blocker.

Retired-producer cleanup completed on 2026-08-27: 141 producer-dependent gate
and refinement wrappers/composites and 33 mixed cache profiles were removed.
Independent source-only meaning gates, deterministic reference carriers,
persisted-Beta responsibilities, and 15 source/reference cache profiles remain.
Path hygiene now rejects both the retired producer role and path throughout
active bridge, refinement, cache, and bootstrap-tool custody. The deleted
producer gates return only after the canonical artifact exists, by consuming
that artifact through its single role rather than rebuilding it as a side
effect.

Acceptance: the closed lattice constructs Delta without Cargo or an external
Delta implementation; repository path checks contain no live Alpha/Beta/Delta
Rust producer role; and the default lattice reports exactly the closure it has
actually verified.

### 1. Numbered product-source checkpoints are abandoned

The `source/omega/source-checkpoints/` subsystem was removed on 2026-08-26.
It duplicated source identity already owned by Git, package resolution, and the
accepted source closure, while encouraging the bootstrap to target frozen
partial snapshots instead of testing the actual compiler chain. Do not
reintroduce authored checkpoint manifests, numbered source freezes, duplicated
hash ledgers, or checkpoint-specific feature profiles.

Bootstrap work consumes the live resolver-derived product source closure. The
default proof is the end-to-end bootstrap run. If the chain cannot yet build
that closure, record the missing compiler capability here and implement it
generally. Feature and resource censuses, when useful, are generated reports
from the selected closure rather than committed identity authorities.

### 2. Complete `omega-bootstrap` in Delta

Current milestone-level evidence:

| Responsibility | Current boundary | Detail owner |
| --- | --- | --- |
| source custody | generic bounded `OMGCOMP1`, target/provider/build-role and generated-source custody, raw-envelope SHA-256, and path-independent V1 snapshots for the canonical Delta compiler source plus a provisional three-root bridge action DAG | [`source/on-ramp/omega-bootstrap/compiler/`](source/on-ramp/omega-bootstrap/compiler/) and [`DELTA_SOURCE_CLOSURE_SNAPSHOT_V1.md`](source/delta/DELTA_SOURCE_CLOSURE_SNAPSHOT_V1.md) |
| source resolution | compositional slices for selected data, calls, views, arithmetic, and static provider planning; OMGRSWC12 now closes the actual `TokenStream::push` shape over eight records, five pure copyable sum families, two actual-capacity record arrays, semantic structural copies, fifteen data writes, a ten-argument call, and indexed payload dispatch | versioned `OMGRSW*` contracts beside the bridge |
| checked lowering/artifacts | OMGLOWL21/CKIR20 close that TokenStream projection through exact modular meaning and a conservative Linux-x86-64 artifact; the separate CKIR17 line retains platform-neutral provider-adapter observations | versioned `OMGLOW*`/`CKIR*` contracts beside the bridge |
| lower-rooted reconstruction | OMGRFN23 joins exact TokenStream source, resolution, checked IR, corrected artifact, native/self producer variants, resources, and independent responsibility-local R1–R5 reconstruction | [`source/refinement/delta-omega-bootstrap/`](source/refinement/delta-omega-bootstrap/) |
| compilation authority | externally gated on the accepted-lock/source-closure projection | [external dependency](#external-contract-dependency) |

Open work:

- [ ] Consume the live resolved product closure and implement its newly used,
  directionally clear facilities as complete vertical slices: compositional
  rules, negative boundary, resource teeth, lower-rung meaning, conservative
  artifact path, and direct reconstruction.
- [ ] After the general bridge has one selected entry and complete build DAG,
  publish separate complete deterministic Delta source manifests for the
  canonical Delta compiler and `omega-bootstrap`. Prove both against the same
  provisional Delta ledger; do not treat historical `.alp` canaries as the
  bridge closure.
- [ ] When the package/security projection lands, reconstruct it and join the
  accepted closure, exact compilation envelope, and expected digest without
  trusting compiler review rows or stored verdicts.
- [ ] When one exact `Console::write_byte` provider occurrence has admitted
  installation and an evaluated target calling-plan identity, carry the
  selected static plan through native effects, final bytes/certificate/entry,
  and direct artifact reconstruction. Until then, fail closed; the existing
  static plan and abstract byte events do not manufacture provider authority.

Bridge acceptance: the complete product manifest closes under general `Ωself`
rules; admitted programs preserve exact Omega meaning and unsupported forms
reject before publication. The bridge need not accept full Omega, optimize its
own output, implement the product optimizer, use production allocation, or host
adjacent product tools. It compiles the ordinary `Ωself` modules that implement
those product capabilities.

### 3. Prepare, then freeze Delta v1 and `Ωself` at one evidence join

These publications are independently scoped and versioned, but neither can be
frozen from a partial source closure or assumed costs in the other.

Directionally clear Delta work does not have to wait for that final join:

- [ ] Publish a machine-readable **provisional** feature census for the exact
  canonical Delta-compiler and `omega-bootstrap` snapshots. For every observed
  construct, record the source requirement or explicit coherence/robustness/
  safety/maintainability justification, current semantic evidence, nearest
  rejected form, and unresolved final disposition. Do not call this census the
  Delta v1 manifest or infer a language contract from producer acceptance.
- [ ] Make the current producer reject accidental surface area before the
  freeze: ignored or malformed type declarations, unknown arithmetic domains,
  skipped boundary declarations, and any source form for which parsing does
  not lead to a complete checked meaning. Give every rejection a focused
  phase-local negative gate.
- [ ] Reconcile x86-64 and AArch64 arithmetic/domain behavior against one
  explicit provisional table. Where the producers differ, fail closed until
  the shared rule is implemented; do not let either backend silently define
  Delta.
- [ ] Generalize source-closure snapshot V1 into a path-independent manifest
  capable of carrying both complete required Delta programs and the exact
  native, self-built, and lower-rung artifact observations. Keep the
  publication provisional until both closures and the ledger are complete.

These tasks harden evidence and remove accidental behavior. They do not select
the final Delta grammar, allocation model, ABI, or feature inventory early.

`Ωself`:

- [ ] Reconcile the complete product closure, provisional disposition ledger,
  and complete bridge. Retain each used general form with its measured cost
  discharged, or land a concrete source refactor and preserve the negative
  bridge canary.
- [ ] Resolve the high-leverage groups explicitly: proof mathematics and
  dependent/proof-indexed types are presumptive exclusions; ordinary ownership,
  records, sums, modules, arrays/views, calls, basic generics, and concrete
  domains are presumptive retentions when used; numeric/schema tags, mixed
  record-plus-sum declarations, domain polymorphism, advanced generic
  constraints, and aggregate transition payloads are measured candidates.
- [ ] Prove each excluded facility remains implemented by production `omega`
  through representative full-Omega conformance tests.

Delta v1:

- [ ] Publish a coherent deterministic C-class compiler-host specification:
  grammar, static/dynamic semantics, ABI/layout, source bundles, resources,
  explicit failure, allocation/exhaustion, and sealed host I/O.
- [ ] Justify every retained facility from either required Delta closure or an
  explicit coherence, robustness, safety, or maintainability argument; remove
  accidental D0/corpus/Rust-producer behavior.
- [ ] Publish classified conformance and rejection corpora with resource teeth,
  native/self/lower-rung differentials, and cross-target layout/arithmetic
  edges. Prove both complete required source closures valid under the same
  contract.

Joint acceptance: `Ωself` governs only the production compiler's ordinary-Omega
source; Delta v1 governs only the two Delta programs; full Omega continues to
govern the resulting product compiler. No source manifest substitutes for a
general language/profile contract.

### 4. Publish Delta through Gamma and build the bridge

- [ ] Execute the exact frozen Delta-written compiler through the canonical
  Beta-written Delta→Gamma elaborator and Gamma's Beta-written interpreter on
  its exact source. Join source, elaboration, Gamma execution, artifact,
  canonical Delta meaning, resources, and direct lower-rooted refinement.
- [ ] Use that exact lower-rung-published compiler to build `omega-bootstrap`.
  Join the bridge's complete Delta closure, artifact, canonical meaning,
  resources, profile-wide positives/negatives, and direct reconstruction.

Acceptance: neither artifact requires Rust, an ambient assembler/linker, or
compiler agreement in the required path. Rust-built and self-built artifacts
may remain differential controls.

### 5. Perform the sole required hosted production build

- [ ] Run the validated Delta-built bridge on the exact frozen `Ωself` manifest.
  Validate the result against canonical meaning, the full-Omega conformance
  manifest, product compiler/language suites, and applicable artifact-refinement
  seams. Exercise facilities omitted from `Ωself` and reach the optimizer and
  advanced lowering in executable tests.

Acceptance: the first bridge-built compiler accepts full Omega and contains the
production optimizer and advanced lowering, although its own executable may be
conservatively generated. This closes the required lattice. A later
`omega → omega` rebuild is optional product optimization and reproducibility work.

## Gate and performance discipline

- Keep one focused gate per capability and run the full lattice only at coherent
  milestones.
- Split transport, resolution, checked-IR validation, artifact reconstruction,
  and orchestration into responsibility-local modules. Compose versioned
  artifacts with cross-pair tests; do not grow an all-version Cartesian
  verifier.
- Split an R1–R5 owner before it becomes the dominant compile/evaluation cost.
  Keep shared fixtures in small libraries and responsibility-specific positives,
  negatives, resources, and targets in separate files.
- A gate approaching tens of minutes must report compiler, evaluator, and
  harness timings before feature growth. Keep human-only HTML, viewers, dumps,
  and debug artifacts opt-in.
- Profile the native compiler phase before arena or compiler-concurrency
  redesign. Focused native/self bridge gates should
  remain in the seconds range; lower-rung reference execution must report its
  separate stage timings and remain precisely cached.
- Do not put OMGRSW9's monolithic Gamma replay in the default lattice. Its
  681,067-byte elaboration took 209.34 seconds and produced an invalid
  observation element. Preserve focused native/self and modular lower-rooted
  gates until phase splitting or profiling makes replay bounded and useful.
- Keep each compilation single-threaded until profiling justifies internal
  concurrency. Parallelize independent fixture compiles first. Paged arenas,
  parallel lowering, optimization, and incremental compilation remain permitted
  engineering work, not Delta or bridge prerequisites.
- Run exhaustive cheap owner-local checks plus a representative expensive
  native/self/lower-rung join by default. Keep historical Cartesian audits
  opt-in and maintain precise cache dependencies.
