# Bootstrap tasks

The owner-adopted [bootstrap mandate](bootstrap/MINIMIZATION.md) governs this
board. Develop competing peer variants through executable construction of the
Omega compiler and reproducible self-compilation. No variant is selected during
exploration. Architectural changes, stage count, language facilities and explicit
acceleration are delegated within that mandate. Preserve required language
semantics and disclose each result's trust assumptions.

Use **bootstrap compiler** for the compiler that first builds the **Omega
compiler**. Existing OMEGA-D/OMEGA-C task IDs and artifact filenames remain stable
references; the old letters below identify historical subjects, not public names.
A bootstrap compiler needs correct coverage for its actual successor, not every
Omega feature. Formal bootstrap correctness certificates are optional future work
and do not block construction. Ordinary Omega proof/safety semantics are unchanged.

Variants may change organization and carry their own source/specification/run
surface. Keep comparisons tied to actual compiler customers, source revisions,
resource profiles and observations. A one- or two-day cold run may be acceptable;
a timeout is an uncompleted observation rather than a semantic verdict. Document
failed experiments and attempted remedies before retiring them. Do not leave an
implementation at a prototype when its path remains viable.

Record genuine changes outside the mandate in
[OWNER_QUESTIONS_BOOTSTRAP.md](OWNER_QUESTIONS_BOOTSTRAP.md). Do not escalate
ordinary authorized engineering or re-open settled Omega semantics merely to
avoid implementation. Continue independent work while a question is pending.

## Active peer construction work

- **INTERPRETED-CONSTRUCTION.** (new-scope) Restore executable verification of
  the numbered interpreted owners, reconcile current source identities with their
  consumers, and establish a completed observation on actual Omega compiler input.
  Carry missing coverage and shared execution-cost repairs through the successor
  route. Existing identity checks alone are not compiler execution acceptance.
- **COMPILED-CONSTRUCTION.** (new-scope) Carry the compiled route through the
  Omega compiler's own source construction and reproducible rebuilds. Measure
  total implementation/audit burden and cold/development costs against the same
  required behavior as the other peers.
- **OMEGA-COMPILER-CONSTRUCTION.** (new-scope) Complete the Omega-written compiler
  through required source checking, portable lowering and target emission, driven
  by its own source closure and end-to-end program behavior. The Rust compiler is
  a comparator, not an undisclosed construction stage. Coordinate with
  OMEGA-PRODUCT-COMPILER-SOURCE in [TASKS.md](TASKS.md).
- **INDEPENDENT-CONSTRUCTION.** (new-scope) Develop the standalone-language peer
  under `bootstrap/independent/`, using an actual successor compiler as its first
  implementation customer. Document its seed and compare total implementation,
  review and execution costs with the other paths. Carry viable construction
  through the Omega compiler and its reproducible self-rebuild; stage count and
  native versus VM output remain engineering choices.

The following retained tasks detail unfinished interpreted-variant and product
work. Their existence gives that variant no priority over alternative routes.

<a id="p4---epsilon-to-omega-and-self-hosting"></a>

## Interpreted variant and Omega self-hosting

- **OMEGA-D.** Complete the Epsilon closure selected by
  `bootstrap/interpreted/5_omega/omega_compiler.epsilon.sources` as the bootstrap compiler for the actual Omega compiler closure. Work against specified Omega semantics and the actual Omega compiler requirements;
  conservative, slow interpreted execution is acceptable when feasible.

  - Migrate the evaluator, independent refinement model, fixtures and full D
    source closure to `epsilon-expression-arm-results` in
    [the Epsilon contract](bootstrap/interpreted/4_epsilon/LANGUAGE.md#8-machines-states-and-transitions):
    direct value results, forwarded machine-call results, explicit `{}` resultless
    completion, unchanged state transfers and ordinary `return expression?;`.
    Remove optional arm-return parsing and its pattern-arrow probe. Re-admit
    changed source/artifact identities and validate the actual D closure plus
    focused value/resultless/never/state-transfer and malformed-legacy controls
    before claiming the migrated edge. Existing receipts are not evidence for
    changed bytes; this migration stays in the deferred bootstrap work.

  Complete the [standalone request](wiki/spec/build/compiler_request.md) field/tag,
  outcome/phase, and scalar-resource tables with C; exact/adjacent vectors,
  malformed-input rejection, bounded arithmetic, and Complete-only publication
  must agree before either implementation claims the V1 boundary. D and C
  both decode phases 0, 1, 2 and 6 and answer
  `tests/product/request/test-request.sh`'s shared vectors alike. Replace the
  old two-tag lineage text with the package manager's canonical structured
  lineage and shared key computation under
  [shared package identity](wiki/spec/build/compiler_request.md#shared-package-identity).
  Specify the encoding and update D, C, producers and vectors together; before
  release, change the request in place without compatibility readers. Phase 3
  must agree on identity/order before later phases publish refusals. Cover
  each supported origin, alias-only changes, changed lineage, and changed
  resolution (same key, different instance).
  With **MODULE-NAMESPACE-RESOLUTION** in `TASKS.md`, extend the
  request for anonymous content owners and default-build provenance. V1's
  named-only rows are not that encoding; validate canonical identity inputs and
  graph sharing in D and C without a compiled-project registry or fake lineage.
  Continue from the [source-to-executable gate](tests/bootstrap/omega-executable/README.md)
  and `bootstrap/interpreted/5_omega/scalar_compilation.epsilon`: extend parsed scalar
  operations and checked call/state sequencing through the existing Alpha emitter.
  Retain changed-source, entry-selection, invalid-body and no-partial-output
  controls as the executable path grows; do not replace the gate with hand-built
  tapes or shape-specific source recognizers. Its diagnostic scalar entry adapter
  is not package/Build admission or the final ProgramEntry contract. Replace that
  adapter through the real request and target route, preserving actual emitted-byte
  execution as the outer acceptance check.
  Acceptance: interpreted D compiles the exact Omega C closure for its ordinary
  `alpha_bootstrap` target and produces `omega0_compiler_bytecode.tape`.
  Depends on the product-source work in `TASKS.md`.
  Rust Alpha emission is not a dependency: the reference compiler may report
  that selected operation as not implemented, per the
  [bootstrap contract](bootstrap/CONTRACT.md#selected-execution-chain).

- **OMEGA-C.** Once the product source and bootstrap compiler are ready, compile the exact
  Omega-written closure rooted at `source/omega/{build.omg,main.omg}` with D,
  then with `omega0`. This is the sole self-host edge.
  Acceptance: `D -> C/omega0 -> C/omega` is deterministic, `omega` recompiles
  C under the same source/target profile, and shared product suites pass.

## P5 - Audited chain closure

- **CHAIN-MANIFEST.** Bind the remaining self-host compiler tapes, disclosed trust assumptions
  and any retained research certificates in edge-owned records and shared
  `tools/bootstrap/` orchestration. Source closures, seed/evaluator artifacts,
  request entries and gate-local drivers already use exact bindings; preserve
  their `*_env.sh` materializers and `tests/bootstrap/*-identity.sh` refusal
  controls rather than creating another binding scheme.

  Missing subjects: the `omega0` and `omega` tapes produced by OMEGA-C, and
  construction records as each variant closes. Each binding must join the exact
  source closure, artifact, semantics and resource/observation profile,
  construction evidence and disclosed assumptions before the consuming gate executes.
  Identity-only tests are not execution or proof evidence.

  Acceptance: a reviewer can follow each dependency to the audited root without
  treating hashes, execution success or producer assertions as proofs. Retain
  only plumbing with an exact customer under
  [whole-chain minimization](bootstrap/MINIMIZATION.md#retention-test).
  OFFLINE-REBUILD owns complete blank-host reconstruction.

- **OFFLINE-REBUILD.** Close `tests/bootstrap/` reconstruction from the audited
  Alpha seed and repository-owned bytes on supported hosts. In this source-owned
  route, Rust/Python development compilers and network acquisition are not semantic
  stages. Accelerated variants disclose those dependencies separately and do not
  claim this cold reconstruction. Expose all assumptions and omit retired rungs.

  Reuse the existing identity/source-closure/hygiene gates and seed execution
  routes. Linux x86-64 has a selected native seed and executable lower-rung
  coverage. Windows PE runs
  under Wine do not validate a Windows host.

  Remaining:
  - Execute Gamma, Delta, Epsilon and `omega-*` customer gates on Windows x64
    through the documented Git Bash route, recording exact observations and
    resource prerequisites. Reuse `tests/bootstrap/alpha-beta-edge.sh` for the
    Alpha/Beta prerequisite; it does not close this higher-rung roster.
    Byte-only identity checks do not replace execution.
  - Complete the chain beyond interpreted D once OMEGA-D, OMEGA-C and their
    executable construction obligations close, then reconstruct/check it on a blank supported
    host without unavailable-host success or an undisclosed authority substitute.
  - Complete [Delta's cumulative pair containment](bootstrap/interpreted/3_delta/implementation/boundary/execution_storage.md#remaining-obligation)
    for the selected Gamma profile. Scaled allocation projections and individual
    full-extent runs do not cover every admitted composition. Derive the complete
    bound, coherently reprovision if needed, and validate the actual Epsilon
    closure and resource-boundary controls without substituting raw evaluator
    failure for the compiler outcome. Capacity tuning is not owner-blocked.

  Acceptance is the complete offline self-host chain, not another passing
  subset or an aggregate of incompatible source/profile pins. Formal proof
  certificates are not prerequisites for this construction milestone.

## Optional proof research: first complete encoding proof

This retained research is outside current construction acceptance. Its stated
proof claims remain conditional on completing the exact checks below. Do not
schedule it as a prerequisite for a runnable bootstrap chain.

- **GAMMA-DERIVATION-CHECKER.** Close the first artifact-specific encoding proof
  with the ordinary-Gamma [checker](bootstrap/proofs/checker/CHECKING.md) and
  [Beta definitions](bootstrap/proofs/beta_encoding/README.md). Follow the
  [full acceptance](bootstrap/proofs/beta_encoding/ACCEPTANCE.md), not a smaller
  equation batch: the subject is the entire historical Gamma evaluator's raw
  Beta source and persisted Alpha tape at
  `d19361fe4deae73e1b488a940a872fec1ed885cb`, with independently reconstructed proposition
  `encode_Beta(S, 0x4000000, 0xfffffc) = Success(T)`.
  This remaining research does not certify the current evaluator or gate
  ordinary bootstrap construction.

  Current frontier: the complete untrusted derivation and source-owned theory
  exist, but full checking and mutation acceptance remain owed. The
  2026-09-27 macOS arm64 evaluator run hit the 43200 s host timeout without
  `Checked`; see `wiki/drafts/measurements/gamma_beta_encoding_check.md`.
  The 135,485,028-byte request fits the selected 136,314,880-byte admission
  bound; `implementation/comparison/session.gamma` uses the selected 2^26
  shared work bound. These are the provisions to exercise, not new blockers.
  Reuse the coupled evaluator/storage provisions in
  [CHECKING.md](bootstrap/proofs/checker/CHECKING.md#complete-generic-execution-provision)
  and [PROFILE.md](bootstrap/proofs/beta_encoding/PROFILE.md); do not rediscover
  the retired small request limit or add a second work allowance.

  Remaining:
  - Produce the complete certificate through the selected evaluator chain from
    the source-owned definitions. `tests/gamma/beta-encoding-theory/run.sh
    --full-subject` reproduces and measures host-stepper output; it is not
    selected-chain production.
  - Run `tests/gamma/beta-encoding-check/run.sh` against the current pinned
    checker on supported native seeds. Require the exact `Checked` observation
    for 3,182,974 rows within the selected provision, record actual time/storage,
    then execute the full-subject mutation controls. A timeout, reference-VM
    result or partial run is not admission.
  - Preserve the retained-role census and justify every remaining checker rule,
    definition and admission against this subject. Remove unjustified helpers;
    totality cases and owner-fixed rules need not occur in this successful
    derivation. Reuse the existing census rather than opening another inventory
    task.

  More native backing, not a new composition rule, is the selected admission
  route. The [encoder candidate continuation condition](bootstrap/proofs/beta_encoding/ENCODER_CANDIDATE.md#continuation-condition)
  still governs that separate experiment. Use the provisioned macOS arm64
  seed's 128 GiB extent; work on another supported seed is unaffected.

  Acceptance: full evidence checks under the exact
  [result/resource profile](bootstrap/proofs/checker/FORMAT.md), with measured
  bytes, storage, depth and time. Malformed, cyclic, missing-premise,
  wrong-subject, wrong-rule and exhausted requests cannot accept. Keep the
  explicit limitation: encoding equality does not prove evaluator semantics.
  No proof search, producer-selected root, trusted assembler primitive or
  general-purpose checker extension.
