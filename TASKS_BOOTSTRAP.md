# Direct compiler lattice — active work

Last pruned: 2026-08-28.

This queue exists to construct exactly one sequence:

```text
audited Alpha VM seed
  → Alpha-written Beta compiler       → beta_compiler_bytecode.tape
  → Beta-written Gamma compiler       → gamma_compiler_bytecode.tape
  → Gamma-written Delta compiler      → delta_compiler_bytecode.tape
  → Delta-written full Omega D        → omega0_compiler_bytecode.tape
  → Omega-written full Omega C        → omega_compiler_bytecode.tape
```

Every compiler artifact is platform-independent Alpha tape. The host-specific
Alpha VM seed is the sole native bootstrap component. `D` and `C` are different
source closures implementing the same complete Omega language; the first may
optimize poorly, while the second is the production self-host.

There is no DDC stage, `omega-bootstrap` language, Delta-to-Gamma bridge,
native Beta/Gamma/Delta compiler, checkpoint generation, or executable proof
kernel rung. Psi is an internal product compiler boundary, not part of this
queue.

## Retention and deletion policy

Repository-owned material starts with a maintenance liability, not a
presumption that keeping it is harmless. It remains only when it directly
specifies, implements, proves, or efficiently tests one canonical edge. A
retained migration component must have a named adaptation below, a canonical
owner, and a deletion condition. “Potentially useful,” historical continuity,
and the cost already spent are not retention arguments. If direct adaptation
fails, becomes uneconomical, or leaves a parallel source of truth, delete the
component. Git history is the archive; the working repository is exclusively
the implementation of the agreed chain.

- [x] Delete the Beta-written Delta-to-Gamma translator, its host encoders and
  decoder, and the entire Darwin-native Delta publication/custody apparatus.
  They crossed the immediate-predecessor boundary and established the wrong
  artifact identity.
- [x] Delete the restricted Delta-written native compiler prototype rather than
  relabeling it as `D`. Its monolithic single-source frontend and Darwin ARM64
  backend implemented neither the Gamma-written Delta edge nor full Omega, and
  no unit-level adaptation was economical. Also delete the 31 `certify-*`
  proof-application programs; they serialized checker certificates but did not
  state Delta semantics or test the replacement compiler.
- [x] Audit every remaining bootstrap viewer, generated report, repeated-run
  receipt, wrapper, fixed-point gate, and differential implementation. Give it
  one bounded diagnostic or canonical-edge role, or delete it. No viewer,
  report, receipt, `bootstrap/`, or canary tree remains in the Alpha–Delta
  lattice. Retained wrappers now divide into exact seed/assembler construction,
  below-Beta checker construction and soundness tests, exact seed/assembler and
  Beta artifact reconstruction, one structure check, one Alpha-written exact
  encoding reconstructor, and a bounded compiler differential. The duplicate
  Beta self-host wrapper was deleted.

## Non-negotiable edge contract

For each compiler edge, bind:

- the exact immediate-predecessor source closure;
- the exact emitted Alpha tape;
- the source and Alpha semantic versions;
- input, observation, and resource profiles;
- independently reconstructed obligations and checked derivations; and
- disclosed VM/hardware realization admissions.

Later fixed points, byte equality, another compiler's agreement, fuzzing, or a
second execution cannot repair a missing proposition. Shell and Python may
invoke, stamp, compare, and report. They may not parse accepted source, lower
code, discover a closure, manufacture proof premises, or decide admission.

## Edge status

| Edge | Reusable work | Missing canonical result |
| --- | --- | --- |
| Alpha seed | written semantics, two native seeds, assembler, checker | keep trust floor small and exact |
| Alpha-written Beta compiler | canonical `beta_compiler.alpha` and direct tape artifact | close remaining language/resource checks and exact source-to-tape refinement |
| Beta-written Gamma compiler | `interp.beta`, `typeck.beta`, Gamma semantics/tests | standalone Gamma-to-Alpha compiler tape and refinement |
| Gamma-written Delta compiler | Delta contract and test corpus | compiler source, tape, and refinement |
| `D → omega₀` | full Omega/Rust implementation as a nonauthoritative reference | correctly owned complete Delta closure `D`, full Omega acceptance, tape, and refinement |
| `C → omega` | Omega/Psi product work and Rust comparator | exact Omega closure, self-build tape, and independent refinement |

## 0. Make the repository tell the truth

- [ ] Apply the target layout atomically:

  ```text
  source/beta/compiler/beta_compiler.alpha
  source/gamma/compiler/gamma_compiler.beta
  source/delta/compiler/delta_compiler.gamma
  source/omega/omega_compiler.delta       # D
  source/omega/{build,main}.omg            # C roots
  ```

  Each owner contains its descriptive `.tape` artifact and adjacent validation.
  Do not create generic `bootstrap/`, `on-ramp/`, `assurance/`, `canaries/`, or
  generation directories. `omega₀` and `omega` are artifacts, not languages or
  source owners.
  - [x] Move the existing Beta tape adjacent to `beta_compiler.alpha`, delete
    its otherwise content-free `artifacts/` bucket, and make path hygiene reject
    nested artifact buckets for every canonical compiler owner.
- [ ] Update path-hygiene and lattice runners to enumerate only the canonical
  owners above. They must fail if a lower rung imports source or a semantic
  executable from beyond its immediate successor.
- [ ] Make retention mechanically auditable: every non-specification subtree
  under the six canonical owners must name its canonical edge and bounded
  failure-detection or proof role in the adjacent README. Delete unowned
  wrappers, comparators, corpora, reports, and generators; do not create an
  indefinite “diagnostic” exemption.
- [x] Make every rung/compiler README distinguish the language accepted by a compiler from
  the language in which it is implemented. The source suffix names the latter;
  the owner directory names the former. The Alpha/Beta/Gamma/Delta/Omega roots,
  compiler owners, rung pages, repository map, and chain manifest now use this
  distinction consistently; paths that still contradict it are migration tasks
  above rather than alternate roles.

## 1. Alpha execution floor

- [x] Keep `source/alpha/SEMANTICS.md`, the audited seed implementations, and
  conformance tests synchronized. A seed consumes an exact length-prefixed tape
  and exposes the exact Alpha observation model. `source/alpha/verify.sh --edge`
  currently passes all 25 conformance cases and exact assembler reconstruction.
- [x] Treat tape stamping as transparent packaging. No Mach-O, PE, ELF, code
  signature, linker receipt, or installation inventory becomes compiler
  identity above the seed. Canonical locators and manifests identify tapes;
  `seed_env.sh` only constructs disposable execution containers.
- [x] Keep the root derivation checker separate from the VM and assembler. Its
  calculus may check every compiler edge, but the checker is not a language
  rung and never decides artifact-specific obligations by itself. It is owned
  by `source/alpha/checker/` and reconstructs independently below Beta.
- [x] Ratify the performance boundary: if execution speed becomes unacceptable, first profile the VM and tape.
  A general checked Alpha-to-native realization may be proposed; source-,
  function-, hash-, or workload-specific jets are forbidden. No current floor
  measurement triggers escalation: the complete Alpha-written Beta compiler
  surface gate runs 121 cases in under three seconds on the development host.
  The largest current retained Beta output, the 199,770-byte checker tape,
  leaves 62,370 bytes in the Alpha payload after replacing repeated inline
  stack-fault blocks with one local terminal block per procedure.

## 2. Alpha-written Beta compiler

- [ ] **ADMIT-ALPHA-BETA-COMPILER.** Audit the canonical
  `source/beta/compiler/beta_compiler.alpha` against the complete Beta v1
  contract. Its exact directly assembled Alpha tape is now the canonical Beta
  compiler artifact. It must accept arbitrary valid Beta within explicit
  resource bounds and reject or return `Incomplete` fail-closed.
  - [x] Remove pinned syntax/runtime defects found by the general-source audit:
    full-range Word literals, zero final fallthrough, `r13=8` stack convention,
    reserved intrinsic names, and disjoint callable procedure regions. The
    focused suite now passes 121 cases and the canonical tape passes the generic
    structural checker.
  - [x] Replace emitted Alpha text plus an external assembler invocation with
    direct Alpha tape emission inside the compiler. The Alpha assembler may
    construct the compiler artifact, but it cannot remain a semantic stage when
    the compiler processes Beta input. The compiler now reserves and encodes a
    private bounded tape, resolves procedure/state/internal fixups, and publishes
    only after complete replay. The former full self-host source was byte-identical to
    the removed text-plus-assembler route; the direct encoder then deliberately
    corrected that assembler's signed-division bug for high-bit `u64` immediate
    bytes. The canonical tape passes the generic structural gate. Every
    production consumer now uses its direct tape output.
  - [ ] **DESIGN-BLOCKED — Q15:** Enforce Beta definite initialization across
    state/transition CFGs after fixing the flat-block formation and guarded-edge
    well-formedness rules. A source-order symbol-table pass alone does not prove
    initialization on every path; the byte-vector must-analysis and bounded
    table/worklist layout are otherwise fully specified implementation work.
  - [x] Separate source-visible raw Beta memory from generated frame/expression
    stacks and bind the call/stack profile that proves non-aliasing. Raw memory
    is a checked, zeroed 32 MiB logical region biased above the data stack. Every
    generated frame/expression reservation is guarded at 262144; the mandatory
    frame word bounds semantic depth and leaves the hidden Alpha return stack
    above 66,322,424 even at the failing edge. A 64-slot recursive stress case
    reaches fail-closed status 250 without output or aliasing.
  - [ ] **DESIGN-BLOCKED — Q16:** Project malformed source and each private
    capacity failure to exact, typed no-partial-artifact outcomes. The Alpha
    boundary currently exposes raw success bytes plus a halt code and has no
    selected canonical carrier for `Complete`, `Reject`, `Incomplete`, and
    internal failure; choosing its framing locally would invent edge semantics.
- [x] Redirect the existing cold construction, exact-tape comparison, and
  focused language tests to the Alpha source subject. Remove any two-stage
  “cold compiler builds a Beta self-host, then that self-host becomes canonical” logic. The
  persisted artifact is now the direct assembly of
  `beta_compiler.alpha`; checker, Gamma, reference, and seed-diamond consumers
  no longer invoke an assembler after compiling Beta.
- [x] Reassess the large historical self-host refinement/admission tree module by module.
  Adapt general Alpha-machine decoding and proof-DAG machinery to the actual
  Alpha-written compiler edge. Delete source-specific
  machinery that exists only to prove the noncanonical Beta fixed point.
  The retained surface is one generic artifact-structure check and a reduced
  bounded symbolic differential. The toy FOL seam, source-only loop checker,
  duplicated Alpha/checker fixtures, and redundant symbolic cases were deleted;
  they reconstructed no canonical source/tape proposition or duplicated cheaper
  owners. About 65,000 historical source-specific lines had already been removed.
- [x] Delete the historical Beta self-host after promotion. Its full-source
  migration comparison helped pin the direct emitter, but it had zero remaining
  executable consumers and no bounded comparison gate; constructing a new gate
  merely to justify retention would reverse the repository policy. Its fixed
  point and source now survive only in Git history.
- [ ] Close exact Alpha-assembly-source-to-Alpha-tape correspondence. First
  specify the authoritative assembly grammar and two-pass encoding, then bind
  the exact raw `beta_compiler.alpha` and tape subjects and check that every
  source span, instruction, label fixup, `db` row, and artifact byte belongs to
  one total encoding partition with no gaps or extras. Exercise source-byte,
  tape-byte, label-target, and extent mutations and measure certificate size and
  checking time. Exact tape equality transports through deterministic Alpha
  semantics in lockstep, preserving every defined termination, trap, output,
  resource, and divergence observation; this first edge needs no stuttering
  rank or new trusted LTS rule. Correctness of the compiler for arbitrary Beta
  source is a separate `ADMIT-ALPHA-BETA-COMPILER` obligation.
  - [x] Freeze `source/alpha/ASSEMBLY.md`: byte-stream lexical form with
    arbitrary ignored comment payloads, exact operand grammar, full
    opcode/width table, string decoding, absolute label meaning, deterministic
    two-pass encoding, and the raw-payload/container boundary. Close the Alpha
    assembler and independent reference implementation over that grammar while
    retaining their byte-identical fixed point.
  - [x] Land the Alpha-written, subject-bound whole-source encoding
    reconstructor and mutation controls against the exact 78,109-byte source
    and 20,977-byte tape. It is a 6,993-byte Alpha tape; its 12-control gate runs
    in under one second and covers source/tape bytes, fixups, extents, and closed
    grammar failures without writing output.
  - [ ] Turn the reconstructed ground judgment into a derivation certificate
    checked by `source/alpha/checker/`; the status-only reconstructor does not
    itself admit the edge.

## 3. Beta-written Gamma compiler

- [ ] **DESIGN-BLOCKED — Q14: BUILD-GAMMA-COMPILER.** Define the complete Gamma source contract, then
  implement `source/gamma/compiler/gamma_compiler.beta` as a standalone
  compiler from Gamma source to Alpha tape. It may reuse or reorganize
  `interp.beta` and `typeck.beta`; no external interpreter may remain part of
  compilation. Q14 must first select one typed executable grammar, entry/stream
  ABI, outcome model, and fuel/resource meaning. The current interpreter and
  type checker implement disconnected untyped-executable and typed-nonexecuting
  languages, so choosing either in code would invent Gamma semantics.
- [ ] Keep `interp.beta` and `typeck.beta` only as reusable compiler components
  or bounded semantic oracles. Delete either if the canonical compiler subsumes
  its useful failure detection.
- [ ] Check the exact Beta-source-to-Alpha-tape refinement and all resource
  outcomes. Measure representative compiler-sized inputs; a 12-hour ceiling is
  emergency containment, not acceptable normal performance.

## 4. Gamma-written Delta compiler

- [ ] **DESIGN-BLOCKED — Q13: FREEZE-DELTA-V1.** Finish one self-contained Delta grammar, static
  semantics, deterministic execution model, sealed byte I/O contract, and
  resource taxonomy. Delta is an independent robust C-like compiler-host
  language; it does not inherit Omega meaning merely by sharing spelling. Q13
  must close the contradictory `Incomplete` placement, exact reject/trap
  taxonomy, keyword policy, optional domains/contracts, builtin resolution,
  Console/string ABI, scalar-transition miss, and closure presentation.
- [ ] **DESIGN-BLOCKED — Q13: BUILD-DELTA-COMPILER.** Implement
  `source/delta/compiler/delta_compiler.gamma` to consume arbitrary valid Delta
  and emit exact Alpha tape directly. No Beta translator, Gamma evaluator
  subprocess, host encoder/decoder, native assembler stream, or older compiler
  participates.
- [ ] Turn the existing Delta tests into positive, negative, trap, and
  `Incomplete` conformance for that compiler. Delete cases that merely pin
  quirks of the removed translator.
  - [x] Delete `exprc.delta` and `minic.delta`; both were demonstrations of the
    removed Darwin-native route rather than authoritative Delta observations.
  - [ ] Classify `contextual-state-identifiers.delta`, `fieldsat.delta`, the
    range/contracts portion of `discharge.delta`, and `calls.delta` after Q13.
    They currently contradict the written keyword/domain/result/builtin rules.
- [ ] Check exact Gamma-source-to-Alpha-tape refinement, including realistic
  source closures large enough to compile `D`.

## 5. Delta-written full Omega compiler `D`

- [ ] **OWN-OMEGA-D.** Author one exact package-resolved closure `D` at
  `source/omega/omega_compiler.delta`; do not preserve historical filenames,
  snapshots, or native-publication adapters as authorities. This is downstream
  of Q13. The deleted prototype remains available in Git for selectively
  re-deriving an isolated algorithm, but it cannot be restored or copied as a
  compiler-shaped starting point.
- [ ] Make `D` implement the complete Omega specification, including difficult
  features even if `D` itself uses only plain Delta. Conservative lowering and
  poor optimization are allowed; weakened Omega semantics are not.
- [ ] Compile `D` with `delta_compiler_bytecode.tape` into
  `omega0_compiler_bytecode.tape`, reconstruct the exact edge, and run the full
  Omega acceptance/rejection suite.
- [ ] Keep product target realization inside Omega. The bootstrap compiler
  itself remains Alpha tape even when the programs it compiles target ARM64,
  x86-64, UEFI, or another product target.

## 6. Omega-written full compiler `C`

- [ ] Publish one deterministic package-resolved Omega closure `C` rooted at
  `source/omega/build.omg`. Psi modules are included only when imported by the
  compiler executable; interpreters, viewers, REPLs, proof explorers, and other
  adjacent tools are excluded unless truly required.
- [ ] Author `C` with a conservative compositional subset of ordinary Omega to
  simplify the first self-build. This is an incidental source profile, never a
  named dialect or permission for `omega₀` to implement less than full Omega.
- [ ] Run `omega₀ C → omega` without rewriting or selectively replacing any
  source. Check this source-to-tape edge independently from `D → omega₀`.
- [ ] Demonstrate full Omega behavior and semantic agreement across the two
  implementations. Rust agreement and byte reproducibility remain diagnostic.

## Owner escalation — stop before changing architecture

Open an owner question when any of these appears:

- representative `delta → omega₀` or `omega₀ → omega` work has terrible wall
  time, memory use, or tape size after ordinary profiling and cleanup;
- Alpha verbosity creates pressure for a new opcode, wider encoding, or hidden
  high-level primitive;
- proof size or checker time remains explosive after DAG sharing,
  compositional lemmas, and removal of redundant evidence;
- useful performance appears to require a jet or special native substitution;
- target ABI/object/runtime details leak below product Omega or native compiler
  identity appears above the Alpha seed;
- an edge requires a compiler/interpreter/script older than its immediate
  predecessor or cannot directly emit the next runnable tape;
- realistic source crosses an unstated capacity, relies on undefined Alpha
  behavior, or cannot fail closed on exhaustion;
- proof completion seems to require a new trusted axiom/kernel rule rather than
  a better untrusted producer;
- conforming Alpha realizations disagree on the same tape and input;
- a retained legacy component requires a second accepted chain, duplicated
  source of truth, or permanent compatibility adapter; or
- correctness pressure encourages weakening a language contract, observation
  profile, exact subject identity, or rejection behavior.

An escalation permits measurement and a written ruling. It does not permit an
unreviewed opcode, jet, bridge, native detour, semantic subset, or new trusted
premise.

Product compiler implementation remains tracked in [`TASKS.md`](TASKS.md),
package authority in [`TASKS_PACKAGE_MANAGER.md`](TASKS_PACKAGE_MANAGER.md),
and unresolved design decisions in [`OWNER_QUESTIONS.md`](OWNER_QUESTIONS.md).
