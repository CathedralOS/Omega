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
| Beta-written Gamma compiler | canonical frontend/emitter source, `interp.beta` oracle, Gamma semantics/tests | complete lowering, adapter, standalone tape, and refinement |
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
  - [ ] Complete the Gamma compiler source, tape, and adjacent validation in
    `source/gamma/compiler/` under D16. The canonical source and its bounded
    frontend/emitter gate now exist; section 3 owns lowering, adapter selection,
    tape publication, and refinement.
  - [ ] Materialize the Delta compiler source, tape, and adjacent validation in
    `source/delta/compiler/` under D17; section 4 owns the implementation.
  - [ ] Author `source/omega/omega_compiler.delta` under D17; section 5 owns
    the implementation. This source work does not wait for the physical
    Gamma/Delta compiler artifacts.
  - [ ] **DEPENDENCY-BLOCKED — missing Gamma/Delta compilers and missing `D`.**
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
    source, and refresh the 104,459-byte source / 27,087-byte tape observations.
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
  model. `source/alpha/verify.sh --edge` currently passes all 25 conformance
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
  surface gate runs 197 cases in about six seconds on the development host.
  The largest current retained Beta output, the 238,926-byte checker tape,
  leaves 23,214 bytes in the Alpha payload after replacing repeated inline
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
    23,214 bytes of Alpha payload headroom. The exact 104,459-byte compiler
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
      present 104,459-byte source is no smaller. Sequential remainder folds
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
      The current framed subjects allocate 395,164 of the 1,048,576 arena
      nodes, leaving 653,412 nodes (15,681,888 bytes) for declarations,
      retained lemmas, and one equality's scratch. A real raw-tree selector plus
      the exact textual-ASCII/comment DFA checks the first 1,024 source bytes in
      the authoritative checker with a 3,134-byte temporary certificate in
      under one second. That closes the traversal and byte-dispatch shape, not
      pass one: parser-rich measurement starts at 256-byte power-of-two
      subtrees and may coarsen cheap regions only after they remain below the
      100,000-reduction and semantic-stack ceilings. The useful first adjacent
      pair is `[4096,4352)` / `[4352,4608)`: it crosses `read_source`, advances
      PC `10 -> 83 -> 164`, and records `source_done = 92`. The terminal
      `[104448,104704)` selector has 11 real leaves plus checker `EMPTY`
      padding and must close the final `db "main"` at source byte 104,459 and
      PC 27,087.
    - Implement the eventual proof in place only when pass one is vertically
      complete: exact D15/token/comment/`db` streaming states, fixed-width
      decimal/register/PC checks, every source subtree equality, balanced
      boundary and unique-label joins, the 457-record frozen map, and the
      104,459-byte / 27,087-byte root. A temporary producer may choose paths,
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
  - **OWNER-BLOCKED — Q3:** freeze global declaration identity, duplicate
    binder handling, lexical scope, and shadowing before the canonical resolver
    assigns meaning to ambiguous source. This blocks resolver/type-checker
    completion only; source-envelope validation, strict grammar parsing, target
    ABI work, and profile-independent emission machinery remain unblocked.
  - Implement the Gamma compiler's `GCOUT` boundary and generate each selected
    compiler-application adapter (the Delta compiler uses `DCOUT`). The adapter
    supplies sealed `Bytes`, validates structured returned rejection values,
    owns private failures, and never emits partial artifact bytes.
    **OWNER-BLOCKED — Q2:** select how the exact Gamma compilation question
    carries the generated application profile. This blocks adapter publication
    and the final tape, not the complete profile-independent front end,
    lowering, or emitter.
  - [x] Materialize `gamma_compiler.beta` by moving the reusable strict frontend
    into its canonical owner rather than copying it. Reserve `[10.5 MiB,11 MiB)`
    for 65,536 exact labels, `[11 MiB,11.5 MiB)` for 32,768 fixups,
    `[16 MiB,31.75 MiB)` for the bounded frontend arena, and
    `[31.75 MiB,32 MiB)` for the private 262,144-byte reserved emitter region.
    The direct
    emitter owns sticky failure, exact byte/word append, every Alpha operand
    shape, labels at PC zero, forward/backward fixups, duplicate/missing-label
    rejection, and the runnable 262,140-byte ceiling. The adjacent gate uses
    fixed temporary entries, pins exact payload bytes and capacity failures,
    and retains no alternate compiler or tape. Generated fixed-offset word
    access is centralized through two emitter helpers using caller-clobbered
    `r249`/`r250`; this changes no layout or runtime path and prevents repeated
    four-instruction address sequences from consuming the compiler's own fixed
    tape budget. The retained source declares 104 procedures; with the frontend
    gate entry, the gate uses 105 of Beta's 128 procedure slots and compiles to
    237,097 bytes, leaving 25,043 bytes below Alpha's runnable payload ceiling.
    That is measured pressure, not evidence that all remaining lowering and the
    adapter will fit; profile each retained milestone and escalate before the
    fixed edge is forced into an alternate architecture.
  - [x] Establish the emitted runtime containment floor without selecting Q2's
    application adapter. Reserve `r252`/`r253` for the downward stack and frame
    base and `r254`/`r255` for the upward heap and its limit. Directly emit heap
    and stack reservation helpers that reject negative, overflowed, and
    adjacent-out-of-range requests before mutation and transfer to a supplied
    terminal failure label. Execute the generated Alpha payload for exact heap
    and stack boundaries, their adjacent one-byte failures, both negative
    requests, and heap-addition/stack-subtraction wrap; no case enters Alpha's
    undefined out-of-range memory behavior.
  - [x] Establish the private arbitrary-arity Gamma frame ABI independently of
    Q3's unresolved source identities. Retain complete two-word values; lay out
    previous-frame and caller-cursor words, fixed local slots, and reverse-
    positioned source-order parameters in one downward explicit frame. Ordinary
    calls use Alpha `call`/`ret`, but every live return owns at least a 16-byte
    explicit frame: the guarded `[256 KiB,16 MiB)` stack therefore exhausts
    after at most 1,032,192 live calls while their 8,257,536 hidden-return bytes
    still lie above the 48 MiB heap ceiling. Tail calls preflight their complete
    replacement extent, copy already-evaluated two-word arguments high-to-low,
    inherit the original caller cursor, and jump without growing either stack.
    Execute 4,096 mutual grow/shrink tail transfers between 48- and 80-byte
    frames, preserve a pending caller spill across non-tail return, carry 600
    nonzero-kind arguments, and distinguish an exact 256 KiB tail landing from
    the adjacent aligned resource failure before relocation. Reject malformed
    compiler-owned frame profiles before emitting bytes. Q3 still blocks
    assigning calls and binder slots from ambiguous source, not this ABI.
  - [x] Establish one Q3-neutral fixed-local access inside that frame ABI.
    Compiler-resolved local indexes address complete two-word values only in
    the aligned prefix after the frame header; one shared emitter expands both
    load and store through the canonical word helpers. It validates the full
    prefix, local count, index, and closed load/store mode before emitting any
    byte, and classifies malformed metadata under the existing private frame
    failure. The frame probe stores, clobbers, and reloads a nontrivial pair in
    the final local of a 48-byte prefix; its existing parameter and root-frame
    checks prove non-overlap and restoration. Focused controls reject a
    misaligned prefix, the adjacent local index, and an unknown mode with no
    payload. Q3 still owns source binder/reference-to-slot assignment, scope,
    and shadowing. Until that ruling produces canonical arm/slot metadata, do
    not retain a tag-only or binderless match-lowering scaffold.
  - [x] Establish one Q3-neutral resolved-parameter accessor inside the same
    frame ABI. Validate the complete fixed prefix, bounded parameter count, and
    opaque source-order index before emission; require the combined fixed-plus-
    parameter extent to remain within the explicit-stack profile; then load the
    complete pair from the settled reverse-positioned parameter region.
    Replace the resolved-call bridge's hand-authored offsets with this accessor
    for both mixed-kind parameters across its ordinary and proper-tail paths.
    Focused controls
    reject a malformed prefix, negative count, adjacent index, and one parameter
    beyond the combined extent under the private frame failure with zero payload.
    Q3 still owns mapping source references to parameter indexes, not their
    runtime placement.
  - [x] Establish the private arbitrary-arity algebraic-value ABI without
    assigning Q3-blocked source constructor identities. Consume an opaque
    resolved kind `>= 2`, copy complete argument pairs from the guarded stack
    into a source-order immutable field vector, return `(kind,pointer)`, and
    represent nullary constructors without allocation. Round odd field counts
    to 32-byte heap rows so algebraic allocation preserves the `Bytes`
    descriptor-alignment invariant. Field loads validate the compiler-owned
    pointer, complete rounded extent, alignment, and static index before memory
    access. Execute a 600-field nonzero-kind vector, nested and nullary values,
    first/last field order, malformed private pointer containment, and exact
    final-row versus adjacent heap exhaustion. Reject malformed compiler-owned
    constructor profiles before emitting bytes. Q3 still blocks connecting
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
    private helper ABI without selecting Q2's application adapter. Reserve one
    canonical zeroed `EMPTY` descriptor, allocate fixed 32-byte `LEAF`,
    `CONCAT`, and `SLICE` rows, preserve empty/full identities, and traverse
    ropes and nested slices iteratively. Validate every descriptor before a
    load; route invalid authored bytes, indices, and ranges to the supplied trap
    label, malformed private descriptors to the supplied internal-failure
    label, and actual allocation exhaustion to the supplied resource label.
    Execute all six operations, cross-boundary and nested slices, a 1,024-node
    rope, 12 invalid/malformed cases, and exact-last-row versus adjacent
    allocation exhaustion.
    **OWNER-BLOCKED — Q6:** classify `bytes_concat` when its compact logical
    length exceeds signed `Int`; the helper accepts a separately selected
    checked-add terminal and otherwise remains complete.
  - [x] Lower all six statically checked `bytes_*` forms through the eventual
    expression dispatcher. Reconstruct `Bytes` as `(1, descriptor)` and scalar
    results as `(0, value)`; evaluate one-, two-, and three-argument forms
    strictly left-to-right through the guarded explicit stack; and call only
    the private runtime helpers above. Execute 11 source-to-code tapes covering
    every form, nested ropes, a cross-rope slice, exact-end zero slicing, lazy
    conditional `Bytes` branches, an outer `Int` spill, and invalid byte/index/
    range traps. Recompile one nested `Bytes` source twice and require identical
    raw payloads. Q6's logical-length overflow is intentionally not asserted.
    Focused emitter/runtime probes compile the canonical emitter section alone
    so unrelated frontend growth cannot force those diagnostics past Beta's
    fixed payload ceiling; the actual lowering probe still compiles the whole
    canonical source.
  - [x] Bridge already-resolved ordinary and tail calls into the eventual
    expression backend without assigning a Q3-blocked source identity. Consume
    the canonical source-order argument list, lower every argument non-tail
    exactly once, preserve complete `(kind,payload)` pairs across guarded
    16-byte spills, and select the existing ordinary-call or replacement-frame
    emitter from an opaque callee label and fixed-prefix profile. Before
    emission, validate the complete forward arena list and bound all fixed
    frame/field arithmetic by the generated-stack profile so malformed private
    metadata cannot wrap, loop, or author a partial payload. Execute one
    compact two-argument mixed-kind payload through both paths, recover the
    source-order values in the callee, restore the root stack/frame after the
    tail return, and require byte-identical reconstruction. The tag-5 source
    connection, callee metadata assignment, and binder slots remain Q3-blocked;
    this seam introduces no resolved-AST serialization or subset compiler.
  - [x] Bridge already-resolved constructor applications into the eventual
    expression backend without assigning Q3-blocked spelling or declaration
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
    being mistaken for zero arity. The tag-7 source connection remains
    Q3-blocked; no parallel verifier, serialized resolved tree, or duplicate
    list walk is retained.
  - [x] Bridge already-resolved local references and lets into the eventual
    expression backend without assigning Q3-blocked source identity. Reuse the
    fixed-frame validator and emitter for complete two-word loads and stores;
    encode the let's bounded `(prefix,index)` metadata in one private scalar to
    respect Beta's four-argument call limit. Validate that profile before any
    initializer byte is emitted, lower the initializer non-tail exactly once,
    retain its complete value, and pass the incoming tail context unchanged to
    the body. Execute a mixed `Bytes` initializer/`Int` body in a real 48-byte
    frame, recover both values from distinct slots, restore the root stack/base,
    reject malformed prefix and adjacent-index profiles with zero payload, and
    require byte-identical reconstruction. Source tag-1/tag-4 connection,
    binder-to-slot assignment, scope, and shadowing remain Q3-blocked.
  - [x] Establish the dormant profile-parameterized sealed-input reader without
    selecting Q2's application profile. The emitted helper consumes stdin once,
    accepts only a compiler-supplied closed maximum, returns canonical `EMPTY`
    without heap movement, and otherwise commits one flat `LEAF` descriptor and
    `r254` only after EOF and complete 32-byte-aligned extent validation. Exact
    maximum EOF succeeds; the adjacent byte and adjacent heap extent transfer
    to the supplied resource label with the descriptor row and heap cursor
    unchanged. Execute empty, binary `00 ff 41`, exact/adjacent maximum,
    maximum-zero, exact/adjacent heap, and malformed private-heap paths; require
    repeated emission to be byte-identical and reject a negative emitter
    profile before emitting bytes.
    Q2 still owns which profile supplies the maximum, which entry receives the
    value, and all result/wire publication.
  - [x] Replay the complete fixed-up Alpha payload before publication without
    trusting the emitter call sequence. Clear and rebuild a private one-byte
    instruction-start map in the otherwise unused `[11.5 MiB,11.75 MiB)`
    compiler region; partition every payload byte under Alpha's closed opcode/
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
    `Int`/`Bytes` exclusions even though Q3 may keep their namespaces separate.
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
    bound the AST arena below the compiler's reserved 31.75 MiB payload edge
    before writes; physical memory above Beta's 32 MiB logical edge is Alpha
    hidden-return-stack allowance. A 2 MiB-boundary canary places a later
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
    data types now implement D16. Q3 still owns duplicate declaration identity.
  - [x] Preserve source-coordinate custody through the reusable front end.
    Every syntax node now retains its zero-based starting byte offset, outer
    envelope rejection records the offending byte before tokenization, and
    parsing, integer overflow, unknown type spelling, and the first failing
    typed subexpression share one sticky first-source-failure coordinate. The
    Boolean oracle does not publish a compiler frame; the direct compiler must
    absorb this metadata into its accepted-language rejection table and final
    `GCOUT` boundary after Q2 is ruled.
- [x] **GAMMA-NO-MATCH-HARDENING.** Make both tail and nested interpreter match
  paths trap rather than fabricate integer zero when no arm matches, and pin
  both with focused no-output trap canaries. The direct compiler task separately
  owns complete static match-exhaustiveness rejection. Keep the
  correlated-oracle warning explicit: the two oracles historically shared the
  omission, demonstrating that agreement alone could not establish it.
- [x] Absorb the reusable static frontend into `gamma_compiler.beta` without a
  duplicate checker source, and keep `interp.beta` only as a bounded semantic
  oracle/candidate algorithm source. Neither the oracle nor the incomplete
  compiler source is an accepted compiler artifact. The retained gates pass 48
  interpreter cases, the fail-closed arena case, 82 compiler-frontend cases,
  one exact emitter probe, six executed runtime-containment probes, 16 checked
  `Int` paths, 31 source-to-code lowering cases, one resolved-call and one
  resolved-constructor bridge payload, four byte-determinism comparisons, 14
  compact-`Bytes` runtime paths, two arbitrary-arity/frame-ABI
  paths, three algebraic-value ABI paths, eight sealed-input runtime paths, one
  sealed-input reconstruction comparison, and 106 independent differential
  cases.
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
  outcomes after lowering and the Q2 adapter are complete. Measure
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
    once and emits the exact raw at-most-262140-byte Alpha payload. It is
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
  - [ ] **OWNER-BLOCKED — Q4.** Collect exact Delta declaration identities and
    reject the earliest duplicate before type formation. D17 does not yet fix
    whether type owners and machines share a namespace, whether boundary
    members collide with qualified machine bodies, or whether parameters and
    ordered locals participate in the early collection phase. Do not retain a
    collector that guesses those accepted-language and rejection-priority
    rules.
- [ ] Derive compact positive, negative, trap, and
  private-budget `Incomplete` conformance directly from the frozen Delta
  contract. Do not recreate cases that merely pin quirks of the removed
  translator.
- [ ] **DEPENDENCY-BLOCKED — missing `delta_compiler.gamma`.** Run that
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
  - [ ] **OWNER-BLOCKED — Q7.** Freeze the canonical sealed Omega compiler
    request: package/source closure, source identities, product/target/admission
    inputs, explicit bootstrap Alpha-tape product, and compiler outcome framing.
    This blocks `D`'s executable entry, source custody, package lookup, and
    publication. It does not block final boundary-independent internals such as
    the complete Alpha encoder.
  - [x] Establish the final Delta-side Alpha tape encoder in `D`. It owns the
    complete closed opcode-shape table, paired-`i32` representation of arbitrary
    64-bit immediates, instruction-atomic capacity checks, bounded address
    fixups, and the exact raw 262,140-byte payload ceiling. Sealing clears and
    reconstructs the complete instruction-start partition, rejects unknown or
    truncated instructions, and requires every direct target to land on a
    reconstructed start. Native seed stamping alone owns the descriptive
    four-byte length prefix; it is not part of the `.tape`. Because Delta has no
    private visibility, even the reserved-write helpers independently enforce
    open state, byte range, and whole-write capacity rather than relying on a
    prose-only caller precondition. `D` deliberately has no `Main`, source
    protocol, package lookup, publication, or placeholder compiler result while
    Q7 is open.
  - [x] Give `D` explicit symbolic control-flow ownership before lowering.
    Monotonic typed label IDs bind once; each label-bearing emitter records the
    exact most-recent instruction and its single address operand; operand
    offsets are strictly increasing; and sealing resolves every recorded fixup
    through its bind-once label between an unpatched partition reconstruction
    and the final target replay. The 29,126-fixup ceiling is dominated by the
    exact Alpha payload extent; the independent fixed label storage is a
    private compiler ceiling and must map to outer `Incomplete` once Q7 freezes
    its resource framing. There is no arbitrary public patch operation or
    unresolved-zero convention. Exact forged-owner, stale-map, forward/
    backward/alias, undefined/end-label, duplicate-bind, interior-target, and
    capacity canaries join the real Delta-compiler gate when that executable
    exists; do not create a host Delta executor to run this incomplete closure.
  - [x] Establish `D`'s source-view UTF-8 framing primitive independently of
    Q7's package/source custody. It accepts an immutable byte view, implements
    the complete one- through four-byte scalar envelope, rejects overlong
    forms, surrogates, values above U+10FFFF, stray continuations, and truncated
    tails, and reports the malformed scalar's lead-byte offset. It does not
    invent source IDs, unit ordering, token custody, or where otherwise-valid
    non-ASCII scalars are permitted by LEXICAL-PROFILE-V1. Exact boundary and
    malformed-family vectors join the real Delta-compiler gate rather than a
    host reimplementation.
  - [x] Establish `D`'s complete source-neutral lexical scanner independently
    of Q7's package/source custody. It implements the exact current Omega
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
    ledger, or decoded-byte mirror. Q7 still owns source size/admission and
    outer `Incomplete` framing. Exact lexical vectors join the real
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
      mirror, standalone cursor, or Q7 outcome tag is introduced. The 4,096-root
      and 16,384-path-member ceilings are private compiler budgets whose eventual
      outer `Incomplete` mapping remains Q7-owned; profile both against the real
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
      order, and Q7 neutrality.
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
      `Complete`-only publication, and Q7 neutrality. Executed and exact-edge
      resource vectors wait for the real Delta-compiler gate.
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
      represents 104 of 112 parameter occurrences and 64 of 72 complete
      parameter lists in current `C`; 39 headers now reach body parsing, but
      zero roots complete because every reached body is nonempty. Keep exact
      modifier/access/flag/postorder/resource/reset vectors at the real Delta-
      compiler gate.
- [ ] **DEPENDENCY-BLOCKED — missing `D`.** Make `D` implement the
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

- [ ] Publish one deterministic package-resolved Omega closure `C` rooted at
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
