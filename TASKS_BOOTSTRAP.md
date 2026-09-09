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
- Surface a compelling case for a new language, replacement rung, or alternate
  dialect in `OWNER_QUESTIONS.md` before implementing it, even experimentally;
  follow the [scope checkpoint](AGENTS.md#scope-checkpoints) and await the decision.

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

## Next decision - measured complexity follow-through

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
  The full numerical request/work/storage ledger remains missing: actual
  equations, integrated proof recipes, and encoder-state sharing have not been
  established. Do not select a larger profile from the partial costs.
  **Strategy pause:** retain the open P1 obligation, but do not add more isolated
  helper families or change provisions without a defensible integrated cost
  argument. Complete definitions and an integrated recipe remain the proposed
  unit for evaluating that route; routine engineering choices do not require
  an owner question. Continue independent bootstrap work while it is paused.
  Resume evidence against `c727cd624a` on macOS arm64: source/algebra review
  projects 4,494,082 work for the existing repeated output-successor recipe
  alone without cross-fact reuse; this is neither a full proof nor a lower bound.
  Earlier whole-source representation, capacity, lexical-state, and fold evidence
  remains in the linked cost review. The actual
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
  Remaining work: complete error-valued Beta encoding definitions, independently
  reconstruct the owner-fixed proposition, and produce the untrusted explicit
  certificate through the selected source-owned route.
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
  [Delta contract](bootstrap/3_delta/LANGUAGE.md), especially remaining canonical
  DCOUT resource and internal failures. Preserve full ordinary source semantics,
  checked arithmetic, exhaustive matching, proper-tail lowering, and canonical
  Gamma emission. The current Epsilon source plus a diagnostic entry already
  compiles; further optimization needs measured customer or conformance pressure,
  not a standing mandate to improve general transformation costs.
  Resume evidence on macOS arm64, canonical Delta closure SHA-256
  `84ab380f0d725061d415efa048db3bd2b7c6fe8a525a70207783cb81ce23dda5`
  and the evaluator identity pinned in
  [its profile](bootstrap/2_gamma/EVALUATOR_PROFILE.md):
  `sh tests/epsilon/checking/run.sh` reconstructs the
  [pinned checking receipt](tests/epsilon/checking/receipt.tsv) and checks its
  exact [judgment inventory](tests/epsilon/checking/fixtures.tsv) with the
  selected evaluator.
  Follow the [selected producer's resource ownership](bootstrap/3_delta/implementation/boundary/README.md#resource-ownership-in-the-selected-producer):
  Alpha local-slot/label/fixup resources have zero use here, and match coverage
  is bounded by admitted constructors. Do not implement fictitious tables or
  chase arbitrary corrupt private pairs. The
  [normalization audit](bootstrap/3_delta/implementation/normalization/README.md)
  gives source-level height and static-environment arguments, including helper
  captures; `sh tests/delta/normalization/run.sh` exercises successful compilation
  and execution at source depth 1,024 and payload width 2,048. The latter source
  compiles in 16.0 seconds with mapped-capture reuse, preserving its exact receipt;
  the original inline-projection baseline exceeded 300 seconds, not an observed
  heap exhaustion. These are not checked
  edge certificates.
  The [emission occurrence argument](bootstrap/3_delta/implementation/emission/README.md#reachable-byte-count-bound)
  separately bounds intermediate and complete byte counts below `2^62`;
  synthetic overflowing metadata is not an admitted-source refusal case.
  Next acceptance: account for remaining compiler-execution allocation against
  the selected profile, using a whole-producer bound or a separately justified
  full admitted source that exposes a gap. The
  [arithmetic allocation inventory](bootstrap/3_delta/implementation/boundary/README.md#arithmetic-allocation-probe)
  and [name-storage argument and regression](tests/delta/resource-boundary/README.md#long-identifier-storage)
  bound particular paths, not the whole producer. Neither justifies further
  arbitrary source scaling, an allocation ledger, or increased provision by
  itself. Preserve these existing controls rather than rediscovering them.
  If it exhausts the evaluator, trace the allocation owner and governing
  observation contract before changing implementation or provision; do not
  assume a Gamma-owned failure is a DCOUT frame. Distinguish cumulative
  compiler allocation from final receipt size; no host-synthesized frames or
  invented Alpha slot/label/fixup allocations. Application runtime exhaustion
  remains separate, not a static promise of unlimited execution.
  Acceptance: Delta conformance and malformed-source gates pass, the exact
  Epsilon evaluator closure compiles through the selected route, and its
  available entries execute with measured resources and unchanged semantics.
  Complete D execution belongs to P3/P4; its absence does not justify extra
  Delta mechanisms after these obligations close.

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
  Before adding expanded-storage analysis, resolve the proposed
  [Epsilon storage-policy revision](OWNER_QUESTIONS.md#epsilon-static-storage-policy).
  The existing section 10 requirement remains in force pending the ruling;
  logical expanded size and actual cumulative evaluator allocation are different
  obligations. This decision does not block checking/runtime conformance or
  lower-chain resource analysis. Do not treat sparse diagnostic success as
  final-profile admission or restore the retired Epsilon Alpha backend.
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
  Acceptance: interpreted D compiles the exact Omega C closure for its ordinary
  `alpha_bootstrap` target and produces `omega0_compiler_bytecode.tape`.
  Depends on P3 and the product-source work in `TASKS.md`.

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

- **OFFLINE-REBUILD.** Close `tests/bootstrap/` reconstruction across all
  completed edges. Acceptance: a blank supported host reconstructs and checks
  the entire chain from the audited Alpha seed and repository-owned bytes;
  Rust, Python, networking, and package managers are never semantic stages.
  The manifest contains no retired rung or undisclosed authority substitute.
