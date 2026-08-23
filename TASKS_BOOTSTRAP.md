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

## Role map and repository migration

The current flat `compiler/` tree is historical. These paths are grouped below
by actual ownership; the target homes are under `bootstrap/` for the seed-built
lattice and `compiler/` for the product implementation.

### Language spine

| Canonical or compatibility source | Role | Target owner |
| --- | --- | --- |
| `bootstrap/rungs/alpha/` (compatibility: `compiler/alpha`, `compiler/beta`) | 21-opcode native seed VM, written semantics, and Alpha-written Alpha assembler | `bootstrap/rungs/alpha/` — moved |
| `bootstrap/rungs/beta/` (compatibility: `compiler/beta-lang`) | Beta language and self-hosting compiler | `bootstrap/rungs/beta/` — moved |
| `bootstrap/rungs/gamma/` (compatibility: `compiler/gamma`) | Gamma language, interpreter, and type checker | `bootstrap/rungs/gamma/` — moved |
| `bootstrap/rungs/delta/` (compatibility: `compiler/delta`, Delta samples through `compiler/delta-rs`) | Delta language corpus, Delta-written compiler, and lattice-built artifacts | `bootstrap/rungs/delta/` — moved |

### Assurance and bootstrap Omega

| Canonical or transitional source | Role | Target owner |
| --- | --- | --- |
| `bootstrap/assurance/proof-kernel/{implementations,tools,corpus,gates}/` (compatibility: `compiler/proof-kernel`) | cross-cutting derivation checking, tools, corpora, and gates | moved and split by responsibility |
| `bootstrap/assurance/refinement/{beta,omega0}/` (compatibility entries remain under Alpha and Omega0 gates) | cross-rung source/meaning-to-artifact obligation reconstruction and checking | moved and split by checked edge |
| `bootstrap/omega0/` | Rust-free meaning, first-Omega compiler source/contracts, and gates | moved and split by responsibility |
| `bootstrap/corpus/` (compatibility: `compiler/lattice-corpus`) | fixtures shared across lattice seams | moved |

### Transitional and product implementations

| Canonical or transitional source | Role | Target owner |
| --- | --- | --- |
| `bootstrap/onramps/delta-rust/` (compatibility: `compiler/delta-rs`) | Delta disposable/reference Rust producer | moved and separated from rung ownership |
| `bootstrap/onramps/alpha-assembler-rust/` (compatibility: `compiler/beta-rs`) | disposable/reference Rust producer of Alpha VM tapes from Alpha assembly | moved and separated from Beta-language ownership |
| `bootstrap/onramps/beta-rust/` (compatibility: `compiler/beta-lang-rs`) | Beta-language disposable/reference Rust producer | moved and separated from rung ownership |
| `bootstrap/rungs/beta/reference/` | executable Beta reference meaning and semantic fuzzing | moved; `compiler/beta-lang-py` forwards compatibility entry points |
| `bootstrap/assurance/refinement/beta/` | fragmentary symbolic reconstruction plus whole-artifact obligation checkers | moved |
| `compiler/psi/`, `compiler/omega/` | current production Psi/Omega implementations | `compiler/psi/`, `compiler/omega/` |

## Current architectural state

- Alpha has written small-step semantics, conformance tests, and two independent
  native seeds.
- Beta's `bc.beta` self-hosts. Its fixed point establishes dependency closure;
  the persisted artifact is now reconstructed entirely through Alpha and used by
  downstream gates. Complete lower-rooted validation of that artifact against
  `bc.beta` remains open.
- Gamma's canonical surface is the functional interpreter-first language. The
  imperative `gamma.alpha` language is parked compatibility material.
- The proof kernel is implemented independently in Beta and Gamma. It checks
  generic derivations; terminal-Psi obligation reconstruction is a separate
  artifact-aware responsibility.
- Delta is the final small bootstrap language. Its meaning is exposed by
  elaboration to Gamma; removing the remaining Rust from that trusted route is
  higher priority than removing Rust from untrusted producers.
- The next compiler dependency closure is Delta → bootstrap Omega → production
  Omega. Bootstrap Omega needs correctness and language coverage, not advanced
  optimization.

## Delta → first Omega readiness

**Present status: compiler-capable with the first O0 vertical canary closed, but
not Omega-bootstrap-ready.** Delta has proved that it can host a substantial
compiler and carry one frozen Omega source shape through canonical meaning to a
runnable artifact, but it has not yet implemented the Omega compiler.
`bootstrap/rungs/delta/samples/lowermachine.alp` is a real
Delta-written Delta-to-ARM64 compiler: it self-compiles to a fixed point and its
output is swept against the Rust reference over the sample corpus. This proves
the basic compiler-host vocabulary—mutable arenas, parsing, recursive calls,
sum types, state-machine control flow, byte I/O, and code emission.

That evidence is necessary but is not the first Omega compiler:

- The Delta-written O0 slice implements its frozen lexer, parser, exact
  name/type/count checks, direct canonical terminal-Psi emission, and a direct
  x86-64 ELF backend for one console shape. It is not yet a general Omega
  frontend or a complete Delta-written Omega backend.
- Delta's general self-host path emits ARM64 assembly and still uses external
  `clang` and `codesign`. The exact O0 terminal-to-ELF edge no longer does.
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

### Work packages and acceptance gates

- [ ] **Freeze the two bootstrap profiles.** Record (a) the Delta surface used
  to implement the first Omega compiler and (b) the minimum Omega surface that
  compiler accepts. The Omega profile must be sufficient to express the full
  Omega-source production compiler; it need not accept every convenience before
  that source uses it.
  - [x] Freeze Delta implementation profile D0 and Omega vertical-canary profile
    O0 in `bootstrap/omega0/compiler/BOOTSTRAP_PROFILES.md`.
  - [x] Freeze O1 at 0–16 literal writes, 1 final nonnegative-i32 exit, 2,048
    source bytes, and 1,024 aggregate decoded literal bytes. The same
    table-driven frontend/emitter/backend handles every admitted count.
  - [ ] Freeze the production-self-host Omega profile against the actual Omega
    compiler source tree. No such source tree exists yet, so O0 must not be
    mislabeled as sufficient evidence.
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
- [ ] **Close the general Delta-written artifact path.** Either emit the canonical
  object/image format directly or add a small lattice-built assembler/linker
  path. `clang`/`codesign` may remain development conveniences but cannot be an
  unrecorded dependency of the claimed closed bootstrap.
  - [x] Close the exact O0 canary edge with
    `bootstrap/omega0/compiler/omega0-terminal-to-elf.alp`. It consumes the
    vocabulary-25 O0 terminal shape, retains the variable literal and
    nonnegative `i32` exit operand, and emits a deterministic 8 KiB Linux x86-64
    ELF directly, with no host assembler or linker. The gate proves canonical
    byte identity with the production image, operand-variant emission, and
    empty-output rejection for truncation, fixed-field tampering, and trailing
    input.
  - [ ] Generalize direct object/image emission only as the accepted Omega0
    source profile grows; do not count the fixed O0 decoder as the complete
    compiler backend.
    - [x] Generalize the direct Linux x86-64 image edge for O1. Canonical
      0/1/2/16-write terminal modules reproduce the product images byte for
      byte; 17 writes, 1,200 aggregate bytes, malformed input, and truncation
      reject before emitting any image byte.
- [ ] **Complete meaning for the used Delta profile.** Replace trusted Rust in
  the Delta-to-Gamma route for every construct used by the first Omega compiler,
  including allocation and exhaustion. Preserve native-versus-meaning
  differential gates. Full unused-Delta coverage may proceed separately.
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
  - [ ] Audit the eventual Omega0 Delta source against D0 and make every construct
    either elaborate through the lower-rung route or reject before it can enter
    the compiler. Keep `gamma_emit.rs` only as a reference differential producer.
    - [x] Extend `omega2gamma.beta` for the Delta-written O0 frontend. Native,
      Delta-self-hosted, and lower-rung elaboration/interpreter routes now return
      the retained operand digest 107 for `cli_mvp`; the lower-rung route also
      preserves semantic rejection at 251. The focused gate pins multi-slot
      void/value method-state threading, bounded per-machine capacity, and the
      private chunked carrier used only for compiler-sized scalar arrays.
    - [x] Re-establish the frontend meaning gate for O1. The 40-machine frontend
      now elaborates completely to 112,780 bytes of Gamma in about 0.22 seconds;
      the end-to-end lower-rung gate completes in about 17 seconds and pins the
      retained digest, zero/two-write dual-channel results, semantic rejection,
      and multi-slot method threading. It is part of the default lattice suite.
- [x] **Build a vertical Omega canary in Delta.** A Delta-written program must
  accept a small Omega source file, perform name/type checks, lower through the
  chosen terminal-Psi path, and produce a runnable artifact whose behavior
  agrees with canonical meaning.
  - [x] Freeze the O0 console source contract and implement the Delta streaming
    decoder for its canonical multi-source input artifact.
  - [x] Extend canonical terminal Psi boundary declarations and calls with exact
    scalar parameter/argument lanes. Use ordered scalar parameter types on the
    bodyless declaration and ordered scalar `ValueId` arguments on the call;
    carry them through the checked-plan producer, canonical codec and vocabulary
    bump (vocabulary 23), semantic call schema, verifier, interpreter effect,
    and Omega abstract operation. Target lowering explicitly rejects nonempty
    scalar boundary arguments until a real native realization exists.
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
    - [x] Add the first-class borrowed byte-sequence structural type, canonical
      literal establishment/place, and generalized structural boundary-argument
      source required by `write_line` (terminal vocabulary 25). Local literal
      sources are admitted only at bodyless boundaries; in-module forwarding
      and nonliteral native layout remain fail-closed.
    - [x] Preserve literal bytes exactly in the canonical codec, verifier, and
      interpreter, including non-UTF-8 bytes; fix the Psi lexer so `\xNN` adds
      the requested byte instead of round-tripping it through Unicode. Syntax,
      resolved, typed, and checked representations now own exact byte payloads;
      the checked-to-terminal path establishes the borrowed literal and passes
      the same place to the bodyless boundary call.
    - [x] Preserve the same structural operand through Psi-to-Omega abstract,
      target, assigned, machine, object, image, and installation custody. The
      exact Linux literal-only realization uses import-free `write`, appends one
      newline, retries short writes, and composes with `exit_group` in one Unit
      body on x86-64 and AArch64. Nonliteral forwarding and Darwin/Windows remain
      fail-closed.
    - [x] Represent O0's `Main { console: Console }` attachment honestly. The
      canonical specialization retains `attachment: Some(Main)`, the relevant
      erased `console` provider field, and exact sorted provider roots for every
      bodyless boundary used through that field. The verifier requires exact
      root/call correspondence and rejects missing, ambiguous, forwarded, or
      tampered shapes.
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
    - [x] No standalone semantic-plus-proof envelope is needed for this slice.
      If a later profile needs one, add one generic length-delimited terminal
      envelope rather than an O0-only container.
  - [x] **Close the Delta artifact-publication sink contract.** D0 `write_byte`
    returns `Unit`, so the compiler cannot observe a physical sink/short-write
    failure. The generic terminal-semantic publisher now gives the producer a
    private same-directory staging sink, persists it, requires the declared
    producer exit and successful canonical decode (plus expected semantic
    identity when supplied), and atomically renames only after acceptance.
    Truncation, malformed or substituted meaning, and producer failure preserve
    the previous accepted destination; successful producer exit alone is not
    artifact acceptance.
  - [x] Implement a genuine target `exit_process(i32)` boundary realization.
    Consume the preserved scalar argument; do not reinterpret it as a machine
    return or route it through the metadata-only port settlement.
    - [x] Close the first native slice with the import-free Linux `exit_group`
      ABI (x86-64 first, with AArch64 byte validation where practical). Emit the
      scalar value into the ABI argument register, record the exact consumed
      value and nonempty settlement byte interval, and trap if the nominally
      nonreturning syscall returns.
    - [x] Keep Darwin and Windows fail-closed until terminal images can carry and
      independently validate the required external import and relocation
      evidence. Their hosted `_exit`/`ExitProcess` paths are not aliases for the
      import-free Linux realization.
  - [x] Gate the runnable O0 artifact: exact output plus newline, requested
    low-byte exit status, deterministic bytes, and canonical-meaning agreement.
    The published vocabulary-25 fixture is decoded and verified with the empty
    proof bundle, executed by canonical terminal meaning, and lowered with
    exact requirement-matched provider executions to deterministic Linux x86-64
    and AArch64 images plus replayed installation records. Matching Linux hosts
    execute the image and compare stdout/status; other hosts validate both
    complete image formats without pretending to execute them.
- [ ] **Implement the first Omega compiler in Delta.** Grow the canary into the
  deliberately simple, spec-compliant compiler. Prefer direct and auditable
  stages over porting the production optimizer or the entire current Rust
  architecture.
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
- [ ] **Validate Delta → Omega.** Gate representative language coverage,
  negative diagnostics, deterministic artifacts, meaning agreement, and the
  relevant proof/translation-validation seams.
- [ ] **Compile production Omega from Omega source.** Use the Delta-built
  compiler to produce the optimized Omega compiler, then validate the self-build
  edge against canonical meaning. The Delta-built compiler remains a supported
  slow, unoptimized endpoint.
  - [ ] Establish the production compiler as an Omega source tree. The current
    compiler implementation is Rust; standard-library and sample `.omg` files
    are not a compiler source tree and cannot define the self-host profile.
  - [ ] Derive and freeze the production-self-host acceptance profile from that
    exact source tree, then make bootstrap Omega accept it without importing the
    production optimizer into Delta.
  - [ ] Build and validate both artifacts explicitly: Delta-built simple Omega,
    then Omega-built optimizing Omega. Stopping after the first remains a valid
    supported configuration.

The first vertical canary is closed through a direct, lattice-written x86-64
ELF. The next evidence boundary is to grow that frozen slice into the
deliberately simple Omega compiler while widening direct artifact emission and
the used-Delta meaning profile only as the compiler source requires. Those
requirements, rather than a wholesale Delta redesign, determine which
additional facilities the bootstrap actually needs.

## Repository-structure work packages

- [ ] **Close the `bc` source-correspondence edge without DDC.** The seed Beta
  compiler is now built through the preceding audited rung; validate the
  complete artifact against `bc.beta` using authority rooted below `bc`.
  Fixed-point or cross-compiler byte agreement is not acceptance evidence.
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
      the exact 262,140-byte tape-hole payload. This does not yet prove dynamic
      memory bounds, call/return discipline, output semantics, or termination.
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
- [x] **Make gate paths relocatable.** Replace hard-coded sibling-relative paths
  with a single repository-root/path helper so ownership moves can be mechanical
  and independently reviewable.
  - [x] Convert all executable gates as one mechanical checkpoint; partial
    conversion does not unlock a move. Include `verify-lattice.sh`, its stable
    cache location, and the cwd-sensitive Python refinement helpers.
  - [x] Add a static path-hygiene gate and verify representative gates from both
    the repository root and an unrelated working directory before moving files.
- [x] **Create the `bootstrap/` ownership root.** Move rungs first without
  changing behavior; retain temporary compatibility wrappers where external
  entry points require them.
  - [x] Move both the native seed/written Alpha semantics from `compiler/alpha/`
    and the Alpha-written Alpha assembler from historical `compiler/beta/` to
    `bootstrap/rungs/alpha/`. The directory name must not assign the assembler
    to the Beta rung. Compatibility symlinks preserve the old entry points;
    canonical gates and dependency hashes use the `alpha-assembler` role.
  - [x] Move the Beta language and self-hosting compiler from
    `compiler/beta-lang/` to `bootstrap/rungs/beta/` independently of the Alpha
    assembler compatibility path. Canonical gates use the `beta` path role;
    `compiler/beta-lang` remains a compatibility symlink.
  - [x] Move canonical Gamma language/interpreter/type-checker ownership to
    `bootstrap/rungs/gamma/`. First classify the parked imperative
    `gamma.alpha` compatibility implementation and the terminal-ledger spike so
    neither is accidentally promoted as Gamma language meaning.
    `compiler/gamma` is now a compatibility symlink. The imperative compiler is
    explicitly parked compatibility material; the ledger spike is a bounded
    artifact-assurance experiment whose execution by Gamma does not make it a
    Gamma language definition.
    - [x] Retire the frozen terminal-ledger feasibility gate rather than rebasing
      its stale 5,000-line monomorphic prototype from terminal format
      18/vocabulary 20 to the live format 22/vocabulary 25. Production's closed
      36-leaf/4-call semantic tables and mutation gates retain the reusable
      schema shape; the historical low-rung feasibility result remains recorded
      in the terminal-Psi architecture document and commit `a5cfd83cc`. The
      deferred lattice branch is removed. The legacy-named product fixture test
      remains temporarily because it is the codec's only round-trip coverage for
      29 operation variants; preserve that evidence when decomposing or renaming
      it with the product-root migration.
      Reusable typed scalar/type/value/UTF-8/structural grammar fragments remain
      gated independently; only their stale fixed-version header was retired.
  - [x] Split Delta by role: lattice-built language/compiler sources and
    artifacts belong under `bootstrap/rungs/delta/`; the Rust producer belongs
    under `bootstrap/onramps/delta-rust/`. Preserve focused compatibility entry
    points while gates switch to path roles.
  - [x] Move the remaining Rust Alpha/Beta producers under
    `bootstrap/onramps/`, separated by the artifact they produce. Their host
    language must not define their architectural ownership.
    - [x] Move historical `compiler/beta-rs` to
      `bootstrap/onramps/alpha-assembler-rust/`. It produces Alpha VM tapes from
      Alpha assembly and has no Beta-language role; the old path and `beta-rs`
      role are compatibility aliases. Its focused gate compares the Rust output
      with the lattice-built assembler and pins malformed-input rejection.
    - [x] Move historical `compiler/beta-lang-rs` to
      `bootstrap/onramps/beta-rust/`. It produces Alpha assembly from Beta
      source and has no ownership over the Beta language rung; the old path,
      `beta-lang-rs` role, and `OMEGA_PATH_BETA_RUST` variable are compatibility
      aliases. Canonical gates use the `beta-rust` role and
      `OMEGA_PATH_BETA_COMPILER_RUST`.
  - [x] Consolidate remaining cross-rung refinement reconstruction under
    `bootstrap/assurance/refinement/`; leave rung-local semantics and
    conformance gates with their rung. The Beta source/Alpha artifact symbolic
    evaluators, fixtures, generators, and gates now share the `beta/` owner;
    Omega0 meaning/translation-validation encoders and certificate replay live
    under `omega0/`. Alpha retains its VM semantics, executable reference,
    opcode conformance, seed fuzzing, and assembler gates. Omega0 retains its
    meaning elaborator, compiler/bundle/artifact gates, convergence checks, and
    meaning-route conformance. Historical Alpha and Omega0 gate paths are
    compatibility symlinks only.
- [x] **Split proof-kernel responsibilities.** Separate Beta/Gamma/reference
  checker implementations, untrusted proof tooling, corpora, and gates under
  `bootstrap/assurance/proof-kernel/`.
  - [x] Move the generic proof-kernel tree to its assurance owner and preserve
    `compiler/proof-kernel` only as a compatibility symlink. Canonical path
    roles and lattice hashes no longer treat it as a compiler rung.
  - [x] Separate checker implementations, tools, corpora, and gates inside the
    assurance owner; move the Gamma checker implementation from its transitional
    language-rung path without changing Gamma's language semantics.
- [x] **Split `beta-lang-py` by role.** Retain the interpreter, symbolic evaluator,
  and useful fuzzing under Beta/refinement owners; remove compiler-comparison
  code that provides no unique semantic or refinement coverage.
  - [x] Extract source recognition into `beta_parser.py`. The interpreter,
    symbolic evaluator, exhaustive-I/O checker, and loop-summary checker now
    share it without importing compiler code. The reference and refinement
    owners import the same recognizer without acquiring a compiler dependency.
  - [x] Remove the retired DDC comparison gate. Compiler diversity is neither a
    repository role nor a prerequisite for closing the `bc` refinement edge.
  - [x] Move executable reference meaning and semantic fuzzing to
    `bootstrap/rungs/beta/reference/`; move symbolic reconstruction to
    `bootstrap/assurance/refinement/beta/`; retain only compatibility wrappers
    under `compiler/beta-lang-py/`.
  - [x] Remove `bc2.py` and its comparison-only gate after confirming they add
    no unique semantic, refinement, or lattice coverage.
- [x] **Move first-Omega work out of the product namespace.** Place the existing
  Rust-free meaning route and future Delta compiler source in
  `bootstrap/omega0/`, split into `meaning/`, `compiler/`, and `gates/`.
  - [x] Promote the Delta-written O0/O1 frontend from the transitional
    `compiler/delta-rs/samples/omega0-frontend.alp` path to
    `bootstrap/omega0/compiler/`; retain a compatibility entry only while Delta
    on-ramp gates still require it.
- [x] **Move the shared lattice corpus to `bootstrap/corpus/`.** Proof-kernel,
  Omega0, and Delta gates use the canonical `corpus` role; the historical
  `compiler/lattice-corpus` path is a compatibility symlink only.
- [x] **Rename product roots last.** The production implementations now occupy
  the physical, role-based `compiler/psi/` and `compiler/omega/` roots. Cargo
  paths, repository-aware tests, path ownership, and documentation moved in the
  same checkpoint. The conflicting Omega0 compatibility directory was retired;
  its canonical owner remains `bootstrap/omega0/`. Product ownership no longer
  encodes the current host implementation language.

## Execution order

1. Close the Alpha-rooted `bc` source-correspondence edge with lower-rooted
   checking.
2. Finish Delta's Rust-free meaning route and preserve the native/meaning
   differential gates.
3. Grow proof-kernel capability and its operational seams only in lockstep with
   real obligation classes.
4. Build translation-validation evidence for native compiler outputs.
5. Grow the closed O0 vertical canary—source through direct ELF—into the
   deliberately simple, spec-compliant Omega compiler.
6. Use the resulting bootstrap Omega compiler to build and validate the full
   optimizing Omega compiler from Omega source.

This ordering follows D1–D6. Producer optimization does not outrank removal of a
trusted Rust meaning or verification dependency.

## Principal gates

Run from the repository root:

```sh
sh compiler/verify-lattice.sh
sh bootstrap/rungs/alpha/assembler/selfhost.sh
sh bootstrap/onramps/alpha-assembler-rust/test.sh
sh bootstrap/onramps/beta-rust/test.sh  # diagnostic producer only
sh bootstrap/rungs/beta/cold-start/test.sh
sh bootstrap/rungs/beta/cold-start/full-source.sh
sh bootstrap/rungs/beta/source-exhaustion.sh
sh bootstrap/assurance/refinement/beta/bc-artifact-structure.sh
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
