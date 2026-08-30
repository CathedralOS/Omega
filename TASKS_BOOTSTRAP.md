# Direct compiler lattice — active work

Last pruned: 2026-08-29.

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
presumption that keeping it is harmless. Every retained owned file must
directly specify, implement, prove, or efficiently test one canonical edge and
must have a present consumer, canonical owner, and deletion condition. This is
a retention proof, not a documentation preference: unadaptable material is
negative value because it consumes review, testing, maintenance, and
architectural attention. There is no neutral category: merely avoiding direct
conflict with the lattice is insufficient. Test coverage, prior investment,
historical continuity, and “potentially useful” are not retention arguments
unless the component strengthens the selected edge more economically than a
direct replacement. If direct adaptation fails, becomes uneconomical, or
leaves a parallel source of truth, compatibility route, or noncanonical
abstraction, delete the component and its bespoke gates together. Git history
is the archive; the working repository is exclusively the implementation of
the agreed chain.

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
  Beta artifact reconstruction, and one structure check. The status-only
  encoding reconstructor was deleted when it could not be adapted into the
  selected derivation. The duplicate Beta self-host wrapper was deleted. The Alpha checker was subsequently cut
  from a 293-file theorem/prover/adapter and overlapping-gate tree to one
  authoritative Beta source/tape, one complete independent reference, one
  bounded semantic seam, and compact positive/negative discriminators.

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
| Gamma-written Delta compiler | Delta contract and feature ledger | compiler source, spec-derived tests, tape, and refinement |
| `D → omega₀` | full Omega/Rust implementation as a nonauthoritative reference | correctly owned complete Delta closure `D`, full Omega acceptance, tape, and refinement |
| `C → omega` | Omega/Psi product work and Rust comparator | exact Omega closure, self-build tape, and independent refinement |

## 0. Make the repository tell the truth

- [x] Establish the canonical owner layout for every implementation that
  currently exists. Do not create placeholders for missing compiler edges:

  ```text
  source/beta/compiler/beta_compiler.alpha
  source/gamma/compiler/gamma_compiler.beta
  source/delta/compiler/delta_compiler.gamma
  source/omega/omega_compiler.delta       # D
  source/omega/{build,main}.omg            # C roots
  ```

  Each implemented owner contains its descriptive `.tape` artifact and
  adjacent validation. Missing Gamma, Delta, and `D` files are work gaps, not
  permission for substitute owners.
  Do not create generic `bootstrap/`, `on-ramp/`, `assurance/`, `canaries/`, or
  generation directories. `omega₀` and `omega` are artifacts, not languages or
  source owners.
  - [x] Move the existing Beta tape adjacent to `beta_compiler.alpha`, delete
    its otherwise content-free `artifacts/` bucket, and make path hygiene reject
    nested artifact buckets for every canonical compiler owner.
  - [ ] **DESIGN-BLOCKED — OWNER Q3.** Materialize the Gamma compiler source, tape,
    and adjacent validation in `source/gamma/compiler/`; section 3 owns the
    implementation.
  - [ ] **DESIGN-BLOCKED — OWNER Q2/Q3.** Materialize the Delta compiler source,
    tape, and adjacent validation in `source/delta/compiler/`; section 4 owns
    the implementation.
  - [ ] **DESIGN-BLOCKED — OWNER Q2.** Author
    `source/omega/omega_compiler.delta` once Delta's source and closure contract
    is selected; section 5 owns the implementation. This source work does not
    wait for the physical Gamma/Delta compiler artifacts.
  - [ ] **DEPENDENCY-BLOCKED — OWNER Q3, missing Gamma/Delta compilers, and missing
    `D`.** Materialize the resulting `omega0_compiler_bytecode.tape` only after
    the predecessor chain and source closure exist. Section 5 owns `D → omega₀`;
    section 6 owns completion of the existing `build.omg`/`main.omg` closure and
    `C → omega`.
- [x] Make one path-hygiene gate enumerate only the canonical owners above and
  fail if a lower rung imports source or a semantic executable from beyond its
  immediate successor. Exact implemented compiler source/tape names are
  positively enumerated, and owner-aware scans reject forward imports and
  alternate native artifacts. Delete the `lattice_path` role facade and the
  `verify-lattice.sh` ceremony wrapper around `source/alpha/verify.sh --edge`;
  the real Alpha gate is invoked directly from its owner.
- [x] Make retention mechanically auditable: every owned file and subtree under
  the canonical Alpha-through-Omega owners must name its canonical edge or
  bounded failure-detection/proof role in the nearest retention inventory,
  including leaf files inside a classified child, and every inventory must
  state deletion conditions. Delete unowned wrappers, comparators, corpora,
  reports, and generators; do not create an indefinite “diagnostic” exemption.
  `check-path-hygiene.sh` enforces this file-level proof. The unrun 43-file Delta
  native-route corpus, completed Alpha extent-migration script, duplicate
  seed/reference random fuzzer, checker theorem museum/prover/adapters, and
  misleading Beta `cold-start/` owner were removed.
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
  surface gate runs 192 cases in about six seconds on the development host.
  The largest current retained Beta output, the 236,076-byte checker tape,
  leaves 26,064 bytes in the Alpha payload after replacing repeated inline
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
    focused suite now passes 192 cases, including signed division/remainder,
    trap-prefix, sealed EOF/write, and left-to-right side-effect discriminators;
    the canonical tape passes the generic structural checker.
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
  - [x] **BETA-FLATTENED-CFG-INITIALIZATION:** enforce the settled recursive
    authoring surface and every-path initialization judgment. Each procedure or
    state body is an ordinary-statement prefix followed by child states; reject
    loose ordinary statements after the first child and ordinary statements
    after `return` or unconditional `to`. Flatten nested states to
    procedure-wide labels in the depth-first lexical order the compiler already
    emits, including fallthrough out of a child subtree to the next outer
    sibling. Preserve procedure-wide state/local identity and source-order local
    visibility. Derive exact guarded target and false-continuation edges without
    constant folding, compute per-procedure reachability, then iterate the
    initialized-slot intersection judgment to a fixed point before validating
    reads. Add positive controls for the checker/Gamma nested-state shapes,
    `boff`-style loop-carried joins, alternate-path assignment establishment,
    subtree fallthrough, and unreachable blocks; add rejections for skipped
    initialization, invalid interleaving, post-terminator statements, and
    traversal-order-sensitive loop handling. Update the reference parser and
    interpreter atomically so they no longer hoist loose statements or reject
    nested states. The Alpha compiler and independent reference now share the
    recursive block formation, depth-first flattening, exact transition-prefix
    facts, reachability, and fixed-point must-initialization judgment. The
    focused compiler surface passes 192 cases, including subtree fallthrough,
    alternate-path establishment, unreachable-block handling, loop-carried
    joins, invalid block shapes, skipped initialization, and traversal-order
    controls; the reference differential and exhaustive-I/O gates agree over
    60 generated programs and 10,240 bounded input cases.
  - [x] Extend the compiler's one checked syntax-recursion budget across nested
    state parsing as well as parentheses, calls, and loads. The current selected
    compiler profile admits combined depth 64; that number is private resource
    policy, not Beta language meaning. Exhaustion must join the settled
    compiler-boundary `Incomplete` outcome rather than become invalid source or
    an Alpha return-stack accident. Reuse initialization work storage per
    procedure (one entry plus at most 128 states) or account for all procedure
    entries explicitly;
    the global 1,024-state ceiling does not include the 128 entry blocks. State
    bodies now use the same checked depth counter as parentheses, calls, and
    loads; exact depth 64 is accepted and the adjacent state-only and mixed
    depth-65 cases return canonical `Incomplete(syntax_depth, 64, 65)`.
  - [x] Separate source-visible raw Beta memory from generated frame/expression
    stacks and bind the call/stack profile that proves non-aliasing. Raw memory
    is a checked, zeroed 32 MiB logical region biased above the data stack. Every
    generated frame/expression reservation is guarded at 262144; the mandatory
    frame word bounds semantic depth and leaves the hidden Alpha return stack
    above 66,322,424 even at the failing edge. A 64-slot recursive stress case
    reaches fail-closed status 250 without output or aliasing.
  - [x] Bind the compiler's practical fixed resource profile and exercise exact
    admitted/adjacent-refused boundaries for the 1 MiB source, 64-byte names,
    shared 64-level syntax-recursion depth (state blocks, expressions, calls,
    and loads), 64 slots, 128 procedures,
    1,024 non-builtin procedure call references, per/global state and transition
    tables, 262,140-byte Alpha
    payload, 32 MiB raw memory, and generated-stack containment. Every refused
    compile publishes no partial tape. The 32,768 fixup and 65,536 internal-PC
    guards are necessarily dominated by the smaller tape extent and are
    documented as corruption teeth rather than falsely advertised independent
    source capacities.
  - [x] **BETA-COMPILER-OUTCOME:** Implement the settled four-case boundary.
    Reserve Alpha halt tags 0/1/2/3 for `Complete`/`Reject`/`Incomplete`/
    `InternalFailure`; leave successful stdout as the exact raw runnable
    payload; and emit the canonical `0xFF BCOUT v1` 40-byte diagnostic frame on
    every failure. Replace parser-stage halt values with closed stable rejection
    reasons and zero-based source offsets. Give each independently reachable
    private source/name/depth/procedure/state/edge/call/slot/payload ceiling a
    closed resource code, limit, and requested amount. Treat the dominated
    fixup/internal-PC guards, replay drift, and post-validation resolution
    impossibilities as internal failures. Stage stdout and publish only after
    the halt tag and frame agree; add malformed-frame, unknown-code,
    noncanonical-field, partial-output/trap, shell-low-byte, and runtime-status
    250/251 separation canaries. Publish the exact version-1 code tables beside
    the compiler and make gates consume rather than invent them.
    The Alpha compiler now records the first decisive typed outcome, emits the
    exact 40-byte `BCOUT` frame only after all fields are fixed, and leaves
    successful tape bytes unwrapped. `outcomes-v1.tsv` owns the closed tables;
    the focused gate consumes them and passes 192 language, ceiling, framing,
    partial-output/trap, runtime-separation, and internal-producer cases. All
    six closed internal reasons are positively exercised through single-site
    temporary compiler mutations that lower otherwise dominated invariants;
    production has no test hook. The rebuilt 26,751-byte compiler artifact
    passes exact reconstruction and structural validation.
  - [ ] **DESIGN-BLOCKED — OWNER Q9:** Freeze Beta v1's byte-level lexical
    contract: source decoding, identifier/digit alphabets, trivia bytes, and
    direct character-literal bytes. The Alpha compiler and Python reference
    currently disagree on these source-validity choices, while the written
    grammar does not select either behavior. Closed comma-list grammar,
    comparison arity, and literal escape sets are independent of this ruling
    and remain implementation work rather than design blockers.
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
  The retained diagnostic surface is one generic artifact-structure check; the
  exact checked source/tape derivation remains the open canonical obligation.
  The status reconstructor, toy FOL seam, source-only loop
  checker, duplicated Alpha/checker fixtures, and symbolic differential were
  deleted; they reconstructed no canonical checked source/tape proposition or
  duplicated cheaper owners. The final symbolic differential had also drifted
  to 13/18 while returning success, making it a false-green parallel semantics.
  About 65,000 historical source-specific lines had already been removed.
- [x] Delete the historical Beta self-host after promotion. Its full-source
  migration comparison helped pin the direct emitter, but it had zero remaining
  executable consumers and no bounded comparison gate; constructing a new gate
  merely to justify retention would reverse the repository policy. Its fixed
  point and source now survive only in Git history.
- [ ] **ALPHA-BETA-EXACT-CONSTRUCTION.** Close exact
  Alpha-assembly-source-to-Alpha-tape correspondence. First
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
  - [x] Retire the Alpha-written status-only encoding reconstructor and its
    parallel mutation gate. It exercised the then-current 78,109-byte source and
    20,977-byte tape, but returned private halt statuses rather than a checked
    derivation and could not be adapted into the selected certificate shape.
    Keeping it after that result would preserve a second assembly semantics and
    false progress on an open edge. Git history is its archive.
  - [x] Bind proof propositions to raw persisted subjects inside the checker.
    The bounded `OMGCHK1` frame carries exact little-endian source, tape, and
    certificate extents; checker-built immutable power-of-two-indexed byte trees are available only
    as the framed `source` and `tape` constants. Identical subjects accept a
    reflexivity control, a one-byte mutation rejects, unframed input cannot
    spoof either constant, and the rebuilt 236,076-byte checker tape retains
    26,064 bytes of Alpha payload headroom. The exact 103,274-byte compiler
    source plus 26,751-byte tape carrier remains within the same bounded
    subject interface. Fixed
    byte/empty/leaf/node constructors give every real byte a stable fixed-depth
    path and make subject structure available to ordinary bounded certificate
    functions at logarithmic recursion depth;
    no assembly-specific checker rule was added.
    Declaration tables are range-checked and immutable before the first checked
    lemma, duplicate IDs and trailing forms reject, and the independent checker
    matches those controls; a later rewrite cannot change an accepted lemma's
    definitional meaning.
  - [ ] **ALPHA-BETA-COMPOSED-CERTIFICATE:** Turn the ground assembly judgment into a derivation certificate
    over those checker-bound subjects. The certificate must check the complete
    two-pass ledger, unique label map, total source/tape partitions, exact
    fixups, and full exhaustion.
    - The only permitted end state here is one artifact-owned fixed `.proof`
      and one artifact-owned acceptance/mutation gate. Retain one closed ground
      root judgment `VERIFY(source, tape, trace) = ACCEPT`, but discharge it
      through bounded named equalities and one checked composition proof rather
      than one compiler-scale conversion. Do not add an assembly rule to the
      generic checker, a theorem-library subtree, a host parser, a generated
      ledger, a persisted receipt, or another acceptance gate.
    - Give pass one and pass two distinct checked chunk schemas. Pass-one chunks
      partition the exact source, parse local rows completely, thread payload
      positions and the unique label map, and derive the total payload length;
      their predicted PC intervals are accounting, not tape ownership. Freeze
      that terminal label map and length through one exact pass joint. Pass-two
      chunks independently partition the source and the tape, use that frozen
      map, and check every opcode, operand, decoded `db` byte, and little-endian
      fixup. Checked composition must prove per-pass adjacency, order, unique
      span ownership, canonical initial/terminal states, and full exhaustion
      without gaps, overlap, duplication, or suffixes.
    - Treat cut locations as untrusted certificate witnesses, not owner-fixed
      authority. The owner fixes the exact framed subjects, Alpha assembly
      relation, pass schemas and joint, composition theorem, canonical
      endpoints, and root proposition. Any cut strategy is accepted only when
      those generic checks establish the same total edge.
    - Measurements closed the representation search rather than merely finding
      a slow implementation. Dynamic balanced cutting accepts 714 canonical
      leaves and fails at 715 with contained memory status 251. A structurally
      recursive balanced carrier traverses all 6,467 leaves in 0.704 seconds,
      and a folded 3,240-leaf carrier in 0.465 seconds, but adding local parsing
      exhausts the same arena. Even a content-free structural visit of all
      78,109 bytes in the then-current subject failed inside one equality; the
      present 103,274-byte source is no smaller. Sequential remainder folds
      instead hit contained semantic-stack status 250. The checker reclaims
      conversion scratch only after a complete equality decision, so one
      compiler-scale reflexive equality retains every branch temporary.
      A checker-native control partitions the same source into 112 named
      subject-bound equality decisions, visits every byte, and composes their
      checked propositions with `use`; it accepts in 1.192 seconds. This proves
      the proposed reclamation boundary is viable but does not yet prove the
      required boundary chain or assembly semantics.
    - Publish the canonical Beta checker's exact arena, semantic-stack, framed
      input, certificate, declaration, and lemma-table profile. The fixed proof
      must fit that profile. The independent Python checker must agree on the
      logical result but is diagnostic and need not reproduce Beta's resource
      ceilings. A future authoritative checker cannot replace the service while
      silently refusing its live certificate/profile.
    - Repeat measurements against the exact current bound subjects rather than
      copying prose byte counts. Across candidate chunk counts record peak
      conversion scratch, permanently retained lemma/boundary-state arena,
      semantic-stack demand, certificate bytes, and checking time. The earlier
      112 content-free equalities prove only that the existing per-equality
      reclamation boundary is viable; the selected certificate must measure
      real two-pass assembler state and composition cost.
    - Neither implementation nor measurement may add an assembly-specific
      primitive, trusted premise, host parser, generated ledger, persisted
      producer/receipt, or second acceptance gate. Enlarging an undocumented
      bound, weakening exact exhaustion, or restoring the deleted status ledger
      is not an option.

## 3. Beta-written Gamma compiler

- [ ] **DESIGN-BLOCKED — OWNER Q3: BUILD-GAMMA-COMPILER.** Define the complete Gamma source contract, then
  implement `source/gamma/compiler/gamma_compiler.beta` as a standalone
  compiler from Gamma source to Alpha tape. It may reuse or reorganize
  `interp.beta` and `typeck.beta`; no external interpreter may remain part of
  compilation. OWNER Q3 must first select one typed executable grammar, entry/stream
  ABI, outcome model, and fuel/resource meaning. The current interpreter and
  type checker implement disconnected untyped-executable and typed-nonexecuting
  languages, so choosing either in code would invent Gamma semantics.
- [x] Keep `interp.beta` and `typeck.beta` only as reusable compiler components
  or bounded semantic oracles. Their inventories now name present gates and
  explicit OWNER Q3 absorption/deletion conditions; neither is accepted as a
  compiler edge. The retained post-prune gates pass 41 interpreter cases, the
  fail-closed arena case, 23 type-checker cases, and 101 independent
  differential cases. OWNER Q3's compiler task owns the later
  absorb-or-delete step.
  - [x] Delete the interpreter's dead environment lookup and the
    `Node`/`Chunks`/`ZeroTree` compact representation plus 524,288-slot
    translator-carrier case. They existed for the deleted cross-rung translator,
    not for Gamma semantics or the canonical compiler edge. Rewrite the
    interpreter-first claims to classify both executables as pre-contract
    oracles.
  - [x] Remove the type checker's retired proof-kernel purpose and reject
    unknown declared types explicitly instead of allowing the shared `-1`
    error/type sentinel to compare equal.
- [ ] **DEPENDENCY-BLOCKED — OWNER Q3 and missing `gamma_compiler.beta`.** Check the
  exact Beta-source-to-Alpha-tape refinement and all resource outcomes. Measure
  representative compiler-sized inputs; a 12-hour ceiling is emergency
  containment, not acceptable normal performance.

## 4. Gamma-written Delta compiler

- [ ] **DESIGN-BLOCKED — OWNER Q2: FREEZE-DELTA-V1.** Finish one self-contained Delta grammar, static
  semantics, deterministic execution model, sealed byte I/O contract, and
  resource taxonomy. Delta is an independent robust C-like compiler-host
  language; it does not inherit Omega meaning merely by sharing spelling. Q3
  must close the contradictory `Incomplete` placement, exact reject/trap
  taxonomy, keyword policy, optional domains/contracts, builtin resolution,
  Console/string ABI, scalar-transition miss, and closure presentation.
- [ ] **DESIGN-BLOCKED — OWNER Q2/Q3: BUILD-DELTA-COMPILER.** Implement
  `source/delta/compiler/delta_compiler.gamma` to consume arbitrary valid Delta
  and emit exact Alpha tape directly. No Beta translator, Gamma evaluator
  subprocess, host encoder/decoder, native assembler stream, or older compiler
  participates.
- [ ] **DESIGN-BLOCKED — OWNER Q2.** Derive compact positive, negative, trap, and
  private-budget `Incomplete` conformance directly from the frozen Delta
  contract. Do not recreate cases that merely pin quirks of the removed
  translator.
- [ ] **DEPENDENCY-BLOCKED — OWNER Q3 and missing `delta_compiler.gamma`.** Run that
  contract-derived suite through the real Gamma-written compiler and bind every
  outcome to its no-partial-tape behavior.
  - [x] Delete `exprc.delta` and `minic.delta`; both were demonstrations of the
    removed Darwin-native route rather than authoritative Delta observations.
  - [x] Delete the unrun 43-file pre-migration Delta corpus rather than
    classifying native-backend slices as language tests. It mixed retired
    Darwin/ARM layout and trap assumptions, deleted `contracts.sh` workflows,
    demonstrations, and unresolved keyword/domain/result/builtin proposals.
    After OWNER Q2 freezes Delta v1, derive a compact positive/negative suite from
    the frozen contract and run it through the actual Gamma-written compiler.
- [ ] **DEPENDENCY-BLOCKED — OWNER Q2/Q3 and missing Gamma/Delta compilers.** Check
  exact Gamma-source-to-Alpha-tape refinement, including realistic source
  closures large enough to compile `D`.

## 5. Delta-written full Omega compiler `D`

- [ ] **DESIGN-BLOCKED — OWNER Q2: OWN-OMEGA-D.** Author one exact package-resolved closure `D` at
  `source/omega/omega_compiler.delta`; do not preserve historical filenames,
  snapshots, or native-publication adapters as authorities. This is downstream
  of the frozen Delta/Gamma contracts and compilers. The deleted prototype
  remains available in Git for selectively re-deriving an isolated algorithm,
  but it cannot be restored or copied as a compiler-shaped starting point.
- [ ] **DEPENDENCY-BLOCKED — OWNER Q2 and missing `D`.** Make `D` implement the
  complete Omega specification, including difficult features even if `D`
  itself uses only plain Delta. Conservative lowering and poor optimization are
  allowed; weakened Omega semantics are not.
- [ ] **DEPENDENCY-BLOCKED — missing Gamma/Delta compilers and `D`.** Compile `D` with `delta_compiler_bytecode.tape` into
  `omega0_compiler_bytecode.tape`, reconstruct the exact edge, and run the full
  Omega acceptance/rejection suite.
- [ ] **DEPENDENCY-BLOCKED — missing `D` and `omega0`.** Verify that product
  target realization remains inside Omega. The bootstrap compiler itself
  remains Alpha tape even when the programs it compiles target ARM64, x86-64,
  UEFI, or another product target.

## 6. Omega-written full compiler `C`

- [ ] **DEPENDENCY-BLOCKED — OWNER Q6 and current ranked-runtime acceptance.**
  Publish one deterministic package-resolved Omega closure `C` rooted at
  `source/omega/build.omg`. Psi modules are included only when imported by the
  compiler executable; interpreters, viewers, REPLs, proof explorers, and other
  adjacent tools are excluded unless truly required.
  The canonical typed-token owner and gate-only diagnostic serialization are
  complete. The remaining regular `TASKS.md` C cleanup is also complete: the
  real production report now retains one canonical package-source/build/
  target/artifact manifest, including zero-dependency `build.omg` projects. The
  mismatched standalone source-snapshot/census command, compiler route, schemas,
  and gates are already deleted rather than retained as a second bootstrap
  observation. Freeze that final checked production closure only when `C`
  itself is complete; do not revive an inspection-only precursor.
- [ ] **DEPENDENCY-BLOCKED — OWNER Q6 and current ranked-runtime acceptance.**
  Author `C` with a conservative compositional subset of ordinary Omega to
  simplify the first self-build. This is an incidental source profile, never a
  named dialect or permission for `omega₀` to implement less than full Omega.
- [ ] **DEPENDENCY-BLOCKED — missing `omega0` and complete `C`.** Run `omega₀ C → omega` without rewriting or selectively replacing any
  source. Check this source-to-tape edge independently from `D → omega₀`.
- [ ] **DEPENDENCY-BLOCKED — missing admitted `omega0` and `omega`.**
  Demonstrate full Omega behavior and semantic agreement across the two
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
- an owned component cannot name the canonical edge or product-compiler phase
  it strengthens, its present consumer, and the cheaper direct replacement it
  defeats; or
- correctness pressure encourages weakening a language contract, observation
  profile, exact subject identity, or rejection behavior.

An escalation permits measurement and a written ruling. It does not permit an
unreviewed opcode, jet, bridge, native detour, semantic subset, or new trusted
premise.

Product compiler implementation remains tracked in [`TASKS.md`](TASKS.md),
package authority in [`TASKS_PACKAGE_MANAGER.md`](TASKS_PACKAGE_MANAGER.md),
and unresolved design decisions in [`OWNER_QUESTIONS.md`](OWNER_QUESTIONS.md).
