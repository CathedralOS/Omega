# Bootstrap tasks

Build the smallest **human-auditable proof chain to self-hosted Omega**.
Runnable compilers and checked edge evidence are both required; neither replaces
the other. Minimize the total human audit burden of semantics, admitted seeds,
implementations, checker rules, certificates, profiles, and permanent tooling.
The governing contracts are [bootstrap minimization](bootstrap/MINIMIZATION.md)
and the [derivation checker](bootstrap/proofs/checker/README.md).

The currently selected construction route is:

```text
audited Alpha VM + admitted Beta compiler tape
  -> Beta-written Gamma evaluator
  -> Gamma-authored Delta compiler
  -> Delta-authored Epsilon evaluator
  -> interpreted Epsilon-authored Omega compiler D
  -> Omega-written product compiler C for alpha_bootstrap
  -> C rebuilds itself
```

The Gamma checker sits beside these edges; it is not another language rung.
Its implementation is a means to explicit, independently rooted proofs, not
a general-purpose proof-system project or an end in itself.

Prioritize the executable path to Omega and reductions in its human audit burden.
Proof-checker experimentation is secondary in scheduling; this does not remove
the required evidence for final chain closure. Retained rung features serve their
actual next compiler/evaluator customer, not general-purpose language completeness
beyond the selected contracts. Language and topology experiments follow
whole-chain minimization without an owner-approval prerequisite. Adoption must
reconcile contracts and evidence; proposed weakening of required assurances
still requires owner escalation. Tag an added item's provenance on its first
line: `(new-scope)` for newly discovered work, `(split-of:<parent-item>)` when
it decomposes an existing item. Items added before 2026-09-14 are untagged.

## Selection and stopping rules

- Apply [scope checkpoints](AGENTS.md#scope-checkpoints) before each milestone.
  Challenge the task's premise against the final audit goal, not just its tests.
  A concrete compiler customer or named proof obligation is necessary but does
  not alone establish that the proposed mechanism is the simplest solution.
- Count progress in execution, human auditability, and measured proof feasibility.
  Helper/test counts, smaller files, and preserving current limits are not goals.
  Compare simpler implementations and coherent private-capacity changes before
  adding workarounds; preserve required semantics and fail-closed behavior.
- No intermediate self-hosting, hypothetical reuse, permanent compatibility
  layers, host semantic stages, or customer-specific acceptance shortcuts.
  Host tools may invoke, stamp, compare, and report, never manufacture authority.
- Keep topology/source ownership green with
  `sh tools/bootstrap/check-chain-hygiene.sh`; this is validation, not an
  evergreen feature task. Remove completed tasks rather than logging milestones.
- Question questionable checker/encoding designs as well as compiler designs.
  Report evidence and alternatives before extending a faulty premise. Changes
  to ratified language/trust contracts follow [owner escalation](AGENTS.md#workflow);
  engineering review does not authorize silently weakening the proof claim.
- Keep language-facility and implementation comparisons on this board under
  [whole-chain minimization](bootstrap/MINIMIZATION.md). Experiments remain
  non-authoritative and do not need an owner ruling. Escalate proposed weakening
  of trust, observation, identity, or fail-closed guarantees before relying on it.

## Dependencies

P1–P5 below are task-group labels, not owner-question numbers or a mandatory
serial schedule. P1 is the Gamma checker's proof that the entire selected
evaluator Beta source encodes to its exact persisted Alpha tape; it is not a
proof that the evaluator implements Gamma. An empty `OWNER_QUESTIONS.md` means
there is no unanswered owner decision, not that these implementation tasks pass.
Skip a task-local pause or blocker and continue independent work on this board.
Proof work can advance against existing exact artifacts before Omega is complete.
Runnable lower-rung development can also proceed under disclosed trust assumptions;
it does not close a proof edge. Do not redirect all effort to execution merely because the
checker has a longer acceptance path.

Full self-hosting remains dependent on settled exercised Omega behavior, the
[Rust product completion plan](wiki/drafts/rust_compiler_completion.md),
complete D, and `OMEGA-PRODUCT-COMPILER-SOURCE` in [TASKS.md](TASKS.md).
Rust remains a comparator, not bootstrap authority. Optimization matters where
measured execution or audit feasibility requires it, not as an unbounded
prerequisite to every lower-rung milestone.

## Measured complexity follow-through

- **BOOTSTRAP-AUTHORING-READABILITY.** Improve the implementation-to-expressiveness
  tradeoff within the selected rungs. Customers are the Gamma-written Delta
  checker and its generated runtime, not language-feature completeness. Next
  comparisons are checked named products and ordinary runtime source. The
  [product comparison](#gamma-product-checking) below owns the exploratory work.
  Ordinary runtime source landed as the bound readable support members under
  `bootstrap/3_delta/support/`; no internal-artifact comparison remains open.
  Neither comparison needed an owner-approval gate.
  Do not repeat independent per-field accessor wrappers as the product solution.
  A source-only direct-recursion rewrite of
  [conditional checking](bootstrap/3_delta/implementation/checking/types/branches.gamma)
  was also rejected against
  `355ea00d55` on macOS: ordinary results and three rejection cases matched, as
  did depths 32, 64, and 128, but depth 256 returned raw Gamma status 250 where
  the selected compiler produced its successful 5,036-byte receipt. This rules
  out that rewrite under the current profile, not an evaluator-owned explicit
  stack. The latter remains an engineering comparison requiring its full
  containment and failure argument; a constant increase is not evidence.
  Acceptance: a real checker/runtime implementation exposes its algorithm with
  fewer manual layout/control obligations, and the added lower-rung semantics,
  state, source, tests, and proof obligations are explicitly accounted for.
  Preserve exact frontend diagnostics, complete Epsilon closure reconstruction,
  and required malformed/resource controls. Byte counts alone and an isolated
  source sketch do not establish human auditability. No rung removal is presumed.

<a id="gamma-product-checking"></a>

- **GAMMA-PRODUCT-COMPARISON.** Compare ordinary pairs, dynamically checked named
  products, and static nominal typing for the seven-field continuation in
  [Delta argument checking](bootstrap/3_delta/implementation/checking/types/calls.gamma).
  This is exploratory engineering, not owner-blocked work or accepted syntax.
  The current producer and consumer manually agree on nested-pair layout;
  grouped bindings expose the decoding but do not check that agreement.
  At `355ea00d55` on macOS, ordinary constructor/accessor wrappers preserved six
  compiler results but grew the module from 4,731 to 5,542 bytes and four to
  twelve definitions, traversing 21 tail links instead of six. They added no
  layout guarantee and were not retained; do not repeat them as the solution.
  The [product comparison experiment](tests/gamma/product-comparison-experiment/README.md)
  at `7595f3028e` pins one construction and one destructuring of that payload
  as nested pairs against a source-emulated named product with identity and
  arity words: same order, scope, and tail behavior; 6 versus 8 pairs and
  101,859 versus 147,419 whole-run Alpha steps under the untrusted reference
  interpreter on Linux (selected-evaluator runs remain unmeasured there); a
  wrong identity or arity maps to a chosen scalar status instead of an anonymous
  trap; and a forged header passes both checks and traps exactly like pairs,
  so source-level checks remove no manual layout obligation. Enforcement needs
  evaluator-minted identities, whose representation, Beta, contract, heap
  capacity, test, and proof costs the experiment README counts but does not
  build. Static typing is a separate candidate with additional declaration and
  call judgments, not an assumed feature bundle; no fixture represents it.
  Human defect-localization evidence from the customer remains unmeasured.
  Acceptance: compare the Beta implementation, representation, validation,
  resources, tests, and proof obligations against the customer code and manual
  layout obligations removed. Check construction order, scope, tail behavior,
  forged identities, arity, and failure mapping. Keep experiments separate from
  the selected language; adoption requires reconciled contracts and evidence,
  not a ruling merely to explore the alternatives.

- **BOOTSTRAP-COMPLEXITY-REVIEW.** Finish the decisions identified by the
  [measured cost review](wiki/drafts/bootstrap_cost_review.md) before
  expanding infrastructure. Owners remain Delta normalization,
  Gamma evaluator/checker/Beta definitions, and selected-chain resource profiles.
  Current Epsilon demand does not justify more depth machinery; retain deep-source
  conformance until a simpler implementation or owner-approved scope replaces it.
  For P1 in `bootstrap/proofs/beta_encoding/`, use the consolidated
  [complete encoder candidate](bootstrap/proofs/beta_encoding/ENCODER_CANDIDATE.md), not
  another isolated helper probe. It removes completed-token/output histories
  from incoming state and accounts explicitly for fragment composition, exact
  emission counts, all Beta cases, limits, failures, EOF, and owner custody.
  **Strategy pause:** retain the open P1 obligation, but do not add more isolated
  helper families or change provisions without a defensible integrated cost
  argument. Complete definitions and an integrated recipe remain the proposed
  unit for evaluating that route; routine engineering choices do not require
  an owner question. Continue independent bootstrap work while it is paused.
  Resume evidence at `50a27b9bfc` on macOS arm64 (prose + gates): the
  consolidated candidate's state removal is a feasibility precondition
  (~10-20GB-class history carriage vs the 8 MiB request), the successor recipe
  reproduces at 4,611,614 unshared / 2,122,796 shared work, encoder-state
  census is 17,130 vs 1,521 leaf keys with/without count, and the physical
  work ceiling is 675,017 under the selected pair arena — every derivable
  integrated scenario lands ~9-41 times over work, so no measured recipe
  family reaches acceptance under selected provisions. Remaining routes:
  recipe restructuring toward ~675k work, checked closed-lemma composition
  (owner escalation), or more native backing (owner decision). The actual
  `encode_Beta(S, limits) = Success(T)` probe has not run and has no repository
  command: complete definitions, owner-root reconstruction, and source-owned
  production remain missing. No partial diagnostic or unimplemented valid case
  may stand in for that root; no unchecked length or independent chunk budget
  may become a premise.
  Acceptance: a bounded simplification/retirement plan and evidence that the
  proposed proof route can plausibly reduce total audit burden, with extrapolated
  cost distinguished from actual full-subject checks. If the strategy is not
  justified, preserve its evidence and continue elsewhere; do not build an
  alternative language or delete unreviewed machinery.

## Alpha execution hardening

- **ALPHA-WINDOWS-CONFORMANCE.** Owners: `bootstrap/0_alpha/` semantics, native
  implementations and audited listings, with `tests/alpha/conformance.sh`.
  Remaining acceptance: execute `sh tests/bootstrap/alpha-beta-edge.sh` and
  `sh tests/alpha/reference/diamond-py.sh` on Windows x64 for the
  [selected seed](bootstrap/0_alpha/README.md#retention-inventory), retaining
  exact bounds/Trap observations and register preservation through host I/O.
  The shared `tests/alpha/io-registers.hex` must return zero and `ABCDEF` for
  input `AB`; all bounds and reconstruction controls remain required too.
  macOS execution and PE byte audits do not substitute for this unavailable host result;
  [coverage limits](tests/alpha/README.md#bounds-conformance) remain explicit.

## P1 - Gamma checker and first complete encoding proof

- **GAMMA-DERIVATION-CHECKER.** Close the first artifact-specific proof using
  the ordinary-Gamma [checker](bootstrap/proofs/checker/CHECKING.md)
  and [Beta definitions](bootstrap/proofs/beta_encoding/README.md), following the
  [complete encoding acceptance](bootstrap/proofs/beta_encoding/ACCEPTANCE.md).
  Remaining work: the produced full-subject certificate cannot be admitted
  under the selected provisions; the residual routes are the owner-level
  decisions in the [cost review](wiki/drafts/bootstrap_cost_review.md),
  filed as owner decision `beta-encoding-certificate-admission`.
  Resume evidence on macOS arm64: the complete 18-sort/361-constructor/
  108-function theory emits from Gamma source at 116,992 bytes, SHA-256
  `b2ab717f574b39e7ef6986b2ec3a43036a2ca4f5e7512c1dcbe3435b564ebb4b`, matching
  the independent restatement in
  [encoding.py](tests/gamma/beta-encoding-theory/encoding.py); the source
  closure is 5,536 manifest bytes / 131,059 packed bytes.
  `sh tests/gamma/beta-encoding-theory/run.sh` passes two identical emissions,
  three producer refusals, and 203 exact checker diagnostics (1,024 lexical
  truths, 256 joins, 512 splits, 256 roundtrips, 13 word emissions, 512 byte
  counter equations, 19 checked successors, 256 nibble, 12 byte, 30 word
  comparisons, plus 200 encoder equations over all functions 58..108 produced
  by the generic [stepper](tests/gamma/beta-encoding-theory/stepper.py) —
  including two tiny end-to-end `encode` calls — and seven encoder-range
  rejections); generic formation checks every new clause on each request.
  `--subject-shape` checks both full-subject spines at 225,305 work in
  557.725s and rejects the altered witness at its fixed coordinate under a
  900-second watchdog; `--counter-cost` preserves the shared-successor
  measurements; `sh tests/bootstrap/proofs-identity.sh` re-binds the
  manifest and packed identities. Formation and these finite equations are
  not artifact admission: the owner-fixed
  `encode_Beta(S, 0x4000000, 0xfffffc) = Success(T)` proposition for the
  selected evaluator source and tape is now independently reconstructed
  (534,208 owner bytes) and its complete untrusted derivation produced
  (3,182,484 rows, maximum depth 204, 135,451,492 request bytes) — 16.1 times
  the request provision and ~70-80 times the work provision and physical
  pair ceiling, so it measures the gap rather than closing it.
  The exact subject is the entire selected Gamma evaluator's raw Beta source
  and persisted Alpha tape. Encoding equality does not prove the evaluator
  implements Gamma; retain that trust assumption explicitly.
  Each retained checker rule and encoding helper must have a demonstrated role
  in this certificate. No proof search, producer-selected root, trusted assembler
  primitive, or general-purpose extension.
  Acceptance: the full certificate checks under the exact
  [result/resource profile](bootstrap/proofs/checker/FORMAT.md), with
  measured bytes, storage, depth, and time and a reviewable account of the
  definitions and trusted assumptions. Malformed, cyclic, missing-premise,
  wrong-subject, wrong-rule, and exhausted requests cannot accept.
  Rule tests, partial proofs, and assembler agreement do not close this task.

## P2 - Gamma to Delta

- **DELTA-COMPILER.** Finish the Gamma closure rooted at
  `bootstrap/3_delta/delta_compiler.gamma` against the
  [Delta contract](bootstrap/3_delta/LANGUAGE.md). Preserve full ordinary
  source semantics, checked arithmetic, exhaustive matching, proper-tail
  lowering, and canonical Gamma emission. The customer is the Epsilon
  evaluator closure, which already compiles with a diagnostic entry; further
  optimization needs measured customer or conformance pressure, not a
  standing mandate to improve general transformation costs.

  The open part is compiler-execution resource containment. The contract
  leaves full generated-profile admission and the other compiler-owned
  resource/internal DCOUT outcomes open, and says evaluator failures do not
  substitute for them. The
  [execution-storage audit](bootstrap/3_delta/implementation/boundary/execution_storage.md#remaining-obligation)
  bounds the compiler's call contexts, lexical rows, and temporary values, and
  every producer phase now has a closed per-occurrence pair charge. That does
  not bound cumulative pair allocation: the closed envelope exceeds the
  40,265,318-pair arena from `N = 26`, so it cannot show that an admitted
  source never ends in a raw Gamma heap failure.

  Remaining work:

  - Establish whole-producer pair containment under the selected profile, by a
    sharper structural argument or by measurement, not by another isolated
    capture or name-path fast path. Keep cumulative compiler allocation
    distinct from receipt size and from generated-application runtime
    exhaustion, which is not a compiler outcome.
  - For a witnessed evaluator exhaustion during compilation, trace its
    allocation owner and observation contract first. A Gamma-owned failure is
    not DCOUT, and the
    [arithmetic probe](bootstrap/3_delta/implementation/boundary/README.md#arithmetic-allocation-probe)
    rules out inventing a general DCOUT heap code.
  - Follow the
    [selected producer's resource ownership](bootstrap/3_delta/implementation/boundary/README.md#resource-ownership-in-the-selected-producer):
    local-slot, label, and fixup resources have zero use here and acquire no
    invented refusals, match coverage is bounded by admitted constructors, and
    corrupt private metadata is not an admitted-source refusal case.
  - Keep the existing controls instead of rediscovering them or scaling source
    arbitrarily: `sh tests/delta/normalization/run.sh`, the
    `tests/delta/resource-boundary/run.sh` selections, and the Epsilon
    checking receipt under `sh tests/epsilon/checking/run.sh`. Both measured
    stress families refuse in existing rows before post-frontend allocation
    is stressed: the
    [full-width reconstruction](bootstrap/3_delta/implementation/normalization/README.md#full-width-payload-refusal)
    at resource 12 after 4,856 seconds and the
    [wide constructor](bootstrap/3_delta/implementation/boundary/README.md#wide-constructor-allocation-probe)
    at resource 7 after 651 seconds. Neither exercises the open question.

  Retain original binding atoms, immutable scopes, and matched
  parameter/argument order. No renaming maps, additional lookup subsystem,
  allocator, or provision increase is justified so far.

  Acceptance: Delta conformance and malformed-source gates pass, the exact
  Epsilon evaluator closure compiles through the selected route, and its
  available entries execute with measured resources and unchanged semantics.
  Complete D execution belongs to P3/P4; its absence does not justify extra
  Delta mechanisms after these obligations close. These obligations do not
  block independent bootstrap work.

  Flag: starting at `8a49b3e011`, the containment work is seven consecutive
  accounting slices with no change in the customer's result. Two of them
  changed compiler algorithms to lower a bound coefficient (name-trie prepend
  `0ed76ef37c`, pairwise capture merge `116758c61f`) while every receipt
  stayed byte-identical. The cost review records the Epsilon subject's
  cumulative allocation at 2,242,373 pairs, 5.6% of the arena, while the
  resulting envelope is vacuous against the arena from `N = 26`, so per-phase
  sharpening has no demonstrated end. Apply the
  [scope checkpoint](AGENTS.md#scope-checkpoints): either one measured
  worst-shape study per admitted extent settles containment, or the gap is
  the [owner-escalation](bootstrap/MINIMIZATION.md#owner-escalation) finding
  that a private bound cannot receive an explicit fail-closed profile.

## P3 - Delta to Epsilon

- **EPSILON-EVALUATOR.** Complete the closure selected by
  `bootstrap/4_epsilon/epsilon_compiler.delta.sources` against the
  [Epsilon contract](bootstrap/4_epsilon/LANGUAGE.md). Grammar-level execution
  forms already have staging implementations; do not treat an unspecified
  missing construct as authorization for another implementation layer.
  Remaining obligations are witnessed checking/runtime conformance gaps,
  resource-contained fixed-storage realization and evaluator entry, complete
  D composition, and independent `RunEpsilon` refinement. For the entry, derive
  one explicit resource/request/observation profile against the selected lower
  chain and test exact/adjacent refusals without publishing an Epsilon
  observation. The private diagnostic adapter is not that boundary.
  Account for actual evaluator storage, including cumulative immutable Gamma
  allocation, under [section 10](bootstrap/4_epsilon/LANGUAGE.md#10-resource-classification).
  Do not add a hypothetical dense-storage sizing pass. Cover a large sparse
  array with few updates and a small array with repeated updates: declared
  bounds and value semantics remain exact, while actual resource exhaustion
  refuses without publishing a successful observation. Sparse diagnostic success
  is not final-profile admission; do not restore the retired Epsilon Alpha backend.
  Justify retained features by the Epsilon-written D source.
  Use concrete existing D slices for intermediate acceptance; do not invent
  speculative language facilities while D is incomplete.
  The [whole-member D customers](tests/epsilon/interpreted-omega-experiment/README.md)
  include actual tape construction and target execution: at base `497e21fb9a`
  with the customer identity pinned in that gate, on macOS arm64,
  `sh tests/epsilon/interpreted-omega-experiment/run.sh --customer 'Omega D Alpha tape buffers'`
  checks D's fixed-up 79-byte echo/count tape and runs its exact emitted bytes
  with the selected Alpha seed. This covers a concrete D dependency, not the
  complete source or final resource envelope; the remaining acceptance below
  must not be replaced by further emitter-only controls.
  At base `564b45e205`, the unchanged 92,229-byte D lexer took 145.273 seconds
  on macOS arm64; exact-current-callable checking reduces the measured run to
  120.518 seconds without a new index or profile. Run
  `sh tests/epsilon/interpreted-omega-experiment/run.sh --customer 'Omega D lexer'`.
  Retain `sh tests/bootstrap/omega-parser/run.sh` as the complete-source parser
  regression. Its [recorded time/allocation comparison](tests/bootstrap/omega-parser/README.md#field-identity-comparison)
  passed, but does not close final checking, resource/entry conformance, or
  whole-D compilation. Do not replace those remaining requirements with more
  parser-only controls.
  Checking-side reason coverage is now complete: `sh tests/epsilon/checking/run.sh`
  runs 97 judgments, of which eleven added controls execute the ten previously
  uncovered closed rejection reasons — `InvalidToken`, `InvalidCharacterLiteral`,
  both `IntegerLiteralOutOfRange` boundaries, `UnexpectedToken`, `MissingEntry`,
  `InvalidBoundary`, `InvalidDataShape`, `InvalidArrayLength`,
  `UseBeforeInitialization`, and `EscapingView` — each at its contract anchor
  under the unchanged 716,212-byte checker receipt `a6d49f0f7b2eca2c66e985daeb9380d0285a160ce468803ee5ce74dce9ac690c`
  on macOS arm64. No evaluator defect was witnessed; runtime conformance,
  resource containment, evaluator entry, complete D composition, and
  `RunEpsilon` refinement remain.
  Final acceptance depends on complete D: that exact source executes through
  the selected lower chain and refines `RunEpsilon`, with no Epsilon-owned
  Alpha backend or hidden host implementation.

## P4 - Epsilon to Omega and self-hosting

- **OMEGA-D.** Complete the Epsilon closure selected by
  `bootstrap/5_omega/omega_compiler.epsilon.sources` as the first full Omega
  compiler. Work against settled product semantics and actual C requirements;
  conservative, slow interpreted execution is acceptable when feasible.
  Complete the [standalone request](wiki/spec/build/compiler_request.md) field/tag,
  outcome/phase, and scalar-resource tables with C; exact/adjacent vectors,
  malformed-input rejection, bounded arithmetic, and Complete-only publication
  must agree before either implementation claims the V1 boundary.
  Continue from the [source-to-executable gate](tests/bootstrap/omega-executable/README.md)
  and `bootstrap/5_omega/scalar_compilation.epsilon`: extend parsed scalar
  operations and checked call/state sequencing through the existing Alpha emitter.
  Retain changed-source, entry-selection, invalid-body and no-partial-output
  controls as the executable path grows; do not replace the gate with hand-built
  tapes or shape-specific source recognizers. Its diagnostic scalar entry adapter
  is not package/Build admission or the final ProgramEntry contract. Replace that
  adapter through the real request and target route, preserving actual emitted-byte
  execution as the outer acceptance check.
  Acceptance: interpreted D compiles the exact Omega C closure for its ordinary
  `alpha_bootstrap` target and produces `omega0_compiler_bytecode.tape`.
  Depends on P3 and the product-source work in `TASKS.md`.
  Rust Alpha emission is not a dependency: the reference compiler may report
  that selected operation as not implemented, per the
  [bootstrap contract](bootstrap/CONTRACT.md#selected-execution-chain).

- **OMEGA-C.** Once the product source and D are ready, compile the exact
  Omega-written closure rooted at `source/omega/{build.omg,main.omg}` with D,
  then with `omega0`. This is the sole self-host edge.
  Acceptance: `D -> C/omega0 -> C/omega` is deterministic, `omega` recompiles
  C under the same source/target profile, and shared product suites pass.

## P5 - Audited chain closure

- **CHAIN-MANIFEST.** In shared `tools/bootstrap/` orchestration and edge-owned
  records, bind each exact source closure, artifact, semantics version,
  observation/resource profile, independently reconstructed obligation,
  certificate, and disclosed admission. Acceptance: a reviewer can follow
  every dependency back to the audited root without treating a digest,
  successful execution, or producer assertion as a proof.
  Bound so far: the audited Alpha seed container identity in
  `tools/bootstrap/alpha/seed_env.sh` (every stamp refuses a non-audited
  container), the Alpha-to-Beta edge source/tape identity in
  `tools/bootstrap/beta/artifact_env.sh`, the selected Gamma evaluator
  source/tape identity in `tools/bootstrap/gamma/evaluator_env.sh`, the Delta
  request entry, source manifest, ordered support-member manifest and packed
  support section, `GammaComposedV2` record, and repacked
  canonical closure in `tools/bootstrap/delta/compiler_env.sh` (which also
  checks the record names the selected evaluator, packed closure, and packed
  support section, and binds the staged-compiler gate's
  `development_driver.gamma` diagnostic entry), the
  Epsilon evaluator manifest and repacked closure, the canonical slice
  driver `execution_driver.delta`, and the reconstructed evaluator receipt
  obligation in
  `tools/bootstrap/epsilon/evaluator_env.sh`, the Epsilon-written Omega D
  manifest and repacked closure in `tools/bootstrap/omega/compiler_env.sh`,
  and the derivation-checker and Beta-encoding theory manifests and repacked
  member closures in `tools/bootstrap/proofs/sources_env.sh`; checked by
  `tests/bootstrap/{alpha,beta,gamma,delta,epsilon,omega,proofs}-identity.sh`
  (identity and refusal coverage for every bound subject — seed container,
  flat-edge source and tape, entry, manifest, member, composed record,
  driver, and receipt — plus cross-pin agreement for every repository
  record of a bound subject: gate pins, rung READMEs, edge profiles,
  owner records, the Beta language subject table, the proofs subject
  statements, and `execution_storage.md`; the sweep already corrected one
  stale packed-development record in `tests/delta/staged-compiler/README.md`
  — without executing the rungs; seed execution
  needs macOS arm64 or Windows x64). Every test gate consuming a canonical
  closure now reaches it through the bound materializers or their
  `require_*_identity` checks; only gate-local diagnostic closures and
  per-gate prefix entries still pack on top of the bound member bytes (the
  shared slice driver and the registered staged-compiler development entry
  are bound; the remaining unbound prefixes are the other gate-local drivers
  and D's gate-local entries).
  Next: bind D's OCREQ request
  entry, the omega0/omega compiler tapes, and the eventual certificate and
  disclosed admission records the same way as those artifacts land.

- **OFFLINE-REBUILD.** Close `tests/bootstrap/` reconstruction across all
  completed edges. Acceptance: a blank supported host reconstructs and checks
  the entire chain from the audited Alpha seed and repository-owned bytes;
  Rust, Python, networking, and package managers are never semantic stages.
  The manifest contains no retired rung or undisclosed authority substitute.
