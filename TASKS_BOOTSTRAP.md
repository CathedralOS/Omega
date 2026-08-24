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
```

The languages become increasingly capable. Delta is an independent, robust
compiler-host language with C-like power and Omega-shaped conventions where
useful; it is not required to be an Omega subset. Delta source builds
`omega-bootstrap`, which accepts only the exact Omega self-hosting profile
`Ωself`. The production compiler source is normal Omega constrained to that
profile and implements the complete Omega specification. `Ωself` is not another
language rung or dialect.

The bridge binary may run slowly and lower the production compiler
conservatively. It must compile the `Ωself` source that implements the product
optimizer and advanced lowering, but need not implement those passes itself. A
later product self-rebuild can optimize the compiler binary and add fixed-point
evidence; it is not required for full functionality or dependency closure.

Do not conflate implementation language, accepted source, implemented language,
and executable optimization quality. `omega-bootstrap` is written in Delta and
accepts a compositional subset of Omega. The production compiler is written in
that subset, accepts full Omega, and contains the full optimizer; only its own
initial executable may be conservatively generated.

The proof kernel is orthogonal to this chain. It has independent Beta and Gamma
implementations and checks certificates emitted at multiple stages.

Only two source-surface freezes remain in this part of the lattice:

1. **Delta v1**, the literal independent language used to write
   `omega-bootstrap`.
2. **`Ωself`**, the ordinary-Omega profile used by the production compiler's
   own complete source closure.

There is no `omega0`, `omega1`, or Epsilon language generation between them.
O0 and O1 below are bounded vertical-canary labels for the current bridge work,
not ancestors of `Ωself` and not compiler artifacts in the final lattice.

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
| `bootstrap/onramps/omega-rust/` | current working Rust Psi/Omega compiler and CLI; migration/reference producer | `bootstrap/onramps/omega-rust/` |
| `compiler/{psi,omega}/` | eventual Omega-written product compiler source | reserved product roots; implementation remains open |

## Current architectural state

- Alpha has written small-step semantics, conformance tests, and two independent
  native seeds.
- Beta's `bc.beta` self-hosts. Its fixed point establishes dependency closure;
  the persisted artifact is now reconstructed entirely through Alpha and used by
  downstream gates. Complete lower-rooted validation of that artifact against
  `bc.beta` remains open.
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

> **Immediate closure:** finish the Alpha-rooted `bc` blockwise correspondence
> check described under [Cross-rung assurance work](#cross-rung-assurance-work).
> Omega-profile growth follows the actual production Omega source tree and must
> carry its direct-artifact and Rust-free-meaning coverage in the same milestone.

## Delta → omega-bootstrap → production Omega readiness

**Present status: compiler-capable with O0 and the variable O1 vertical slice
closed, but not `Ωself`-bootstrap-ready.** Delta has proved that it can host a
substantial compiler and carry a bounded family of Omega source shapes through
canonical meaning to runnable artifacts, but it has not yet implemented the
bridge compiler.
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
language. It may therefore use deliberately plain facilities when the complete
`omega-bootstrap` source demonstrates that they reduce total implementation and
assurance cost. It need not reproduce Omega's final allocation and container
model. Candidate concessions include:

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
integer arithmetic or ordinary fixed arrays—Delta v1 should omit the unused
ones. Allocation, exhaustion, input assembly, and every retained host operation
must still have explicit semantics and appear in the trust/validation story.

Product Psi/Omega implementation work belongs in `TASKS.md`. This file may name
a required product interface as an input to a lattice gate, but it must not own
or prescribe work inside the Rust on-ramp or the eventual product compiler.

### Dependency split

Two source efforts can proceed in parallel:

- establish the Omega-written production compiler and publish its exact
  transitive source manifest under `OMEGA-PRODUCT-COMPILER-SOURCE` in
  `TASKS.md`; and
- grow the Delta-written bridge from O0/O1 while maintaining a provisional,
  explicit Delta feature ledger and lower-rung meaning for every used
  construct.

The product manifest permits `Ωself` to be derived. `Ωself` then determines the
complete work of `omega-bootstrap`; that bridge's complete Delta source closure
provides the evidence from which Delta v1 is pruned and frozen. The contracts
remain distinct even though discovery is iterative. O0/O1 may continue before
those joins, but neither current source nor producer acceptance may silently
define a language contract.

### Active work packages and acceptance gates

The remaining bootstrap dependencies are:

```text
provisional Delta profile ◀──▶ omega-bootstrap source ──▶ Delta v1 freeze
                                      ▲
production compiler source ──▶ Ωself ┘

Delta v1 + omega-bootstrap + Ωself ──▶ hosted production build

bc source-correspondence refinement proceeds independently
```

Delta v1 defines the language used to write the bridge. `Ωself` defines the
ordinary-Omega programs the bridge accepts. Their source manifests provide the
evidence used to select and prove closure under the contracts; they do not
replace general specifications with file identities or AST shapes. A Delta
feature decision belongs to Delta v1; an Omega feature used or rejected by the
production compiler source belongs to `Ωself`. Do not solve one contract by
silently widening the other.

- [ ] **Discover and freeze Delta's literal compiler-host specification.**
  There is no remaining owner-level feature-list ruling. The fixed constraints
  are deterministic specified behavior, no undefined behavior or ambient host
  authority, specified failure, lower-rung meaning for every admitted construct,
  and Omega spelling/ordinary meaning where Delta retains the same construct.
  Derive the smallest coherent v1 surface from the complete `omega-bootstrap`
  source closure:
  - [ ] maintain a provisional feature ledger naming the concrete bridge need,
    simpler rejected alternative, and meaning/gate coverage for every admitted
    construct, beginning with
    [`bootstrap/rungs/delta/FEATURE_LEDGER.md`](bootstrap/rungs/delta/FEATURE_LEDGER.md);
  - [ ] treat D0, the sample corpus, and Rust-producer acceptance as discovery
    evidence only; remove every unused or accidental feature before the freeze;
  - [ ] test whether Exact arithmetic, fixed arrays, direct tags, and a sealed
    compiler-host interface suffice before retaining general arithmetic
    policies, allocators, sum machinery, or boundary traits;
  - [ ] publish versioned normative grammar, static/dynamic semantics,
    representation/ABI, source-bundle, and retained resource/host-interface
    documents under `bootstrap/rungs/delta/`; do not require an allocation or
    boundary-trait subsystem when the final bridge does not retain one;
  - [ ] reject every excluded source, type, module, boundary, and resource form
    explicitly;
  - [ ] align every retained arithmetic, layout, call, allocation, trap, and I/O
    edge across native targets, the Delta self-host, and the Rust-free
    Delta-to-Gamma meaning route;
  - [ ] replace newline-concatenated source input with the ruled deterministic,
    byte-preserving source-unit/bundle contract; and
  - [ ] publish a classified conformance corpus and feature manifest with
    positive observations, one phase-isolated negative per exclusion,
    exhaustion teeth for every retained resource mechanism, cross-target
    layout/arithmetic edges, and
    native/self-host/lower-rung differentials.

  Acceptance: the literal Delta v1 contract is self-consistent, mechanically
  enforced, and contains the complete deterministic `omega-bootstrap` source
  closure. It has no feature justified only by the current producer or corpus.
  Later widening is an explicit versioned language change, never silent bridge
  pressure.

**External input, not a bootstrap task:**
`OMEGA-PRODUCT-COMPILER-SOURCE` in `TASKS.md` owns the Omega-written product
source. It may proceed in parallel with Delta/bridge discovery. The bootstrap
lane consumes its deterministic transitive manifest; product Psi/Omega
implementation work must not be duplicated here. Compiler-adjacent tools are
outside that manifest unless the compiler executable imports them.

- [ ] **Derive and freeze the Omega self-hosting profile (`Ωself`).**
  Blocked only on the exact production-compiler source manifest above. Standard
  library samples and current Rust source cannot substitute for it.
  - [ ] measure every feature used by the complete source closure against its
    production-source benefit and its Delta implementation/assurance cost;
  - [ ] freeze compositional syntax, static-semantics, resource, ABI/layout, and
    lowering rules—not file identities, statement counts, or AST permutations;
  - [ ] presumptively exclude mathematical proof/program syntax and
    linear/dependent types;
  - [ ] presumptively retain ordinary named fields, payload-bearing sums, and
    basic generics unless source evidence makes their removal cheaper overall;
  - [ ] decide domains, advanced generics, numeric/schema field tags, complex
    transition payloads, and mixed field-plus-case data from the actual source;
    and
  - [ ] gate the complete manifest plus one negative canary per excluded
    capability.

  Acceptance: every program admitted by `Ωself` is ordinary Omega with exact
  Omega meaning; unsupported Omega rejects. The profile is a true subset, not a
  dialect or another lattice rung.

- [ ] **Implement `omega-bootstrap` in Delta.**
  Reuse the closed O0/O1 test path as vertical-canary evidence only. O0/O1 are
  not implementation stages that must be extended in numerical order. Grow the
  bridge by general capabilities required by the product source rather than by
  hard-coded source permutations.
  - [ ] publish the complete deterministic Delta source manifest and prove each
    transitive unit valid under the provisional profile, then under frozen Delta
    v1; one entry source file alone is not the closure;
  - [ ] accept exactly `Ωself` with general parsing, checking, diagnostics,
    terminal representation, and conservative lowering;
  - [ ] reject unsupported Omega before artifact publication;
  - [ ] carry every admitted capability through the Rust-free meaning route and
    direct artifact path in the same milestone; and
  - [ ] compile, rather than duplicate, the production optimizer and advanced
    lowering source.

  Acceptance: the bridge compiles every admitted `Ωself` program with exact
  Omega semantics. It need not accept full Omega, optimize its own output, use
  the production allocator architecture, or host unrelated product tools.

- [ ] **Validate Delta → `omega-bootstrap`.**
  Gate the complete Delta source closure, exact `Ωself` coverage,
  excluded-feature diagnostics, deterministic publication, canonical-meaning
  agreement, conservative lowering, and relevant proof/translation-validation
  seams. Include profile-wide compositional canaries so compiling the exact
  product source cannot conceal source-shape specialization. The Rust compiler
  may remain a differential reference but is never authority or a release
  dependency.

- [ ] **Compile the production Omega compiler once.**
  Run the Delta-built bridge on the exact `Ωself` manifest and validate the
  result against canonical meaning plus the full compiler/language suites. The
  required artifact accepts full Omega and contains the production optimizer
  and advanced lowering, although its own binary may have been conservatively
  generated. A later production self-rebuild is optional optimization and
  reproducibility evidence, not another rung or dependency.

## Cross-rung assurance work

- [ ] **Close the `bc` source-correspondence edge by checked refinement.**
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
    `gen_stmts`/expression cutpoints;
  - fixed emitters, bounded decimal emission, parse-number, parse-character,
    operator classifiers, `cmp_op`, the nine fixed keyword recognizers, and
    conditional `name_eq`/`lookup` have exact shape/meaning/negative modules;
    lookup retains distinct hit-slot-zero versus no-match provenance despite
    their deliberate numeric-zero alias;
  - the bounded `WSTR`/decimal emitter family has exact conditional contracts
    for `gen_read_byte`, `emit_pop_into`, `emit_push`, `emit_mnemonic`,
    `emit_combine`, `emit_slot_addr`, `emit_load_slot`, and `emit_store_slot`,
    with value-parameterized child custody and no full-word decimal claim; and
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
  - [ ] add the separate full-word `emit_dec` theorem. Preserve the source's
    signed comparison/division behavior: negative words take the one-byte base
    case; no unimplemented minus-sign or unsigned-format claim is allowed;
  - [ ] compose `new_label`, `emit_lref`, `emit_str_body`, `gen_emit`,
    and `emit_cmp`, retaining exact trace order and malformed-tail cursor
    bounds;
  - [ ] close `gen_load`, `gen_write_byte`, `gen_call`, `gen_factor`,
    `gen_term`, `gen_sum`, and `gen_expr` together under the checked
    `EXPRDEPTH<=64` induction, including every resource exit;
  - [ ] build `gen_stmt`'s branch relation and the guarded greatest fixed point
    for `gen_stmts`/`gen_block`/`gen_state`/`gen_stmt`, preserving finite
    or infinite stdout prefixes without assuming output productivity;
  - [ ] compose the `parse_proc`/PFXS body cutpoint through the unconditional
    epilogue and return, including finite child returns after numeric resource
    status 252;
  - [ ] classify each checked resource outcome from its exact proved guard,
    resource profile, and requested amount. Status 252/253 is only a process
    projection and must not be used to recover `ResourceKind`; this is an
    implementation obligation under the already-fixed observable, not a new
    Beta-language or owner-design blocker; and
  - [ ] close the root loop and prove equality of maximal stdout plus
    `Halt`/`Trap`/`Exhaust`/`Diverge` for every finite source stream and
    supported resource profile in `BOOTSTRAP_OBSERVABLE.md`.

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

1. Close the Alpha-rooted `bc` source-correspondence edge with lower-rooted
   checking.
2. Keep Delta's Rust-free meaning route as a rolling invariant: every newly
   admitted compiler construct lands with native/meaning differential coverage.
3. Maintain Delta's provisional feature ledger while growing the bridge; once
   `OMEGA-PRODUCT-COMPILER-SOURCE` is complete in `TASKS.md`, derive and gate
   `Ωself`, complete the bridge source closure, then prune and freeze Delta v1.
4. Grow proof-kernel capability and its operational seams only in lockstep with
   real obligation classes.
5. Build translation-validation evidence for native compiler outputs.
6. Use the closed O0/O1 source-through-ELF canaries as regression evidence while
   implementing `omega-bootstrap` directly against `Ωself`; do not manufacture
   an O2/O3 profile ladder. Carry correct conservative lowering sufficient for
   the required hosted compile.
7. Use `omega-bootstrap` once to build and validate the full optimizing Omega
   compiler from the `Ωself` production source manifest. Treat any later
   product self-rebuild as optional assurance evidence, not another rung.

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
