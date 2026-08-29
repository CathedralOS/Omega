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

Owned code remains only when it directly specifies, implements, proves, or
efficiently tests one canonical edge. A retained migration component must have
a named adaptation below. If adaptation fails or ceases to be economical,
delete it. Git history is the archive; the repository is not.

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
- [ ] Audit every remaining bootstrap viewer, generated report, repeated-run
  receipt, wrapper, fixed-point gate, and differential implementation. Give it
  one bounded diagnostic or canonical-edge role, or delete it.

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
| Alpha-written Beta compiler | `cold-start/bc-alpha.alpha`, construction tests, tape machinery | promote one general Alpha implementation and prove its source-to-tape edge |
| Beta-written Gamma compiler | `interp.beta`, `typeck.beta`, Gamma semantics/tests | standalone Gamma-to-Alpha compiler tape and refinement |
| Gamma-written Delta compiler | Delta contract and test corpus | compiler source, tape, and refinement |
| `D → omega₀` | partial Delta-written compiler work | correctly owned complete `D`, full Omega acceptance, tape, and refinement |
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
- [ ] Update path-hygiene and lattice runners to enumerate only the canonical
  owners above. They must fail if a lower rung imports source or a semantic
  executable from beyond its immediate successor.
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
  surface gate runs 104 cases in under three seconds on the development host.

## 2. Alpha-written Beta compiler

- [ ] **PROMOTE-ALPHA-BETA-COMPILER.** Audit
  `source/beta/compiler/cold-start/bc-alpha.alpha` against the complete Beta v1
  contract. Generalize any pinned-source assumptions, rename/move it to
  `beta_compiler.alpha`, and make its exact Alpha tape the canonical Beta
  compiler artifact. It must accept arbitrary valid Beta within explicit
  resource bounds and reject or return `Incomplete` fail-closed.
- [ ] Redirect the existing cold construction, exact-tape comparison, and
  focused language tests to the Alpha source subject. Remove any two-stage
  “cold compiler builds `bc.beta`, then `bc.beta` becomes canonical” logic.
- [x] Reassess the large `bc.beta` refinement/admission tree module by module.
  Adapt general Alpha-machine decoding, observation, stuttering, and proof-DAG
  machinery to the actual Alpha-written compiler edge. Delete source-specific
  machinery that exists only to prove the noncanonical `bc.beta` fixed point.
  The retained surface is the generic artifact-structure check, generic FOL
  trace seam, stress/refinement harness, and bounded fixed-point comparator;
  about 65,000 source-specific lines and their wrapper scripts were removed.
- [ ] Retain `bc.beta` only as a bounded independent Beta compiler comparison
  and Beta-language regression subject. If it does not expose failures not
  covered more cheaply, delete it and its fixed-point gate.
- [ ] Close exact Alpha-source-to-Alpha-tape refinement with termination, trap,
  resource exhaustion, output, and divergence observations. Ordinary checked
  first-order simulation and well-founded stuttering remain the selected proof
  strategy; no new trusted LTS rule is implied.

## 3. Beta-written Gamma compiler

- [ ] **DESIGN-BLOCKED — Q19: BUILD-GAMMA-COMPILER.** Define the complete Gamma source contract, then
  implement `source/gamma/compiler/gamma_compiler.beta` as a standalone
  compiler from Gamma source to Alpha tape. It may reuse or reorganize
  `interp.beta` and `typeck.beta`; no external interpreter may remain part of
  compilation. Q19 must first select one typed executable grammar, entry/stream
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

- [ ] **DESIGN-BLOCKED — Q18: FREEZE-DELTA-V1.** Finish one self-contained Delta grammar, static
  semantics, deterministic execution model, sealed byte I/O contract, and
  resource taxonomy. Delta is an independent robust C-like compiler-host
  language; it does not inherit Omega meaning merely by sharing spelling. Q18
  must close the contradictory `Incomplete` placement, exact reject/trap
  taxonomy, keyword policy, optional domains/contracts, builtin resolution,
  Console/string ABI, scalar-transition miss, and closure presentation.
- [ ] **DESIGN-BLOCKED — Q18: BUILD-DELTA-COMPILER.** Implement
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
    range/contracts portion of `discharge.delta`, and `calls.delta` after Q18.
    They currently contradict the written keyword/domain/result/builtin rules.
- [ ] Check exact Gamma-source-to-Alpha-tape refinement, including realistic
  source closures large enough to compile `D`.

## 5. Delta-written full Omega compiler `D`

- [ ] **OWN-OMEGA-D.** Author one exact package-resolved closure `D` at
  `source/omega/omega_compiler.delta`; do not preserve historical filenames,
  snapshots, or native-publication adapters as authorities. This is downstream
  of Q18. The deleted prototype remains available in Git for selectively
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
