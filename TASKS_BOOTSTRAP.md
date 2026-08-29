# Direct compiler lattice — active work

Last pruned: 2026-08-28.

This queue closes one artifact sequence. It is organized by producer edge and
the owner of the artifact being produced, not by historical bootstrap scripts,
validation experiments, or compiler generations.

## Fixed sequence

Let `C` be the exact production compiler source closure rooted at
`source/omega/build.omg`, including its sibling `source/psi/` dependency. It is
ordinary Omega deliberately authored with only the language surface needed to
express a robust compiler.

```text
audited Alpha VM seed
    → Alpha assembler + Alpha-written Beta cold start → bc
    → bc builds the canonical Gamma evaluator and type checker
    → Delta-to-Gamma elaboration + Gamma evaluation → delta
    → delta compiles C → omega₀
    → omega₀ compiles the same C → omega
```

`omega₀` and `omega` implement the same full Omega language from the same
source. `omega₀` may lower conservatively; `omega` may use the optimizer and
advanced backend already implemented in `C`. Difficult Omega features may be
absent from the source of `C` even though the resulting compiler accepts them.
That incidental source profile is not a language, dialect, or second compiler.

The artifact chain is the bootstrap. There is no `omega-bootstrap` compiler,
Omega subset language, checkpoint generation, DDC stage, or source directory
for either `omega₀` or `omega`.

## Repository contract

```text
source/alpha/assembler/             Alpha source-to-tape construction
source/alpha/checker/               separate derivation-checker artifact
source/beta/compiler/               bc source, artifact, cold start, admission
source/gamma/                       canonical evaluator and type checker
source/delta/compiler/              delta source, artifact, adjacent admission
source/delta/meaning/               canonical Delta-to-Gamma elaboration
source/psi/                         target-neutral compiler package inside C
source/omega/                       Terminal-Psi consumer and product root of C
source/omega-rust/                  optional implementation/comparator
tests/omega/                        product-language acceptance cases
tools/lattice/                      replaceable command ordering
```

The Alpha checker is a separate binary from the Alpha VM and assembler. It is a
trust-floor service beside compiler edges, not the compiler that builds Beta
and not another rung. Gamma has no required compiler binary: `bc` builds its
Beta-written evaluator and type checker, which provide the canonical execution
route used to realize Delta.

The artifact being checked owns its validation. Do not recreate generic
`bootstrap/`, `canaries/`, `assurance/`, `refinement/`, `on-ramp/`,
`proof-kernel/` or private Omega-corpus owners.
Product compiler implementation belongs to **OMEGA-PRODUCT-COMPILER-SOURCE**
in [`TASKS.md`](TASKS.md), not in this queue.

- [x] **NORMALIZE-BOOTSTRAP-FILE-NAMES.** Apply D10's format-and-role naming
  convention atomically across sources, scripts, locators, manifests, tests,
  and documentation:

  - rename the 76 Delta `*.alp` files to `*.delta`;
  - rename the 254 proof-source `*.elab` files to `*.proof` without changing
    the raw certificate format or trusting their elaborator;
  - retain `.alpha` for Alpha assembly and `.tape` for Alpha VM bytecode; and
  - replace opaque canonical artifact basenames with descriptive names such as
    `beta_compiler_bytecode.tape`, `proof_checker_bytecode.tape`,
    `gamma_interpreter_bytecode.tape`, and
    `delta_to_gamma_bytecode.tape`.

  The ratification-time proof inventory said 248; the atomic migration found
  and moved all 254 proof sources present at D10's landing commit. All 76 Delta
  moves and both committed tape moves were first verified byte-identical to
  their old paths; a separate comment-only pass then corrected stale `.alp`
  examples in 12 noncanonical Delta test sources. The canonical `main.delta`
  bytes remain unchanged. Delta's
  path-independent content-set and closure digests must remain unchanged; only
  its locations companion changes. Do not rewrite historical receipts. Any
  in-flight attempt that pinned a changed locator, verifier, or artifact path
  is obsolete and must be prepared again under the new names.

## Evidence minimality

- [ ] **SEPARATE-EDGE-CORRECTNESS-FROM-REPRODUCIBILITY.** Audit every required
  bootstrap receipt, repeated execution, process marker, custody record, and
  installation gate. Keep an item mandatory only when it closes a named
  source-to-artifact refinement, obligation-reconstruction, target-realization,
  or disclosed-admission edge. Repeated runs, producer pedigree, and byte
  reproducibility remain useful diagnostics and supply-chain controls but do
  not establish semantic correctness and must not gate an otherwise checked
  edge merely by existing.

  Preserve exact source and artifact subjects: authenticity asks whether the
  produced artifact corresponds to the approved source, while correctness asks
  whether the checked refinement and obligation evidence hold. Do not erase
  that distinction while removing ceremonial custody machinery.

  Acceptance: every mandatory artifact in the fixed sequence has a one-line
  semantic obligation it discharges; deleting any diagnostic-only receipt or
  second execution leaves the correctness verdict unchanged; and the direct
  lattice can run without a provenance ceremony masquerading as proof.

  - [x] Prune the default runner to the presently closed producer spine. Alpha
    seed verification already contains assembler reproduction; the canonical
    `bc` admission already reconstructs artifact framing. Gamma regression
    suites, path policy, Delta source-snapshot checks, and publication-verifier
    fixtures remain directly invocable but no longer appear as compiler edges.
    The former `full-source.sh` mixed cold construction with another fixed-point
    replay and the complete Beta corpus; `rebuild-artifact.sh --check` now does
    only the non-mutating Alpha-rooted reconstruction required here. The stale
    post-migration `tests/canaries/` tree contained 14,468 tracked generated
    build/viewer files (about 738 MiB) and no source cases; the live cases were
    already under `tests/omega/`, so the generated residue is removed.
  - [x] Remove two remaining reproducibility ceremonies from the direct
    verdict. The lattice now invokes Alpha's `--edge` mode, which checks seed
    behavior and exact assembler construction without rebuilding the native
    seed container; full `source/alpha/verify.sh` retains that provenance
    diagnostic for seed work. The below-Beta derivation checker is constructed
    once, compared with the exact committed tape, and exercised by accept/reject
    controls instead of being constructed twice. The chain manifest now gives
    every mandatory subject/edge one explicit obligation and classifies second
    runs, pedigree, viewers, elapsed markers, and installation inventories as
    diagnostics.
  - [ ] Replace the historical Delta V1 two-execution/heartbeat receipt with a
    minimal one-execution publication join before starting the replacement
    attempt. Keep the exact source, elaboration, closed Gamma input, raw
    observation, decoded assembly, bounded outcome, Delta-to-Gamma refinement,
    and target-realization custody. A second execution and process telemetry
    may be run and retained diagnostically but may not change the correctness
    verdict.

## Edge status

| Producer edge | Current state | Required result |
| --- | --- | --- |
| Alpha seed and cold start → `bc` | exact source/tape, fixed point, bounded reconstruction | reduce admission size; add the missing checker-calculus derivation |
| `bc` → Gamma evaluator/type checker | canonical Beta-written programs and bounded gates | keep compiler-sized evaluation practical |
| Gamma meaning route → `delta` | exact publication/custody machinery; canonical execution active | finalize repeated execution, realize, verify, install |
| `delta + C` → `omega₀` | source owners fixed; compiler and final `C` incomplete | accept the exact ordinary-Omega surface used by complete `C` |
| `omega₀ + C` → `omega` | model fixed | rebuild unchanged `C` and check the second edge independently |

## 1. Alpha seed and cold start → `bc`

Canonical subjects:

- `source/beta/compiler/bc.beta`: 32,605 bytes;
- `source/beta/compiler/artifacts/beta_compiler_bytecode.tape`: 40,693 bytes;
- exact maximal-observation ROOT: 82,660 bytes,
  `f4dde19077478e240c6aed629c1d25169d3210ad0d2ef2e3cc6a47d32a587867`.

- [x] Reduce the remaining admission implementation without merging distinct
  proof responsibilities. The bounded gate now has 191 Alpha modules,
  60,429 lines, and a 1,008,382-byte Checker A source. Shape, control, data,
  memory, stack, effect, ranged-store, and meaning modules may share canonical
  decoded facts and structural indexes; they must retain separate semantic
  theorems. Owner-local cursor pooling has removed ten duplicate helper bodies
  and 2,492 source bytes across the parse-procedure and ROOT observation
  families without merging either family's semantic obligations. The frame,
  ranged-store, and stack-custody owners now reuse the already imported exact
  cell-increment primitive for 19 calls, removing three more duplicate helper
  bodies and 235 source bytes without changing the ROOT identity. Expression
  and effect census construction now also share one register-contract prefix
  accumulator for eight calls, removing two duplicate bodies and 286 source
  bytes while retaining separate arrays, terminal checks, and mutation teeth.
  Four more owner-local tails now rejoin identical cursor restoration, operand,
  target, and one-destination checks, removing 583 source bytes without merging
  their memory, effect, transition, or stack classifications. The final
  mechanical pass routes 26 summary reads through the existing exact-cell
  helper, converges two identical root-memory resolvers, and removes one
  unreachable historical mutation helper, saving another 1,168 source bytes
  and 29 lines. The remaining parallel tables and helpers encode distinct
  owners or obligations; further consolidation requires a newly measured
  duplication or performance case rather than another unbounded sweep.
- [x] Make repeated structural queries O(1) only where the source tables admit
  a proved canonical index. The procedure-span inventory is complete: all 53
  endpoint binders are constant-time and the remaining 44 block identities
  either return a consumed PC or retain an explicit relational boundary. The
  47 expression-census callers now use four checked boundary-prefix tables
  instead of rescanning all 1,236 primitive/push rows. Each family has an
  internal mutation tooth; primitive and push meaning remains with its existing
  owner. The 57 direct effect-census calls likewise use four checked prefixes,
  replacing 80,320 repeated local/memory/transition/event row visits per full
  traversal with one construction and constant-time queries; four independent
  teeth bind those families without moving their semantics. A literal census
  rejected generic pooling across semantic owners and retained only one
  statement-family-local label-suffix literal.
- [ ] **BETA-COMPILER-FOL-REFINEMENT.** Encode and discharge the exact
  `bc.beta` to persisted-Alpha-tape refinement theorem using the accepted
  intuitionistic first-order calculus; do not add a coinductive or LTS-specific
  kernel rule.

  Beta already has canonical small-step semantics. Give the proof subject a
  constructive total `next_beta` presentation, totalize terminal outcomes with
  self-loops, and route malformed or semantically undefined states to an
  explicit invalid proof state whose reachability rejects. Do the same for the
  exact Alpha subject. Define both traces by primitive recursion over `Nat`;
  determinism alone may not be used to extract a successor from `exists!`.

  Represent the simulation as finitely many symbolic relation schemas. Define
  a nondecreasing synchronization function because one Beta step may lower to
  zero or many Alpha steps. Every unmatched step must be observationally silent
  and decrease one well-founded rank over the related state pair. Prove exact
  halt, trap, typed exhaustion, output-stream, and divergence agreement for the
  independently reconstructed `B_bc1` profile and observation identity.

  Use ordinary checked lemmas, natural-number induction, and DAG-shared proof
  terms. An untrusted elaborator may produce the certificate but may not choose
  its subjects, premises, input profile, observations, or terminal cases.
  Measure certificate bytes, peak checker storage, and check time; optimize
  sharing and reusable proved lemmas before treating size as evidence for a new
  primitive rule.

  Acceptance: the rooted checker accepts finite, genuinely divergent,
  zero-artifact-step, and multi-artifact-step seams. It rejects an unguarded
  self-cycle, infinite single-sided stuttering, a non-silent unmatched step,
  missing successor, changed output or resource result, reachable invalid
  state, swapped subject, and a certificate valid only under a weaker input or
  observation profile. ROOT and the Gamma checker remain differential evidence,
  never premises or alternate authorities.

  - [x] The first ordinary-FOL architecture seam lives under
    `source/beta/compiler/validation/admission/fol/`. One 3,588-byte checked
    certificate proves a finite erased source step with silent observation and
    strict rank decrease, a two-Alpha-step lowering with explicit
    nondecreasing synchronization, and a primitive-recursive two-state cycle
    that remains running with an unchanged output observation at every index.
    It also checks one reusable opaque-schema trace induction lemma. The rooted
    Beta checker, reference checker, and Gamma checker agree on the positive
    certificate and reject constant-rank stuttering, a non-silent unmatched
    step, and a missing successor. The focused gate reports certificate bytes,
    rooted check time, and peak child storage. This is an expressiveness seam,
    not `bc.beta` admission.
  - [x] The 1,856-byte reusable
    `source/alpha/checker/corpus/proofs/operational-refinement-core.proof`
    theorem derives observation preservation for every finite symbolic walk
    from three owner-reconstructed premises: zero-step endpoint identity,
    successor decomposition into a shorter walk plus one unmatched step, and
    silence of that unmatched step. It uses ordinary Nat induction and
    `def`/`use`; all three checker implementations accept it. This replaces
    per-length unrolling without granting a producer control over a machine,
    relation schema, or premise.
  - [x] The architecture seam also checks one ordinary-Nat state-invariant
    theorem. Given an owner-reconstructed exact-trace base fact and exact
    one-successor preservation rule, it carries an arbitrary fixed payload to
    every finite trace index. This is the reusable induction needed for the
    root return slot; it neither chooses slot `39` nor supplies the still-open
    exact `bc` preservation premise.
  - [x] Expose the already-checked frame product through narrow FP-kind and
    saved-token accessors beside its depth accessor. Checker A now binds the
    exact `main.resource` start to PC `40251`, relative depth `16`, active-main
    FP kind `1`, and saved-caller-FP token `1`. This is the relative-frame
    premise for the reachability theorem; it does not yet derive absolute
    registers or hidden-stack contents from the initial state.
  - [x] The initial 4,254-byte `bc-main-resource-refinement.proof` authoring
    candidate established a four-stage cleanup model and the proof-suffix
    custody protocol. Review found that the stages compressed seven Alpha
    instructions and that its subject/profile tokens were disconnected
    reflexive conjuncts. The custody mechanism was sound, but the stronger
    exact-subject interpretation was not; the replacement below removes those
    tautologies rather than preserving the overclaim.
  - [x] Replace that candidate with the 7,539-byte instruction-level cutpoint
    theorem. Its indexed `next_beta(exact_bc, state)`,
    `next_alpha(exact_tape, state)`, `obs(maximal_observation, state)`, and
    profile gate normalize only at the owner-selected identities. One Beta
    resource return maps, conditional on the cutpoint relation's root return
    slot, through controls for `40251 imm`, `40261 load`,
    `40264 mov`, `40267 load`, `40270 add`, `40273 ret`, and `39 halt`; all six
    running-to-running steps are silent, seven instruction-debt decreases are
    exact, the sticky resource payload reaches typed `Exhaust`, and terminal
    and Invalid states self-loop. The five reachable resource origins occur in
    the indexed final goal. The fixed-subject successors are total on their
    declared State/Control domains: cross-machine and malformed control tags
    route to Invalid. The focused three-checker seam accepts it.
  - [x] Bind that cutpoint theorem to the exact owner without running the ROOT
    GFP. A FOL-specific shape successor directly builds and rechecks the five
    resource joins, rejoins block354's literal/load/epilogue instruction
    boundaries and the root halt, emits the fixed declarations and goal, and
    accepts only the untrusted proof suffix after byte comparison. The rooted
    checker rejects isolated subject, profile, observation, and
    `ret`-successor mutations; the Alpha ledger separately rejects changed
    source and tape bundles.
    ROOT/GFP/maximal-observation cells and success tokens are absent from this
    ledger program.
  - [x] Add the conditional body-to-resource seam needed before whole-run
    reachability. An alternate Checker-A continuation reconstructs the exact
    `main.body` parse call/fallthrough, RESOURCE_FAIL address/load and staged
    push, zero literal, `!=` comparison, guarded transition to block354, full
    body censuses, and the existing MFC1 relative-frame row. Its 134,172-byte
    owner tape emits a 490-byte FOL certificate proving that
    `MainBody(kappa, Ret252(rho))` advances to
    `MainResource(kappa, rho)` under the exact source, tape, and frame gates.
    The rooted checker rejects changed subject, tape, frame, child outcome, or
    result mapping; both focused FOL ledgers reject changed source/tape bundles.
    This theorem is explicitly conditional on PPRC's `Ret252(rho)` premise and
    does not claim initial-state reachability, absolute registers, or the hidden
    root return slot.
  - [ ] Prove reachability of the instruction-level resource cutpoint from the
    exact initial states. In particular, carry the root call's hidden return
    slot `39` through balanced intervening calls so the dynamic `40273 ret ->
    39` edge is derived rather than merely included in the cutpoint relation;
    prove the exact explicit-frame/register values and Invalid unreachability.
    The linear synthetic epilogue at `40274..40283` is decoded but unreachable
    after the explicit return and must never be used as its successor.
- [x] Keep the default edge bounded to cold construction, artifact framing,
  and exact maximal-observation reconstruction. Alternate checkers, fuzzing,
  exhaustive mutations, and developer reports remain optional. The copied
  240-file lattice corpus and its three regex-driven cross-language demo gates
  are retired; focused Omega behavior belongs in `tests/omega/`, and checker
  propositions belong in `source/alpha/checker/corpus/`.

Acceptance: changing a shared compiler macro changes `bc.beta`, one canonical
shape owner, generated identities, and directly relevant semantic obligations.
No cached viewer, receipt matrix, source-row permutation suite, or debug output
is required by the edge.

## 2. `bc` → canonical Gamma meaning

`source/gamma/interp.beta` and `source/gamma/typeck.beta` are the canonical
Gamma programs built by `bc`. Gamma supplies safe definitional evaluation; it
does not contribute a separately published native compiler between Beta and
Delta.

- [ ] Keep the exact compiler-sized evaluation bounded and practical without
  changing Alpha or Gamma meaning, hiding semantics in a runner, or weakening
  evidence joins. A 12-hour ceiling is emergency containment, not an acceptable
  normal gate duration. Profile the exact input before each optimization and
  retain byte-identical output plus focused semantic tests. A live sample of the
  active publication found 86.7% of samples in Alpha dispatch and no allocator
  or kernel hotspot. After that attempt finalizes, the next candidate is to
  cache frame-relative variable byte displacements plus the current value-column
  base, with nested non-tail restoration and tail-transfer tests. That change
  alters the canonical interpreter/tape identities and must start a new pinned
  attempt rather than invalidating the current one. The two required regression
  cases now live in `source/gamma/test-interp.sh` and pass against the frozen
  interpreter without changing its identity: one reads a cached caller value
  after a nested non-tail return, and one alternates mutual tail transfers with
  different frame arities.
- [x] The admitted dispatch, fuel-boundary, cached-variable, and canonical-u32
  changes are reflected in the current 50,762-byte interpreter source and its
  72,810-byte tape
  (`37e5610b9bbc487e5140c5071bbf66549da200e7a1df915216658733be50fd58`).
- [x] Retain canonical evaluator input/output at the Delta producer edge and
  evaluator/type-checker source/tape identities at the `bc` → Gamma edge. The
  Delta publication evaluates an already elaborated closed Gamma program; a
  second type-checker execution there would invent another semantic stage.

## 3. Gamma meaning route → `delta`

The canonical source is `source/delta/compiler/main.delta`; its source bytes
and path-independent closure identity did not change during D10's path-only
migration. The independently declared lower-rung route is
`source/delta/meaning/delta2gamma.beta` followed by the canonical Gamma
evaluator. Publication binds the exact closure and tools, reconstructs the
packed Gamma program, checks one canonical assembly observation, and validates
the bounded Darwin ARM64 target dialect.

- [x] Retire exact attempt
  `cfcaaee8786d3f12b8102140546b7520a3dd661170d50b2187a0858557cd2322`
  from the publication path before applying D10's naming migration. Its two
  obsolete evaluator processes were stopped after 9h16m; the partial attempt
  remains diagnostic evidence only and cannot admit or install the renamed
  closure. Prepare a fresh exact attempt after the locator, verifier, source
  naming, and Delta contract inputs are final.
- [ ] When the one canonical replacement execution passes, finalize the
  assembly-publication receipt,
  replay exact realization with the pinned compiler/linker/SDK inputs, verify
  executable identity, and generate the terminal artifact-custody receipt.
- [ ] Install only the admitted result under
  `source/delta/compiler/artifacts/darwin-arm64-v1/`. Retain the unsigned
  `delta-compiler`, assembly-publication receipt, realization observation,
  artifact-custody receipt, one canonical raw execution, and a non-authoritative
  installation manifest. Reconstruct tapes, packed input, decoded assembly,
  ordinal wrappers, and empty diagnostics temporarily. Keep install/verify
  commands under adjacent `validation/`; create no generic evidence archive.
  The atomic six-file installer, reconstruction verifier, and fail-closed
  artifact loader are implemented and tested; the canonical installation stays
  absent until the replacement exact attempt finishes and its custody receipt
  passes.
  An optional second execution may diagnose nondeterminism but is not retained
  in the installation and cannot gate the checked edge.
  The initial realization is now an explicit-input, no-discovery command that
  binds stable assembly/toolchain snapshots, exact command order, empty process
  streams, and the existing observation verifier before exclusively publishing
  its four-file staging result; no hand-assembled clang invocation is required.
  From the installed six-file result, one explicit command now reconstitutes
  every disposable verification input from the exact current repository and
  five caller-supplied tool paths, then runs the existing installed-artifact
  verifier. It decodes the retained execution once and does not rerun Gamma.
  This proves current reconstruction and receipt consistency; because process
  markers are not installed, it does not recreate historical proof that two
  independent evaluator processes ran.
- [x] Realization replay, strict target validation, source closure custody, and
  reconstruction-bearing receipt machinery are implemented under
  `source/delta/compiler/validation/`.
- [ ] **DELTA-V1-CONTRACT.** Author `source/delta/LANGUAGE.md` as the
  self-contained semantic subject ratified by D10. Freeze the sealed input-byte
  alphabet and admissible input profile first, together with artifact and
  diagnostic bytes and terminal exit, rejection, trap, and semantic-exhaustion
  observations. Shared Omega spelling is permitted; citing Omega to define a
  Delta rule is not.

  Write the resource section while auditing every fixed bound in
  `delta2gamma.beta`: classify each as source-visible semantics, an explicit
  resource-profile parameter, or a private implementation budget whose
  exhaustion yields `Incomplete`. A syntactically invalid Delta program
  rejects; a valid program exceeding a private translator/checker budget does
  not acquire a semantic result.

  Remove the canonical elaborator's implicit role as an Omega meaning route.
  A file in the intersection of the two grammars may be tested through either
  route, but Delta acceptance proves only Delta meaning. Any compatibility
  harness remains explicitly non-authoritative and outside the publication
  edge.

- [ ] **DELTA-ELABORATION-VALIDATION.** Reconstruct the declared
  Delta-to-Gamma relation independently of `delta2gamma.beta`; add complete
  grammar/elaboration coverage and negative mutations first. Then bind the
  exact source, elaborated Gamma subject, selected input/resource and
  observation profiles, and produced artifact into the checked refinement
  route required by D5 and D9. Coverage may land before that authority;
  translator agreement alone grants none.

## 4. `delta + C` → `omega₀`

Delta is an independent robust compiler-host language. It need not be valid
Omega. The Delta compiler needs to accept only the compositional ordinary-Omega
forms actually used by complete `C`; accepted forms retain ordinary Omega
meaning and unsupported forms reject deterministically.

- [ ] Consume the deterministic transitive compiler manifest published by
  **OMEGA-PRODUCT-COMPILER-SOURCE**. Maintain no bootstrap-private source list,
  AST profile, feature list, or checkpoint tree.
- [ ] Consume **IMMUTABLE-TARGET-ACTIVATION-AND-REACH-CLOSURE** from
  `TASKS.md`: the invocation supplies one exact immutable target, the product
  build authors target-qualified roots/providers and one complete runtime-reach
  ceiling, and successful role-specific closure validation establishes target
  admissibility. Then finalize the durable product build entry and bind the
  package-resolved manifest for `C`; bootstrap owns no parallel target list or
  `Host` interpretation.
- [ ] Derive the exact ordinary-Omega surface used by the resolved closure and
  implement it in Delta with checked semantics, conservative lowering, target
  realization, explicit resource ceilings, and deterministic rejection outside
  that surface.
- [ ] Keep generated/compile-time source, package acceptance, build inputs,
  imported tools, target selection, and emitted-artifact custody explicit. Omit
  interpreters, REPLs, viewers, proof explorers, debuggers, and other tools not
  imported by the compiler executable.
- [ ] Run `delta C → omega₀`, reconstruct and check the exact source/artifact
  edge, retain target dependencies/admissions, and run the compiler acceptance
  suite with `omega₀`.

Acceptance: the first Omega build is one direct Delta invocation over the
product-owned closure. No shell/Python translation, private IR generation, or
second source tree participates.

## 5. `omega₀ + C` → `omega`

- [ ] Run `omega₀ C → omega` without modifying, regenerating, translating, or
  selectively replacing any part of `C`.
- [ ] Reconstruct and check the second source/artifact edge independently.
- [ ] Demonstrate that conservative and production lowering implement the same
  pinned source meaning.
- [x] Treat binary equality and Rust agreement as reproducibility or diagnostic
  evidence only. Correctness comes from checked edges and explicit admissions.

## Tooling and external dependencies

Every required producer, checker, and gate remains directly invocable.
`tools/lattice/` may order exact commands and print failures; it may not parse,
resolve, lower, discover source, manufacture evidence, or make trust decisions.

The first authoritative product build also requires the package/security owner
to publish the accepted-lock/source-closure projection used by `C`. Until then,
compiler-issued package-review rows are review data rather than acceptance
authority. Track product compiler work in [`TASKS.md`](TASKS.md), package
authority in [`TASKS_PACKAGE_MANAGER.md`](TASKS_PACKAGE_MANAGER.md), and
language-design blockers in [`OWNER_QUESTIONS.md`](OWNER_QUESTIONS.md).
