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
              Omega (Delta-built, simple)
                           ↓
              Omega (Omega-built, optimized)
```

The languages become increasingly capable. Delta builds a deliberately simple,
spec-compliant Omega compiler. That compiler is a valid self-sufficient endpoint,
although it may compile slowly and emit minimally optimized code. It then builds
the full optimizing compiler from Omega source. The repeated Omega is one
self-host dependency, not another language rung.

The proof kernel is orthogonal to this chain. It has independent Beta and Gamma
implementations and checks certificates emitted at multiple stages.

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

### Assurance and bootstrap Omega

| Canonical or transitional source | Role | Canonical owner |
| --- | --- | --- |
| `bootstrap/assurance/proof-kernel/{implementations,tools,corpus,gates}/` (compatibility: `compiler/proof-kernel`) | cross-cutting derivation checking, tools, corpora, and gates | `bootstrap/assurance/proof-kernel/` |
| `bootstrap/assurance/refinement/{beta,omega0}/` (compatibility entries remain under Alpha and Omega0 gates) | cross-rung source/meaning-to-artifact obligation reconstruction and checking | `bootstrap/assurance/refinement/` |
| `bootstrap/omega0/` | Rust-free meaning, current first-Omega compiler slices/contracts, and gates | `bootstrap/omega0/` |
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
- The next compiler dependency closure is Delta → bootstrap Omega → production
  Omega. Bootstrap Omega needs correctness and language coverage, not advanced
  optimization.

> **Immediate closure:** finish the Alpha-rooted `bc` blockwise correspondence
> check described under [Cross-rung assurance work](#cross-rung-assurance-work).
> Omega-profile growth follows the actual production Omega source tree and must
> carry its direct-artifact and Rust-free-meaning coverage in the same milestone.

## Delta → first Omega readiness

**Present status: compiler-capable with O0 and the variable O1 vertical slice
closed, but not Omega-bootstrap-ready.** Delta has proved that it can host a
substantial compiler and carry a bounded family of Omega source shapes through
canonical meaning to runnable artifacts, but it has not yet implemented the
Omega compiler.
`bootstrap/rungs/delta/samples/lowermachine.alp` is a real
Delta-written Delta-to-ARM64 compiler: it self-compiles to a fixed point and its
output is swept against the Rust reference over the sample corpus. This proves
the basic compiler-host vocabulary—mutable arenas, parsing, recursive calls,
sum types, state-machine control flow, byte I/O, and code emission.

That evidence is necessary but is not the first Omega compiler:

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
- The exact bootstrappable Omega source profile sufficient to express the
  production Omega compiler has not been frozen.

### Permitted Delta bootstrap concessions

Delta is a bootstrap implementation language, not the product language. It may
therefore use deliberately plain facilities that make the first Omega compiler
practical, without first reproducing Omega's final allocation and container
model:

- Permit runtime-sized allocations from an explicit, fixed backing extent
  supplied at compiler startup. A deterministic bump allocator or paged arena
  is sufficient; general-purpose `free`, compaction, and garbage collection are
  not prerequisites.
- Permit multiple typed/indexed arenas over that allocator. Arena handles, not
  ambient host pointers, remain the normal durable identity.
- Permit bulk reclamation at the end of a compilation. Exhaustion must have a
  specified result—checked failure or a defined trap—and must never silently
  truncate input or tables.
- Permit source bundling or a simple manifest/concatenation step before a full
  package/module implementation. The input consumed by the compiler must still
  be deterministic and preserved as an auditable artifact.
- Permit direct, conservative lowering and poor generated code. Parallelism,
  advanced register allocation, optimization, incremental compilation, and the
  production `PagedArena` architecture are explicitly not gates for the first
  Omega artifact.

These are implementation concessions, not holes in meaning. Allocation,
exhaustion, input assembly, and any host boundary used to create the backing
extent must still have explicit semantics and appear in the trust/validation
story.

Product Psi/Omega implementation work belongs in `TASKS.md`. This file may name
a required product interface as an input to a lattice gate, but it must not own
or prescribe work inside the Rust on-ramp or the eventual product compiler.

### Work packages and acceptance gates

- [x] **Freeze the current bootstrap profiles.** Record the Delta surface used
  by the current compiler slices and the Omega surface those slices accept.
  - [x] Freeze Delta implementation profile D0 and Omega vertical-canary profile
    O0 in `bootstrap/omega0/compiler/BOOTSTRAP_PROFILES.md`.
  - [x] Freeze O1 at 0–16 literal writes, 1 final nonnegative-i32 exit, 2,048
    source bytes, and 1,024 aggregate decoded literal bytes. The same
    table-driven frontend/emitter/backend handles every admitted count.
- [x] **Add scalable compiler storage to Delta.** Implement the explicit
  fixed-backing allocator, runtime-sized byte/source storage, and indexed arenas;
  gate deterministic allocation, alignment, exhaustion, and bulk reset. Remove
  silent fixed-capacity truncation from the compiler path.
  - [x] Gate the D0 fixed-backing allocator convention for deterministic aligned
    allocation, indexed handles, exhaustion-state preservation, and bulk reset
    across native, Rust-reference, and Rust-free Delta-to-Gamma meaning routes.
  - [x] Make `lowermachine.alp` reject source-buffer exhaustion instead of
    silently compiling a truncated prefix.
  - [x] Replace `lowermachine`'s dedicated fixed source/table arrays with logical
    byte and typed arenas over explicit backing, preserving checked exhaustion.
    Source bytes now reserve one contiguous indexed cell per input byte; all
    compiler tables are offset-addressed logical arenas inside one explicitly
    reserved typed extent. The self-build remains an assembly-byte fixed point,
    the native corpus remains green, and source exhaustion still exits before a
    truncated prefix can be compiled.
- [x] **Choose and gate source packaging.** `bootstrap/omega0/compiler/OMEGA0_BUNDLE.md`
  defines the canonical, length-delimited version-1 multi-source artifact.
  Its gate covers deterministic ordering, exact byte preservation, canonical
  paths, and malformed/truncated input rejection. The packer is untrusted; the
  Delta streaming decoder canary implements the same acceptance contract with
  explicit local-storage exhaustion and is gated natively and through the
  Rust-free meaning route.
- [x] **Close the Delta-written artifact path for O0/O1.** Emit the canonical
  image directly, without an unrecorded assembler or linker dependency.
  - [x] Close the exact O0 canary edge with
    `bootstrap/omega0/compiler/omega0-terminal-to-elf.alp`. It consumes the
    vocabulary-25 O0 terminal shape, retains the variable literal and
    nonnegative `i32` exit operand, and emits a deterministic 8 KiB Linux x86-64
    ELF directly, with no host assembler or linker. The gate proves canonical
    byte identity with the production image, operand-variant emission, and
    empty-output rejection for truncation, fixed-field tampering, and trailing
    input.
  - [x] Generalize the direct Linux x86-64 image edge for O1. Canonical
    0/1/2/16-write terminal modules reproduce the product images byte for
    byte; 17 writes, 1,200 aggregate bytes, malformed input, and truncation
    reject before emitting any image byte.
  - [x] Gate the frozen O1 compiler-program composition through Delta's
    `lowermachine`. Both `omega0-frontend.alp` and
    `omega0-terminal-to-elf.alp` are recompiled through the Delta-written
    compiler, then bundle → vocabulary-25 terminal Psi → ELF reproduces the
    independent product terminal and image bytes for 0/1/2/16 writes; semantic
    rejection and every frontend/backend O1 exhaustion boundary publish no
    partial artifact. This is a deliberately partial dependency-closure claim:
    the initial `lowermachine` executable still comes from the disposable Rust
    on-ramp, and native assembly/signing still uses Darwin `clang`/`codesign`.
    It does not claim the production Omega compiler exists or a Rust-free
    compiler lineage.
- [x] **Complete lower-rung meaning for the current D0/O1 profile.** Cover every
  admitted construct, including allocation and exhaustion, through the
  Delta-to-Gamma route and preserve native-versus-meaning differential gates.
  - [x] Evaluate D0 fixed-backing allocation and exhaustion through the
    Beta-written `omega2gamma.beta` elaborator and Gamma interpreter, with a
    source perturbation that changes the observed result.
  - [x] Exercise byte input/output and real Delta certifiers through the same
    Rust-free route in `bootstrap/omega0/gates/convergence-reference.sh`.
  - [x] Gate the actual Delta-written backend through lower-rung meaning for the
    canonical, operand-variant, and rejection observations.
    - [x] Remove the compiler-scale elaboration cliff. Per-machine metadata
      tables previously overlapped at machine 25, corrupting machine zero's
      local count to 29,620 and repeating 29,620 phantom arguments in every
      definition. The tables now reserve the documented 128-machine capacity,
      and scalar receiver fields use one indexed carrier. The 28-machine backend
      now elaborates completely to 87,979 bytes of Gamma in about 0.16 seconds,
      rather than exceeding 116 MiB without completing.
    - [x] Lower the used nonnegative byte-extraction form `x & 255` to
      `x % 256`; reject every broader single-`&` expression explicitly instead
      of silently truncating the expression at `&`.
    - [x] Bound compiler-sized evaluation without normalizing a larger fixed
      arena. The canonical Gamma interpreter now interns the dense integer range
      used by compiler loops, represents ordinary two-field `Cons` values in one
      arena node instead of three, and trampolines tail-position
      `let`/`if`/`match`/call chains. These are representation-only evaluator
      optimizations: Gamma's matching and printed values are unchanged, while
      the backend's 3,920/4,096-byte fill loops no longer consume the evaluator
      arena or Beta/Alpha return stack per logical iteration.
    - [x] Add the focused native-versus-Gamma backend meaning gate. One bounded
      87,979-byte elaboration is reused across the frozen canonical input, an
      operand variant, malformed magic, and both O1 exhaustion controls. The
      lower-rung `(Pair status stdout)` observation is decoded strictly and its
      complete 8,192-byte success image (or empty rejection image) must equal
      native Delta execution; the canonical image also equals the independent
      product reference. The fixed signed x86 branch displacement is emitted as
      its four literal bytes rather than silently claiming general signed
      shift/bit-mask coverage.
  - [x] Audit the current O0/O1 Delta sources against D0 and make every construct
    either elaborate through the lower-rung route or reject before it can enter
    the compiler. Keep `gamma_emit.rs` only as a reference differential producer.
    - [x] Extend `omega2gamma.beta` for the Delta-written O0 frontend. Native,
      Delta-self-hosted, and lower-rung elaboration/interpreter routes now return
      the retained operand digest 107 for `cli_mvp`; the lower-rung route also
      preserves semantic rejection at 251. The focused gate pins multi-slot
      void/value method-state threading, bounded per-machine capacity, and the
      private chunked carrier used only for compiler-sized scalar arrays.
    - [x] Re-establish the frontend meaning gate for O1. The 40-machine frontend
      elaborates completely within the 1 MiB and timeout bounds; the end-to-end
      lower-rung gate pins the
      retained digest, zero/two-write dual-channel results, semantic rejection,
      and multi-slot method threading. It is part of the default lattice suite.
  - [x] Admit the current 695-state `lowermachine.alp` source to the Rust-free
    elaboration route with explicit capacity contracts. State metadata
    now has a checked 1,024-state-per-machine ceiling, state parameter rows have
    a checked four-parameter ceiling, and compiler-sized scalar arrays use a
    bounded persistent tree rather than materializing their full zero backing.
    The focused gate pins exact/plus-one state capacity, parameter overflow,
    large-array updates, explicit array overflow, and the canonical Gamma
    interpreter's exact 4 MiB source boundary. The real compiler elaborates
    marker-free to a bounded Gamma program.
- [x] **Build a vertical Omega canary in Delta.** A Delta-written program must
  accept a small Omega source file, perform name/type checks, lower through the
  chosen terminal-Psi path, and produce a runnable artifact whose behavior
  agrees with canonical meaning.
  - [x] Freeze the O0 console source contract and implement the Delta streaming
    decoder for its canonical multi-source input artifact.
  - [x] Implement the Delta O0 lexer/parser and complete its positive and
    name/type/count rejection matrix against the frozen source contract. The
    focused native gate covers canonical, variant, malformed, and exhaustion
    cases, and a Delta-written `lowermachine` recompilation preserves both
    acceptance and rejection. It retains the decoded `write_line` carrier and
    `exit_process` literal and exposes their digest until terminal-Psi emission
    consumes them.
  - [x] Emit the O0 terminal-Psi semantic artifact while retaining
    `write_line`'s exact structural byte carrier and custody through its
    boundary call. This is implementation work, not an unresolved language
    ruling, and it must use the shared terminal representation rather than an
    O0-private IR.
    - [x] Emit canonical terminal semantic bytes directly from Delta and gate
      them through the shared codec/verifier with the canonical empty proof
      bundle for proof-free O0. Do not route this milestone through the Rust
      checked-plan producer trees.
      - [x] After the attachment representation is honest, freeze one canonical
        O0 terminal-module fixture: stable declaration/value/operation IDs,
        ordered `write_line` then `exit_process` calls, the exact byte literal,
        and the exact scalar exit operand. Generate the fixture through the
        shared codec only as conformance evidence, not as a bootstrap producer.
      - [x] Add the Delta emitter using ordinary checked `write_byte` output and
        explicit length/integer encoding. Its emitted bytes must decode and
        verify through the shared vocabulary-25 path and must be byte-identical
        to the frozen canonical fixture for the same retained operands.
      - [x] Gate custody perturbations independently: changing any literal byte,
        its length, the newline-producing call order, or the exit scalar must
        change the decoded semantic artifact or reject. Truncation and emitter
        storage exhaustion must reject without a partial artifact being
        accepted. The direct emitter streams and has no artifact buffer to
        exhaust; source/text exhaustion occurs before output, and every partial
        prefix is rejected by the shared decoder.
  - [x] **Close the Delta artifact-publication sink contract.** D0 `write_byte`
    returns `Unit`, so the compiler cannot observe a physical sink/short-write
    failure. The generic terminal-semantic publisher now gives the producer a
    private same-directory staging sink, persists it, requires the declared
    producer exit and successful canonical decode (plus expected semantic
    identity when supplied), and atomically renames only after acceptance.
    Truncation, malformed or substituted meaning, and producer failure preserve
    the previous accepted destination; successful producer exit alone is not
    artifact acceptance.
  - [x] Gate the runnable O0 artifact: exact output plus newline, requested
    low-byte exit status, deterministic bytes, and canonical-meaning agreement.
    The published vocabulary-25 fixture is decoded and verified with the empty
    proof bundle, executed by canonical terminal meaning, and lowered with
    exact requirement-matched provider executions to deterministic Linux x86-64
    and AArch64 images plus replayed installation records. Matching Linux hosts
    execute the image and compare stdout/status; other hosts validate both
    complete image formats without pretending to execute them.
- [ ] **Derive and freeze the production-self-host acceptance profile.** This is
  blocked on `OMEGA-PRODUCT-COMPILER-SOURCE` in `TASKS.md`; standard-library and
  sample `.omg` files cannot substitute for that exact source tree. Once it
  exists, freeze only the language surface the product compiler actually uses.
- [ ] **Implement the first Omega compiler in Delta.** Grow the canary into the
  deliberately simple, spec-compliant compiler. Prefer direct and auditable
  stages over porting the production optimizer or the entire current Rust
  architecture.
  - [x] Close execution of the full current `lowermachine` through canonical
    Gamma meaning. Allocation profiling identified evaluator-private tail-call
    argument lists—not the translated compiler's persistent arrays—as the arena
    cliff. A checked 4 KiB scratch stack now carries pending evaluated arguments
    without changing source-visible Gamma values. The translator also skips
    quoted strings atomically while scanning states; previously the `//` in an
    emitted banner hid the compiler's final 18 states. A block-final
    `write_line` without `;` now stops at `}` instead of scanning into the next
    machine. The focused gate compiles
    `arith.alp` through the complete route and requires decoded status 0 plus all
    800 output bytes to equal native execution (and a frozen SHA-256 everywhere).
  - [x] Implement O1 as the first genuinely variable source slice: preserve the
    O0 declaration/entry shell, accept a bounded sequence of zero or more
    literal `write_line` statements followed by exactly one literal
    `exit_process`, and reject an exit anywhere but the end. One statement-
    table parser/emitter/backend loop must handle 0, 1, 2, and many writes; do
    not encode source-count permutations as separate paths or fixtures.
  - [x] Generalize terminal-Psi emission and direct ELF lowering together for
    O1. Allocate dense variable place/operation IDs, preserve ordered effects,
    preflight all declared table/text/image ceilings before publishing bytes,
    and compare several generated cases against the shared product codec and
    lowering. Terminal vocabulary 25 already represents this slice; no new
    language ruling is required.
  - [ ] Grow subsequent monotonic profiles from requirements of the actual
    Omega-source production compiler. Each profile must add its frontend,
    terminal representation, direct artifact path, lower-rung meaning coverage,
    diagnostics, and negative controls as one vertical capability—not a matrix
    of hard-coded sample permutations.
    This item owns only the Delta/bootstrap implementation and its lattice
    gates. Any required product Psi/Omega or Rust-reference implementation work
    is tracked in `TASKS.md`.
    Every newly admitted construct must elaborate through the Rust-free meaning
    route or reject before entering the compiler; `gamma_emit.rs` remains a
    differential reference only.
- [ ] **Validate Delta → Omega.** Gate representative language coverage,
  negative diagnostics, deterministic artifacts, meaning agreement, and the
  relevant proof/translation-validation seams.
- [ ] **Compile production Omega from Omega source.** Use the Delta-built
  compiler to produce the optimized Omega compiler, then validate the self-build
  edge against canonical meaning. The Delta-built compiler remains a supported
  slow, unoptimized endpoint.
  - [ ] Build and validate both artifacts explicitly: Delta-built simple Omega,
    then Omega-built optimizing Omega. Stopping after the first remains a valid
    supported configuration.

The O0/O1 vertical path is closed through a direct, lattice-written x86-64 ELF.
The next evidence boundary is to grow that frozen slice into the
deliberately simple Omega compiler while widening direct artifact emission and
the used-Delta meaning profile only as the compiler source requires. Those
requirements, rather than a wholesale Delta redesign, determine which
additional facilities the bootstrap actually needs.

## Cross-rung assurance work

- [ ] **Close the `bc` source-correspondence edge by checked refinement.** The seed Beta
  compiler is now built through the preceding audited rung; validate the
  complete artifact against `bc.beta` using authority rooted below `bc`.
  A fixed point alone is not acceptance evidence.
  - [x] Specify the compiler observable as the complete output byte stream plus
    halt, trap, divergence, and checked resource exhaustion—not merely an exit
    byte or a finite set of executions. `bootstrap/rungs/beta/BOOTSTRAP_OBSERVABLE.md`
    fixes maximal traces, terminal classifications, independently reconstructed
    closure obligations, and the first exact supported profile `B_bc1`.
  - [x] Make `bc.beta` reject source-arena exhaustion before it can overwrite
    adjacent compiler tables or emit a truncated Alpha assembly artifact. The
    exact 1 MiB boundary and empty-output failure projection are gated.
  - [x] Implement the exact `bc.beta` bootstrap profile in an Alpha-written Beta
    compiler assembled and run only through the audited Alpha/Beta seed path.
    The current Python symbolic model cannot cover `bc.beta`'s data-dependent
    branching, word memory, or full-stream emission and is not this authority.
    - [x] Land Slice A under `bootstrap/rungs/beta/cold-start/`: an Alpha-written,
      5 KiB compiler tape with checked 1 MiB input capture, two-pass
      validate-before-publish parsing, source-span identifiers, comments,
      decimal/character literals, and precedence-correct `+ - * / %` lowering.
      Its focused gate covers valid execution, malformed empty-output rejection,
      exact-limit acceptance, and one-byte-over checked exhaustion. This is the
      first monotonic implementation slice, not the complete `bc.beta` profile.
    - [x] Extend the same compiler with Slice B: up to 128 framed procedures,
      four parameters/arguments, 64 function-scoped frame slots, assignment,
      variable references, and nested forward/backward calls. Validation freezes
      final frame metadata, resolves up to 512 calls after EOF, enforces arity,
      and reserves output before the publication pass. The focused gate covers
      four live arguments, nested calls, late-entry `main`, name/arity failures,
      malformed-late empty output, and every new bounded table's exhaustion.
    - [x] Extend it with Slice C: all six comparison operators (signed ordering,
      full-width equality) and Beta's procedure-scoped `state` blocks plus
      guarded/unconditional `to` edges.
      The validation pass freezes and resolves state targets before publication;
      Beta-unspellable generated labels prevent source collisions. Checked limits
      are 64 states/transitions per procedure and 512 globally. The focused gate
      covers signed/nested comparisons, optional guard grouping, forward/backward
      flow, loops, fallthrough, scoping, adversarial names, and exact/overflow
      capacity boundaries.
    - [x] Complete Slice D with nested byte/word memory, call statements,
      `read_byte`/`write_byte`, and decoded fixed-string `emit`. The Alpha-written
      compiler now accepts all 32,045 pinned source bytes and emits valid Alpha.
  - [x] Adopt the resulting lattice-built `bc` artifact throughout the bootstrap.
    - [x] Persist the 51,647-byte platform-independent fixed-point `bc.tape` and
      gate byte-for-byte reconstruction, another self-build generation, and the
      complete retained Beta corpus through it. No Rust producer is in its
      construction lineage.
    - [x] Switch proof-kernel, Gamma, Delta, Omega0, and refinement gates away
      from their ephemeral Rust-produced `bc0` setup to the shared artifact
      loader, retaining Rust comparisons only where explicitly diagnostic.
  - [ ] Discharge whole-compiler source correspondence for the exact persisted
    artifact. Reconstruct the complete observable from `bc.beta` and the tape
    with authority rooted below `bc`, including output bytes and every terminal
    classification in `BOOTSTRAP_OBSERVABLE.md`; fixed-point identity and the
    retained corpus remain supporting evidence, not this proof.
    - [x] Add the first lower-rooted whole-artifact structural checker in Alpha.
      It independently walks reachable instructions in `bc.tape`, permits
      jump-skipped inline data, proves instruction framing and direct target
      boundaries, rejects overlap/unknown/truncation/range mutations, and pins
      the exact 262,140-byte tape-hole payload.
    - [x] Reconstruct the persisted artifact's static procedure regions and
      call/return discipline below `bc`. Direct calls define 70 non-root entries;
      entry zero alone may halt, every callee region has a reachable return,
      call continuations remain in their caller, and every non-call edge remains
      inside its region. Focused fixtures reject root returns, callee halts,
      returnless callees, and cross-region jumps. Dynamic call-depth bounds,
      frame contents, output semantics, and termination remain open.
    - [x] Check the whole-compiler control skeleton against exact `bc.beta`.
      A lower-rooted Alpha checker independently scans 70 procedures, 355 entry/
      state blocks, 291 `to` sites, and 180 guarded sites; resolves exact
      procedure-local state names; reconstructs Alpha instruction boundaries;
      and checks ordered, unique block/site mappings plus unconditional and
      guarded successors. The one source block absent from the pc-zero CFG is
      admitted only as an additional decode root under the same global framing,
      overlap, and interior-target checks. Missing/duplicate/reordered witnesses,
      operand-interior PCs, and a structurally valid branch retarget reject.
      Expression/data effects, dynamic calls/returns, output traces, terminal
      classes, and cyclic progress remain open.
    - [x] Bind every source effect site to the exact artifact below `bc`. The
      same Alpha process now owns 309 ordinary calls, two `read_byte` sites, five
      `write_byte` sites, 113 fixed-string emits carrying 829 decoded bytes, and
      183 explicit returns. Exact prelude/helper/fallthrough accounting gives
      one owner to all 423 artifact calls, two reads, six writes, 254 returns,
      and the sole halt. Emit sites check jump-skipped bytes, pointer, length,
      helper target, and the exact helper loop. Valid-entry call retargets,
      I/O register/opcode changes, helper mutations, unreachable literal edits,
      emit pointer/length changes, and malformed event witnesses reject while
      remaining structurally valid. This establishes static custody and the
      fixed-emit macro when reached, not argument/value correspondence, frame
      behavior, reachability, global trace order, or terminal correspondence.
    - [x] Check source-derived frame shape and immediate parameter handoff. A
      separate Alpha module in the same checker process derives 27 parameters
      and 51 function-scoped `let`s, then validates all 70 base prologues, 47
      nonempty allocations covering 78 slots, and 27 ordered parameter stores.
      All 309 ordinary calls match their source callee arity and pop 134 staged
      arguments into `r0..r1` in exact reverse-stack order. Structurally valid
      frame-size, fp-register, parameter-offset/register, pop-order, and
      pop-step mutations reject. This proves static allocation and handoff
      conditional on staged values; staged argument-value association, live
      stack depth, and dynamic frame contents remain open.
    - [x] Bind every function-scoped local access to its source slot. The BCT8
      Alpha phase independently records all 27 parameters and 51 `let`
      declarations, resolves exact source names, distinguishes assignment
      targets from comparison operands and calls, and checks 169 reads plus 73
      `let`/assignment writes against their 19-byte fp-relative macros.
      Valid-slot retargets, frame-base changes, same-width load/store swaps,
      duplicate locations, and reordered witnesses reject while remaining
      structurally valid. Static slot/opcode custody is closed; carried values,
      definite assignment, expression evaluation, value association, and dynamic
      aliasing remain open.
    - [x] Bind every raw memory operation to its source width and artifact site.
      The same Alpha process classifies matching source brackets and checks 56
      word loads, six byte loads, 32 word stores, and one byte store against
      exact opcodes/registers; each store additionally owns its immediate
      address-pop macro. Width/register/pop-step mutations and malformed BCT8
      locations reject while retaining valid instruction framing. Address and
      value correspondence, aliasing, alignment, and the 64 MiB bounds proof
      remain open.
    - [x] Classify every raw-store source address into the compiler's exact
      address families. The BC10 grammar-composition phase independently pins
      30 aligned fixed-global word addresses in `[2097064, 2097145)`, the sole
      source-byte store spelling `2097152 + n`, and the paired local-name-table
      spellings `3145728 + s * 8` and `3153920 + s * 8`. A store moved between
      fixed and ranged families, an unaligned/out-of-window fixed address, or a
      different ranged expression rejects before artifact execution. This
      closes the finite source-site classification only: the guards and carried
      `n`/`s` values, disjointness from the explicit stack and depth counters,
      and all raw-load bounds remain blockwise-simulation obligations.
    - [x] Bind source literals and arithmetic primitives to exact lowering
      macros. The BCT8 Alpha phase independently scans all 582 decimal/character
      literals and 57 `+`/`-`/`*`/`/`/`%` operators. It checks exact
      `imm r0,value` sites and exact 22-byte left-value-pop/operator macros. An
      independent artifact inventory reserves the 360 comparison-result and 113
      fixed-emit address immediates, then requires ownership of all 582 remaining
      literal candidates and all 57 arithmetic macros. Structurally valid
      literal value/register, same-valued synthetic-site retarget, arithmetic
      opcode/register, pop-step, duplicate-location, and reordered-record
      mutations reject. This flat phase leaves recursive expression
      composition, arithmetic traps, and dynamic stack bounds open. Identical
      same-valued primitives within one block remain
      mutually swappable, so this phase claims block-local multiset/shape custody
      rather than unique per-occurrence provenance.
    - [x] Bind all six source comparison operators to exact lowering macros. The
      BCT8 phase checks all 180 comparison sites against the source-selected
      signed `jlt` or full-word `jeq` variant, exact operand order, 16-byte
      left-value pop, branch-taken/done targets, and complementary 0/1 results.
      Same-width branch-opcode, operand-order, valid-boundary target,
      materialized-result, and pop-step mutations retain Alpha framing and
      reject. This establishes static comparison-macro custody conditional on
      staged operands; this flat phase leaves recursive value composition,
      reachability, identical-site ordering, and dynamic stack bounds open.
    - [x] Bind every source-required data-stack push to an exact artifact macro.
      The BCT8 phase reconstructs 237 binary-left pushes, 134 left-to-right
      ordinary-call argument pushes, and 33 store-address pushes from the
      already independent primitive, arity, and memory tables. It validates all
      404 exact 16-byte macros and exhaustively owns every decoded artifact
      occurrence. Stack-step/register/value/opcode, duplicate-location, and
      cross-block witness mutations retain Alpha framing and reject. Since the
      macro bytes are identical across categories, this proves block-local
      multiset/shape custody; recursive value association, identical same-block
      order, and live stack bounds remain open.
    - [x] Compose the flat expression/staging sites by the exact Beta grammar.
      A separate Alpha module reparses all 70 procedures and 355 blocks with
      source precedence and statement boundaries, consumes every primitive,
      local, raw-memory, call/effect, transition, and push table in lexical
      order, then requires their owned PCs in recursive lowering order. It binds
      left/push/right/operator, nested loads, left-to-right argument evaluation
      and pushes plus reverse pops, address/push/value/store, local stores,
      guarded transitions, and return epilogues. Every complete statement
      expression restores its entry-relative `r15`; exact `bc.beta` has an
      independently reconstructed high-water mark of two temporary words.
      Same-valued literal, argument-push, and store/binary-push permutations
      retain every preceding flat-custody property and reject only in this
      phase. Syntax-directed composition and relative temporary balance are
      closed; absolute `B_bc1` stack bounds, dynamic frames, carried
      local/memory/callee values, reachability, traps, and global terminal/trace
      correspondence remain open. Byte-identical complete statements/effects
      within one block may still be mutually swappable until cross-statement
      artifact order is closed by the blockwise simulation.
    - [x] Reconstruct the finite whole-compiler call recurrence below `bc`.
      The BCT9 Alpha phase resolves all 309 ordinary source calls to the 70
      independently scanned procedures, derives every checked prologue weight,
      records the exact grammar-reconstructed temporary height at each call,
      and includes all 113 synthesized fixed-emit helper calls. It recognizes
      the complete `gen_expr`, `gen_stmts`, and signed-positive `/ 10`
      `emit_dec` ranking schemas, explicitly charges the rejected depth-65
      probe frame and guard temporary, and checks untrusted 64-level
      expression/block plus root summary potentials edge by edge. The resulting
      conservative root bounds are 12,720 explicit-stack bytes and 662 hidden
      returns, respectively below the reserved 524,288 bytes and 8,192 returns;
      underreported probe and root certificates reject. This closes the static
      call graph, finite recurrence, and numerical margin conditional on the
      two depth counters and saved-frame words retaining their source/ABI
      values. Absolute `B_bc1` stack safety remains part of the blockwise
      simulation until raw-memory bounds/aliasing prove unrelated stores cannot
      corrupt those locations; carried local values, return values, and
      reachability also remain open.
    - [x] Freeze supported resource profile `B_bc1` and make its source-side
      ceilings checked. `bc.beta` now refuses a 1,025th name slot, fifth live
      parameter/argument, and expression or nested-block depth 65 before any
      compiler-owned overlap. The focused gate pins exact/+1 boundaries,
      statuses 252/253, empty source-exhaustion output, and deterministic maximal
      prefixes for later structural exhaustion.
    - [x] Write the canonical small-step Beta source semantics needed by the
      relation: left-to-right expressions/calls, CFG fallthrough/transitions,
      finite byte memory, byte I/O, wrapping/signed arithmetic, and maximal
      halt/trap/exhaustion/divergence observations. The Python interpreter is
      now explicitly finite-run regression evidence rather than authority for
      sparse memory or its step cap.
    - [ ] Reconstruct and check the blockwise forward simulation from the exact
      parsed `bc.beta` CFG to the decoded Alpha CFG. Cover stack/memory bounds,
      call/return frames, streamed output, terminal classes, and cyclic progress;
      do not expand the current closed-form symbolic branch tree.
  - [x] Enlarge the x64 seed's former 32 KiB image extent before claiming
    cross-platform closure; both committed seeds now reserve 256 KiB, sufficient
    for the current roughly 52 KiB self-hosted tape.
### Completed ownership normalization

- [x] Make gates relocatable through `bootstrap/paths.sh` and enforce path
  hygiene from both repository and unrelated working directories.
- [x] Move Alpha, Beta, Gamma, and Delta to `bootstrap/rungs/`; move disposable
  Rust producers to `bootstrap/onramps/`; retain old `compiler/` names only as
  compatibility paths.
- [x] Split generic proof checking and cross-rung refinement under
  `bootstrap/assurance/`, without assigning either role to a language rung.
- [x] Split the former `beta-lang-py` directory by responsibility: executable
  Beta meaning under the Beta rung and symbolic reconstruction under assurance.
  Remove its obsolete backend and facade gate; compatibility wrappers remain.
- [x] Move first-Omega work to `bootstrap/omega0/` and shared seam fixtures to
  `bootstrap/corpus/`.
- [x] Move the current Rust Psi/Omega compiler and CLI out of unsuffixed product
  roots and into `bootstrap/onramps/omega-rust/{psi,omega,apps/omega-cli}/`.
- [x] Reserve `compiler/{psi,omega}/` for the eventual Omega-written product
  compiler. The placeholder roots do not satisfy the open production-source
  task above.

## Execution order

1. Close the Alpha-rooted `bc` source-correspondence edge with lower-rooted
   checking.
2. Keep Delta's Rust-free meaning route as a rolling invariant: every newly
   admitted compiler construct lands with native/meaning differential coverage.
3. Once `OMEGA-PRODUCT-COMPILER-SOURCE` is complete in `TASKS.md`, derive the
   bootstrap acceptance profile from the code that must actually self-host.
4. Grow proof-kernel capability and its operational seams only in lockstep with
   real obligation classes.
5. Build translation-validation evidence for native compiler outputs.
6. Grow the closed O0/O1 vertical path—source through direct ELF—into the
   deliberately simple, spec-compliant Omega compiler.
7. Use the resulting bootstrap Omega compiler to build and validate the full
   optimizing Omega compiler from Omega source.

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
sh bootstrap/omega0/gates/kernel-diamond.sh
sh bootstrap/omega0/gates/omega-meaning.sh
sh bootstrap/assurance/refinement/omega0/meaning-cert-diamond.sh
sh bootstrap/assurance/refinement/omega0/translation-validation.sh
sh bootstrap/omega0/gates/delta-terminal-to-elf.sh
sh bootstrap/omega0/gates/delta-terminal-to-elf-meaning.sh
sh bootstrap/onramps/delta-rust/omega0-frontend-meaning.sh
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
