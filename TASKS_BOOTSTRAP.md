# Direct compiler lattice — active work

Last pruned: 2026-08-30.

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
  authoritative Beta source/tape, one temporary complete independent reference, one
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
| Beta-written Gamma compiler | canonical frontend/direct emitter, resolved whole-function lowering, `interp.beta` oracle, Gamma semantics/tests | implement D30's physical profiles, emit adapters, publish the standalone tape, and close refinement |
| Gamma-written Delta compiler | Delta contract/ledger; canonical source through parsing, D22/D24 census, D31 structural type formation, source-backed resolution catalog, and symbolic Alpha encoding | resolve Q4 entry diagnostics and Q6 callable ambiguity, complete body/control checking and lowering, implement D34 physical storage refusal, publish the tape, and close refinement |
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

  Each completed compiler owner contains its descriptive `.tape` artifact and
  adjacent validation. Incomplete canonical sources and missing Gamma, Delta,
  or `D` artifacts are work gaps, not permission for substitute owners.
  Do not create generic `bootstrap/`, `on-ramp/`, `assurance/`, `canaries/`, or
  generation directories. `omega₀` and `omega` are artifacts, not languages or
  source owners.
  - [x] Move the existing Beta tape adjacent to `beta_compiler.alpha`, delete
    its otherwise content-free `artifacts/` bucket, and make path hygiene reject
    nested artifact buckets for every canonical compiler owner.
  - [ ] Complete the Gamma compiler source, tape, and adjacent validation in
    `source/gamma/compiler/` under D16. The canonical source and its bounded
    frontend/emitter gate now exist; section 3 owns lowering, adapter selection,
    tape publication, and refinement.
  - [ ] Complete the existing Delta compiler source, tape, and adjacent
    validation in `source/delta/compiler/` under D17; section 4 owns the
    implementation.
  - [ ] Complete `source/omega/omega_compiler.delta` under D17; section 5 owns
    the existing incomplete implementation. This source work does not wait for
    the physical Gamma/Delta compiler artifacts.
  - [ ] **DEPENDENCY-BLOCKED — incomplete Gamma/Delta compilers and `D`.**
    Materialize the resulting `omega0_compiler_bytecode.tape` only after
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
- [x] **BOOTSTRAP-ASCII-SOURCE:** Implement D15's one source-byte envelope for
  Alpha assembly, Beta, the fixed Gamma contract, and Delta. Reject before
  tokenization every byte other than HT, LF, CR, and printable ASCII; use
  explicit ASCII identifier/digit predicates, exactly space/tab/CR/LF trivia,
  CR/LF/source-end comment termination, and printable direct literal bytes plus
  each language's closed escapes. Clean the currently checked-in compiler and
  oracle sources mechanically, translating the checker's logic comments into
  the ASCII vocabulary its certificates actually parse. Enforce the invariant
  over exact source-closure membership rather than filename suffixes. Replace
  NUL-filled extent controls with valid space padding. Because this changes
  exact source subjects, refresh hashes, coordinate evidence, measurements, and
  affected construction certificates before the next D14 capacity experiment;
  rerun the assembler, compiler, checker, and diamond gates. Short-term Python
  references must implement the same byte contract if retained, but no Python
  implementation survives completion of the checked direct chain.
  - [x] Enforce the envelope in the Alpha assembler and its independent
    reference, clean the retained Alpha corpus, refresh both stamped platform
    realizations, and pin CR comments plus NUL/VT/DEL/high-byte rejection.
  - [x] Enforce the envelope and exact rejection coordinates in the
    Alpha-written Beta compiler and temporary Python reference; replace the
    extent control with space padding, clean every retained Beta implementation
    source, and refresh the current 104,572-byte source / 27,087-byte tape observations.
  - [x] Enforce the envelope, explicit ASCII identifier classes, and CR/LF
    comments in both bounded Gamma oracle surfaces and the temporary Python
    evaluator. The existing gates now retain matching positive and negative
    byte controls.
  - [x] Apply D15's fixed outer envelope and D17's exact Delta lexical rules in
    `delta_compiler.gamma`. `check_source_bytes` rejects the complete source
    before tokenization, and the retained lexical phase owns explicit ASCII
    identifiers/digits, exact trivia/comment termination, printable literal
    bytes, closed escapes, and exact rejection offsets.

## 1. Alpha execution floor

- [x] Keep `source/alpha/SEMANTICS.md`, the audited seed implementations, and
  conformance tests synchronized. The canonical `.tape` is the raw Alpha
  payload; transparent seed stamping prepends its exact four-byte length inside
  the native container. The seed then exposes the exact Alpha observation
  model. `source/alpha/verify.sh --edge` currently passes all 26 conformance
  cases and exact assembler reconstruction.
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
  surface gate runs 197 cases in about nine seconds on the development host.
  The largest current retained Beta output, the 238,926-byte checker tape,
  leaves 809,646 bytes in the V2 Alpha payload after replacing repeated inline
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
    focused suite now passes 197 cases, including the closed source-byte
    envelope, signed division/remainder, trap-prefix, sealed EOF/write, and
    left-to-right side-effect discriminators;
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
    focused compiler surface passes 197 cases, including subtree fallthrough,
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
    the global 1,024-state ceiling does not include the 256 entry blocks. State
    bodies now use the same checked depth counter as parentheses, calls, and
    loads; exact depth 64 is accepted and the adjacent state-only and mixed
    depth-65 cases return canonical `Incomplete(syntax_depth, 64, 65)`.
  - [x] Separate source-visible raw Beta memory from generated frame/expression
    stacks and bind the call/stack profile that proves non-aliasing. The D23
    compiler source uses a checked, zeroed 128 MiB logical region biased at
    physical byte 4 MiB. Every generated frame/expression reservation is
    guarded at 1 MiB below a 2 MiB stack top; the mandatory frame word bounds
    semantic depth and leaves the hidden Alpha return stack above 267,386,872
    even at the failing edge. A 64-slot recursive stress case reaches
    fail-closed status 250 without output or aliasing.
  - [x] Bind the compiler's practical fixed resource profile and exercise exact
    admitted/adjacent-refused boundaries for the 1 MiB source, 64-byte names,
    shared 64-level syntax-recursion depth (state blocks, expressions, calls,
    and loads), 64 slots, 256 procedures,
    1,024 non-builtin procedure call references, per/global state and transition
    tables, 1,048,572-byte Alpha
    payload, 128 MiB raw memory, and generated-stack containment. Every refused
    compile publishes no partial tape. The 116,508 fixup and 262,144 internal-PC
    guards are necessarily dominated by the tape extent and are
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
    the focused gate consumes them and passes 197 language, ceiling, framing,
    partial-output/trap, runtime-separation, and internal-producer cases. All
    six closed internal reasons are positively exercised through single-site
    temporary compiler mutations that lower otherwise dominated invariants;
    production has no test hook. The rebuilt 27,087-byte compiler artifact
    passes exact reconstruction and structural validation.
  - [x] Close the non-proof implementation/resource audit over the selected
    compiler profile. The 197-case surface gate now requires a numeric
    coordinate for every retained Reject, Incomplete, and InternalFailure
    producer rather than accepting coordinate-space-only evidence. Its
    consumed-prefix convention is fixed beside the boundary, and the existing
    grammar, resolution, CFG, initialization, private-ceiling, phase-priority,
    and six single-site internal cases pin the exact values. The independent
    semantic differential still agrees on 60 generated programs and the
    bounded I/O comparison on all 10,240 selected inputs.
  - [ ] **BETA-COMPILER-OPERATIONAL-REFINEMENT:** close actual compiler
    correctness for arbitrary accepted Beta under the selected resource
    profile. Reconstruct the complete written Beta small-step judgment and the
    emitted Alpha observation, including traps, output prefixes, fail-closed
    runtime containment, and every finite input, then check the refinement in
    the rooted calculus. The focused suite and temporary Python differential
    are regression evidence only; they do not turn the parent admission into a
    theorem. This semantic obligation is distinct from
    `ALPHA-BETA-EXACT-CONSTRUCTION`, which proves that the persisted compiler
    tape is exactly the assembly of `beta_compiler.alpha`.
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
  - [x] Freeze `source/alpha/ASSEMBLY.md`: byte-stream lexical form, exact operand grammar, full
    opcode/width table, string decoding, absolute label meaning, deterministic
    two-pass encoding, and the raw-payload/container boundary. Close the Alpha
    assembler and independent reference implementation over that grammar while
    retaining their byte-identical fixed point. D15 subsequently narrowed the
    shared outer source envelope; `BOOTSTRAP-ASCII-SOURCE` owns that atomic
    implementation and evidence refresh without reopening this encoding work.
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
    spoof either constant, and the rebuilt 238,926-byte checker tape retains
    809,646 bytes of AlphaBootstrapV2 payload headroom. The exact 104,572-byte compiler
    source plus 27,087-byte tape carrier remains within the same bounded
    subject interface. Fixed
    byte/empty/leaf/node constructors give every real byte a stable fixed-depth
    path and make subject structure available to ordinary bounded certificate
    functions at logarithmic recursion depth;
    no assembly-specific checker rule was added.
    Declaration tables are range-checked and immutable before the first checked
    lemma, duplicate IDs and trailing forms reject, and the independent checker
    matches those controls; a later rewrite cannot change an accepted lemma's
    definitional meaning. The independent reference now also decodes the exact
    `OMGCHK1` frame, reserves the same raw constructors, builds the same
    source/tape trees, and agrees on framed equality, mutation, computation,
    constructor-spoofing, and unframed-name controls.
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
      present 104,572-byte source is no smaller. Sequential remainder folds
      instead hit contained semantic-stack status 250. The checker reclaims
      conversion scratch only after a complete equality decision, so one
      compiler-scale reflexive equality retains every branch temporary.
      A checker-native control partitions the same source into 112 named
      subject-bound equality decisions, visits every byte, and composes their
      checked propositions with `use`; it accepts in 1.192 seconds. This proves
      the proposed reclamation boundary is viable but does not yet prove the
      required boundary chain or assembly semantics.
      The checker now also preserves pointer identity during substitution only
      inside its recorded immutable raw-subject arena interval. A maximum-size
      framed subject crosses equality transport without being copied, while a
      certificate-spelled raw-constructor lookalike still substitutes normally.
      With that closed-term boundary, a temporary two-chunk composition over
      the exact current Beta source/tape checks both named chunk equalities and
      the second nested `eqelim`; this validates the selected composition shape,
      but no skeleton proof is retained and assembly-row semantics remain open.
      The current framed subjects allocate 395,493 of the 4,893,354-node V2
      arena, leaving 4,497,861 nodes (107,948,664 bytes) for declarations,
      retained lemmas, and one equality's scratch. A real raw-tree selector plus
      the exact textual-ASCII/comment DFA checks the first 1,024 source bytes in
      the authoritative checker with a 1,606-byte, 73-declaration temporary
      certificate in 0.19 seconds. That closes the traversal and byte-dispatch
      shape, not pass one. The current compact parser prototype uses five user
      constructors and 371 of 768 function IDs; fixed 16-nibble words carry PC,
      source coordinates, decimal/register accumulators, and label
      `(start,end,pc)` spans. Its shared prefix has 2,348 declarations. The
      exact `[4096,4352)` slice accepts at PC 80 and coordinate 4,352 with
      `read_source` span `4111..4122@10` in a 68,571-byte certificate and 0.40
      seconds. The adjacent `[4352,4608)` slice starts from that state, records
      `source_done` as `4396..4407@92`, and ends cross-cut in register digits at
      PC 151 and coordinate 4,608 in a 71,020-byte certificate and 0.47 seconds.
      One computation over both checked subtrees accepts in 0.62 seconds; this
      validates real state transfer but is not yet the required named-equality
      adjacency composition. Exact `r255`/`u64::MAX` accept; `r256`,
      `u64::MAX + 1`, PC/source-coordinate overflow, unknown mnemonics, D15 NUL,
      and a removed label colon normalize to explicit `Reject` in 0.05--0.24
      seconds rather than consuming the 100,000-reduction ceiling. A comment
      split across raw subtrees restores its pre-comment parser continuation.
      The former documented `10 -> 83 -> 164` accounting is stale and must not
      seed the final certificate. Parser-rich measurement may coarsen 256-byte
      power-of-two subtrees only after they remain below the reduction and
      semantic-stack ceilings. The complete implementation still needs all 21
      mnemonics, `db`, delimiter-time classification from source spans rather
      than a carried reverse token list, bytewise label-span comparison, the
      balanced uniqueness/frozen-457-map join and accessor, generic checked cut
      witnesses and named adjacency/ownership composition, canonical EOF
      finalization, and full-source resource measurement. The terminal
      `[104448,104704)` selector has 124 real leaves plus checker `EMPTY`
      padding and must close the final `db "main"` at source byte 104,572 and
      PC 27,087.
      A later temporary frozen-map prototype measured the exact current census
      at 457 label definitions and 1,010 symbolic references. One balanced
      accessor with 31 in-order pieces represented all rows using 3,670
      declarations and 512 of 768 function IDs; its certificate was 445,785
      bytes and its complete framed input 577,476 bytes. Exact `source_done ->
      92` lookup accepted in 0.89 seconds, while a same-length undefined spelling
      rejected in 0.76 seconds. Exact duplicate, nonduplicate, and interior-span
      discriminators completed in 0.55--0.61 seconds, so bytewise span identity
      and maximal-boundary rejection are viable without hash authority. The
      aggregate map is not viable yet: its 31-piece proof rejected after 16.75
      seconds at roughly 120 MiB RSS, localized to one 27-row fold even though
      all 26 of that piece's adjacency judgments accepted. Prefixes of two,
      four, and eight named pieces accepted in 2.32, 4.15, and 8.82 seconds;
      sixteen rejected in 16.76 seconds. No artifact was retained. Fix the
      fold/environment defect and keep label-token/maximal-boundary ownership in
      pass one rather than rescanning every label through the accessor.
    - Implement the eventual proof in place only when pass one is vertically
      complete: exact D15/token/comment/`db` streaming states, fixed-width
      decimal/register/PC checks, every source subtree equality, balanced
      boundary and unique-label joins, the 457-record frozen map, and the
      104,572-byte / 27,087-byte root. A temporary producer may choose paths,
      boundary states, and compact label IDs, but none survives as a parser,
      ledger, receipt, or authority. Use a compact label-record dispatch rather
      than repeating the full map in every proposition. Do not retain a
      one-chunk demonstration or advertise partial pass-one acceptance as edge
      admission; extend the same final `.proof` and single gate with pass two
      and `VERIFY` after the complete pass-one checkpoint.
    - [x] Publish the canonical Beta checker's exact arena, semantic-stack,
      framed input, certificate, declaration, function, and lemma-table profile.
      Arena and proof-context exhaustion are now explicit fail-closed guards,
      and complete stdin is bounded to the exact largest permitted frame rather
      than overlapping later checker tables. The fixed proof must fit that
      profile. While retained, the independent Python checker must
      agree on the logical result but is diagnostic and need not reproduce
      Beta's resource ceilings. Delete it when this checked route subsumes the
      comparison. A future authoritative checker cannot replace the service
      while silently refusing its live certificate/profile.
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

- [ ] **BUILD-GAMMA-COMPILER.** Implement D16 and
  `source/gamma/LANGUAGE.md` in
  `source/gamma/compiler/gamma_compiler.beta` as a standalone compiler from
  Gamma source to Alpha tape. Type-check before emission, use the private
  arbitrary-arity Gamma frame ABI, preserve proper tail calls, and emit Alpha
  tape directly. It may reuse or reorganize `interp.beta`; no
  external interpreter or serialized-AST runtime may remain part of
  compilation.
  - Derive positive and negative canaries directly from the fixed grammar and
    static semantics: forward/mutual recursion, arbitrary arity, proper tail
    calls, exhaustive matches and complete static rejection of every
    nonexhaustive shape, checked `Int` traps, every `Bytes` operation, and
    invalid byte/range access.
  - Implement D20's canonical resolver: collect exact type, constructor, and
    function identities with within-namespace duplicate rejection; resolve
    mutually visible declaration types; then assign local slots through an
    explicit no-active-shadow lexical environment. Preserve separate grammar-
    selected namespaces and exact later-conflict source coordinates. Pin
    duplicate globals and active-local conflicts as rejection canaries; pin
    same-named type/constructor, function/local, and disjoint-scope binders as
    positive canaries; and reject duplicate pattern binders without equality
    meaning.
    - [x] Reject within-namespace global duplicates before type resolution and
      reject every active-local conflict before environment mutation. The
      bounded global pass reuses coverage scratch, checks hash collisions by
      exact spelling, retains the earliest later-declaration coordinate across
      namespaces, and keeps the 32,768-function capacity canary linear in
      ordinary hash behavior. Exact-offset negatives and cross-namespace,
      colliding-hash, branch, and arm positives are adjacent. Global identities
      now retain exact table rows; the two-phase whole-function emitter below
      publishes their runtime labels.
    - [x] Retain exact one-based function and constructor table identities on
      every checked ordinary call, constructor application, and constructor
      pattern. Zero remains the unresolved/builtin sentinel. The adjacent
      metadata gate pins a forward function call, a same-spelled type and
      constructor, both constructor applications, and exhaustive pattern
      identities without serializing a resolved tree.
    - [x] Assign every parameter, `let`, constructor-pattern field, catch-all,
      and variable reference through the same lexical environment. Parameters
      retain one-based source-order indexes; locals retain one-based fixed-slot
      indexes while disjoint scopes reuse slots. Each function profile packs
      its resolved return type, maximum simultaneously live locals, and exact
      parameter count. The 16-bit local field admits at most 65,535
      simultaneously live locals, while active parameters may exhaust the
      shared lexical environment earlier. The adjacent publication is a private
      resource failure before environment, AST, or wrapped-profile mutation. The
      metadata gate pins parameter/local kinds, slots, references, pattern
      fields, reuse-derived maximum, exact/adjacent local capacity, and profile
      counts. The variable/let/call/constructor/match source joins and
      whole-function body emission consume those profiles.
  - [x] Resolve D19's two exact application schemas without selecting a boundary
    from source names. `ConformanceBytesV1` requires the unique resolved
    `main : Bytes -> Bytes`. `DeltaCompilerV1` requires the exact source-owned
    `DeltaRejectReason`, `DeltaCompileOutcome`, `Complete(Bytes)`,
    `Reject(DeltaRejectReason, Int)`, and `main` identities. A private exact-name
    table covers identifiers through 24 bytes and fixes D17's 26 reason codes
    independently of declaration order; staged private identities are
    consumable only when the validator succeeds, and it authors no candidate
    adapter bytes. Seven adjacent cases accept both profiles, reverse every reason
    declaration while preserving the mapping, and reject a wrong entry, missing
    reason, extra outcome constructor, and wrong `Reject` payload.
  - [x] **D30 — PHYSICAL GAMMA APPLICATION PROFILES.** Fix `GCREQ` V1,
    profile IDs 1/2, both 4-MiB sealed-input maxima, the 4-MiB Conformance output
    maximum, AlphaBootstrapV2's 1,048,572-byte Delta output maximum, the shared
    generated-runtime block, and the exact `GCOUT`/`DCOUT` magics, coordinates,
    and closed tables. The checked TSV projections live beside the compiler;
    their constants belong in the offline artifact rather than becoming host
    runtime inputs. Implementation must now retain the semantic `GCOUT` reason
    alongside `FAIL_OFF`, generate the two PC-zero adapters, validate D21's
    profile invariant, supply sealed `Bytes`, preflight every success/failure
    publication, and emit no partial bytes.
  - [x] Preserve D30's generated resource identity at the sealed-input seam.
    The emitted reader now transfers input-extent and heap-extent failures to
    distinct adapter-owned terminals while containment remains independent;
    neither resource path commits `r254` or a partial descriptor. Exact and
    adjacent input/heap, zero-capacity, binary, containment, and deterministic-
    reconstruction canaries exercise the split.
  - [ ] **OWNER-BLOCKED — Q3 Beta call-row profile for the complete Gamma
    compiler.** The retained source consumes exactly 994 of D23's 1,024
    non-builtin call rows before a production entry or either adapter. A focused
    adjacent probe admits thirty further calls and refuses the thirty-first as
    canonical `Incomplete(call_rows, 1024, 1025)`. Do not hide the required
    total-`Bytes` preflight or adapters in a host-generated table/blob, weaken
    publication, or silently revise the Alpha-written compiler profile.
  - [x] **D33 — BOUNDED GCOUT ADMISSION AND TOTAL SCHEMA DIAGNOSIS.** Check the
    fixed header and profile before the selected source provision; reject an
    oversized declared length at request byte 12 without consuming its body;
    and perform body exact-end validation only for admitted lengths. Retain all
    schema reason/coordinate candidates and apply category order 19/20/21,
    truthful none-or-source coordinates, and the normative table's request-
    profile availability. The completed adapter and gate must reject a code
    impossible for the originating request rather than treating a detached
    GCOUT frame as sufficient context.
  - [x] Materialize `gamma_compiler.beta` by moving the reusable strict frontend
    into its canonical owner rather than copying it. Reserve `[10.5 MiB,11 MiB)`
    for 65,536 exact labels, `[11 MiB,13 MiB)` for 116,508 fixups,
    `[16 MiB,125 MiB)` for the bounded frontend arena, and
    `[126 MiB,128 MiB)` for emitter scratch and its downward start table.
    The direct
    emitter owns sticky failure, exact byte/word append, every Alpha operand
    shape, labels at PC zero, forward/backward fixups, duplicate/missing-label
    rejection, and the runnable 1,048,572-byte ceiling. The adjacent gate uses
    fixed temporary entries, pins exact payload bytes and capacity failures,
    and retains no alternate compiler or tape. Generated fixed-offset word
    access is centralized through two emitter helpers using caller-clobbered
    `r249`/`r250`; this changes no layout or runtime path and prevents repeated
    four-instruction address sequences from consuming the compiler's own fixed
    tape budget. The retained source now declares 116 procedures; with the
    frontend gate entry, the gate uses 117 of the persisted V2 artifact's 256
    procedure slots. It compiles to 333,928 bytes, leaving 714,644 bytes below
    the V2 runnable payload ceiling.
    That is measured pressure, not evidence that all remaining lowering and the
    adapter will fit; profile each retained milestone and escalate before the
    fixed edge is forced into an alternate architecture.
  - [x] Establish the emitted runtime containment floor before selecting D19's
    application adapter. Reserve `r252`/`r253` for the downward stack and frame
    base and `r254`/`r255` for the upward heap and its limit. Directly emit heap
    and stack reservation helpers that reject negative, overflowed, and
    adjacent-out-of-range requests before mutation and transfer to a supplied
    terminal failure label. Execute the generated Alpha payload for exact heap
    and stack boundaries, their adjacent one-byte failures, both negative
    requests, and heap-addition/stack-subtraction wrap; no case enters Alpha's
    undefined out-of-range memory behavior.
  - [x] Establish the private arbitrary-arity Gamma frame ABI independently of
    D20's source identities. Retain complete two-word values; lay out
    previous-frame and caller-cursor words, fixed local slots, and reverse-
    positioned source-order parameters in one downward explicit frame. Ordinary
    calls use Alpha `call`/`ret`, but every live return owns at least a 16-byte
    explicit frame: the guarded `[1 MiB,16 MiB)` stack therefore exhausts
    after at most 983,040 live calls while their 7,864,320 hidden-return bytes
    remain within AlphaBootstrapV2's separate hidden-return allowance. Tail calls preflight their complete
    replacement extent, copy already-evaluated two-word arguments high-to-low,
    inherit the original caller cursor, and jump without growing either stack.
    Execute 4,096 mutual grow/shrink tail transfers between 48- and 80-byte
    frames, preserve a pending caller spill across non-tail return, carry 600
    nonzero-kind arguments, and distinguish an exact 1 MiB tail landing from
    the adjacent aligned resource failure before relocation. Reject malformed
    compiler-owned frame profiles before emitting bytes. D20 now governs
    assigning calls and binder slots from source, not this ABI.
  - [x] Establish one resolver-neutral fixed-local access inside that frame ABI.
    Compiler-resolved local indexes address complete two-word values only in
    the aligned prefix after the frame header; one shared emitter expands both
    load and store through the canonical word helpers. It validates the full
    prefix, local count, index, and closed load/store mode before emitting any
    byte, and classifies malformed metadata under the existing private frame
    failure. The frame probe stores, clobbers, and reloads a nontrivial pair in
    the final local of a 48-byte prefix; its existing parameter and root-frame
    checks prove non-overlap and restoration. Focused controls reject a
    misaligned prefix, the adjacent local index, and an unknown mode with no
    payload. D20 owns source binder/reference-to-slot assignment and scope.
    Until its canonical arm/slot metadata is implemented, do
    not retain a tag-only or binderless match-lowering scaffold.
  - [x] Establish one resolver-neutral parameter accessor inside the same
    frame ABI. Validate the complete fixed prefix, bounded parameter count, and
    opaque source-order index before emission; require the combined fixed-plus-
    parameter extent to remain within the explicit-stack profile; then load the
    complete pair from the settled reverse-positioned parameter region.
    Replace the resolved-call bridge's hand-authored offsets with this accessor
    for both mixed-kind parameters across its ordinary and proper-tail paths.
    Focused controls
    reject a malformed prefix, negative count, adjacent index, and one parameter
    beyond the combined extent under the private frame failure with zero payload.
    D20 governs mapping source references to parameter indexes, not their
    runtime placement.
  - [x] Establish the private arbitrary-arity algebraic-value ABI without
    assigning source constructor identities. Consume an opaque
    resolved kind `>= 2`, copy complete argument pairs from the guarded stack
    into a source-order immutable field vector, return `(kind,pointer)`, and
    represent nullary constructors without allocation. Round odd field counts
    to 32-byte heap rows so algebraic allocation preserves the `Bytes`
    descriptor-alignment invariant. Field loads validate the compiler-owned
    pointer, complete rounded extent, alignment, and static index before memory
    access. Execute a 600-field nonzero-kind vector, nested and nullary values,
    first/last field order, malformed private pointer containment, and exact
    final-row versus adjacent heap exhaustion. Reject malformed compiler-owned
    constructor profiles before emitting bytes. D20 now governs connecting
    constructor spellings and pattern binders to these resolved tags/slots.
  - [x] Directly emit checked signed-add, subtract, multiply, divide, and
    remainder helpers. Ordinary results return in `r0`; both overflow
    directions, zero divisors, and `INT64_MIN / -1` transfer to a supplied
    terminal label before Alpha can trap. Execute 16 generated paths covering
    ordinary negative division/remainder, all exceptional classes,
    multiplication reconstruction, and the valid `INT64_MIN * 1` edge.
  - [x] Retain the eventual `lower_expr(expr, tail_position)` dispatcher and its
    first actual source-to-code slice inside the sole compiler source. After
    canonical parsing and static checking, lower closed `Int` literals, all
    seven primitive operators, and `if` directly to Alpha;
    evaluate nested operands left-to-right through the guarded explicit stack;
    call the checked helpers; and reconstruct `(kind,payload)` results.
    Conditional lowering makes its condition non-tail, propagates the caller's
    tail context to both arms, evaluates the condition once, and branches before
    its arms. Execute 20 emitted tapes covering nesting, each operation class,
    both comparison results, selected and unselected trap-bearing arms, an
    outer spill across conditional lowering, balanced stack restoration, and
    contained failures. Recompile one nested source twice and require identical
    raw payloads. This is general-pipeline material and publishes no subset
    compiler or tape.
  - [x] Establish the compact immutable `Bytes` runtime representation and its
    private helper ABI before selecting either D19 application adapter. Reserve
    one canonical zeroed `EMPTY` descriptor, allocate fixed 32-byte `LEAF`,
    `CONCAT`, and `SLICE` rows, preserve empty/full identities, and traverse
    ropes and nested slices iteratively. Validate every descriptor before a
    load; route invalid authored bytes, indices, and ranges to the supplied trap
    label, malformed private descriptors to the supplied internal-failure
    label, and actual allocation exhaustion to the supplied resource label.
    Execute all six operations, cross-boundary and nested slices, a 1,024-node
    rope, 12 invalid/malformed cases, and exact-last-row versus adjacent
    allocation exhaustion.
    D21 now classifies logical-length overflow as an authored trap. The helper
    already loads stored logical lengths, checked-adds before allocation, and
    writes the exact sum into the successful descriptor.
  - [x] Lower all six statically checked `bytes_*` forms through the eventual
    expression dispatcher. Reconstruct `Bytes` as `(1, descriptor)` and scalar
    results as `(0, value)`; evaluate one-, two-, and three-argument forms
    strictly left-to-right through the guarded explicit stack; and call only
    the private runtime helpers above. Execute 11 source-to-code tapes covering
    every form, nested ropes, a cross-rope slice, exact-end zero slicing, lazy
    conditional `Bytes` branches, an outer `Int` spill, and invalid byte/index/
    range traps. Recompile one nested `Bytes` source twice and require identical
    raw payloads.
    Focused emitter/runtime probes compile the canonical emitter section alone
    so unrelated frontend growth cannot force those diagnostics past Beta's
    fixed payload ceiling; the actual lowering probe still compiles the whole
    canonical source.
  - [x] Pin D21 with one focused emitted-runtime canary that repeatedly doubles
    a valid rope through stored logical lengths, accepts the final representable
    value, and traps on the adjacent concatenation before allocation. Distinguish
    that trap from malformed-descriptor `InternalFailure` and actual allocator
    `Incomplete`; retain the heap cursor to prove no mutation on overflow.
  - [x] Bridge already-resolved ordinary and tail calls into the eventual
    expression backend without assigning a source identity. Consume
    the canonical source-order argument list, lower every argument non-tail
    exactly once, preserve complete `(kind,payload)` pairs across guarded
    16-byte spills, and select the existing ordinary-call or replacement-frame
    emitter from an opaque callee label and fixed-prefix profile. Before
    emission, validate the complete forward arena list and bound all fixed
    frame/field arithmetic by the generated-stack profile so malformed private
    metadata cannot wrap, loop, or author a partial payload. Execute one
    compact two-argument mixed-kind payload through both paths, recover the
    source-order values in the callee, restore the root stack/frame after the
    tail return, and require byte-identical reconstruction. The D20 source
    connection below consumes this seam; it introduces no resolved-AST
    serialization or subset compiler.
  - [x] Bridge already-resolved constructor applications into the eventual
    expression backend without assigning source spelling or declaration
    identity. Calls and constructors now share one guarded canonical argument
    routine: validate the complete forward arena list before emission, lower each
    child non-tail exactly once, spill each complete value through the guarded
    stack, and derive arity from that list. The constructor seam validates only
    the resolver-supplied opaque kind `>= 2`, then delegates source-order field
    copying, nullary construction, heap rounding, and allocation containment to
    the existing algebraic-value ABI. Execute one mixed `Bytes`/`Int` result,
    recover its second field, restore the root stack, and require byte-identical
    reconstruction. A focused malformed-child discriminator requires failure
    with no emitter error or payload, preventing an incomplete lowering from
    being mistaken for zero arity. The D20 source connection below consumes
    this seam; no parallel verifier, serialized resolved tree, or duplicate
    list walk is retained.
  - [x] Bridge already-resolved local references and lets into the eventual
    expression backend without assigning source identity. Reuse the
    fixed-frame validator and emitter for complete two-word loads and stores;
    encode the let's bounded `(prefix,index)` metadata in one private scalar to
    respect Beta's four-argument call limit. Validate that profile before any
    initializer byte is emitted, lower the initializer non-tail exactly once,
    retain its complete value, and pass the incoming tail context unchanged to
    the body. Execute a mixed `Bytes` initializer/`Int` body in a real 48-byte
    frame, recover both values from distinct slots, restore the root stack/base,
    reject malformed prefix and adjacent-index profiles with zero payload, and
    require byte-identical reconstruction. The D20 source connection below
    consumes this seam.
  - [x] Connect D20's resolved source metadata to general lowering. Source tag 1
    selects the retained parameter/local kind and runtime index; tag 4 combines
    its one-based fixed slot with the current function prefix; tag 5 consumes
    the one-based function identity, compiler-owned label table, and retained
    maximum-live-local profile; and tag 7 maps the one-based constructor
    identity into runtime kinds starting at two. Execute source-derived ordinary
    and proper-tail calls, a parameter return, constructor allocation, and a
    let/local `Bytes` read; compile each bridge twice byte-identically. Malformed
    resolved metadata fails before publication. The two-phase function emitter
    below consumes the resulting labels and profiles.
  - [x] Lower D20's resolved selected matches through the general expression
    backend. Validate the complete forward arm spine, arm/pattern nodes,
    constructor identities, exact payload arities, fixed binder slots, and
    required label extent before evaluating the scrutinee. Evaluate it exactly
    once; test constructor kinds in source order; bind payload fields in source
    order or retain the complete pair for a catch-all; execute only the selected
    body; and pass the caller's tail-position bit into that body. Six executed
    source programs cover nullary and payload arms, field order, a catch-all
    rematch, sibling slot reuse, unselected traps, and a proper-tail arm call
    with root frame restoration. A one-row heap ceiling distinguishes one
    scrutinee evaluation from two; focused malformed identity, slot, and cyclic
    arm controls retain sticky private failure and zero payload; repeated
    compilation is byte-identical.
  - [x] Emit every checked Gamma function without taking ownership of D19's
    application adapter. Validate the complete function table, body pointers,
    packed return/local/parameter profiles, and exact forward parameter spines;
    preflight label capacity and allocate all source-order function labels
    before authoring any body so forward, mutual, and self calls are ordinary
    cases. Define bodies in source order, install each retained frame profile,
    lower the body in tail position, and emit the common return-frame epilogue.
    One consolidated full-source test emitter replaces five redundant compiler
    variants and executes a forward ordinary call, 4,000 mutual proper-tail
    transfers across different frame sizes, constructor/match/local/`Bytes`
    composition, selected-only traps, and deterministic reconstruction. Focused
    malformed parameter, body-tag, prepared-label, and exact/adjacent capacity
    controls require sticky failure, zero payload, and no label-table mutation.
    The test-only PC-zero wrapper deliberately selects row zero; D19 alone still
    owns canonical entry selection, sealed input, result framing, and final
    publication.
  - [x] Perform the bounded ordinary compaction required before D12 escalation.
    Merge identical global lookup and root-form scans, share the call/
    constructor type-argument zipper, remove the one-use local wrapper, and
    merge the checked divide/remainder emitters without changing emitted Alpha.
    This reduces the live source-join compiler with a trivial entry from
    254,109 to 250,761 bytes; the then-complete adjacent gate remained 193/193.
  - [x] **PROFILE-REVISION — D23: ALPHA-BOOTSTRAP-V2.** Migrate the complete
    lattice profile atomically to a one-MiB stamped seed hole and 1,048,572-byte
    raw-tape maximum without changing Alpha instructions. Both native seeds,
    stamping and assembler containers now carry 256 MiB of semantic memory and
    the exact hole. Beta-generated programs use a one-MiB stack, two-MiB
    separation, 128 MiB biased raw region, 256 procedure rows, and
    payload-dominated 262,144-PC/116,508-fixup tables. Gamma-generated programs
    use `[1 MiB,16 MiB)` stack and `[16 MiB,128 MiB)` heap regions; the Gamma,
    Delta, and Omega encoders share the new cap, with Delta's depth-20 target
    trie and Omega's 116,508 fixups derived from it. The authoritative checker
    admits a 2,810,748-byte frame and 4,893,354-node nonpaged arena; its executed
    gate retains a real exact-maximum compiler tape with representative source,
    named lemmas, normalization, scratch, simultaneous outer maxima, balanced
    arena exhaustion, and adjacent fail-closed cases. Rebuilt Beta and checker
    tapes reproduce exactly; the current consolidated Gamma gate is 205/205
    after replacing five redundant full-source compiler variants with one
    whole-function emitter. Ordinary density work remains useful but is no
    longer a condition for the retired V1 ceiling.
  - [x] Establish the dormant profile-parameterized sealed-input reader before
    selecting D19's application profile. The emitted helper consumes stdin once,
    accepts only a compiler-supplied closed maximum, returns canonical `EMPTY`
    without heap movement, and otherwise commits one flat `LEAF` descriptor and
    `r254` only after EOF and complete 32-byte-aligned extent validation. Exact
    maximum EOF succeeds; the adjacent byte and adjacent heap extent transfer
    to the supplied resource label with the descriptor row and heap cursor
    unchanged. Execute empty, binary `00 ff 41`, exact/adjacent maximum,
    maximum-zero, exact/adjacent heap, and malformed private-heap paths; require
    repeated emission to be byte-identical and reject a negative emitter
    profile before emitting bytes.
    D19 now fixes which two profiles may supply the maximum, which entry receives
    the value, and which result/wire contract applies; their exact constants and
    adapter emission remain implementation work above.
  - [x] Replay the complete fixed-up Alpha payload before publication without
    trusting the emitter call sequence. Clear and rebuild a private one-byte
    instruction-start map in the reserved `[126 MiB,127 MiB)` compiler region;
    partition every payload byte under Alpha's closed opcode/
    width table; reject unknown or truncated instructions; and require every
    encoded jump, conditional, or call target to land on a reconstructed start.
    This fixes an instruction-only generated-tape invariant rather than
    admitting jump-skipped inline data. Pin unknown opcode, truncated immediate,
    interior target, and repeated-replay scratch clearing as sticky internal
    failures after fixup resolution.
  - [x] Compact the frontend's closed identifier recognition before treating
    tape pressure as an architecture problem. Replace five dedicated keyword/
    builtin/type recognizers and the hand-unrolled `bytes_*` suffix tree with
    one exact packed-ASCII matcher. Preserve identifier-boundary checks and pin
    `bytes_emptyx`, `matchx`, `Bytesx`, and `Intx` as ordinary user spellings.
    This recovers 16,761 tape bytes and four procedure slots without changing
    Gamma meaning or emitted code.
  - [x] Reuse that packed matcher for top-level `data` lookahead and merge the
    byte-identical declared-type/constructor spelling validators into one
    nominal-name predicate. D16 gives both forms the same capitalization and
    `Int`/`Bytes` exclusions while D20 keeps their namespaces separate.
    The then-complete 170-case gate was unchanged; the retained compiler drops one
    procedure and 2,549 compiled bytes without changing accepted names,
    source coordinates, or later identity ownership.
  - [x] Close the reusable candidate front end's algebraic-match coverage rule:
    require a nonempty match on an algebraic scrutinee, reject duplicate
    constructor arms and every arm after a catch-all, and require either a final
    catch-all or every constructor of the nominal type. The epoch-marked
    constructor table and 82-case gate now live in `gamma_compiler.beta` and its
    adjacent validation; no standalone checker source remains.
  - [x] Close the first strict-parser slice in the reusable front end: require a
    nonempty function-declaration sequence and exact source exhaustion; check
    every consumed delimiter; reject malformed or unterminated argument,
    parameter, constructor, and match-arm lists without nonprogress; enforce
    D16's identifier capitalization and reserved-name rules; and require every
    data declaration to contain a constructor. These 24 parser discriminators
    are candidate compiler material, not a substitute compiler edge.
  - [x] Remove the reusable front end's compiler-sized-input alias: the 4 MiB
    source buffer no longer overlaps its type, constructor, function,
    environment, or match-coverage tables. Reserve one readable error node and
    bound the AST arena below the compiler's reserved 125 MiB edge before
    writes; Beta exposes 128 MiB of biased logical raw memory while Alpha keeps
    its hidden-return-stack allowance disjoint. A 2 MiB-boundary canary places a later
    declaration exactly where the former function table corrupted source.
  - [x] Remove recursive list parsing from the reusable front end's argument,
    constructor-field, parameter, pattern-binder, and match-arm paths. Iterative
    builders preserve source order and pass 600-argument/function and
    600-field/constructor/pattern canaries, crossing the retired interpreter's
    unrelated 512-value scratch bound.
  - [x] Bound the retained front end's AST, type, constructor, function, and
    lexical-environment writes before mutation. Private exhaustion is sticky and
    returns a readable error node rather than an out-of-range pointer; binding
    failure propagates through `let`, patterns, and function checking. A
    32,769-declaration canary crosses the exact function-table capacity without
    output or memory corruption. A 300,000-argument source exhausts the AST
    arena without crossing into generated status 251. Iterative builders stop
    linking on the first failed allocation and leave the shared readable error
    node immutable. The eventual compiler boundary maps this class to
    `Incomplete`; the Boolean oracle still reports only unsuccessful checking.
  - [x] Bound recursive expression parsing at one explicit 1,024-level private
    profile while leaving list arity iterative. A 900-level valid program checks;
    a 1,100-level program fails closed through private resource state rather
    than overflowing the Beta/Alpha return stack. This ceiling is an eventual
    `Incomplete` outcome and never a Gamma validity rule.
  - [x] Separate declaration collection from type-spelling resolution in the
    reusable front end. Constructor fields, parameters, and function results
    retain source spellings during the single strict parse and resolve only
    after every nominal declaration exists, so forward and mutually recursive
    data types now implement D16. D20 now owns duplicate declaration identity.
  - [x] Preserve source-coordinate custody through the reusable front end.
    Every syntax node now retains its zero-based starting byte offset, outer
    envelope rejection records the offending byte before tokenization, and
    parsing, integer overflow, unknown type spelling, and the first failing
    typed subexpression share one sticky first-source-failure coordinate. The
    Boolean oracle does not publish a compiler frame; the direct compiler must
    absorb this metadata into its accepted-language rejection table and final
    `GCOUT` boundary under D19's selected profile.
- [x] **GAMMA-NO-MATCH-HARDENING.** Make both tail and nested interpreter match
  paths trap rather than fabricate integer zero when no arm matches, and pin
  both with focused no-output trap canaries. The direct compiler task separately
  owns complete static match-exhaustiveness rejection. Keep the
  correlated-oracle warning explicit: the two oracles historically shared the
  omission, demonstrating that agreement alone could not establish it.
- [x] Absorb the reusable static frontend into `gamma_compiler.beta` without a
  duplicate checker source, and keep `interp.beta` only as a bounded semantic
  oracle/candidate algorithm source. Neither the oracle nor the incomplete
  compiler source is an accepted compiler artifact. The retained compiler gate
  passes 201 cases spanning 97 frontend discriminators, direct emitter and
  containment probes, checked `Int` and compact-`Bytes` runtime paths, 37
  source-to-code cases, source-derived ordinary/tail call, constructor, and
  local/let/match payloads, repeated byte-identical reconstruction, frame/algebraic
  ABIs, and sealed input. Separate oracle gates retain 48 interpreter cases,
  the fail-closed arena case, and 106 independent differential cases.
  - [x] Delete the interpreter's dead environment lookup and the
    `Node`/`Chunks`/`ZeroTree` compact representation plus 524,288-slot
    translator-carrier case. They existed for the deleted cross-rung translator,
    not for Gamma semantics or the canonical compiler edge. Rewrite the
    interpreter-first claims to classify both executables as pre-contract
    oracles.
  - [x] Remove the type checker's retired proof-kernel purpose and reject
    unknown declared types explicitly instead of allowing the shared `-1`
    error/type sentinel to compare equal.
- [ ] **DEPENDENCY-BLOCKED — incomplete `gamma_compiler.beta` and missing
  tape.** Check the exact Beta-source-to-Alpha-tape refinement and all resource
  outcomes after lowering and both D19 adapters are complete. Measure
  representative compiler-sized inputs; a 12-hour ceiling is emergency
  containment, not acceptable normal performance.

## 4. Gamma-written Delta compiler

- [x] **FREEZE-DELTA-V1.** D17 and `source/delta/LANGUAGE.md` fix one
  self-contained grammar, static semantics, execution model, boundary,
  rejection/trap taxonomy, closure presentation, and resource classification.
- [ ] **BUILD-DELTA-COMPILER.** Implement
  `source/delta/compiler/delta_compiler.gamma` to consume arbitrary valid Delta
  and emit exact Alpha tape directly. No Beta translator, Gamma evaluator
  subprocess, host encoder/decoder, native assembler stream, or older compiler
  participates.
  - [x] Author the exact 26-constructor `DeltaRejectReason`, the two-constructor
    `DeltaCompileOutcome`, and the complete first checking phase in the final
    source path. `check_lexical` validates the whole D15 envelope before a
    second whole-source token/literal scan, preserves D17 lexical phase
    priority, reports exact lexical reason/offset pairs, rescans source spans
    without retaining a token ledger, and type-checks as Gamma. The source has
    no `main` or placeholder artifact until all checking and direct lowering
    phases can return an honest complete outcome.
  - [x] Establish the complete D17 syntax representation and transient parser
    cursor foundation in native Gamma values. Every retained syntax form owns
    exact source spans; identifiers and literals retain source-span identity;
    token rescanning covers the closed keyword, literal, punctuation, and
    operator set without a whole-source ledger. Token start, code, end, and
    literal value are scalar `Int` facts, so repeated parser lookahead allocates
    no token/outcome objects in Gamma's fixed immutable heap. A
    semicolon-terminated postfix form remains syntactically neutral until
    resolution classifies an ordinary call or final `never` terminal. No
    byte-rope arena, numeric node reference, parser-time semantic guess, or
    private arity bound is introduced.
  - [x] Implement complete D17 type and expression recursive descent over that
    scalar cursor: primitive, named, array, and view types; every primary and
    postfix form; source-order arbitrary argument lists; unary minus; and all
    eighteen binary operators with fixed precedence and left association.
    Positive, array-length, and postfix-decorated `2147483648` uses reject at
    the literal while the direct unary-minus operand remains admitted. Parser
    success wrappers retain only the native AST value and are reused across
    no-op postfix/binary stages; a trivial integer expression authors 112 heap
    bytes rather than the rejected token-object design's roughly 528. The
    remaining syntax milestones are closed below; whole-closure semantics and
    direct lowering remain later phases.
  - [x] Implement D17 transition syntax: integer, Boolean, wildcard, and
    qualified-case patterns; optional source-order binders; postfix and
    `return expression?` continuations; nonempty source-order arm lists; and
    complete transition delimiters. The ambiguous expressionless arm return
    uses allocation-free scalar lookahead for a complete following
    `pattern ->` prefix rather than retaining tokens or resolving names.
    Pattern magnitude `2147483648`, trailing binder commas, missing arrows,
    empty arm sets, and incomplete delimiters fail at their exact source
    coordinate. Duplicate-pattern and sum-exhaustiveness decisions remain in
    the later body/control checker, not syntax.
  - [x] Implement reusable parameter, statement, and explicit-return parsing.
    Typed parameter lists preserve arbitrary source order without recursive
    arity growth or trailing commas. `let`, assignment, neutral postfix,
    `assert`, and `return expression?;` nodes own exact spans through their
    semicolon. The parser does not guess whether a postfix form is an assignable
    place, ordinary call, or final `never` call; resolved body checking owns
    those D17 classifications. Body, state, and top-level assembly are closed
    by the following milestones.
  - [x] Implement state declarations, state bodies, and machine bodies with
    exact brace spans and source-order tail-built statement/state lists.
    Machine parsing advances one way from statements to an optional explicit
    return/transition and then to states; after the first state only states or
    the body close are admitted. A state body must close immediately after its
    explicit terminal. Neutral postfix statements remain available for later
    resolved `never` classification rather than becoming a parser guess. The
    following milestone closes top-level declaration and whole-program syntax.
  - [x] Implement the complete top-level D17 grammar: boundary traits and their
    machine signatures, record fields and sum cases with optional payloads,
    qualified and unqualified machine declarations, exact `& mut self`
    receiver forms, optional returns, and machine bodies. All member and
    declaration lists are tail-built then restored to source order. The
    whole-program entry preserves lexical-phase priority, rejects an empty
    source at its extent, requires at least one declaration, and consumes the
    exact source through trivia to EOF. `delta_parse_program_syntax` now parses
    every D17 grammar form without claiming collection, type/control checking,
    lowering, or a compiler artifact.
  - [x] Implement the pure final symbolic-Alpha encoder used after Delta
    lowering. Its closed, nonempty compiler IR covers all 21 Alpha instructions
    plus dense symbolic labels and admits forward/backward references and
    aliases. Successful encoding requires every allocated label to bind exactly
    once and emits the exact raw at-most-1,048,572-byte AlphaBootstrapV2
    payload. It is
    intentionally not a general Alpha
    assembler: empty instruction streams are outside the compiler relation and
    a referenced label must resolve to an instruction start, not the payload
    end. Layout stops at the first over-limit instruction and constructs the
    exact short oversize candidate for the adapter's mandatory output preflight;
    no partial payload is publishable. Balanced immutable serialization avoids
    linear rope depth, then an independent two-pass replay partitions the raw
    opcodes and proves every distinct direct target is an instruction start.
    The implementation uses no host encoder, decoder, evaluator, or older rung
    and type-checks through the Gamma frontend gate. Executed exact-vector and
    mutation canaries join the real Gamma-compiler gate once that edge exists.
    Immutable heap use scales with authored instructions and distinct direct
    targets, so private exhaustion may honestly become outer `Incomplete` below
    Alpha's payload cap. Profile the real `D` closure; terrible performance,
    unacceptable heap pressure, or pressure to extend Alpha instructions is an
    owner-escalation trigger, not permission for a hidden alternate backend.
    Its exact preflight, depth-20 target trie, oversize sentinel, and adjacent
    vectors now share the common V2 profile.
  - [x] **IMPLEMENTATION — D24: DELTA-CENSUS-BINDERS-PRIORITY-V1.** Implement
    D22 and D24's exact Delta identity census before type formation. The
    source-shaped collector first builds complete owner and exact qualified-
    machine rows, then covers grammar-selected member, state, parameter, let,
    and transition-binder scopes with byte-exact authored-name comparison.
    It rejects the globally earliest later conflict, preserves ordered let
    visibility without active shadowing, permits local/member and disjoint-
    state spelling reuse, and diagnoses authored bodies on uniquely classified
    boundary owners as `InvalidBoundary`. Every syntactic transition binder is
    collected even when later case or arity checking will fail; sibling arms
    are disjoint; duplicate and boundary candidates share one coordinate
    minimum; and every duplicate owner table, including same-kind duplication,
    is ambiguous rather than first-wins classified. The complete source
    type-checks through the Gamma frontend. Runtime behavior remains owned by
    the dependency-blocked real-compiler suite below; no host collector was
    introduced.
  - [x] Begin D17 type formation with the declaration-order-independent portion
    that is already exact. After successful D22/D24 census, validate every
    named type in boundary signatures, record fields, sum payloads, machine and
    state parameters, returns, and `let` declarations against the complete
    owner table. Retain the globally earliest unknown spelling candidate at its
    exact type start beside the collected native syntax for later shape,
    recursion, body, and lowering passes. Do not promote that candidate to a
    rejection before D31's structural priority is implemented. The source
    type-checks through the full Gamma frontend gate;
    behavioral canaries remain dependency-blocked on the real Gamma compiler
    edge.
  - [x] **D31 — DELTA-TYPE-FORMATION-V1.** Fix lengths `1..INT32_MAX`, empty
    data as one zero-field record, mixed-data rejection, exact
    `never`/view/`Console` placement, and disjoint structural anchors. Valid
    source that exceeds one selected application-static-storage profile returns
    attributed or aggregate outer `Incomplete`; it is never a Delta rejection.
  - [x] **IMPLEMENTATION — D31 STRUCTURAL TYPE FORMATION.** Replace the
    unknown-name-only precursor with the complete profile-independent
    structural judgment. It classifies every empty declaration explicitly as
    a zero-field record, rejects mixed data at the declaration name, enforces
    positive array lengths, admits `never` and views only at their exact outer
    placements, suppresses every child defect beneath a forbidden view, and
    treats exact `Console` spelling as the sealed `Main.console` capability
    before ordinary owner lookup. One source-coordinate candidate sum merges
    all placement, shape, unknown, and recursive-value failures by offset;
    same-class ties are idempotent and distinct same-anchor reasons take the
    private internal-contradiction path rather than a reason-table priority.
    The accepted formed program retains source-ordered record/sum rows and a
    complete immutable data-containment edge graph. Per-edge visited-set
    reachability marks every recursive edge at its named-reference coordinate
    without path-exponential expansion through acyclic diamonds. The native
    pass now promotes the winning candidate after D22/D24 census and type-
    checks through the Gamma frontend gate. Exact behavioral vectors remain in
    the adjacent contract-derived plan until the real Gamma compiler can
    execute them; no host evaluator is introduced.
  - [x] **D31 OUTCOME-SCHEMA PLUMBING.** Retain the two source-owned
    `StorageIncompleteAt`/`StorageIncompleteTotal` constructors and the D19
    Gamma-profile schema check. This establishes nominal plumbing only and
    claims no storage-demand calculation or refusal behavior.
  - [x] **D34 — BOUNDED APPLICATION-STATIC-STORAGE WITNESS.** Preserve D31's
    profile-independent validity and existing V1 outcome/frame. Require the
    selected limit below `INT64_MAX`; report exact demand while representable
    and `INT64_MAX` otherwise; compute through nontrapping
    `Exact | Overflowed` addition/multiplication; and select an attributed
    array structurally by outermost then packed coordinate. Composition-only
    excess remains aggregate with no coordinate. Reserved frame bytes stay
    zero and no refusal publishes tape bytes.
  - [ ] **OWNER-BLOCKED — Q4 DELTA ENTRY-SHAPE TOTALITY.** The accepted
    `Console`/`Main`/`Main::main` headline is fixed, but `MissingEntry` versus
    `InvalidEntry`, absent-component and malformed-component coordinates,
    boundary member order/binder-name sensitivity, and ties with ordinary
    body/control failures are not total. Retain entry candidates for the final
    phase, but do not promote a rejection or publish golden coordinates until
    Q4 settles them. This does not block the independent expression, statement,
    state, transition, and return judgments.
  - [ ] **IMPLEMENTATION — DELTA BODY/CONTROL CHECKING.** Resolve every value,
    type owner, callable, constructor, field, state, and control target against
    the complete census and formed shape graph. Check ordered initialization,
    value/place/call classification, arity and type equality, terminal and
    return obligations, duplicate patterns, and sum exhaustiveness. Accumulate
    all final-phase candidates by packed coordinate and merge the eventual Q4
    entry candidates before accepting one resolved program; traversal and wire
    reason order must not select the diagnostic.
  - [x] **IMPLEMENTATION — DELTA RESOLUTION-CATALOG FOUNDATION.** Build one
    source-ordered row per already formed top-level declaration while retaining
    the original AST owner for boundary members, fields/cases, machine bodies,
    and states. Qualified machine rows resolve their data owner or retain the
    exact unknown owner; unqualified rows remain explicitly ownerless. Exact
    owner, unqualified/qualified machine, boundary-member, data-member, and
    machine-local state lookups consume those rows without numeric node IDs or
    a second flattened syntax tree. Structural type equality compares nominal
    names and semantic array lengths, and a neutral final-phase bucket retains
    every reason tied at the smallest coordinate until Q4/Q6 settle
    composition. The foundation type-checks through the real Gamma frontend;
    it claims no completed body judgment or behavioral execution.
  - [ ] **OWNER-BLOCKED — Q6 DELTA RECEIVER/CALLABLE AMBIGUITY.** Do not assign
    a type to unqualified `&mut self`, or choose between a same-spelled
    constructor and qualified machine by expected type, arity, or traversal.
    Q6 blocks only those cases; retain both callable lookups and continue every
    unambiguous body/control judgment.
  - [ ] **DEPENDENCY-BLOCKED — D31/D34 APPLICATION STATIC STORAGE.** After
    complete body/control checking and the final nonaliasing generated-program
    map exist, derive its selected static-storage limit and expand only
    reachable roots. Implement D34's bounded arithmetic, deterministic
    attributed/aggregate refusal, exact-versus-witness boundary, zero-factor
    path, adapter validation, and no-publication canaries. Do not trap, choose
    undocumented saturation, impose a Delta validity limit, or report a
    traversal prefix.
- [x] Derive compact positive, negative, trap, and private-budget `Incomplete`
  conformance directly from settled portions of the Delta contract. Include
  D22's namespace, boundary-owner, duplicate-priority, active-shadowing, and
  disjoint-state vectors. Do not recreate cases that
  merely pin quirks of the removed translator or materialize another unrun
  corpus.
  - [x] Record the compact settled-contract matrix adjacent to the compiler.
    It covers lexical/parse phase priority and coordinates, all grammar and
    checking families, symbolic Alpha encoding/replay, all nine runtime traps,
    and boundary/adjacent private-resource obligations without materializing an
    unrun file corpus or claiming execution evidence.
  - [x] Complete D24's transition-binder and mixed `DuplicateName`/
    `InvalidBoundary` rows in the adjacent conformance plan. It splits sibling
    reuse from the `UnknownName` sibling-reference rejection; pins active-outer
    collisions and both `DuplicateName -> UnknownName` and `DuplicateName ->
    ArityMismatch` two-round diagnostics; and covers both unrelated-failure
    source orders plus the boundary/data-ambiguous owner. These are contract-
    derived planned vectors, not claimed execution evidence.
- [ ] **DEPENDENCY-BLOCKED — incomplete `delta_compiler.gamma`.** Materialize
  and run that
  contract-derived suite through the real Gamma-written compiler and bind every
  outcome to its no-partial-tape behavior.
  - [x] Delete `exprc.delta` and `minic.delta`; both were demonstrations of the
    removed Darwin-native route rather than authoritative Delta observations.
  - [x] Delete the unrun 43-file pre-migration Delta corpus rather than
    classifying native-backend slices as language tests. It mixed retired
    Darwin/ARM layout and trap assumptions, deleted `contracts.sh` workflows,
    demonstrations, and unresolved keyword/domain/result/builtin proposals.
    Derive a compact positive/negative suite from D17 and run it through the
    actual Gamma-written compiler.
- [ ] **DEPENDENCY-BLOCKED — missing Gamma/Delta compilers.** Check
  exact Gamma-source-to-Alpha-tape refinement, including realistic source
  closures large enough to compile `D`.

## 5. Delta-written full Omega compiler `D`

- [ ] **OWN-OMEGA-D.** Author one exact package-resolved closure `D` at
  `source/omega/omega_compiler.delta`; do not preserve historical filenames,
  snapshots, or native-publication adapters as authorities. This is downstream
  of the frozen Delta/Gamma contracts; source authoring need not wait for the
  physical compiler artifacts. The deleted prototype
  remains available in Git for selectively re-deriving an isolated algorithm,
  but it cannot be restored or copied as a compiler-shaped starting point.
  - [ ] **OWNER-BLOCKED — Q2: COMPLETE D25 OMEGA-COMPILER-REQUEST-WIRE-V1.**
    Complete the byte-exact `OCREQ` and `OCOUT` profiles shared by `D` and `C`.
    Encode the committed canonical subject and invocation, structural package
    keys, separately selected immutable revisions, graph indices, closed-tree
    snapshots, products, targets, and admissions. V1 carries no preaccepted
    `PackageInstance`. Reject every raw `u32` high bit before signed Delta
    conversion, validate exact end and canonical order, and implement the
    shared 40-byte `OCOUT` header plus its sole eight-byte package/source
    coordinate tail. Retain the edge-owned closed reason/resource tables and
    exact scalar profile provisions; neither compiler may use the Rust request
    object, host serde, paths, replay, or a private replacement as wire
    authority.
    - [x] Implement the settled outer `OCREQ` envelope in `D` without claiming
      inner request admission. `OmegaRequestEnvelope::frame_ocreq` validates
      the exact identity and reserved bytes, rejects each raw little-endian
      `u32` high bit before signed conversion, uses subtraction-dominated
      section bounds, requires exact end, and retains only the subject and
      invocation spans on success. It does not call the source parser, invent a
      package/source identity, add `Main`, or publish `OCOUT`. Exact malformed
      frame vectors remain assigned to the real Delta-compiler gate.
  - [ ] **OWNER/DEPENDENCY-BLOCKED — Q2 AND INCOMPLETE D:
    D18/D25 OMEGA-COMPILER-REQUEST-V1.** Implement the canonical sealed Omega
    compiler edge for both `D -> omega0` and `C -> omega`: encode the resolved
    `OmegaCompilationSubject` and bound `OmegaInvocation`, complete deterministic
    build-visible package snapshots, explicit bootstrap Alpha-tape product, and
    `OCOUT` boundary. Decode and validate the graph, identities, custody,
    lengths, and exact end before source processing. Inside each compiler,
    retain the existing coherent typed frontend together with its prepared
    static-machine-specialized projection, reach plans, source commitment, and
    authority verdict as one activation-local admitted checkpoint. Execute the
    selected root build only from that projection; place its generated source in
    a later one-way-visible scope; then continue checked lowering from the
    retained base. Dependency activations publish durable generated-source
    bundles and evidence rather than cascading builds or retaining live partial
    checkpoints. Generated-source failures retain their local generated path
    and offset but re-anchor `OCOUT` to the authored `include_source` handoff.
    Add exact request/framing, scope-stratum, no-reread, dependency-bundle,
    diagnostic-order, resource, and no-partial-output gates.
  - [x] Establish the final Delta-side Alpha tape encoder in `D`. It owns the
    complete closed opcode-shape table, paired-`i32` representation of arbitrary
    64-bit immediates, instruction-atomic capacity checks, bounded address
    fixups, and the exact raw 1,048,572-byte AlphaBootstrapV2 payload ceiling.
    Its 116,508-fixup bound is dominated by the shortest nine-byte direct
    reference. Sealing clears and
    reconstructs the complete instruction-start partition, rejects unknown or
    truncated instructions, and requires every direct target to land on a
    reconstructed start. Native seed stamping alone owns the descriptive
    four-byte length prefix; it is not part of the `.tape`. Because Delta has no
    private visibility, even the reserved-write helpers independently enforce
    open state, byte range, and whole-write capacity rather than relying on a
    prose-only caller precondition. `D` deliberately has no `Main`, source
    protocol, package lookup, publication, or placeholder compiler result while
    `OMEGA-COMPILER-REQUEST-V1` is unimplemented. Its ceiling, replay bounds,
    request/output resource row, and adjacent failure now share the common V2
    seed/checker profile.
  - [x] Give `D` explicit symbolic control-flow ownership before lowering.
    Monotonic typed label IDs bind once; each label-bearing emitter records the
    exact most-recent instruction and its single address operand; operand
    offsets are strictly increasing; and sealing resolves every recorded fixup
    through its bind-once label between an unpatched partition reconstruction
    and the final target replay. The 116,508-fixup ceiling is dominated by the
    exact Alpha payload extent; the independent fixed label storage is a
    private compiler ceiling and maps to outer `Incomplete` through D18's
    resource framing. There is no arbitrary public patch operation or
    unresolved-zero convention. Exact forged-owner, stale-map, forward/
    backward/alias, undefined/end-label, duplicate-bind, interior-target, and
    capacity canaries join the real Delta-compiler gate when that executable
    exists; do not create a host Delta executor to run this incomplete closure.
  - [x] Establish `D`'s source-view UTF-8 framing primitive independently of
    D18's package/source custody. It accepts an immutable byte view, implements
    the complete one- through four-byte scalar envelope, rejects overlong
    forms, surrogates, values above U+10FFFF, stray continuations, and truncated
    tails, and reports the malformed scalar's lead-byte offset. It does not
    invent source IDs, unit ordering, token custody, or where otherwise-valid
    non-ASCII scalars are permitted by LEXICAL-PROFILE-V1. Exact boundary and
    malformed-family vectors join the real Delta-compiler gate rather than a
    host reimplementation.
  - [x] Establish `D`'s complete source-neutral lexical scanner independently
    of D18's package/source custody. It implements the exact current Omega
    keyword and punctuation sums, maximal-munch number behavior, strings and
    fixed escapes, nested block comments, line-comment span/advance split,
    ASCII identifier/profile rules, and whole-view UTF-8 diagnostic priority.
    The canonical standalone token query validates the entire immutable view;
    the linear whole-view pass uses a bounded current-token stage only after
    that same view's preflight. Because D17 has no private machine visibility,
    every view-accepting factored stage independently rejects invalid cursor
    and extent shapes before indexing; construction helpers guard their local
    arithmetic. Only the two canonical entry machines make lexical judgments.
    The scanner retains only relative token/diagnostic spans and
    decoded string length: it invents no source identity, package order, token
    ledger, or decoded-byte mirror. D18 fixes source size/admission and outer
    `Incomplete` framing. Exact lexical vectors join the real
    Delta-compiler gate; do not add a host lexer or test executor.
  - [ ] Parse semantic tokens inside one canonical parser-machine invocation.
    That entry validates the complete immutable view once, then threads the
    same view through private internal states that call the bounded scanner
    stage, skip trivia by `next_cursor`, and build source-shaped syntax. Do not
    retain a standalone validate-once cursor: D17 has neither private machines
    nor a storable source view, so a public `initialized`/`preflighted` bit can
    authorize a substituted source. Revalidating the whole view on every
    public advance is correct but quadratic and is not an acceptable final
    workaround. The parser must expose no token ledger, decoded mirror,
    source-identity guess, or transferable lexical fact.
    - [x] Establish the first retained source-shaped slice in that canonical
      invocation: sequence empty/trivia-only views and ordinary
      `use path::member;` roots, skip trivia only by the scanner's guarded
      `next_cursor`, and preserve ordered path members and relative byte spans
      in fixed parser-owned tables. Require exact progress after every bounded
      scan and require token absence to coincide with view end. A lexically
      valid unimplemented root records implementation-incomplete rather than a
      false Omega rejection; malformed use paths retain only two internal
      parser distinctions with relative spans. Repeated invocation resets all
      observable counts/status, and no source ID, alias, token ledger, decoded
      mirror, standalone cursor, or `OCOUT` tag is introduced. The 4,096-root
      and 16,384-path-member ceilings are private compiler budgets whose eventual
      outer `Incomplete` mapping remains D18-owned; profile both against the real
      `C` closure before publication rather than treating the provisional
      values as semantic necessities. Executed vectors wait for the real
      Delta-compiler gate; do not add a host Delta executor.
    - [x] Extend that same invocation with one mixed root ledger and basic
      `[pub] data` syntax. Preserve authored use/data order; optional `[copy]`;
      empty, field-only, case-only, and mixed bodies; contextual
      `case: Type` fields; payload-free cases; bare named type references; an
      optional final member semicolon; and relative child spans reachable from
      compact kind/index ledgers into separate live-prefix tables.
      Only `Complete` authorizes a consumer to inspect those tables; every
      other status may leave unowned partial prefixes and publishes no tree.
      At that checkpoint, other public roots and rich valid forms such as
      payload cases, generic/array/qualified types, and numbered members or
      properties record implementation-incomplete rather than a false
      rejection. No parser
      helper machine, source/package identity, symbol or type resolution,
      public outcome code, or second file is introduced. Provisional backing
      tables retain at most 4,096 roots and 16,384 path or data members. Root
      capacity dominates the separate use/data tables, and data-member capacity
      dominates the direct-field/case tables.
      Governed built-ins and plausible richer suffixes remain incomplete until
      their full forms are retained. Exact/adjacent resource controls and the
      source-shaped positive/negative/incomplete vectors join the real
      Delta-compiler gate rather than a host executor.
    - [x] Retain structured case payloads in that same parser invocation.
      Cases own a contiguous span in one separate payload-field arena; empty,
      multiple, trailing-comma payloads and an optional final case semicolon
      share the direct-field name/colon/type control path through an explicit
      destination mode. This slice deliberately accepts only the existing
      bare named type leaf. Numbered identities, field relevance, and richer
      payload types remain implementation-incomplete. Combined direct and
      payload fields make the 16,384-row type-reference ceiling genuinely
      independent; that ceiling dominates the equally sized payload-field
      arena, so no fake payload resource identity is added. Preserve
      `Complete`-only publication, relative source spans, authored root/member
      order, and D18 request neutrality.
    - [x] Replace the named-only type row with a compact tagged type-node arena
      and retain one unqualified `Base in Domain` suffix for direct and payload
      fields. The constrained root points backward to its named base and to one
      source-shaped domain constraint; import and domain components share the
      bounded path-member arena. Unknown domain names remain valid syntax for
      later resolution. Missing domain names reject syntactically, while
      qualified/indexed/intersected domains, range constraints, recursive
      types, generics, and other richer valid forms remain
      implementation-incomplete. Reserve all type-node payload words in one
      final-extensible record rather than allocating a full side arena per
      future variant. Atomically check the one- or two-node requirement and
      the optional path component before publishing a field row. `TypeNodes`
      is now independently exhaustible; its equal ceiling dominates the
      payload-field and constraint tables in this slice. Preserve the same
      invocation-local view custody, relative spans, reset behavior,
      `Complete`-only publication, and D18 request neutrality. Executed and
      exact-edge resource vectors wait for the real Delta-compiler gate.
    - [x] Retain the first range-refined field type on the same constrained
      node path: `Base [minimum..=maximum]` with nonnegative integer-literal
      bounds in the unsuffixed decimal spelling needed by the current compiler
      closure. Other bases and suffixes remain incomplete until literal parsing
      owns their validity. The constraint row keeps the exact authored bound
      spans and kind; it does not evaluate, normalize, or choose a numeric
      carrier during parsing. Direct and payload fields reuse one range control
      path and the same constrained-node finalizer as domains. Empty, named,
      exclusive, expression-bound, multiple, combined-domain, and otherwise
      richer valid constraint forms remain implementation-incomplete; a
      missing bound or close at source end is a syntax failure. Generalize the
      constraint row into kind-selected payload words without allocating
      parallel full-size variant arenas, and preserve the existing TypeNodes
      dominance and atomic field publication. Exact semantic and capacity
      vectors still belong to the unavailable Delta-compiler gate, not a host
      parser.
    - [x] Retain recursively nested fixed-array field types over one bare named
      leaf: `[Type; length]`, with unsuffixed decimal length spans and the
      existing optional unqualified domain on the completed outer array. One
      128-row invocation-local frame stack records authored outer-to-inner
      lengths; finalization walks it backward to emit Named, inner-to-outer
      FixedArray, and optional Constrained nodes in strict postorder. Compute
      and check the entire `1 + depth + optional_constraint` TypeNodes demand
      before emitting any node, and retain `TypeDepth` as an independently
      meaningful private resource. Slice forms, rich element types, named or
      called lengths, other literal spellings, and inner constraints remain
      implementation-incomplete rather than false rejection. Route ordinary
      fields and contextual `case: Type` fields through one shared per-field
      type reset so no prior frame or constraint can leak. This completes the
      type shapes used by every current `C`-closure data field, without claiming
      that the surrounding root grammar or full closure is implemented. Keep
      exact depth/node/delimiter vectors at the real Delta-compiler gate.
    - [x] Retain the first durable machine root without inventing a body skip:
      a bare ordinary machine with an arbitrary name-like path, optional empty
      parentheses, and an immediately empty body. Publish it in the existing
      mixed root ledger and retain the one implicit empty entry state required
      by the canonical parser even for a zero-parameter Unit machine. A free
      path records the generated `entry` identity; an attached path records the
      final authored member as the entry identity, while the full path reuses
      the shared path-member arena. Root capacity dominates the equal machine
      and implicit-state tables, and only `Complete` publishes their prefixes.
      Parameters, receivers, returns, generics, clauses, `pub`/`boundary`/
      target-scoped and bodyless forms, and every nonempty body remain
      implementation-incomplete rather than being skipped, guessed, or falsely
      accepted. No current `C` machine has an empty body, so this checkpoint
      establishes the durable representation and control boundary without
      claiming present `C`-closure progress. Exact positive, malformed,
      incomplete, capacity, reset, and mixed-order vectors wait for the real
      Delta-compiler gate.
    - [x] Generalize the existing field-type continuation into one explicit
      consumer-neutral type engine, then use it for complete comma-separated
      ordinary `name: Type` machine parameter lists. Direct fields, case
      payload fields, and parameters share the same named, unqualified-domain,
      inclusive-literal-range, and nested-fixed-array parser and postorder node
      materializer; no signature-specific type parser exists. The implicit
      entry state owns the machine's contiguous parameter span and each row
      retains canonical const/mutable/self flags as false until those forms are
      implemented. Reject a trailing comma exactly as the canonical parser
      does. Because machine and domain paths share one arena, snapshot the
      declaration path before parameter types append domain members so free-vs-
      attached identity and the final entry-name member cannot drift. Every
      parameter consumes at least one type node, so `TypeNodes` dominates the
      equal parameter table without a fabricated resource kind. Modifiers,
      receivers, references, returns, generics, clauses, prefixed forms, and
      nonempty bodies remain implementation-incomplete. This still completes
      zero current `C` machine roots; it is the reusable signature foundation,
      not a closure-coverage claim. Exact list/delimiter/path-isolation/
      capacity/reset vectors remain assigned to the real Delta-compiler gate.
    - [x] Retain the complete simple state-parameter prefix matrix and one
      elided-lifetime outer Reference type without creating a receiver or data-
      field special case. Parse canonical optional `const`, then optional
      leading `mut`, then optional shared/mutable/write-only binding `&`; retain
      consuming or referenced `self` and ordinary named binders with exact
      const/mutable/self flags. A leading `mut` overrides a receiver reference
      to Mutable exactly as the canonical parser does, while a non-self binding
      `&` affects flags but does not wrap the post-colon type. Data fields, case
      payload fields, and parameters all accept the same outer shared/mutable/
      write-only Reference around the existing named/domain/range/fixed-array
      tree. Emit SelfType or Named first, then inner-to-outer FixedArray,
      optional Constrained, and optional Reference nodes in strict postorder;
      Reference payloads reserve the final lifetime shape while this checkpoint
      sets `has_lifetime = 0`. Include that optional node in the atomic
      TypeNodes demand before every write, preserving TypeNodes dominance of
      the equal parameter table and Complete-only publication. Explicit
      lifetimes, general type-position `Self`, Slice, returns, prefixes,
      clauses, and nonempty bodies remain implementation-incomplete. This
      represents 105 of 113 root-header parameter occurrences and 65 of 73
      complete root parameter lists in current `C`; 40 headers now reach body
      parsing, but zero roots complete because every reached body is nonempty.
      Keep exact
      modifier/access/flag/postorder/resource/reset vectors at the real Delta-
      compiler gate.
    - [x] Retain bracket slice syntax `[T]` through the existing shared type
      engine and bounded bracket stack. Classify each completed bracket frame
      as FixedArray or Slice, then emit its node while walking the authored
      nesting inside-out; both kinds point backward to their element and only a
      FixedArray owns a length span. This permits arrays of slices, slices of
      arrays, and nested slices without recursion, a consumer-specific parser,
      or a second resource budget. A Slice frame still consumes exactly one
      type node, preserving the existing atomic TypeNodes demand and TypeDepth
      ceiling. Constrained slice elements, comma constraints, the `Slice<T>`
      spelling, and richer element types remain implementation-incomplete
      pending their general grammar rather than receiving closure-specific
      shortcuts.
      The eight previously missing current `C` root-header parameter
      occurrences are plain bracket slices, so all 113 occurrences and all 73
      root parameter lists are now representable; 41 headers reach body parsing,
      but zero roots complete because every reached body remains nonempty. Keep
      exact bracket-kind,
      nesting, delimiter, postorder, resource, and reset vectors at the real
      Delta-compiler gate.
    - [x] Retain an ordinary machine's optional immediate `-> Type` on its
      implicit entry state through the same consumer-neutral type engine used
      by fields and parameters. Both parameterized and zero-parameter machines
      enter the shared parser from the arrow, and only an opening body commits
      the completed postorder type root to machine scratch; the state row still
      publishes atomically only after its empty body closes. Missing and
      unterminated returns retain machine-return-specific private diagnostics.
      Return nodes consume TypeNodes but need no new row arena or resource
      class. Returns after clauses, generic return types, clauses themselves,
      prefixed roots, bodyless declarations, and nonempty bodies remain
      implementation-incomplete. Five current `C` roots have simple returns,
      but four are already stopped by target prefixes; the remaining private
      root now reaches body parsing, raising the current total from 41 to 42
      without claiming a completed root. Keep exact arrow/type/delimiter,
      consumer-isolation, resource, reset, and state-publication vectors at the
      real Delta-compiler gate.
    - [x] Generalize the existing `pub` root dispatcher from Data-only handling
      to ordinary machines without cloning the machine parser. The direct root
      path supplies `is_public = 0`; the recursive public-root path preserves
      the `pub` coordinate as item start and supplies `is_public = 1`; both then
      enter the same root-capacity check, path/signature/body states, implicit
      entry construction, and final mixed-root publication. Unsupported public
      declaration kinds remain implementation-incomplete instead of silently
      losing visibility. All eight public machine roots in current `C` have
      otherwise representable headers, so they now reach body parsing and raise
      the current total from 42 to 50; their bodies are nonempty, so no root
      completes. Keep exact direct/public reset, span, visibility, capacity,
      mixed-order, and publication vectors at the real Delta-compiler gate.
    - [x] Retain canonical identifier-led target-scoped machines as ordinary
      machine rows with an exact optional target-selector span. A target prefix
      must be followed by the `machine` keyword, then enters the same root
      capacity, path, signature, body, implicit-state, and publication path as
      every unscoped machine. No target is selected or activated during parsing,
      no target ABI enters the bootstrap lattice, and the four empty `target`
      declarations in `build.omg` remain deletion-bound scaffolding rather than
      motivation for a second root model. Public target-scoped combinations and
      non-machine identifier-led roots remain implementation-incomplete. Of the
      20 current target-scoped machines, 16 stop later at unimplemented clauses;
      the four `provider_defaults` headers now reach their nonempty bodies,
      raising the current body-boundary total from 50 to 54 without completing a
      root. Keep exact target span/presence, unscoped-reset, malformed-prefix,
      capacity, mixed-order, and publication vectors at the real Delta-compiler
      gate.
    - [x] Complete two settled leaves of the shared type engine without adding
      a consumer special case. General `Self` now emits the same payload-free
      SelfType base as receivers, including inside nested fixed arrays and
      slices; only a structural outer bracket may subsequently take a domain
      suffix, matching the canonical parser's early return for bare `Self`.
      Outer references accept one optional `'name` before `mut`/`write`, retain
      its exact span in the existing reserved Reference payload, and reject a
      missing, strict-keyword, repeated, or lifetime-after-access ordering
      violation through the ordinary private type diagnostics. Reference node
      demand and postorder are unchanged; the retired `&relaxed` spelling now
      rejects instead of masquerading as future syntax. Current `C` signatures
      use neither form, so body-boundary coverage remains 54 of 73; this is
      full-language progress rather than a closure-shaped shortcut. Keep exact
      Self nesting, bare-versus-structural domain, lifetime ordering/span,
      access, reset, postorder, and consumer-isolation vectors at the real
      Delta-compiler gate.
    - [x] Retain the payload-free Unit type `()` through the same shared base
      node path as Named and SelfType. Exact paired delimiters admit Unit as a
      direct field, parameter, return, or outer-reference referee, and as the
      leaf of the existing bounded fixed-array/slice stack. Bare Unit rejects a
      domain suffix while a structural outer bracket may take one, matching the
      canonical parser. Unit consumes the already-counted one base TypeNode and
      needs no arena, resource class, or consumer branch. Current `C` signatures
      do not use Unit, so body-boundary coverage remains 54 of 73. Keep exact
      delimiter/EOF, nested bracket, reference, domain, reset, postorder, and
      consumer-isolation vectors at the real Delta-compiler gate.
    - [x] Retain the first honest nonempty machine bodies: zero or more ordinary
      semicolon-terminated path-call statements whose arguments are name/self
      paths. Mirror the canonical call-statement shape instead of preserving an
      opaque body span: flatten the receiver into an exact member span plus
      starts-at-self bit, retain the exact target member, store argument roots
      in a contiguous handle table, and represent each current argument as a
      tagged path-expression node. The implicit entry state owns a contiguous
      statement span; machine/root publication still occurs only after the
      closing brace, while any failed or incomplete parse leaves its partial
      prefixes unpublished. Calls accept zero arguments, multiple arguments,
      member paths, and a trailing comma. A bare `self()` is not misclassified
      as a callable target. Static/evidence arguments, operational
      acknowledgements, discarded results, nested/richer argument expressions,
      call chaining, assignments, locals, transitions, and every other
      statement kind remain implementation-incomplete rather than being
      skipped. Statements dominates the equal call table, Expressions dominates
      the equal current argument-handle table, and call/argument paths reuse
      PathMembers only after the declaration path is snapshotted. This completes
      four real current `C` roots—`Lexer::{append_source_byte,reject,push_token}`
      and `Parser::parse`—while all 54 representable headers still reach the
      body boundary. Keep exact receiver/target/argument spans, trailing-comma,
      malformed delimiter, capacity, reset, multi-statement, mixed-root, and
      Complete-only publication vectors at the real Delta-compiler gate.
    - [x] Retain ordinary `target = value;` assignments alongside calls in the
      same machine-body statement ledger. This first value slice preserves
      self/name place paths, self-member and qualified-name value paths,
      booleans, and unsuffixed nonnegative decimal integer literals as tagged
      expression nodes; it neither evaluates literal values nor flattens an
      assignment into a call. Each assignment row owns exact target/value
      expression handles and its authored statement span. Richer postfix,
      unary, binary, cast, indexed, struct, string, and floating expressions,
      compound assignments, locals, transitions, and final expressions remain
      implementation-incomplete. `Statements` dominates the equal assignment
      table, while the independently exhaustible `Expressions` budget accounts
      for both sides before Complete-only machine/root publication. This
      completes seven more real current `C` roots—`SourceUnit::clear`,
      `TokenStream::{clear,reject}`, `SyntaxTrees::{clear,reject}`,
      `Parser::initialize`, and `Parser::initialize_cursor`—and also completes
      the previously omitted `Lexer::tokenize` root through the same ordinary
      call/assignment/transition grammar. Completed roots therefore rise from
      four to twelve while all 54 representable headers still reach the body
      boundary. Keep exact path-kind, literal-span, delimiter,
      expression/statement-capacity, reset, mixed-call/assignment, and partial-
      publication vectors at the real Delta-compiler gate.
    - [x] Retain the first canonical static machine-call argument lane:
      nonempty comma-separated path arguments in `<...>` immediately before
      the value-argument list. Each path-only static argument owns its exact
      member span and authored extent in a separate tagged arena; each call
      owns the corresponding contiguous span independently of its runtime
      arguments. Qualified paths are retained, while const arguments, evidence
      projections, nested static applications, lifetime arguments, empty or
      trailing-comma lists, and non-call comparison uses of `<` remain
      implementation-incomplete. Every retained static argument owns at least
      one same-capacity path-member row, so `PathMembers` dominates the static-
      argument table; no unreachable resource distinction is invented. The
      lane does not overload `Expressions` or instantiate generics during
      parsing.
      This completes all four target-scoped
      `ConsoleNativeProvider::provider_defaults` roots, raising completed
      current `C` roots from twelve to sixteen while body-boundary coverage
      remains 54 of 73. Keep exact path/list/delimiter, static-versus-value
      ownership, capacity, reset, and Complete-only publication vectors at the
      real Delta-compiler gate.
    - [x] Share one retained primary-literal path between call arguments and
      assignment values, and add source-shaped string expressions beside the
      existing booleans and unsuffixed nonnegative decimal integers. Boolean
      rows own their value and exact span; integer rows own exact spelling by
      span without parser-time evaluation; string rows own the exact token span
      and scanner-proven decoded byte length without a decoded-byte mirror.
      Consumer-specific delimiter states attach the completed expression to a
      call's value-argument handle span or an assignment's value handle, so the
      parser does not clone literal grammars or confuse static and runtime
      arguments. Other integer spellings, floats, unary/binary expressions,
      arrays, struct literals, casts, indexing, and nested calls remain
      implementation-incomplete. This completes the real `psi` package build
      root (`builder.package("psi");`), raising completed current `C` roots from
      sixteen to seventeen while body-boundary coverage remains 54 of 73. Keep
      exact literal/span/decoded-length, consumer/delimiter, capacity, reset,
      and partial-publication vectors at the real Delta-compiler gate.
    - [x] Retain shallow named struct literals as ordinary expression nodes for
      call arguments and assignment values. Accept the canonical one-member
      record or two-member case type path, empty or comma-separated named-field
      bodies, multiple fields, and a trailing comma; each field owns its exact name
      span and one value handle from the already-retained path/boolean/decimal-
      integer/string primary slice. The struct expression points to a dedicated
      row owning its exact type-member and contiguous field spans. Every field
      owns a value expression and every struct owns an expression row, so
      `Expressions` dominates the equal field and struct tables without a fake
      resource kind. Nested struct literals and all other richer field values
      remain implementation-incomplete until a real bounded expression-frame
      design exists; no recursive scratch state may overwrite an outer literal.
      This completes `Lexer::initialize` and the canonical Omega package build
      root, raising completed current `C` roots from seventeen to nineteen while
      body-boundary coverage remains 54 of 73. Keep exact record-versus-case
      type paths, field/value/delimiter spans, empty/trailing-comma behavior,
      capacity, reset, nested-incomplete, and Complete-only publication vectors
      at the real Delta-compiler gate.
    - [x] Retain canonical source-ordered explicit states without forcing every
      machine through a fabricated entry. A machine owns zero or one implicit
      entry followed by its authored states; parameters, a return, implicit
      statements, or an otherwise empty body require the entry, while a
      signature-free explicit-state-only body does not. Every state owns its
      exact authored name when present plus independent parameter, return-type,
      and contiguous mixed-statement spans. Because one root may own multiple
      states, `States` is independently exhaustible rather than hidden under
      `Roots`. Retain the first canonical transition core over one path subject:
      boolean, unsuffixed nonnegative decimal integer, and wildcard arms each
      expand into one ordinary transition statement targeting a named zero-
      argument state. `Statements` dominates the equal transition table;
      subject paths use the existing expression and path-member arenas.
      Computed or multiple subjects, richer patterns and guards, target
      arguments, terminal/value/self targets, continuations, `match`, state
      arrival contracts, and richer state bodies remain implementation-
      incomplete rather than being skipped or misrejected. This completes
      `Lexer::{is_whitespace,push_decoded}`, raising completed current `C` roots
      from nineteen to twenty-one while all 54 representable headers still reach
      the body boundary. Keep exact implicit-entry presence, state ordering,
      per-state ownership, transition expansion, guard/target spans, resource,
      reset, and partial-publication vectors at the real Delta-compiler gate.
    - [x] Retain the first source-shaped machine clause ledger instead of
      treating header clauses as disposable punctuation. Exact non-generic
      `satisfies Trait::requirement` bindings and nonempty `reaches` ceilings
      over comma- or plus-separated service identifiers append in source order,
      and each machine owns their contiguous span. Every clause consumes at
      least one path-member row, so `PathMembers` dominates the equal clause
      table without another resource class. Generic satisfies arguments,
      aliases, external `via` bindings, empty or installation-bound reach rows,
      adjacency-separated services, and all other clauses remain
      implementation-incomplete. This completes
      `ConsoleNativeProvider::{write,write_line}`, raising completed current
      `C` roots from twenty-one to twenty-three and body-boundary coverage from
      54 to 56 of 73. Keep exact clause/member/order/delimiter, no-parentheses header,
      resource, reset, and Complete-only publication vectors at the real Delta-
      compiler gate.
    - [x] Materialize the first source-ordered binary-expression layer instead
      of recognizing one closure-specific assignment spelling. Assignment
      values accept a left-associated `+`/`-` chain over the already-retained
      path, boolean, decimal-integer, string, and shallow-struct primaries;
      every operator owns a dedicated `(left, operator, right)` row and a tagged
      expression node. `Expressions` dominates the equal binary table, and no
      operand is evaluated or type-classified during parsing. Other operators,
      unary and grouped operands, postfix continuations, and binary expressions in other
      consumers remain implementation-incomplete until their shared precedence
      frame exists. This completes `Lexer::emit_punctuation`, raising completed
      current `C` roots from twenty-three to twenty-four while body-boundary
      coverage remains 56 of 73. Keep exact associativity/operator/delimiter,
      mixed-primary, expression-capacity, reset, and partial-publication vectors
      at the real Delta-compiler gate.
    - [x] Add one bounded nonrecursive precedence frame for transition subjects
      instead of accumulating comparison-shaped parser branches. It retains
      path, boolean, and unsuffixed decimal-integer primaries; left-associated
      `+`/`-`; equality and ordered comparisons; then `&&` and `||` in their
      canonical precedence order. A 128-row invocation-local value/operator
      frame reduces into the shared binary-expression arena before the
      transition block is published. `ExpressionDepth` is an independent
      private resource; `Expressions` still dominates the equal binary table.
      Parenthesized, unary, postfix, multiplicative, membership, and other
      subject forms remain implementation-incomplete, as do richer arm patterns
      and targets. Extending the assignment layer to subtraction shares the
      same operator representation. This completes
      `Lexer::{is_identifier_start,is_identifier_continue,hex_digit_value}`,
      raising completed current `C` roots from twenty-four to twenty-seven while
      body-boundary coverage remains 56 of 73. Keep exact precedence,
      associativity, reduction-depth, mixed-primary, resource, reset, and
      Complete-only publication vectors at the real Delta-compiler gate.
    - [x] Retain the first indexed assignment place as an ordinary expression
      composition: one self/name/member base path, one self/name/member path
      index, and an exact bracket span materialize a dedicated `(base, index)`
      row selected by the assignment target handle. Base and index remain
      independently retained path expressions; no array bound, element type,
      or assignability decision occurs during parsing. Every indexed row owns
      its tagged expression node, so `Expressions` dominates the equal table.
      Literal/computed/multiple indices, chained indexing, postfix members,
      and compound assignment remain implementation-
      incomplete. This completes `SourceUnit::append` and
      `TokenStream::push_decoded`, raising completed current `C` roots from
      twenty-seven to twenty-nine while body-boundary coverage remains 56 of
      73. `TokenStream::push` still requires its transition target argument.
      Keep exact base/index/bracket/equal spans, expression/path capacity,
      richer-postfix incompleteness, reset, and Complete-only publication
      vectors at the real Delta-compiler gate.
    - [x] Retain canonical terminal expression statements without inventing a
      separate state-result channel. `Expression` statements point directly to
      the shared expression arena and may close a state with a retained
      self/name/member path, Boolean, unsuffixed nonnegative decimal integer,
      string, or one path-indexed expression with a path index. The indexed
      node is now position-neutral: `=` selects it as an assignment target,
      while the state-closing `}` selects it as a terminal value. Richer
      postfix/index forms and nonterminal bare expressions remain
      implementation-incomplete. This completes `SourceUnit::byte_or_nul`,
      raising completed current `C` roots from twenty-nine to thirty while
      body-boundary coverage remains 56 of 73. Keep exact terminal/bracket
      spans, value/place discrimination, statement/expression capacity, reset,
      and Complete-only publication vectors at the real Delta-compiler gate.
    - [x] Retain subjectless `transition { ... }` blocks as the canonical
      zero-subject form rather than manufacturing a unit/path expression.
      Every expanded arm now records subject presence separately from its
      optional expression handle; the existing explicit-subject precedence
      frame remains unchanged. This covers all 49 subjectless transitions in
      the current `C` closure. It does not raise the thirty complete-root count
      yet because every owning root also contains target arguments, local
      bindings, casts, or another unretained form. Keep zero-versus-one-subject,
      wildcard, reset, and Complete-only publication vectors at the real
      Delta-compiler gate.
    - [x] Retain nonempty named transition-target argument lists through the
      same expression-handle ledger used by ordinary calls. The current lane
      accepts comma-separated self/name/member paths, Booleans, unsuffixed
      nonnegative decimal integers, strings, and shallow struct literals, with
      an optional trailing comma; richer argument expressions remain
      implementation-incomplete. Each expanded transition row owns its exact
      contiguous argument span, and no target-specific expression arena is
      introduced. This legitimately completes `TokenStream::push`, raising
      completed current `C` roots from thirty to thirty-one while body-
      boundary coverage remains 56 of 73. Keep exact argument order,
      delimiter/trailing-comma behavior, expression/argument capacity, reset,
      and Complete-only publication vectors at the real Delta-compiler gate.
    - [x] Retain canonical local-data statements as dedicated source-shaped
      rows: `let [mut] name: Type [= expression];`. `mut` is contextual only
      when followed by another name-like token, so `let mut: T;` remains an
      immutable binding named `mut`. Each row owns the exact name and statement
      spans, one shared type-engine handle, explicit initializer presence and
      handle, and mutability. Initializers reuse the assignment lane's existing
      left-associated `+`/`-` grammar over retained path, Boolean, unsuffixed
      decimal-integer, string, and shallow-struct primaries; no local-only type
      or expression parser is introduced. `Statements` and `TypeNodes`
      dominate the equal local-data table, so it adds no resource kind. Call,
      cast, indexed, unary, grouped, and other richer initializers remain
      implementation-incomplete. This completes `Lexer::{retain_token,
      classify_keyword,lex_identifier,lex_whitespace,consume_suffix}`, raising
      completed current `C` roots from thirty-one to thirty-six while body-
      boundary coverage remains 56 of 73. Keep exact contextual-mut,
      initialized/uninitialized, type/expression consumer isolation, capacity,
      reset, and Complete-only publication vectors at the real Delta-compiler
      gate.
    - [x] Retain ordinary call expressions as expression nodes and share their
      receiver, target, static-argument, runtime-argument, and delimiter parser
      with call statements without reusing the statement-only flattened row as
      a fake initializer. This is the current first blocker in twelve of the
      twenty body-reaching incomplete `C` roots: `Main::main` and
      `Lexer::{decode_at,span_equals,lex_line_comment,lex_block_comment,
      copy_source_to_decoded,lex_cooked_string,consume_digits,lex_number,
      lex_punctuation,reject_raw_string_candidate,is_raw_string_candidate}`.
      Preserve general expression ownership and leave chained calls and other
      richer postfix forms incomplete until their shared representation lands.
      This completes `Lexer::{lex_line_comment,lex_block_comment,
      copy_source_to_decoded,is_raw_string_candidate}`, raising completed
      current `C` roots from thirty-six to forty while body-boundary coverage
      remains 56 of 73. `reject_raw_string_candidate` advances to an additive
      ordinary call argument rather than being counted prematurely.
    - [x] Generalize the existing additive expression engine into ordinary call
      arguments rather than adding an argument-only binary parser. This is the
      current first blocker for `Lexer::{validate_utf8,dot_starts_float,
      reject_raw_string_candidate,lex_next}`. It completes the first three and
      advances `lex_next` to its later cast, raising completed current `C` roots
      from forty to forty-three while preserving each call's contiguous
      argument-handle span and keeping body-boundary coverage at 56 of 73.
    - [x] Retain path-valued casts through the shared expression and type
      engines rather than adding assignment- or transition-only conversions.
      The retained row owns its value and target-type handles, exact source
      extent, and optional single-name `in Domain` suffix. The admitted slice
      works in assignment/local values, ordinary call arguments, and transition-
      target arguments, including the existing additive continuation where its
      consumer permits one. Recasts, domain argument packs, richer postfix
      values, and other canonical continuations remain implementation-
      incomplete rather than being misrepresented or rejected. This completes
      `Lexer::{consume_digits,lex_punctuation,lex_next}` and
      `Parser::load_current`, raising completed current `C` roots from forty-
      three to forty-seven while body-boundary coverage remains 56 of 73.
      `Lexer::decode_at` advances to a grouped/multiplicative RHS,
      `Lexer::lex_cooked_string` to a grouped/multiplicative cast argument, and
      `Lexer::lex_number` to a unary RHS.
    - [x] Generalize the existing destination-neutral indexed-expression
      builder into the local-initializer lane without adding a local-only AST
      row or index grammar. The base, path-valued index, and final indexed node
      remain in postorder in the shared expression arena; statement place/
      terminal destinations retain their existing behavior, while a local
      value restores its outer consumer and re-enters the shared additive
      continuation. This completes `Lexer::span_equals`, raising completed
      current `C` roots from forty-seven to forty-eight while body-boundary
      coverage remains 56 of 73. `Parser::{parse_data,skip_trivia,parse_roots}`
      advance to qualified token-pattern guards. Numeric, range, additive,
      nested, and chained indexes remain implementation-incomplete.
    - [x] Retain bare/qualified path guards and the qualified two-member case
      pattern `Type::Case { fields }` as source-shaped transition data. Braces
      are explicit; a dedicated contiguous pattern-field ledger distinguishes
      shorthand bindings from fixed path-expression matches, while every field
      still owns a shared path-member entry so `PathMembers` dominates the
      equal ledger. This preserves the information needed to rewrite bindings
      against the transition subject later without treating target names as
      accidental locals. Empty braces, comma-separated mixtures, trailing
      commas, and `..` remain explicit. Record/tuple patterns, renamed/waived
      fields, proof selectors, `if` guards, and non-path fixed values remain
      implementation-incomplete. This completes `Main::main`,
      `Lexer::digit_in_base`, and `Parser::{parse_data,skip_trivia,parse_roots}`,
      raising completed current `C` roots from forty-eight to fifty-three while
      body-boundary coverage remains 56 of 73. The three remaining body-
      reaching roots need grouped/multiplicative or unary expressions; none
      remains blocked on a closure-specific pattern rewrite.
    - [x] Replace the assignment/call additive scalar lane with the existing
      bounded precedence frame as the single expression reducer for assignment
      values, local initializers, ordinary call arguments, and transition
      subjects. It retains canonical `||`, `&&`, equality, comparison,
      additive, and multiplicative precedence; parenthesized groups record and
      rewind explicit value/operator bases without inventing a syntax node.
      Completed grouped primaries may enter the shared cast/type engine before
      returning to the same postfix tail. Each binary row still owns one
      expression row, while the three 128-entry group ledgers share the
      existing `ExpressionDepth` resource. This completes
      `Lexer::{decode_at,lex_cooked_string}`, raising completed current `C`
      roots from fifty-three to fifty-five while body-boundary coverage remains
      56 of 73. `Lexer::lex_number` is the sole remaining body-reaching
      incomplete root and stops at unary `!`.
    - [x] Retain recursive logical `!` through one bounded prefix stack shared
      by assignment values, local initializers, ordinary call arguments, and
      transition subjects. Each group snapshots the live prefix base so an
      outer prefix wraps the completed group while an inner prefix is retained
      before group closure; postfix casts therefore preserve canonical
      distinctions such as `!x as T` versus `(!x) as T`. Every unary row owns
      one expression row, and the 128-entry prefix/group ledgers remain private
      `ExpressionDepth` budgets. This completes `Lexer::lex_number`, raising
      completed current `C` roots from fifty-five to fifty-six. All 56 roots
      that reach body parsing are now complete; the remaining seventeen stop
      in their headers.
    - [x] Generalize that same prefix stack from logical `!` to canonical
      fixed-width integer complement `~` and arithmetic negation `-` without a
      second unary reducer.
      Transition subjects, assignment values, local initializers, and ordinary
      call arguments select `LogicalNot`, `BitwiseNot`, or `Negate` at the three
      shared operand dispatches, then retain the exact operator span and unwind
      the same inside-out prefix stack. Mixed and nested prefixes therefore keep
      canonical precedence and share the existing 128-entry
      `ExpressionDepth` failure boundary. Borrow/reference and other contextual
      unary forms remain implementation-incomplete rather than receiving a
      consumer-specific shortcut. This is full-language
      parser progress and changes no current `C` root count. Exact nesting,
      precedence, depth, reset, and no-partial-publication vectors remain at
      the unavailable real Delta-compiler gate.
    - [x] Complete the canonical multiplicative tier by routing `/` and `%`
      through the same bounded reducer as `*`. Both retain source-ordered binary
      rows at the canonical multiplicative tier, so mixed
      multiplication/division/remainder is left-associated and still binds
      above `+`/`-` in transition subjects,
      assignment values, local initializers, and ordinary call arguments.
      Division by a literal zero remains valid syntax for later semantic proof
      or policy checking; `/=` and `%=` remain distinct scanner tokens and do
      not enter this binary lane. No arena, consumer, resource, or current `C`
      census changes. Exact association, precedence, grouping/prefix, malformed
      operand, reset, and no-partial-publication vectors remain assigned to the
      real Delta-compiler gate.
    - [x] Retain the complete `<<`/`>>` shift tier in that same reducer. Shifts
      are source-ordered, left-associated, looser than additive forms, and
      tighter than comparison in transition subjects, assignments, locals, and
      call arguments; path and cast left operands use the same rejoin seams as
      the multiplicative tier. The private precedence numbers place shifts
      above the canonical intervening membership/bitwise tiers without a later
      tree-shape migration. Shift-count proof and arithmetic-policy decisions
      remain semantic work, so `x << 64` is valid syntax rather than a parser
      refusal.
      Spaced `< <`/`> >` and unsupported compound assignment do not become
      shifts. No arena, resource, consumer, or current `C` census changes.
      Exact association, precedence, operator-span, malformed-operand, reset,
      and no-partial-publication vectors remain assigned to the real Delta-
      compiler gate.
    - [x] Retain the complete `|`/`^`/`&` bitwise tiers in that same reducer.
      They are source-ordered and left-associated within each tier, with
      canonical precedence `|` below `^` below `&` below shifts; all three
      remain tighter than the contextual membership tier.
      An infix `&` is selected only after a completed left operand; a leading
      `&` remains outside this slice and is not reinterpreted. Path and cast
      left operands use the same reducer rejoin seams. Carrier compatibility
      and integer result types remain semantic work rather than parser guesses.
      No arena, resource, consumer, or current `C` census changes. Exact
      association, cross-tier precedence, operator-span, malformed-operand,
      reset, and no-partial-publication vectors remain assigned to the real
      Delta-compiler gate.
    - [x] Retain single-domain contextual membership as a distinct expression
      shape: `value in Domain::Path`. Tighter arithmetic, shift, and bitwise
      forms reduce into the value before membership; comparison, equality, and
      logical forms remain outside it. The tagged expression row directly owns
      the value handle, shared domain-path span, and exact `in` span, avoiding a
      membership-only arena or a false value expression for the domain.
      Ordinary primary and path left operands rejoin the same reducer. The
      separate cast-domain spelling `value as Type in Domain` keeps its existing
      parse, while a parenthesized completed cast can be a membership value.
      Direct `in A | B` and `in A & B` domain composition remains
      implementation-incomplete and is stopped before either token can be
      misclassified as integer bitwise syntax. An unparenthesized completed
      membership cannot re-enter tighter arithmetic, shift, or bitwise tiers;
      `(value in Domain) + other` is the explicit grouped shape. Every retained
      membership owns an expression and at least one shared path member, so no
      new resource or current `C` census row is needed. Exact precedence,
      repeated-membership, domain-path, malformed-domain, capacity, reset, and
      no-partial-publication vectors remain assigned to the real Delta-compiler
      gate.
    - [x] Retain the first closed external-leaf declaration as one coherent
      source-shaped form: `satisfies Trait::requirement via
      Binding::CompilerIntrinsic;`. The satisfying clause owns explicit
      optional-binding presence plus the exact `via`, `Binding`, and case spans;
      the machine owns an explicit bodyless bit and the canonical empty implicit
      entry carrying its existing parameters and return type. Other Binding
      cases and richer clause continuations remain implementation-incomplete,
      never false Omega rejections. No external-binding arena or resource kind
      is introduced: every clause still owns its requirement path members, and
      the bodyless entry uses the existing independently bounded state arena.
      This completes all sixteen target-scoped
      `ConsoleNativeProvider::{read_line,read_byte,write_byte,exit_process}`
      leaves across the four selected targets, raising completed current `C`
      roots from fifty-six to seventy-two. The sole remaining root,
      `console_write_bytes`, stops in its termination-witness header.
    - [x] Retain the first private termination witness as one source-shaped
      clause: `terminates by <path> -> <View::Path>;`. Its subject and ranking
      view occupy separate spans in the shared path-member arena, while exact
      `by`, arrow, and full-clause extents preserve the authored form. This does
      not set or imply bare `terminates;`, which is the distinct public
      eventual-terminal guarantee. Tuple or non-path subjects, omitted or
      argumented views, ranges, and the bare guarantee remain
      implementation-incomplete rather than being weakened or guessed. Every
      retained clause owns at least one subject and one view member, so the
      existing path-member budget dominates the equal clause ledger without a
      termination-only arena or resource kind. All 73 current `C` headers now
      reach their canonical disposition: sixteen bodyless external leaves and
      57 bodyful roots. Seventy-two roots complete; `console_write_bytes` is
      now the sole body-incomplete root and first stops at the indexed
      transition-target argument `bytes[0]`.
    - [x] Generalize the shared indexed-expression builder into transition-
      target arguments and retain canonical integer indices plus explicit-start
      open ranges. `bytes[0]` is the ordinary composition of a path collection,
      integer index, and indexed node. `bytes[1..]` adds a separate source-
      shaped range node with an explicit start handle, absent end, exclusive
      separator, and exact operator span before the same indexed wrapper is
      built. Path-valued indices and range starts continue to use the same path
      engine; assignment places, terminal values, and local initializers gain
      the same numeric/open-range capability without a consumer-specific AST.
      Open-start, bounded, and inclusive
      ranges plus arbitrary, nested, or chained index expressions remain
      implementation-incomplete. Every range and indexed row owns a tagged
      expression node, so `Expressions` dominates both equal arenas. This
      completes `console_write_bytes` and closes the current source census at
      73 of 73 complete roots: 57 bodyful machines and sixteen bodyless external
      leaves. This is parser coverage of the current `C` closure, not semantic,
      lowering, emission, or full-spec parser closure for `D`.
    - [x] Complete the canonical range bound-presence/inclusivity matrix through
      that same indexed-expression builder. `[..]`, `[..end]`, `[..=end]`,
      `[start..]`, `[start..end]`, and `[start..=end]` now retain independent
      start/end presence, ordinary retained path/self/decimal bound handles,
      exact separator spans, and the inclusive-end bit before constructing the
      shared indexed node. Either inclusive spelling without an end is an exact
      syntax failure rather than `Incomplete`; a second range separator in the
      end expression cannot be mistaken for the outer close. `Expressions`
      still dominates the equal range and indexed tables, so this adds no
      resource kind. At that checkpoint, arbitrary richer bound expressions
      and nested/chained indexing remained implementation-incomplete. The
      current `C` census was unchanged; exact shape, malformed-inclusive,
      capacity, reset, and publication vectors remain assigned to the real
      Delta-compiler gate.
    - [x] Generalize that indexed-expression builder into a true repeatable
      postfix over the retained framed-value lane. Ordinary assignment values,
      local initializers, call arguments, transition subjects, and transition-
      target arguments may begin indexing from a path and may continue a
      completed literal, call, cast, or indexed value with another bracket;
      indexed assignment/terminal places may likewise continue with another
      bracket before their final `=` or state close. Every link owns one
      source-shaped indexed row whose base is the preceding expression handle,
      so `value[first][second]` is a left-to-right chain rather than a flattened
      special form. Each bracket reuses the complete six-shape range builder,
      exact spans, and existing `Expressions` domination, adding no syntax
      variant or resource. An index operand that itself contains indexing still
      remains implementation-incomplete: supporting `outer[inner[index]]`
      requires a bounded nested-index context stack rather than overwriting the
      live outer builder. The current `C` census is unchanged; exact chain,
      precedence, assignment-place, capacity, reset, and no-partial-publication
      vectors remain assigned to the real Delta-compiler gate.
- [ ] **IMPLEMENTATION-INCOMPLETE — `D` exists but is not yet a compiler.**
  Complete `D` against the full Omega specification, including difficult
  features even if `D` itself uses only plain Delta. Conservative lowering and
  poor optimization are
  allowed; weakened Omega semantics are not. Q1 blocks the proof-only
  `FloatMeaning` equality/source-correspondence slice. D31 unblocks the earlier
  Delta compiler's type-formation implementation, while Q2 blocks the
  standalone Omega compiler's exact inner wire and failure profile.
  D25 fixes that edge's logical request and outer envelope, and D24 unblocks the
  Delta census implementation. None prevents implementation of independently
  settled source-shaped parser slices.
- [ ] **DEPENDENCY-BLOCKED — incomplete Gamma/Delta compiler edge and `D`.**
  Compile `D` with `delta_compiler_bytecode.tape` into
  `omega0_compiler_bytecode.tape`, reconstruct the exact edge, and run the full
  Omega acceptance/rejection suite.
- [ ] **DEPENDENCY-BLOCKED — incomplete `D` and absent `omega0`.** Verify that product
  target realization remains inside Omega. The bootstrap compiler itself
  remains Alpha tape even when the programs it compiles target ARM64, x86-64,
  UEFI, or another product target.

## 6. Omega-written full compiler `C`

- [ ] **IMPLEMENTATION-INCOMPLETE — D18 fixes the canonical request.** Publish
  one deterministic package-resolved Omega closure `C` rooted at
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
  itself is complete; do not revive an inspection-only precursor. D18 fixes
  final source custody and publication; only implementation completion remains.
- [ ] Author `C` with a conservative compositional subset of ordinary Omega to
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
