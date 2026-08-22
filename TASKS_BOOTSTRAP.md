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

| Current path | Role | Target owner |
| --- | --- | --- |
| `compiler/alpha/` | 21-opcode native seed VM and written semantics | `bootstrap/rungs/alpha/` |
| `compiler/beta/`, `compiler/beta-lang/` | Alpha assembler plus Beta language/compiler | `bootstrap/rungs/beta/` |
| `compiler/gamma/` | Gamma language, interpreter, and type checker | `bootstrap/rungs/gamma/` |
| lattice-built sources/artifacts in `compiler/delta*/` | Delta language and compiler | `bootstrap/rungs/delta/` |

### Assurance and bootstrap Omega

| Current path | Role | Target owner |
| --- | --- | --- |
| `compiler/proof-kernel/` plus Gamma checker sources | cross-cutting derivation checking, tools, corpora, and gates | `bootstrap/assurance/proof-kernel/`, split by implementation/tool/corpus/gate |
| refinement tooling spread across `alpha/`, `omega/`, and Python helpers | source-to-artifact obligation reconstruction and checking | `bootstrap/assurance/refinement/` |
| `compiler/omega/` | Rust-free first-Omega meaning/refinement experiments | `bootstrap/omega0/` |
| `compiler/lattice-corpus/` | fixtures shared across lattice seams | `bootstrap/corpus/` |

### Transitional and product implementations

| Current path | Role | Target owner |
| --- | --- | --- |
| `compiler/beta-lang-rs/`, Rust producer portion of `compiler/delta-rs/` | disposable/reference on-ramps | `bootstrap/onramps/` |
| semantic and symbolic portions of `compiler/beta-lang-py/` | Beta meaning/refinement references | Beta reference or refinement owner, not a Python-named peer rung |
| `compiler/beta-lang-py/bc2.py` and DDC gate | legacy compiler-comparison scaffolding | archive/remove after the `bc` cold-start edge is checked |
| `compiler/psi-rs/`, `compiler/omega-rs/` | current production Psi/Omega implementations | `compiler/psi/`, `compiler/omega/` |

## Current architectural state

- Alpha has written small-step semantics, conformance tests, and two independent
  native seeds.
- Beta's `bc.beta` self-hosts. Its fixed point establishes dependency closure;
  complete lower-rooted validation of the Rust-cold-started `bc` artifact
  against `bc.beta` remains open.
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

**Present status: compiler-capable, not Omega-bootstrap-ready.** Delta has proved
that it can host a substantial compiler, but it has not yet proved that it can
host the Omega compiler. `compiler/delta-rs/samples/lowermachine.alp` is a real
Delta-written Delta-to-ARM64 compiler: it self-compiles to a fixed point and its
output is swept against the Rust reference over the sample corpus. This proves
the basic compiler-host vocabulary—mutable arenas, parsing, recursive calls,
sum types, state-machine control flow, byte I/O, and code emission.

That evidence is necessary but is not the first Omega compiler:

- There is no Delta source implementing an Omega lexer, parser, checker,
  terminal-Psi lowering, or backend.
- The Delta-written native path emits ARM64 assembly and still uses external
  `clang` and `codesign` to obtain a runnable image.
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
    O0 in `compiler/omega/BOOTSTRAP_PROFILES.md`.
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
- [x] **Choose and gate source packaging.** `compiler/omega/OMEGA0_BUNDLE.md`
  defines the canonical, length-delimited version-1 multi-source artifact.
  Its gate covers deterministic ordering, exact byte preservation, canonical
  paths, and malformed/truncated input rejection. The packer is untrusted; the
  Delta streaming decoder canary implements the same acceptance contract with
  explicit local-storage exhaustion and is gated natively and through the
  Rust-free meaning route.
- [ ] **Close the Delta-written artifact path.** Either emit the canonical
  object/image format directly or add a small lattice-built assembler/linker
  path. `clang`/`codesign` may remain development conveniences but cannot be an
  unrecorded dependency of the claimed closed bootstrap.
- [ ] **Complete meaning for the used Delta profile.** Replace trusted Rust in
  the Delta-to-Gamma route for every construct used by the first Omega compiler,
  including allocation and exhaustion. Preserve native-versus-meaning
  differential gates. Full unused-Delta coverage may proceed separately.
  - [x] Evaluate D0 fixed-backing allocation and exhaustion through the
    Beta-written `omega2gamma.beta` elaborator and Gamma interpreter, with a
    source perturbation that changes the observed result.
  - [x] Exercise byte input/output and real Delta certifiers through the same
    Rust-free route in `compiler/omega/convergence-reference.sh`.
  - [ ] Audit the eventual Omega0 Delta source against D0 and make every construct
    either elaborate through the lower-rung route or reject before it can enter
    the compiler. Keep `gamma_emit.rs` only as a reference differential producer.
    - [ ] Extend `omega2gamma.beta` for the Delta-written O0 frontend. The native
      on-ramp and Delta-written `lowermachine` both return its retained operand
      digest 107 for `cli_mvp`; the current lower-rung elaboration/interpreter
      route returns rejection 251 for that same canonical bundle. Preserve this
      as an explicit meaning-coverage gap rather than weakening the frontend.
- [ ] **Build a vertical Omega canary in Delta.** A Delta-written program must
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
    acceptance and rejection. It
    retains the decoded `write_line` carrier and `exit_process` literal and
    exposes their digest until terminal-Psi emission consumes them.
  - [ ] Emit the O0 terminal-Psi semantic artifact while retaining
    `write_line`'s exact structural byte carrier and custody through its
    boundary call. This is implementation work, not an unresolved language
    ruling, and it must use the shared terminal representation rather than an
    O0-private IR.
    - [ ] Add the first-class borrowed byte-sequence structural type, canonical
      literal establishment/place, and generalized structural boundary-argument
      source required by `write_line` (terminal vocabulary 24).
    - [ ] Preserve literal bytes exactly in the canonical codec, verifier, and
      interpreter, including non-UTF-8 bytes; fix the Psi lexer so `\xNN` adds
      the requested byte instead of round-tripping it through Unicode.
    - [ ] Preserve the same structural operand through Psi-to-Omega abstract
      lowering. Keep native target lowering fail-closed until `write_line` has a
      real target realization.
    - [ ] Represent O0's `Main { console: Console }` attachment honestly. Either
      verify a canonical specialization that erases the dynamic-trait field
      into the retained boundary requirement plus provider-installation seam,
      or carry the provider-backed attachment descriptor; do not substitute an
      empty record or `attachment: None` while claiming the original shape.
    - [ ] Emit canonical terminal semantic bytes directly from Delta and gate
      them through the shared codec/verifier with the canonical empty proof
      bundle for proof-free O0. Do not route this milestone through the Rust
      checked-plan producer trees.
    - [ ] If a standalone semantic-plus-proof file becomes necessary, add one
      generic length-delimited terminal envelope; do not invent an O0-only
      container. The proof-free semantic-slice gate does not depend on this
      packaging work.
  - [ ] Implement a genuine target `exit_process(i32)` boundary realization.
    Consume the preserved scalar argument; do not reinterpret it as a machine
    return or route it through the metadata-only port settlement.
    - [ ] Close the first native slice with the import-free Linux `exit_group`
      ABI (x86-64 first, with AArch64 byte validation where practical). Emit the
      scalar value into the ABI argument register, record the exact consumed
      value and nonempty settlement byte interval, and trap if the nominally
      nonreturning syscall returns.
    - [ ] Keep Darwin and Windows fail-closed until terminal images can carry and
      independently validate the required external import and relocation
      evidence. Their hosted `_exit`/`ExitProcess` paths are not aliases for the
      import-free Linux realization.
  - [ ] Gate the runnable O0 artifact: exact output plus newline, requested
    low-byte exit status, deterministic bytes, and canonical-meaning agreement.
- [ ] **Implement the first Omega compiler in Delta.** Grow the canary into the
  deliberately simple, spec-compliant compiler. Prefer direct and auditable
  stages over porting the production optimizer or the entire current Rust
  architecture.
- [ ] **Validate Delta → Omega.** Gate representative language coverage,
  negative diagnostics, deterministic artifacts, meaning agreement, and the
  relevant proof/translation-validation seams.
- [ ] **Compile production Omega from Omega source.** Use the Delta-built
  compiler to produce the optimized Omega compiler, then validate the self-build
  edge against canonical meaning. The Delta-built compiler remains a supported
  slow, unoptimized endpoint.

The first vertical canary—not a wholesale Delta redesign—is the next evidence
boundary. It will distinguish facilities the Omega bootstrap actually needs
from attractive but deferrable language work.

## Repository-structure work packages

- [ ] **Close the `bc` cold-start edge without DDC.** Build the seed Beta
  compiler through the preceding audited rung or validate the complete artifact
  against `bc.beta` using authority rooted below `bc`. Fixed-point or
  cross-compiler byte agreement is not acceptance evidence.
  - [ ] Specify the compiler observable as the complete output byte stream plus
    halt, trap, divergence, and checked resource exhaustion—not merely an exit
    byte or a finite set of executions.
  - [ ] Implement the exact `bc.beta` bootstrap profile in an Alpha-written Beta
    compiler assembled and run only through the audited Alpha/Beta seed path.
    The current Python symbolic model cannot cover `bc.beta`'s data-dependent
    branching, word memory, or full-stream emission and is not this authority.
  - [ ] Persist the resulting lattice-built `bc` artifact, run its self-build and
    Beta corpus gates, then switch proof-kernel, Gamma, Delta, and Omega gates
    away from the ephemeral Rust-produced `bc0`.
  - [ ] Enlarge or replace the x64 seed's current 32 KiB image extent before
    claiming cross-platform closure; the present self-hosted tape is about 48
    KiB. The arm64 seed's 256 KiB extent is sufficient for the current artifact.
- [x] **Make gate paths relocatable.** Replace hard-coded sibling-relative paths
  with a single repository-root/path helper so ownership moves can be mechanical
  and independently reviewable.
  - [x] Convert all executable gates as one mechanical checkpoint; partial
    conversion does not unlock a move. Include `verify-lattice.sh`, its stable
    cache location, and the cwd-sensitive Python refinement helpers.
  - [x] Add a static path-hygiene gate and verify representative gates from both
    the repository root and an unrelated working directory before moving files.
- [ ] **Create the `bootstrap/` ownership root.** Move rungs first without
  changing behavior; retain temporary compatibility wrappers where external
  entry points require them.
- [ ] **Split proof-kernel responsibilities.** Separate Beta/Gamma/reference
  checker implementations, untrusted proof tooling, corpora, and gates under
  `bootstrap/assurance/proof-kernel/`.
- [ ] **Split `beta-lang-py` by role.** Retain the interpreter, symbolic evaluator,
  and useful fuzzing under Beta/refinement owners. Archive or remove `bc2.py` and
  the DDC comparison after the `bc` cold-start edge closes.
- [ ] **Move first-Omega work out of the product namespace.** Place the existing
  Rust-free meaning/refinement experiments and future Delta compiler source in
  `bootstrap/omega0/`.
- [ ] **Rename product roots last.** Move `psi-rs`/`omega-rs` to role-based
  `compiler/psi`/`compiler/omega` only after Cargo paths and documentation can be
  changed atomically. The architecture must not depend on their current host
  implementation language.

## Execution order

1. Close the `bc` cold-start source-to-artifact edge with lower-rooted checking;
   the current DDC comparison is not the closure criterion.
2. Finish Delta's Rust-free meaning route and preserve the native/meaning
   differential gates.
3. Grow proof-kernel capability and its operational seams only in lockstep with
   real obligation classes.
4. Build translation-validation evidence for native compiler outputs.
5. Execute the Delta → first Omega work packages above, beginning with the two
   profiles, scalable storage, and one vertical Omega canary.
6. Use the resulting bootstrap Omega compiler to build and validate the full
   optimizing Omega compiler from Omega source.

This ordering follows D1–D6. Producer optimization does not outrank removal of a
trusted Rust meaning or verification dependency.

## Principal gates

Run from the repository root:

```sh
sh compiler/verify-lattice.sh
sh compiler/beta/selfhost.sh
sh compiler/beta-lang/selfhost.sh
sh compiler/beta-lang/test.sh
sh compiler/gamma/test-interp.sh
sh compiler/gamma/test-typeck.sh
sh compiler/gamma/test-checker.sh
sh compiler/proof-kernel/test.sh
```

The current Delta-written ARM64 path additionally uses these platform-specific
gates (and requires the external assembler/linker/signing tools noted above):

```sh
sh compiler/delta-rs/test_aarch64.sh
sh compiler/delta-rs/selfhost-sweep.sh
sh compiler/delta-rs/delta-meaning-diamond.sh
sh compiler/delta-rs/convergence-selfhost.sh
```

The current Rust-free Omega kernel/meaning experiments are gated separately:

```sh
sh compiler/omega/kernel-diamond.sh
sh compiler/omega/omega-meaning.sh
sh compiler/omega/meaning-cert-diamond.sh
sh compiler/omega/translation-validation.sh
```

## Persistent implementation facts

- The Alpha tape hole is platform-specific: arm64 macOS currently has 256 KiB;
  x64 Windows retains 32 KiB until its seed is rebuilt. Build scripts read
  `compiler/alpha/seed_env.sh` rather than assuming one universal size.
- The Alpha VM hidden stack stores return addresses. Beta maintains a separate
  explicit data stack with `r15` as stack pointer and `r14` as frame pointer.
- Build fixed points establish determinism and dependency closure, not compiler
  correctness.
- Rust reference producers may remain during migration. Rust in meaning,
  artifact reconstruction, or proof checking remains an explicit trusted
  dependency until replaced by the audited route.
- Exact corpus counts and “N cases passed” snapshots are intentionally omitted
  here because they drift; the gate output is authoritative.
