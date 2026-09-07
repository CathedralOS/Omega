# Bootstrap tasks

Build the smallest **human-auditable proof chain to self-hosted Omega**.
Runnable compilers and checked edge evidence are both required; neither replaces
the other. Minimize the total human audit burden of semantics, admitted seeds,
implementations, checker rules, certificates, profiles, and permanent tooling.
The governing contracts are [bootstrap minimization](wiki/design_briefs/bootstrap_minimization.md)
and the [derivation checker](wiki/architecture/bootstrap_chain/proof_kernel.md).

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

P1–P5 group obligations, not a mandatory serial schedule. Proof work can advance
against existing exact artifacts before Omega is complete. Runnable lower-rung
development can also proceed under disclosed trust assumptions; it does not
close a proof edge. Do not redirect all effort to execution merely because the
checker has a longer acceptance path.

Full self-hosting remains dependent on settled exercised Omega behavior, the
[Rust product completion contract](wiki/releases/rust_compiler_completion_contract.md),
complete D, and `OMEGA-PRODUCT-COMPILER-SOURCE` in [TASKS.md](TASKS.md).
Rust remains a comparator, not bootstrap authority. Optimization matters where
measured execution or audit feasibility requires it, not as an unbounded
prerequisite to every lower-rung milestone.

## Next decision - measured complexity follow-through

- **BOOTSTRAP-COMPLEXITY-REVIEW.** Finish the decisions identified by the
  [measured cost review](wiki/design_briefs/bootstrap_cost_review.md) before
  expanding infrastructure. Owners remain Delta normalization/name checking,
  Gamma evaluator/checker/Beta definitions, and the retained comparison routes.
  Current Epsilon demand does not justify more depth machinery; retain deep-source
  conformance until a simpler implementation or owner-approved scope replaces it.
  Compare name-checking storage costs against actual customer pressure, separately
  from maximum-boundary controls. Map unique legacy assertions to selected gates
  before removing the old execution routes and their adapters together.
  For P1, resolve capacity-accounting cost within the reviewed encoder/scanner
  outline: global sharing still leaves the existing per-byte Word-counter recipe
  outside current input/work provisions before encoding. Cost grouped/binary
  accounting or a coherent larger profile, then a contiguous sequence covering
  operands, trivia, assertions, and exact endpoints before another helper family.
  The existing list and counter probes are prerequisite measurements, not
  full-certificate feasibility; do not promote unchecked source lengths or
  independent chunk budgets into proof premises.
  Acceptance: a bounded simplification/retirement plan and evidence that the
  proposed proof route can plausibly reduce total audit burden, with extrapolated
  cost distinguished from actual full-subject checks. Return for user direction
  if the strategy is not justified; do not build an alternative language or
  delete unreviewed machinery.

## P1 - Gamma checker and first complete encoding proof

- **GAMMA-DERIVATION-CHECKER.** Close the first artifact-specific proof using
  the ordinary-Gamma [checker](bootstrap/2_gamma/derivation_checker/CHECKING.md)
  and [Beta definitions](bootstrap/2_gamma/beta_encoding/README.md), following the
  [ground equality design](wiki/architecture/bootstrap_chain/derivation_calculus.md).
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
  [result/resource profile](bootstrap/2_gamma/derivation_checker/FORMAT.md), with
  measured bytes, storage, depth, and time and a reviewable account of the
  definitions and trusted assumptions. Malformed, cyclic, missing-premise,
  wrong-subject, wrong-rule, and exhausted requests cannot accept.
  Rule tests, partial proofs, and assembler agreement do not close this task.

## P2 - Gamma to Delta

- **DELTA-COMPILER.** Finish the Gamma closure rooted at
  `bootstrap/3_delta/compiler/delta_compiler.gamma` against the
  [Delta contract](bootstrap/3_delta/LANGUAGE.md), especially remaining canonical
  DCOUT resource and internal failures. Preserve full ordinary source semantics,
  checked arithmetic, exhaustive matching, proper-tail lowering, and canonical
  Gamma emission. The current Epsilon source plus a diagnostic entry already
  compiles; further optimization needs measured customer or conformance pressure,
  not a standing mandate to improve general transformation costs.
  Acceptance: Delta conformance and malformed-source gates pass, the exact
  Epsilon evaluator closure compiles through the selected route, and its
  available entries execute with measured resources and unchanged semantics.
  Complete D execution belongs to P3/P4; its absence does not justify extra
  Delta mechanisms after these obligations close.

## P3 - Delta to Epsilon

- **EPSILON-EVALUATOR.** Complete the closure selected by
  `bootstrap/4_epsilon/compiler/epsilon_compiler.delta.sources` against the
  [Epsilon contract](bootstrap/4_epsilon/LANGUAGE.md): remaining checking,
  fixed-storage realization, deterministic outcomes, execution, and evaluator
  entry. Justify retained features by the Epsilon-written D source.
  Use concrete existing D slices for intermediate acceptance; do not invent
  speculative language facilities while D is incomplete.
  Final acceptance depends on complete D: that exact source executes through
  the selected lower chain and refines `RunEpsilon`, with no Epsilon-owned
  Alpha backend or hidden host implementation.

## P4 - Epsilon to Omega and self-hosting

- **OMEGA-D.** Complete the Epsilon closure selected by
  `bootstrap/5_omega/omega_compiler.epsilon.sources` as the first full Omega
  compiler. Work against settled product semantics and actual C requirements;
  conservative, slow interpreted execution is acceptable when feasible.
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
