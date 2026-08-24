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
                           ↓
              omega-bootstrap (accepts Ωself)
                           ↓
              omega (full optimizing compiler; own binary may be conservative)
                           │
                           └── optional self-rebuild ──▶ omega (same compiler; optimized binary)
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
  product roots are reserved for the eventual Omega-written compiler source.

Only the exact Delta-v1 inventory and the exact compositional `Ωself` inventory
remain open at this architectural layer. Their feature choices are resolved by
the source-and-assurance measurements below, not by creating another rung.

## Role map

The former flat `compiler/` inventory has been split by actual ownership.
Canonical homes are under `bootstrap/` for the seed-built lattice and under
`compiler/` for the product implementation; selected old paths remain only as
compatibility symlinks.

### Language spine

| Canonical or compatibility source | Role | Canonical owner |
| --- | --- | --- |
| `bootstrap/rungs/alpha/` (compatibility: `compiler/alpha`, `compiler/beta`) | 21-opcode native seed VM, written semantics, and Alpha-written Alpha assembler | `bootstrap/rungs/alpha/` |
| `bootstrap/rungs/beta/` (compatibility: `compiler/beta-lang`) | Beta language and self-hosting compiler | `bootstrap/rungs/beta/` |
| `bootstrap/rungs/gamma/` (compatibility: `compiler/gamma`) | Gamma language, interpreter, and type checker | `bootstrap/rungs/gamma/` |
| `bootstrap/rungs/delta/` (compatibility: `compiler/delta`, Delta samples through `compiler/delta-rs`) | Delta language corpus, Delta-written compiler, and lattice-built artifacts | `bootstrap/rungs/delta/` |

### Assurance and the bootstrap bridge

| Canonical or transitional source | Role | Canonical owner |
| --- | --- | --- |
| `bootstrap/assurance/proof-kernel/{implementations,tools,corpus,gates}/` (compatibility: `compiler/proof-kernel`) | cross-cutting derivation checking, tools, corpora, and gates | `bootstrap/assurance/proof-kernel/` |
| `bootstrap/assurance/refinement/{beta,omega-bootstrap}/` (compatibility entries remain under Alpha and bridge gates) | cross-rung source/meaning-to-artifact obligation reconstruction and checking | `bootstrap/assurance/refinement/` |
| `bootstrap/omega-bootstrap/` | Rust-free meaning, current bridge-compiler slices/contracts, and gates | `bootstrap/omega-bootstrap/` |
| `bootstrap/corpus/` (compatibility: `compiler/lattice-corpus`) | fixtures shared across lattice seams | `bootstrap/corpus/` |

### Reference producers and future product implementations

| Canonical or transitional source | Role | Canonical owner |
| --- | --- | --- |
| `bootstrap/onramps/delta-rust/` (compatibility: `compiler/delta-rs`) | Delta disposable/reference Rust producer | `bootstrap/onramps/delta-rust/` |
| `bootstrap/onramps/alpha-assembler-rust/` (compatibility: `compiler/beta-rs`) | disposable/reference Rust producer of Alpha VM tapes from Alpha assembly | `bootstrap/onramps/alpha-assembler-rust/` |
| `bootstrap/onramps/beta-rust/` (compatibility: `compiler/beta-lang-rs`) | Beta-language disposable/reference Rust producer | `bootstrap/onramps/beta-rust/` |
| `bootstrap/rungs/beta/reference/` | executable Beta reference meaning and semantic fuzzing | `bootstrap/rungs/beta/reference/`; `compiler/beta-lang-py` forwards compatibility entry points |
| `bootstrap/assurance/refinement/beta/` | fragmentary symbolic reconstruction plus whole-artifact obligation checkers | `bootstrap/assurance/refinement/beta/` |
| `bootstrap/onramps/omega-rust/` | current working Rust Psi/Omega compiler and CLI; maintained parallel comparator and differential producer, never bootstrap authority | `bootstrap/onramps/omega-rust/` |
| `compiler/{psi,omega}/` | eventual Omega-written product compiler source | reserved product roots; implementation remains open |

### Compatibility retirement

- [ ] **Make `compiler/` product-only.** Migrate remaining bootstrap consumers
  to canonical `bootstrap/` owners, then remove the flat rung/on-ramp/assurance
  compatibility entries, including the `compiler/beta-lang-py/` forwarding
  facade and `compiler/proof-kernel` symlink. Remove their compatibility roles
  from `bootstrap/paths.sh` and invert the path-hygiene tests so they reject
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

> **Immediate next dependency:** establish the Omega-written production compiler
> source closure while the Delta bridge and its Rust-free meaning route grow.
> Profile growth follows that actual source tree and must carry direct-artifact
> and Rust-free-meaning coverage in the same milestone.

## Delta → omega-bootstrap → production Omega readiness

**Present status: compiler-capable with O0 and the variable O1 vertical slice
closed, but not `Ωself`-bootstrap-ready.** Delta has proved that it can host a
substantial compiler and carry a bounded family of Omega source shapes through
canonical meaning to runnable artifacts, but it has not yet implemented the
complete bridge compiler.
`bootstrap/rungs/delta/samples/lowermachine.alp` is a real
Delta-written Delta-to-ARM64 compiler: it self-compiles to a fixed point and its
output is swept against the Rust reference over the sample corpus. This proves
that Delta can host substantial compiler work. The mutable arenas, recursive
calls, sums, arithmetic policies, boundary declarations, and other facilities
used by that experiment remain candidates for the final compiler-host
vocabulary, not automatically admitted Delta-v1 features.

That evidence is necessary but is not yet `omega-bootstrap`:

- The Delta-written O0/O1 slice implements its frozen lexer, parser, exact
  name/type/count checks, direct canonical terminal-Psi emission, and a direct
  x86-64 ELF backend for 0–16 literal `write_line` operations followed by one
  literal `exit_process`. It is not yet a general Omega frontend or a complete
  Delta-written Omega backend.
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

### Permitted Delta bootstrap candidates

Delta is an independent bootstrap implementation language, not the product
language. It should be a robust C-like compiler-host language, not the smallest
grammar capable of recognizing one pinned source tree. It may therefore use
deliberately plain facilities when they reduce total implementation and
assurance cost, and it need not reproduce Omega's final allocation and
container model. Candidate concessions include:

- Permit runtime-sized allocations from an explicit, fixed backing extent
  supplied at compiler startup. A deterministic bump allocator or paged arena
  is sufficient; general-purpose `free`, compaction, and garbage collection are
  not prerequisites.
- Permit multiple typed/indexed arenas over that allocator. Arena handles, not
  ambient host pointers, remain the normal durable identity.
- Permit bulk reclamation at the end of a compilation. Exhaustion must have a
  specified result—checked failure or a defined trap—and must never silently
  truncate input or tables.
- Permit one deterministic length-delimited source bundle before a native
  package/module implementation. Ordered text concatenation is not a source-unit
  contract; labels and exact bytes must remain auditable.
- Permit direct, conservative lowering and poor generated code. Parallelism,
  advanced register allocation, optimization, incremental compilation, and the
  production `PagedArena` architecture are explicitly not gates for
  `omega-bootstrap`.

These are available design moves, not a minimum feature list or holes in
meaning. If simpler bridge source needs fewer facilities—for example only Exact
integer arithmetic or ordinary fixed arrays—Delta v1 need not inherit the
broader producer surface. Conversely, direct textual use is not the only valid
reason to retain a small companion feature: regularity, safe composition,
debuggability, and avoiding brittle source contortions count in the total-cost
decision. Allocation, exhaustion, input assembly, and every retained host
operation must still have explicit semantics and appear in the
trust/validation story.

Product Psi/Omega implementation work belongs in `TASKS.md`. This file may name
a required product interface as an input to a lattice gate, but it must not own
or prescribe work inside the Rust on-ramp or the eventual product compiler.

### Two-contract discovery loop

Two source efforts can proceed in parallel:

- establish the Omega-written production compiler and publish its exact
  transitive source manifest under `OMEGA-PRODUCT-COMPILER-SOURCE` in
  `TASKS.md`; and
- grow the Delta-written bridge from O0/O1 while maintaining a provisional,
  explicit Delta feature ledger and lower-rung meaning for every used
  construct.

The product manifest permits `Ωself` to be derived. `Ωself` determines the
accepted-source work of `omega-bootstrap`; implementing that bridge exposes its
complete Delta source closure, from which Delta v1 is pruned and frozen. This is
an iterative discovery loop with two eventual freezes, not a circular runtime
or build dependency. O0/O1 may continue before those joins, but neither current
source nor producer acceptance may silently define a language contract.

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
source. It may proceed in parallel with Delta/bridge discovery. The bootstrap
lane consumes its deterministic transitive manifest; product Psi/Omega
implementation work must not be duplicated here. Compiler-adjacent tools are
outside that manifest unless the compiler executable imports them. Under the
current product architecture, terminal-Psi representation and lowering modules
used by the compiler belong to the manifest. That does not pull in a standalone
Terminal-Psi interpreter, verifier, viewer, or debugger, and it does not require
`omega-bootstrap` to use Terminal Psi as its own internal compiler IR.

#### Stage 2 — derive the source profile and implement the bridge

- [ ] **Derive and enforce provisional `Ωself` from the product source.**
  Dependency: the exact production-compiler source manifest above must exist
  before the profile can be derived. Standard library samples and current Rust
  source cannot substitute for it; provisional bridge work may continue before
  that manifest exists. This task produces the general candidate contract used
  to implement the bridge. It does not freeze the profile: measured bridge and
  assurance costs still have to settle every retain-versus-refactor choice.
  - [ ] measure every feature used by the complete source closure against its
    production-source benefit and the cost of implementing and assuring it in
    the Delta-written bridge;
  - [ ] for every disputed facility, record one explicit outcome: retain a
    general compositional form, or refactor it out of the product source and
    keep a negative canary. Do not reward feature removal when it merely creates
    monomorphic duplication or source-shape permutations;
  - [ ] publish candidate compositional syntax, static-semantics, resource,
    ABI/layout, and lowering rules—not file identities, statement counts, or
    AST permutations—and enforce them provisionally;
  - [ ] resolve every row in the working feature-disposition table in
    [`compiler_source_profile.md`](wiki/architecture/bootstrap_lattice/compiler_source_profile.md):
    proof and linear/dependent features are presumptively excluded; ordinary
    named fields, payload sums, and basic generics are presumptively retained;
    domains, advanced generics, numeric/schema field tags, complex transition
    payloads, and mixed field-plus-case data remain measurement questions;
  - [ ] keep standalone terminal-Psi tools, interpreters, REPLs, proof explorers,
    viewers, and debuggers outside the manifest unless the compiler executable
    imports them; and
  - [ ] gate the complete manifest plus one negative canary per excluded
    language capability. Separately run the full-Omega product suites so a
    feature omitted from compiler *source* cannot be confused with a feature
    omitted from the compiler it implements.

  Acceptance: publish the deterministic source manifest and a mechanically
  enforced, compositional candidate feature/resource profile as distinct
  artifacts. Every currently retained program is ordinary Omega with exact
  Omega meaning; unsupported Omega rejects. Each unresolved candidate names the
  product-source benefit and bridge-cost evidence still needed at the freeze
  join. The candidate profile is a true subset, not a dialect, a source-file
  whitelist, or another lattice rung.

- [ ] **Implement `omega-bootstrap` in Delta.**
  Reuse the closed O0/O1 test path as vertical-canary evidence only. O0/O1 are
  not implementation stages that must be extended in numerical order. Grow the
  bridge by general capabilities required by the product source rather than by
  hard-coded source permutations.
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

  This task grows against the working `Ωself` policy and feeds measured cost
  back into the profile decision. A capability is complete only when its
  general profile rule, unsupported-form rejection, Rust-free meaning, and
  artifact path land together; recognizing just the current product source
  shape does not count.

  Acceptance: the bridge compiles every admitted `Ωself` program with exact
  Omega semantics. It need not accept full Omega, optimize its own output, use
  the production allocator architecture, or host unrelated product tools.

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

## Cross-rung assurance work

- [x] **Close the `bc` source-correspondence edge by checked refinement.**
  Validate the exact persisted Alpha tape against `bc.beta` with authority
  rooted below `bc`. Fixed-point identity and corpus agreement establish
  dependency closure and regression evidence, not this theorem.

  Established boundary:
  - the Alpha-built `bc.tape`, exact source/tape fingerprints, complete source
    observable, structural decode, procedure/control/effect ownership, frame and
    local layout, raw-memory classification, stack/call potentials, and source
    storage bounds are checked;
  - the proof carries exact quiet/cursor and trace summaries through
    `main.ready`, loop entry, `parse_proc`'s prefix, and the current
    `gen_stmts`/expression cutpoints, and now composes the conditional
    `parse_proc.genbody` tail over every exact entry block depth;
  - fixed emitters, bounded decimal emission, parse-number, parse-character,
    operator classifiers, `cmp_op`, the nine fixed keyword recognizers, and
    conditional `name_eq`/`lookup` have exact shape/meaning/negative modules;
    lookup retains distinct hit-slot-zero versus no-match provenance despite
    their deliberate numeric-zero alias;
  - the bounded `WSTR`/decimal emitter family has exact conditional contracts
    for `gen_read_byte`, `emit_pop_into`, `emit_push`, `emit_mnemonic`,
    `emit_combine`, `emit_slot_addr`, `emit_load_slot`, and `emit_store_slot`,
    with value-parameterized child custody and no dependence on a full-word
    decimal claim;
  - an independent full-word `emit_dec` theorem now covers the complete signed
    Word partition. Nonnegative words append canonical decimal; negative words
    take the source's one-byte base edge and append `48 + srem(n,10)`, with no
    invented minus sign or unsigned-format claim;
  - `new_label`, `emit_lref`, `emit_str_body`, `gen_emit`, and `emit_cmp` now
    compose through an independent lower-rooted checker. It preserves modular
    label wrap, the source's signed-negative label spelling, blind string-body
    opening, escaped-NUL continuation, exact `[92,0]` malformed-tail behavior,
    the 48-byte/four-label `gen_emit` trace, both final-expect cursor classes,
    invalid-comparison no-op output, signed high-bit materialization, and exact
    `set,done,set,done` label order;
  - the ROOT checker composes source slurp, the reusable main loop,
    complete `parse_proc`, all root-reachable resource joins, guarded
    divergence, and maximal trace equality. Its independent memory-safety
    closure rescans all 95 raw-memory rows and joins the five SRC-indexed and
    two table-indexed loads to exact guards and nonwrapping extents; and
  - every claimed source/artifact join remains lower-rooted and mutation-toothed.

  Remaining proof plan:
  - [x] establish the bounded conditional name-table/query-slice domain and
    exact terminating `name_eq` relation, including length short circuit,
    first mismatch, and full byte equality;
  - [x] compose conditional `lookup` over that carried domain and `name_eq`,
    preserving the source's deliberate no-match alias to slot zero while
    retaining distinct proof provenance;
  - [x] close the bounded `WSTR`/bounded-decimal emitters:
    `gen_read_byte`, `emit_pop_into`, `emit_push`, `emit_mnemonic`,
    `emit_combine`, `emit_slot_addr`, `emit_load_slot`, and
    `emit_store_slot`;
  - [x] add the separate full-word `emit_dec` theorem. Preserve the source's
    signed comparison/division behavior: negative words take the one-byte base
    case; no unimplemented minus-sign or unsigned-format claim is allowed;
  - [x] compose `new_label`, `emit_lref`, `emit_str_body`, `gen_emit`,
    and `emit_cmp`, retaining exact trace order and malformed-tail cursor
    bounds;
  - [x] close `gen_load`, `gen_write_byte`, `gen_call`, `gen_factor`,
    `gen_term`, `gen_sum`, and `gen_expr` together under the checked
    `EXPRDEPTH<=64` induction, including every resource exit;
  - [x] build `gen_stmt`'s branch relation and the guarded greatest fixed point
    for `gen_stmts`/`gen_block`/`gen_state`/`gen_stmt`, preserving finite
    or infinite stdout prefixes without assuming output productivity:
    - [x] bind exact p26/p46/p62..p67 source/artifact shape in an independent
      82,588-byte checker, with every module below 20 KB, an assembler diamond,
      and twelve phase-isolated teeth;
    - [x] establish the finite helper/dispatch relation, including every
      post-resource suffix and name-table provenance branch; and
    - [x] close the block-depth-stratified guarded greatest fixed point, using a
      completed child/backedge machine step—not cursor or stdout progress—as
      the coinductive guard. The independent 80,138-byte conditional semantic
      checker is conjoined with its six prerequisite owners over the identical
      canonical bundle and has twenty-two phase-isolated teeth;
  - [x] compose the `parse_proc`/PFXS body cutpoint through the unconditional
    epilogue and return, including finite child returns after numeric resource
    status 252:
    - [x] discharge the statement theorem's fifteen antecedents across six
      owners at the same-bundle gate boundary rather than importing SPUB as its
      conclusion;
    - [x] quantify over entry `BLOCKDEPTH D=0..64`, with 64 Ret/status0,
      65 Ret/numeric252, and 64 Div rows; `D=64` admits only its immediate
      depth-exhaustion Ret/numeric252 base; and
    - [x] preserve exact `P || child || 49-byte-epilogue` finite order and
      `P || maximal-child` divergence, including status/provenance, restored or
      live frames/depth, and no cursor/output productivity premise. The
      independent 63,560-byte checker has an assembler diamond, four modules
      below 20 KB, and twenty-five phase-isolated teeth;
  - [x] classify each checked resource outcome from its exact proved guard,
    resource profile, and requested amount. Status 252/253 is only a process
    projection and is never used to recover `ResourceKind`:
    - [x] map seven exact checked origins to five `B_bc1` kinds while retaining
      distinct actual/formal arity and declaration/preflight provenance;
    - [x] retain exact literal requests and the symbolic preflight
      `nslots=nparams+count_lets()` request with its proved nonwrapping
      `[1025,1048580]` range, rather than clamping it to the lower bound; and
    - [x] conjoin Checker A and expression-family ownership over one immutable
      bundle in an independent 65,069-byte checker, with five modules below
      20 KB, an assembler diamond, a scanned origin/kind/projection census, and
      thirty-six phase-isolated teeth; and
  - [x] close the root loop and prove equality of maximal stdout plus
    `Halt`/`Trap`/`Exhaust`/`Diverge` for every finite source stream and
    supported resource profile in `BOOTSTRAP_OBSERVABLE.md`:
    - [x] bind the exact `main.body` call and `parse_proc` entry prefix through
      `NLOC` reset, whitespace/identifier helpers, saved procedure-name fields,
      permissive `expect('(')`, and entry to the already-published parameter
      loop, carrying root trace, frame, depth-zero, and resource provenance;
    - [x] instantiate the existing parameter, capacity, output-prefix, and
      parse-body relations at root block depth zero, partitioning ordinary
      return, each root-reachable checked resource origin, and child divergence
      without recovering a resource kind from numeric status 252; prove any
      conditionally published but root-unreachable origin impossible. Publish
      this entry-to-return/divergence composition as its own independently
      checked conditional `parse_proc` relation rather than folding it into the
      root checker;
    - [x] bind the exact post-return status split: ordinary return traverses
      `skip_ws` and republishes every reusable loop invariant, while resource
      return preserves first-failure provenance through `main.resource` and its
      deterministic output suffix;
    - [x] generalize the reusable loop split across the honest post-parse cursor
      bound `0 <= CUR <= LEN+2`: in-range NUL and every `LEN <= CUR` miss halt,
      while only an in-range nonzero byte enters the body. Do not silently reuse
      the narrower initial-loop theorem or normalize malformed-tail overshoot;
    - [x] close the guarded greatest fixed point over any number of completed
      parse/backedge iterations without assuming cursor or stdout productivity,
      then join the source-oversize wrapper; and
    - [x] prove the final partition excludes invalid-opcode/arithmetic traps and
      undefined stack or memory states, and accept the root publication only
      after the control, parse-body, resource-classification, and completed
      `parse_proc` owners have checked the identical canonical bundle. The root
      checker must consume those relations as hypotheses and must not import a
      process-local publication cell as authority.

  Acceptance: one independently reconstructed, lower-rooted proposition proves
  the complete observable of the exact persisted artifact for the supported
  profile. No second compiler, finite corpus, or byte-identical fixed point
  substitutes for that result.

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
   ledger live while `OMEGA-PRODUCT-COMPILER-SOURCE` establishes the Omega-written
   product source and deterministic closure in `TASKS.md`.
2. Derive and mechanically enforce a provisional `Ωself` from that product
   closure, then implement `omega-bootstrap` directly against its compositional
   rules. Use O0/O1 only as regression canaries; do not manufacture an O2/O3
   ladder. Feed measured bridge and assurance cost back into each
   retained/excluded profile decision.
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
