# Bootstrap lattice — status and onboarding

This document is the implementation-status index and active task list for the
bootstrap lattice. Standing architectural decisions live in
[`wiki/architecture/bootstrap_lattice/decisions.md`](wiki/architecture/bootstrap_lattice/decisions.md);
target ownership and placement live in
[`wiki/architecture/bootstrap_lattice/repository_structure.md`](wiki/architecture/bootstrap_lattice/repository_structure.md);
broader production-compiler work lives in [`TASKS.md`](TASKS.md). Exact test,
proof, and corpus counts belong beside the scripts that produce them, not in
this overview.

## Build lattice

```text
Alpha → Beta → Gamma → Delta
Delta bridge source ──[lattice-built Delta compiler]──▶ omega-bootstrap
Ωself product source ──[omega-bootstrap]──────────────▶ omega (full Ω; conservative binary)
Ωself product source ──[optional omega rebuild]───────▶ omega (same compiler; optimized binary)
```

The languages become increasingly capable. Delta is an independent, robust
compiler-host language with C-like power and Omega-shaped conventions where
useful; it is not required to be an Omega subset. Delta source builds
`omega-bootstrap`, which accepts only the exact Omega product-compiler source
profile `Ωself`. The production compiler source is normal Omega constrained to
that profile; the compiler it defines implements the complete Omega
specification. `Ωself` is not another language rung or dialect.

The bridge binary may run slowly and lower the production compiler
conservatively. It must compile the `Ωself` source that implements the product
optimizer and advanced lowering, but need not implement those passes itself. A
later product self-rebuild can optimize the compiler binary and add fixed-point
evidence; it is not required for full functionality or dependency closure.

In strict compiler terminology, the required top edge is a **hosted production
build**, not yet a self-rebuild: the Delta-written `omega-bootstrap` compiles
Omega source. Only the optional final `omega` → `omega` edge is self-hosting.

Do not conflate implementation language, accepted source, implemented language,
and executable optimization quality. `omega-bootstrap` is written in Delta and
accepts a compositional subset of Omega. The production compiler is written in
that subset, accepts full Omega, and contains the full optimizer; only its own
initial executable may be conservatively generated.

The proof kernel is orthogonal to this chain. It has independent Beta and Gamma
implementations and checks certificates emitted at multiple stages.

The shape of the hosted edge is settled. Only two source-surface contracts still
need their exact inventories frozen:

| Contract | Evidence that selects it | Working owner | Freeze point |
| --- | --- | --- | --- |
| **Delta v1** — the literal independent language used to write `omega-bootstrap` | the bridge's complete deterministic Delta source closure, plus explicit language-coherence and robustness arguments | [`bootstrap/rungs/delta/FEATURE_LEDGER.md`](bootstrap/rungs/delta/FEATURE_LEDGER.md) | after the complete bridge source exists and accidental producer/corpus behavior is pruned |
| **`Ωself`** — the incidental ordinary-Omega product-compiler source profile | the product compiler's complete deterministic Omega source closure, with retain/refactor choices settled by measured bridge cost | [`compiler_source_profile.md`](wiki/architecture/bootstrap_lattice/compiler_source_profile.md) and the bootstrap profile gate | at the general bridge join, after the source closure exists and every disputed feature is implemented or refactored away |

Delta's source manifest supplies the primary need for its independent language;
explicit compiler-host coherence arguments may retain modest companion
facilities. Omega's product-source manifest supplies the feature candidates for
`Ωself`; measured bridge and assurance cost settles retain-versus-refactor.
Neither manifest is itself a grammar, and neither contract may be inferred from
the other.

There is no third feature inventory for either compiler artifact. Facilities
used to *write* `omega-bootstrap` are Delta-v1 questions; Omega programs it may
*accept* are `Ωself` questions. Facilities implemented by the resulting product
compiler are governed by the already-authoritative full Omega specification,
not selected again during bootstrap. Whether either generated executable is
well optimized is an artifact-quality question, not a language-surface choice.

“Selected from the source closure” does not mean Delta is a whitelist of the
constructs textually exercised by one compiler revision. The closure supplies
the primary implementation pressure. Delta may retain a modest companion
facility when doing so makes the literal language safer, more regular, easier
to specify, or materially easier to use for compiler work. That justification
must be recorded explicitly; acceptance by a disposable producer or presence
in an old sample is not one.

The same total-cost rule applies to `Ωself`. The goal is not the fewest possible
Omega features. A cheap, compositional feature should remain when excluding it
would make the production compiler source substantially larger, less regular,
or harder to audit. The profile is deliberately restricted, not deliberately
crippled.

There is no `omega0`, `omega1`, or Epsilon language generation between them.
O0 and O1 below are bounded vertical-canary labels for the current bridge work,
not ancestors of `Ωself` and not compiler artifacts in the final lattice.
The open work is to discover and enforce the two inventories from real source,
not to revisit whether Delta is an Omega subset, whether the bridge accepts full
Omega, or whether a second hosted rebuild is required.

### Resolved architecture — not bootstrap tasks

- Do not add another language or compiler generation between Delta and the
  product compiler. `omega-bootstrap` is a compiler role; `Ωself` is an Omega
  source profile; O0/O1 remain regression canaries only.
- Require exactly one hosted production build: the Delta-built
  `omega-bootstrap` compiles the `Ωself` product source into the full optimizing
  product compiler. No second compile is a bootstrap task. Rebuilding that same
  compiler with itself is optional product performance/reproducibility work.
- Do not require Delta to be valid Omega or align Delta v1 with `Ωself`. Shared
  spelling is preferred where cheap, but the two source closures select two
  independent contracts.
- Do not put the proof kernel in the language spine or make Gamma “the proof
  checker.” Gamma may host one checker implementation; cross-cutting assurance
  owns the kernel and artifact-specific obligation reconstruction.
- Do not add DDC or redundant compiler implementations as trust gates. Direct
  lower-rooted source-to-artifact refinement grants authority; maintained
  reference compilers are optional bug-finding tools.
- Do not move Rust product code into unsuffixed `compiler/{psi,omega}/`. The
  current Rust implementation remains the explicitly named on-ramp; those
  product roots own the Omega-written compiler source.

Only the exact Delta-v1 inventory and the exact compositional `Ωself` inventory
remain open at this architectural layer. Their feature choices are resolved by
the source-and-assurance measurements below, not by creating another rung.

## Role map

The former flat `compiler/` inventory has been split by actual ownership.
Canonical homes are under `bootstrap/` for the seed-built lattice and under
`compiler/` for the product implementation. The flat compatibility facade has
been retired; bootstrap callers resolve canonical owners through
`bootstrap/paths.sh`.

### Language spine

| Source | Role | Canonical owner |
| --- | --- | --- |
| `bootstrap/rungs/alpha/` | 21-opcode native seed VM, written semantics, and Alpha-written Alpha assembler | `bootstrap/rungs/alpha/` |
| `bootstrap/rungs/beta/` | Beta language and self-hosting compiler | `bootstrap/rungs/beta/` |
| `bootstrap/rungs/gamma/` | Gamma language, interpreter, and type checker | `bootstrap/rungs/gamma/` |
| `bootstrap/rungs/delta/` | Delta language corpus, Delta-written compiler, and lattice-built artifacts | `bootstrap/rungs/delta/` |

### Assurance and the bootstrap bridge

| Source | Role | Canonical owner |
| --- | --- | --- |
| `bootstrap/assurance/proof-kernel/{implementations,tools,corpus,gates}/` | cross-cutting derivation checking, tools, corpora, and gates | `bootstrap/assurance/proof-kernel/` |
| `bootstrap/assurance/refinement/{beta,omega-bootstrap}/` (compatibility entries remain under Alpha and bridge gates) | cross-rung source/meaning-to-artifact obligation reconstruction and checking | `bootstrap/assurance/refinement/` |
| `bootstrap/omega-bootstrap/` | Rust-free meaning, current bridge-compiler slices/contracts, and gates | `bootstrap/omega-bootstrap/` |
| `bootstrap/corpus/` | fixtures shared across lattice seams | `bootstrap/corpus/` |

### Reference producers and product implementations

| Source | Role | Canonical owner |
| --- | --- | --- |
| `bootstrap/onramps/delta-rust/` | Delta disposable/reference Rust producer | `bootstrap/onramps/delta-rust/` |
| `bootstrap/onramps/alpha-assembler-rust/` | disposable/reference Rust producer of Alpha VM tapes from Alpha assembly | `bootstrap/onramps/alpha-assembler-rust/` |
| `bootstrap/onramps/beta-rust/` | Beta-language disposable/reference Rust producer | `bootstrap/onramps/beta-rust/` |
| `bootstrap/rungs/beta/reference/` | executable Beta reference meaning and semantic fuzzing | `bootstrap/rungs/beta/reference/` |
| `bootstrap/assurance/refinement/beta/` | source/artifact reconstruction plus whole-artifact obligation checkers | `bootstrap/assurance/refinement/beta/` |
| `bootstrap/onramps/omega-rust/` | current working Rust Psi/Omega compiler and CLI; maintained parallel comparator and differential producer, never bootstrap authority | `bootstrap/onramps/omega-rust/` |
| `compiler/{psi,omega}/` | Omega-written product compiler source | first Psi lexical checkpoint landed; remaining product phases open |
| `compiler/source-checkpoints/` | exact product source closures and provisional `Ωself` censuses | product checkpoint evidence |
| `apps/omega-compiler/` | hosted product compiler entrypoint | product application owner |

### Completed compatibility retirement

- [x] **Make `compiler/` product-only.** Bootstrap consumers now use canonical
  `bootstrap/` owners. The flat rung/on-ramp/assurance entries, forwarding
  facade, and compatibility roles are retired; path-hygiene tests reject their
  reintroduction.

  Acceptance: `compiler/` contains only product-source ownership and its
  documentation; bootstrap gates import canonical rung, on-ramp, assurance, and
  corpus paths. This is a placement cleanup, not a new trust argument or a
  request for redundant compiler implementations.

## Current architectural state

- Alpha has written small-step semantics, conformance tests, and two independent
  native seeds.
- Beta's `bc.beta` self-hosts. Its fixed point establishes dependency closure;
  the persisted artifact is now reconstructed entirely through Alpha and used by
  downstream gates. The independently reconstructed ROOT proposition now proves
  its complete maximal observable against `bc.beta` for `B_bc1`, closing the
  lower-rooted source-correspondence edge.
- Gamma's canonical surface is the functional interpreter-first language. It is
  capable of hosting one proof-kernel implementation, but proof checking is not
  Gamma's role in the build chain. The imperative `gamma.alpha` language is
  parked compatibility material.
- The proof kernel is implemented independently in Beta and Gamma. It checks
  generic derivations; terminal-Psi obligation reconstruction is a separate
  artifact-aware responsibility.
- Delta is the final small bootstrap language. The admitted D0/O1 compiler
  profile elaborates to Gamma through the lower-rung route; every future profile
  extension must preserve that coverage or fail closed.
- The next compiler dependency closure is Delta → `omega-bootstrap` → production
  Omega. The bridge needs exact `Ωself` coverage and correct conservative output,
  not general full-Omega input acceptance or the product optimizer itself.

> **Immediate next dependency:** use checkpoint 000001's mechanically enforced
> provisional normalized-syntax/resource profile to select the first general
> Delta-written bridge capabilities, while extending later checkpoints with
> typed-semantic, ABI/layout, and lowering evidence. Continue expanding the
> Omega-written product source and publishing deterministic closures in
> parallel. Every bridge capability must carry direct-artifact and Rust-free-
> meaning coverage in the same milestone.

## Delta → omega-bootstrap → production Omega readiness

**Present status: compiler-capable with O0, the variable O1 vertical slice, and
the bounded scalar-call conformance tranche closed, but not
`Ωself`-bootstrap-ready.** Delta has proved that it can host a substantial
compiler and carry bounded families of Omega source shapes through canonical
meaning to runnable artifacts, but it has not yet implemented the complete
bridge compiler.
`bootstrap/rungs/delta/samples/lowermachine.alp` is a real
Delta-written Delta-to-ARM64 compiler: it self-compiles to a fixed point and its
output is swept against the Rust reference over the sample corpus. This proves
that Delta can host substantial compiler work. The mutable arenas, recursive
calls, sums, arithmetic policies, boundary declarations, and other facilities
used by that experiment remain candidates for the final compiler-host
vocabulary, not automatically admitted Delta-v1 features.

That evidence is necessary but is not yet `omega-bootstrap`:

- The Delta-written bridge substrate implements its frozen O0/O1 console lane
  plus a table-driven scalar-call lane: one program unit, up to 16 arbitrarily
  named machines, four signed-`i32` parameters/arguments, 16 operations per
  machine, literals, parameter references, results, and acyclic calls. It emits
  canonical terminal Psi and a direct x86-64 ELF, rejecting malformed graphs,
  identifiers, types, and adjacent resource overflows before publication. This
  is not yet a general Omega frontend or a complete Delta-written Omega backend.
- Delta's general self-host path emits ARM64 assembly and still uses external
  `clang` and `codesign`. The exact O0/O1 terminal-to-ELF edge no longer does.
- `lowermachine.alp` is a large, effectively single-source compiler. Its source
  and table capacities are still bounded by explicit backing extents, but input
  and compiler tables now use checked logical byte/typed arenas rather than one
  dedicated array per table.
- The Rust `gamma_emit.rs` Delta-to-Gamma route is incomplete for the whole
  implemented language and remains a trusted Rust dependency while the
  Rust-free route is widened.
- Delta's complete literal specification has not yet been frozen as the robust,
  independent compiler-host language required by the complete
  `omega-bootstrap` Delta source closure.
- The exact `Ωself` profile sufficient to express the production Omega compiler
  and every transitive build dependency has not been frozen.

### Delta implementation latitude

Delta is an independent, robust C-like compiler host, not a minimal recognizer
for one pinned source tree and not an Omega subset. It may use plain fixed
backing, deterministic bump or paged allocation, typed/indexed arenas, bulk
reclamation, a byte-preserving source bundle, and conservative lowering when
those choices reduce total implementation and assurance cost. It does not need
Omega's production allocator/container architecture, optimization pipeline, or
parallel compiler design. Every retained allocation, exhaustion, arithmetic,
input, and host-I/O behavior must nevertheless have specified meaning and live
validation; silent truncation and ambient host authority remain forbidden.

This paragraph grants design latitude, not language features. The canonical
candidate inventory and freeze test live in
[`bootstrap/rungs/delta/FEATURE_LEDGER.md`](bootstrap/rungs/delta/FEATURE_LEDGER.md);
the cross-surface decision procedure lives in
[`compiler_source_profile.md`](wiki/architecture/bootstrap_lattice/compiler_source_profile.md).
Do not maintain a second Delta feature list in this task index.

Product Psi/Omega implementation work belongs in `TASKS.md`. This file may name
a required product interface as an input to a lattice gate, but it must not own
or prescribe work inside the Rust on-ramp or the product compiler.

### Two-contract discovery loop

Two source efforts can proceed in parallel:

- establish the Omega-written production compiler and publish versioned,
  deterministic transitive source-closure snapshots under
  `OMEGA-PRODUCT-COMPILER-SOURCE` in `TASKS.md`; and
- grow only the bounded profile-neutral Delta-written compiler substrate while
  maintaining a provisional, explicit Delta feature ledger and lower-rung
  meaning for every used construct.

Checkpoint 000001 supplies the first coherent product manifest and a
mechanically enforced provisional normalized-syntax/resource profile; later
snapshots update and deepen it. `Ωself` determines the accepted-source work of
`omega-bootstrap`; implementing that bridge exposes its complete Delta source
closure, from which Delta v1 is pruned and frozen. This is an iterative
discovery loop with two eventual freezes, not a circular runtime or build
dependency. The pre-snapshot exception for explicitly bounded profile-neutral
substrate is now closed by checkpoint 000001; further accepted-source growth
must trace a measured checkpoint need. Neither canary succession, current
source, nor producer acceptance may silently define a language contract.

### Active work packages and acceptance gates

The remaining bootstrap dependencies are:

```text
production compiler source ──▶ Ωself ──┐
                                         ▼
provisional Delta ledger ◀──▶ omega-bootstrap source ──▶ Delta v1 freeze

Delta v1 + omega-bootstrap + Ωself ──▶ hosted production build
```

Delta v1 defines the language used to write the bridge. `Ωself` defines the
ordinary-Omega programs the bridge accepts. Their source manifests provide the
evidence used to select and prove closure under the contracts; they do not
replace general specifications with file identities or AST shapes. A Delta
feature decision belongs to Delta v1; an Omega feature used or rejected by the
production compiler source belongs to `Ωself`. Do not solve one contract by
silently widening the other.

The work below has three stages, not three additional contracts:

1. expose the product and bridge source closures while maintaining provisional
   Delta and `Ωself` records;
2. implement the general bridge and use its measured cost to settle both
   inventories; and
3. freeze, validate, and perform the one required hosted production build.

O0/O1 remain regression inputs throughout these stages. They do not define a
numbered route to `Ωself`.

#### Stage 1 — expose both source closures

**Rolling invariant — maintain Delta's provisional compiler-host feature
ledger.** This is not a separate completion gate or a prerequisite freeze. The
ledger co-evolves with bridge source until the Delta-v1 freeze below:

- name the concrete bridge need, simpler rejected alternative, and meaning/gate
  coverage for every provisionally used construct in
  [`bootstrap/rungs/delta/FEATURE_LEDGER.md`](bootstrap/rungs/delta/FEATURE_LEDGER.md);
- treat D0, the sample corpus, and Rust-producer acceptance as discovery evidence
  only, never as admission to Delta v1;
- compare Exact arithmetic, fixed arrays, explicit tags, and a sealed
  compiler-host interface against broader arithmetic policies, allocators, sum
  machinery, or boundary traits by total source-and-assurance cost;
- keep every provisionally used arithmetic, layout, call, allocation, trap, and
  I/O edge aligned across native targets, the Delta self-host, and the Rust-free
  Delta-to-Gamma meaning route; and
- use the ruled deterministic, byte-preserving source-unit/bundle contract, not
  newline-concatenated source input.

At every bridge milestone, each used construct must have a concrete need, a
recorded simpler alternative, explicit semantics, and live lower-rung/gate
coverage. No provisional construct is presented as frozen Delta v1.

**External input, not a bootstrap task:**
`OMEGA-PRODUCT-COMPILER-SOURCE` in `TASKS.md` owns the Omega-written product
source. It may proceed in parallel with Delta/bridge discovery and should
publish a versioned deterministic transitive closure at each coherent source
checkpoint. The bootstrap lane consumes those snapshots to derive provisional
profiles; it requires the exact final manifest only for the freeze and hosted
build. Product Psi/Omega implementation work must not be duplicated here.
Compiler-adjacent tools are outside that manifest unless the compiler executable
imports them. Under the current product architecture, terminal-Psi
representation and lowering modules used by the compiler belong to the
manifest. That does not pull in a standalone Terminal-Psi interpreter,
verifier, viewer, or debugger, and it does not require `omega-bootstrap` to use
Terminal Psi as its own internal compiler IR.

The first external snapshot is now published at
`compiler/source-checkpoints/checkpoint-000001.json`, with its distinct canonical
provisional admission artifact in `profile-000001.json` and explanatory record
in `profile-000001.md`. It covers the product Psi source-to-token phase only.
Bootstrap may consume its mechanically admitted normalized syntax and resources
now; it must not extrapolate parser, checker, terminal-Psi, optimizer, or emitter
needs from this partial closure or treat unresolved typed/ABI/lowering rows as
already proved.

- [x] **Complete bundle-wide source-unit ingestion in the canonical Delta
  frontend.** This is pre-profile bridge infrastructure, not O2, `Ωself`
  admission, or a package/namespace decision. Apply the already-ruled canonical
  bundle contract to the real frontend rather than only the decoder canary:
  - [x] decode every unit before publication, preserve each source ID, label,
    exact byte span, and label-local offset, and validate UTF-8 per unit without
    concatenation, injected separators, or cross-unit token fusion;
  - [x] retain the present bounded storage model with checked descriptor, label,
    and content exhaustion and deterministic status 252;
  - [x] as the bounded end-to-end canary, accept exactly one O1 program-bearing
    unit plus empty/trivia-only ordinary Omega units, while two nontrivial units
    remain explicit unsupported status 251; and
  - [x] carry identical accepted/rejected observations through the native
    frontend, Rust-free meaning route, and direct terminal-to-ELF composite,
    including invalid auxiliary UTF-8 and every resource boundary. Publish no
    terminal bytes before the whole bundle validates.

  Acceptance: an auxiliary-trivia bundle produces byte-identical terminal and
  ELF output to its single-source equivalent; multiple program units,
  cross-file token fragments, malformed UTF-8, and exhausted backing fail with
  their exact status and no artifact publication. No new Omega grammar, module
  semantics, Delta feature, or `Ωself` row is selected by this task.

#### Stage 2 — converge the product-source profile and the bridge

The profile-derivation and profile-neutral-substrate work packages in this
stage are deliberately not one serial queue. Profile-neutral compiler substrate
may advance before the product source manifest exists, while provisional
`Ωself` derivation cannot. The boundary is strict:

- before the manifest, implement only reusable, independently specified
  compiler capabilities with their own positive, negative, resource, meaning,
  and artifact gates;
- after the manifest, select accepted Omega capabilities from the actual
  product-source closure and measured bridge cost; and
- do not turn canary succession, producer coverage, or speculative usefulness
  into an `Ωself` feature decision.

- [ ] **Derive, deepen, and enforce provisional `Ωself` from the product source.**
  **Required input:** the first coherent Omega-written production-compiler
  checkpoint and its deterministic transitive manifest. The final product
  closure need not already be frozen; rerun this task as later versioned
  snapshots land. Standard-library samples and current Rust source cannot
  substitute for Omega-written product source. The first-snapshot dependency is
  now satisfied by checkpoint 000001; accepted-source growth beyond the closed
  profile-neutral substrate must trace that or a later snapshot. This task
  produces the general candidate contract used to implement the bridge. It does
  not freeze the profile: measured bridge and assurance costs still have to
  settle every retain-versus-refactor choice.
  - [x] publish checkpoint 000001 as a deterministic 12-source closure with
    separate generated/toolchain inputs and a compositional feature census;
    this satisfies the first external-input dependency without pretending the
    final compiler closure or profile is frozen. The standard checkpoint gate
    now replays all four declared target resolutions, requires exact loaded
    source/alias/import-edge equality, binds all metadata into the closure hash,
    and rejects omitted, padded, rewired, duplicate, bogus-root, and external-
    checksum mutations;
  - [x] make checkpoint census generation compiler-owned and structurally
    exhaustive for its used source forms. Source-closure snapshot v3 retains
    target qualification, bodylessness, `satisfies`/`via`, conformance bounds,
    ranking arguments/ranges, data `where` facts, cast domain/form, case
    construction/projection, local mutability, qualification flags, and
    reference lifetimes; the versioned feature catalog enumerates zero-count
    alternatives, operator spellings, call and transition flags, parameter
    flags, and bounded resources. All four target resolutions currently agree;
  - [x] publish and enforce a separately hashed provisional normalized-syntax
    and resource profile for checkpoint 000001. `profile-000001.json` binds the
    manifest closure/content digests, complete catalog partition, every-target
    census, rounded ceilings, unresolved evidence, and hashed canaries. The
    gate proves each negative fixture remains valid checked full Omega before
    profile admission rejects it, admits a compositional positive fixture, and
    carries schema/feature/resource/canary plus exact-limit/adjacent-over-limit
    mutation teeth. Typed semantic distinctions, ABI/layout, lowering, Delta
    capacity behavior, and bridge-cost settlement are not claimed by this
    tranche;
  - [ ] measure every feature used by each complete checkpoint closure against
    its production-source benefit and the cost of implementing and assuring it
    in the Delta-written bridge. Absence from a partial checkpoint is provisional
    evidence only, not a final exclusion from the completed source profile;
  - [ ] for every disputed facility exercised by a checkpoint, record an explicit
    provisional outcome: retain a general compositional candidate, refactor it
    out of the product source and keep a negative canary, or leave it unresolved
    with the exact bridge-cost evidence still missing. Do not reward feature
    removal when it merely creates monomorphic duplication or source-shape
    permutations;
  - [ ] publish candidate compositional syntax, static-semantics, resource,
    ABI/layout, and lowering rules—not file identities, statement counts, or
    AST permutations—and enforce them provisionally. Normalized syntax and
    resource admission are now mechanically enforced; typed semantic, ABI/
    layout, and lowering rules still require corresponding compiler-owned
    census facts and bridge evidence;
  - [ ] update every applicable row in the canonical working feature-disposition
    table in [`compiler_source_profile.md`](wiki/architecture/bootstrap_lattice/compiler_source_profile.md).
    Preserve explicit unresolved rows where later product source or general
    bridge-cost evidence is still required. Do not copy that inventory into this
    task list: final resolution belongs to the freeze join below;
  - [ ] keep standalone terminal-Psi tools, interpreters, REPLs, proof explorers,
    viewers, and debuggers outside the manifest unless the compiler executable
    imports them; and
  - [ ] gate the complete manifest plus one negative canary per excluded
    language capability. Separately run the full-Omega product suites so a
    feature omitted from compiler *source* cannot be confused with a feature
    omitted from the compiler it implements.

  Acceptance for each checkpoint: publish the versioned deterministic source
  manifest and a mechanically enforced, compositional candidate
  feature/resource profile as distinct artifacts. Every currently retained
  program is ordinary Omega with exact Omega meaning; unsupported Omega rejects.
  Each unresolved candidate names the product-source benefit and bridge-cost
  evidence still needed at the freeze join. A partial checkpoint may enforce a
  narrower provisional profile without pretending that features absent from that
  checkpoint are finally excluded. Every candidate profile is a true subset,
  not a dialect, a source-file whitelist, or another lattice rung.

- [x] **Advance profile-neutral bridge substrate in Delta.** **Bounded
  pre-snapshot tranche complete.** Reuse the closed O0/O1 path as
  vertical-canary evidence only. This work was permitted before the product
  manifest because each tranche is a general compiler capability with an
  independent specification and stop condition. O0/O1 are not implementation
  stages that must be extended in numerical order, and this lane must not grow
  an open-ended approximation of Omega from guessed product needs.
  - [x] establish profile-neutral, source-unit-bounded nested block-comment
    scanning in the real frontend. Program and auxiliary comments use the same
    reusable scanner; nesting is exact, delimiters cannot cross units, and an
    unterminated comment rejects before terminal or image publication. Native,
    lower-rung meaning, and lowermachine-built terminal-to-ELF observations
    agree. This is ordinary lexical substrate, not `Ωself` admission or a
    module/package decision;
  - [x] publish the differential-only vocabulary-28 scalar-`Call` reference
    needed by the next general tranche. The product-owned fixture is exact and
    deterministic, carries one signed-`i32` argument/result through a two-machine
    call, verifies and interprets with fixed fuel, lowers through the Linux
    x86-64 internal-relocation path, and rejects arity, callee, argument-ID, and
    result-type mutations. It is comparison evidence, never bootstrap authority;
  - [x] implement a bounded, table-driven scalar in-module call/return
    conformance slice in the Delta frontend and backend. Admit multiple
    arbitrarily named machines in one program unit, signed-`i32` parameters and
    results, literals, parameter references, and forward acyclic calls through
    general symbol/signature/value/call tables. Declaration/name permutations
    must use the same implementation; duplicate/unknown names, arity/type/result
    mismatches, cycles, malformed terminal IDs, and every table/code ceiling
    reject before publication. Carry exact native, lowermachine-built,
    lower-rung meaning, terminal-validation, and runnable-artifact observations.
    Recursion, modules, records, generics, domains, proofs, and general control
    flow remain outside this conformance slice; the slice does not admit a row
    to `Ωself`. Its unique zero-parameter indegree-zero root and process-status
    shim are bounded conformance conventions, not Omega's authored
    target-qualified `target::ProgramEntry`. Native, lowermachine-built,
    lower-rung meaning, product validation, terminal mutation, boundary, and
    runnable-artifact gates carry the tranche;

  Acceptance: each named tranche is table-driven rather than fixture-shaped,
  rejects every unsupported or exhausted form before publication, and agrees
  across native, lower-rung meaning, and direct artifact observations. After
  the scalar call/return tranche, further accepted-source growth waits for a
  concrete provisional `Ωself` requirement; maintenance and assurance work on
  the landed substrate may continue. Checkpoint 000001 now supplies that
  concrete provisional requirement; subsequent accepted-source growth belongs
  to the next task and must trace the checkpoint profile.

- [ ] **Implement `omega-bootstrap` in Delta against provisional `Ωself`.**
  **Required input:** a versioned deterministic product-source manifest and its
  mechanically enforced provisional profile above. Grow and revise the bridge
  by general capabilities required by those snapshots rather than by hard-coded
  source permutations. The exact final manifest is required at the freeze join,
  not before the first implementation tranche.
  - [ ] publish the complete deterministic Delta source manifest and prove each
    transitive unit valid under the provisional profile; final validity under
    frozen Delta v1 belongs to the subsequent freeze task, and one entry source
    file alone is not the closure;
  - [ ] accept exactly `Ωself` with general parsing, checking, diagnostics,
    and the conservative lowering path selected for the profile. The bridge may
    lower directly; compiling product modules that implement or manipulate
    Terminal Psi does not require a Terminal-Psi interpreter in the bridge;
  - [ ] reject unsupported Omega before artifact publication;
  - [ ] carry every admitted capability through the Rust-free meaning route and
    direct artifact path in the same milestone; and
  - [ ] compile, rather than duplicate, the production optimizer and advanced
    lowering source.

  This task feeds measured implementation and assurance cost back into the
  provisional profile decision. A capability is complete only when its
  general profile rule, unsupported-form rejection, Rust-free meaning, and
  artifact path land together; recognizing just the current product source
  shape does not count.

  Acceptance: the bridge compiles every admitted `Ωself` program with exact
  Omega semantics and compiles the complete product-source manifest. It need
  not accept full Omega, optimize its own output, use the production allocator
  architecture, or host unrelated product tools.

#### Stage 3 — freeze both inventories and close the hosted edge

- [ ] **Freeze `Ωself` at the completed bridge join.**
  Reconcile the exact production-source closure, the provisional compositional
  profile, and the complete general bridge implementation. For every disputed
  Omega source feature, either retain it with its implementation and assurance
  cost discharged or refactor it out of the production source and keep an
  explicit negative canary. Freeze the manifest and profile together only after
  all transitive build inputs satisfy the same general rules.

  Acceptance: every profile row is resolved as retained or excluded; every
  retained form has general parsing, checking, meaning, lowering, resource, and
  negative-boundary coverage; the exact production source closes under those
  rules; and no rule recognizes a particular file, declaration count, or AST
  permutation. This freeze governs what `omega-bootstrap` accepts. It does not
  select which full-Omega features the resulting product compiler implements.

- [ ] **Freeze Delta v1 from the complete `omega-bootstrap` source closure.**
  This follows implementation of the complete bridge source, although normative
  documents and conformance gates should be maintained while it grows.
  - [ ] classify every retained construct as required by the deterministic
    bridge manifest or justified by an explicit language-coherence,
    robustness, safety, or maintainability argument; remove accidental D0,
    corpus, and Rust-producer behavior;
  - [ ] publish versioned normative grammar, static/dynamic semantics,
    representation/ABI, source-bundle, and retained resource/host-interface
    documents under `bootstrap/rungs/delta/`;
  - [ ] reject every excluded source, type, module, boundary, and resource form
    explicitly;
  - [ ] publish a classified conformance corpus and feature manifest with
    positive observations, one phase-isolated negative per exclusion,
    exhaustion teeth for every retained resource mechanism, cross-target
    layout/arithmetic edges, and native/self-host/lower-rung conformance and
    meaning differentials (bug-finding evidence, not DDC authority); and
  - [ ] prove the complete deterministic `omega-bootstrap` source closure valid
    under the frozen contract.

  Acceptance: the literal Delta v1 contract is self-consistent, mechanically
  enforced, robust enough for compiler implementation, and sufficient for the
  complete bridge closure. It is not source-file-shaped and contains no feature
  justified only by the current producer or corpus. Later widening is an
  explicit versioned language change, never silent bridge pressure.

- [ ] **Validate Delta → `omega-bootstrap`.**
  Build the exact bridge artifact from its published Delta source closure using
  the canonical lattice path. Join that source manifest, the produced artifact,
  and the lower-rooted source-to-artifact refinement in one gate; a Rust-built
  bridge or native/self-host agreement cannot substitute for that join. Gate
  exact `Ωself` coverage, excluded-feature diagnostics, deterministic
  publication, canonical-meaning agreement, conservative lowering, and the
  relevant proof/translation-validation seams. Include profile-wide
  compositional canaries so compiling the exact product source cannot conceal
  source-shape specialization. The Rust compiler may remain a differential
  reference but is never authority or a release dependency.

  Acceptance: the exact lattice-built `omega-bootstrap` executable is bound to
  its exact Delta source and canonical meaning, requires no Rust producer or
  ambient assembler/linker in the required path, and correctly accepts and
  rejects the published compositional `Ωself` profile.

- [ ] **Perform the sole required hosted production build.**
  Run the Delta-built bridge on the exact `Ωself` manifest and validate the
  result against canonical meaning plus the full compiler/language suites. The
  required artifact accepts full Omega and contains the production optimizer
  and advanced lowering, although its own binary may have been conservatively
  generated. This closes the required build lattice. This cross-language edge
  is not itself an Omega self-rebuild. A later production `omega` → `omega`
  rebuild is optional product optimization and reproducibility work, not a
  bootstrap task, rung, or dependency.

## Cross-rung assurance status

The Beta `bc` source-correspondence edge is closed. One independently
reconstructed, lower-rooted proposition now proves the complete maximal
observable of the exact persisted artifact for every finite source stream and
supported `B_bc1` resource profile. It joins exact source/artifact custody,
procedure and control summaries, memory safety, typed resource provenance,
guarded divergence, and phase-isolated mutation teeth. Fixed-point identity and
cross-compiler agreement remain regression evidence rather than authority.

The detailed theorem decomposition, checker ownership, resource partitions,
and current gate entry points belong in
[`bootstrap/assurance/refinement/beta/README.md`](bootstrap/assurance/refinement/beta/README.md)
beside the implementation. No open Beta-refinement dependency remains in this
task list. Reopening that edge requires a concrete defect, a widened `B_bc1`
claim, or a changed persisted artifact—not routine Delta or Omega bootstrap
growth.

## Gate and performance discipline

- Keep one focused gate per active proof/compiler capability and run the full
  historical/lattice gate at coherent checkpoints.
- A gate approaching tens of minutes must publish subgate timings before further
  feature growth. Profile the compiler, evaluator, and harness separately.
- Keep individual compiler invocations single-threaded until profiling justifies
  an architectural change; parallelizing independent fixture compiles in the
  harness is the first concurrency option.
- Debug HTML, viewers, exhaustive dumps, and similar human-only artifacts must
  be opt-in. Default CI and agent paths produce only evidence consumed by a
  checker or concise failure diagnostics.
- Paged arenas, parallel lowering, advanced optimization, and incremental
  compilation may improve the product compiler, but none is a prerequisite for
  Delta or `omega-bootstrap`.

## Execution order

1. In parallel, keep Delta's Rust-free meaning route and provisional feature
   ledger live while `OMEGA-PRODUCT-COMPILER-SOURCE` expands the Omega-written
   product source and publishes later deterministic closure snapshots in
   `TASKS.md`. The first snapshot and the bounded pre-snapshot substrate are
   complete; accepted-source growth now requires a measured checkpoint need.
2. Derive and mechanically enforce provisional `Ωself` from each coherent
   product-source snapshot, then implement the remaining `omega-bootstrap`
   capabilities directly against its compositional rules. Use O0/O1 only as
   regression canaries; do not manufacture an O2/O3 ladder or continue
   speculative accepted-source growth after the bounded substrate stop. Feed
   measured bridge and assurance cost back into each retained/excluded profile
   decision.
3. At the completed bridge join, freeze `Ωself` from the exact Omega product
   closure and general accepted-source implementation. Separately freeze a
   coherent Delta v1 from the bridge's complete Delta source closure after
   removing accidental producer and corpus surface. These are the only two
   feature inventories being settled.
4. Build the exact bridge through the canonical lattice path, join source,
   meaning, artifact, and negative-profile evidence, then use it once to build
   and validate the full optimizing Omega compiler. Any later product
   self-rebuild is optional performance/reproducibility work.
5. Throughout those steps, grow proof-kernel seams and translation-validation
   evidence only for real obligation classes introduced by the compiler edges.

This ordering follows D1–D6. Producer optimization does not outrank removal of a
trusted Rust meaning or verification dependency.

## Principal gates

Run from the repository root:

```sh
sh bootstrap/verify-lattice.sh
sh bootstrap/rungs/alpha/assembler/selfhost.sh
sh bootstrap/onramps/alpha-assembler-rust/test.sh
sh bootstrap/onramps/beta-rust/test.sh  # diagnostic producer only
sh bootstrap/rungs/beta/cold-start/test.sh
sh bootstrap/rungs/beta/cold-start/full-source.sh
sh bootstrap/rungs/beta/source-exhaustion.sh
sh bootstrap/assurance/refinement/beta/bc-artifact-structure.sh
sh bootstrap/assurance/refinement/beta/bc-block-control.sh
sh bootstrap/rungs/beta/selfhost.sh
sh bootstrap/rungs/beta/test.sh
sh bootstrap/rungs/gamma/test-interp.sh
sh bootstrap/rungs/gamma/test-typeck.sh
sh bootstrap/assurance/proof-kernel/gates/gamma-checker.sh
sh bootstrap/assurance/proof-kernel/gates/test.sh
```

The current Delta-written ARM64 path additionally uses these platform-specific
gates (and requires the external assembler/linker/signing tools noted above):

```sh
sh bootstrap/onramps/delta-rust/test_aarch64.sh
sh bootstrap/onramps/delta-rust/selfhost-sweep.sh
sh bootstrap/onramps/delta-rust/delta-meaning-diamond.sh
sh bootstrap/onramps/delta-rust/convergence-selfhost.sh
```

The current Rust-free Omega kernel/meaning experiments are gated separately:

```sh
sh bootstrap/omega-bootstrap/gates/kernel-diamond.sh
sh bootstrap/omega-bootstrap/gates/omega-meaning.sh
sh bootstrap/assurance/refinement/omega-bootstrap/meaning-cert-diamond.sh
sh bootstrap/assurance/refinement/omega-bootstrap/translation-validation.sh
sh bootstrap/omega-bootstrap/gates/delta-terminal-to-elf.sh
sh bootstrap/omega-bootstrap/gates/delta-terminal-to-elf-meaning.sh
sh bootstrap/onramps/delta-rust/omega-bootstrap-frontend-meaning.sh
```

## Persistent implementation facts

- Both committed Alpha seeds have a 256 KiB tape hole. Build scripts source the
  canonical `bootstrap/rungs/alpha/seed_env.sh` owner through
  `bootstrap/paths.sh`, so future platform realizations may declare their audited
  capacity without embedding a universal assumption elsewhere.
- The Alpha VM hidden stack stores return addresses. Beta maintains a separate
  explicit data stack with `r15` as stack pointer and `r14` as frame pointer.
- Build fixed points establish determinism and dependency closure, not compiler
  correctness.
- Rust reference producers may remain during migration. Rust in meaning,
  artifact reconstruction, or proof checking remains an explicit trusted
  dependency until replaced by the audited route.
- Exact corpus counts and “N cases passed” snapshots are intentionally omitted
  here because they drift; the gate output is authoritative.
