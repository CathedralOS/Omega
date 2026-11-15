# Bootstrap tasks

The owner's [bootstrap mandate](bootstrap/MINIMIZATION.md) governs this board.
Build competing peer variants until each builds the Omega compiler and
reproduces self-compilation; none is chosen during exploration. Architecture,
stage count, language facilities and explicit acceleration are left to the
agents. Keep the required semantics and state each result's trust assumptions.

The **bootstrap compiler** first builds the **Omega
compiler**. The OMEGA-D/OMEGA-C task IDs and artifact filenames stay; their
letters name historical subjects, not public names. A bootstrap compiler must
cover what its successor uses, not every Omega feature. Formal correctness
certificates are optional future work and do not block construction. Ordinary Omega proof and safety rules still apply.

A variant may organize itself differently, with its own source, specification
and run commands. Tie comparisons to the actual consuming compilers, source
revisions, resource profiles and observations. A one- or two-day cold run may be
acceptable; a timeout means the run did not finish, not that the program is
wrong. Document failed experiments and fixes tried before retiring them. Do not
stop at a prototype of a viable path.

Record real changes outside the mandate in
[OWNER_QUESTIONS_BOOTSTRAP.md](OWNER_QUESTIONS_BOOTSTRAP.md). Do not escalate
ordinary authorized engineering or reopen settled Omega semantics to avoid
implementing them. Keep working while a question is open.

## Active peer construction work

- **INTERPRETED-CONSTRUCTION.** (new-scope) Make the numbered interpreted rungs
  run under test again, reconcile their source identities with their consumers,
  and complete one run on actual Omega compiler input. Carry missing coverage and
  shared execution-cost fixes through to the successor. Identity checks alone do
  not show the compiler runs.
- **COMPILED-CONSTRUCTION.** (new-scope) Take the compiled route to building
  the Omega compiler from its own source, with reproducible rebuilds. Measure
  total implementation, audit, cold and development costs against the same
  required behavior as the other peers.
- **OMEGA-COMPILER-CONSTRUCTION.** (new-scope) Finish the Omega-written compiler
  through source checking, portable lowering and target emission, judged by its
  own sources and end-to-end program behavior. The Rust compiler is a
  comparator, not a hidden construction stage. Coordinate with
  OMEGA-PRODUCT-COMPILER-SOURCE in [TASKS.md](TASKS.md).
- **INDEPENDENT-CONSTRUCTION.** (new-scope) Develop the standalone-language peer
  under `bootstrap/independent/`, with an actual successor compiler as its first
  customer. Document its seed and compare total implementation, review and
  execution costs with the other paths. If viable, carry it through the Omega
  compiler and its reproducible self-rebuild; stage count and native versus VM
  output are engineering choices.

The tasks below detail unfinished work on the interpreted variant and the
product. Listing them gives that variant no priority over the other routes.

<a id="p4---epsilon-to-omega-and-self-hosting"></a>

## Interpreted variant and Omega self-hosting

- **OMEGA-D.** Finish the Epsilon sources listed in
  `bootstrap/interpreted/5_omega/omega_compiler.epsilon.sources` as the bootstrap
  compiler for the Omega compiler sources, following specified Omega semantics
  and the compiler's needs; slow interpreted execution is fine when feasible.

  - Migrate the evaluator, independent refinement model, fixtures and full D
    source closure to `epsilon-expression-arm-results` in
    [the Epsilon contract](bootstrap/interpreted/4_epsilon/LANGUAGE.md#8-machines-states-and-transitions):
    direct value results, forwarded machine-call results, explicit `{}` for no
    result, unchanged state transfers and ordinary `return expression?;`. Remove
    optional arm-return parsing and its pattern-arrow probe. Re-admit changed source
    and artifact identities and validate the actual D sources plus focused
    value/no-result/never/state-transfer and malformed-legacy controls before
    claiming the migrated edge. Old receipts do not cover changed bytes; this migration stays in the deferred bootstrap work.

  - Finish the [standalone request](wiki/spec/build/compiler_request.md)
    field/tag, outcome/phase and scalar-resource tables with C. Exact
    and adjacent vectors, malformed-input rejection, bounded arithmetic and
    Complete-only publication must agree before either side claims the V1
    boundary. D and C both decode phases 0, 1, 2 and 6 and answer
    `tests/product/request/test-request.sh`'s shared vectors alike.
  - Replace the old two-tag lineage text with the package manager's
    structured lineage and shared key computation under
    [shared package identity](wiki/spec/build/compiler_request.md#shared-package-identity).
    Specify the encoding and update D, C, producers and vectors together; before
    release, change the request in place with no compatibility readers. Phase 3
    must agree on identity and order before later phases publish refusals. Cover
    each supported origin, alias-only changes, changed lineage, and changed
    resolution (same key, different instance).
  - With **MODULE-NAMESPACE-RESOLUTION** in `TASKS.md`,
    extend the request for anonymous content owners and default-build
    provenance. V1's named-only rows are not that encoding; validate canonical
    identity inputs and graph sharing in D and C without a compiled-project
    registry or fake lineage.
  - Continue from the [source-to-executable gate](tests/bootstrap/omega-executable/README.md)
    and `bootstrap/interpreted/5_omega/scalar_compilation.epsilon`: extend parsed
    scalar operations and checked call and state sequencing through the existing
    Alpha emitter. Keep the changed-source, entry-selection, invalid-body and
    no-partial-output controls as the path grows; do not replace the gate with
    hand-built tapes or shape-specific source recognizers. Its diagnostic
    scalar entry adapter is neither package/Build admission nor the final
    ProgramEntry contract; replace it through the real request and target route,
    keeping execution of the emitted bytes as the outer acceptance check.
  Acceptance: interpreted D compiles the exact Omega C closure for its ordinary
  `alpha_bootstrap` target and produces `omega0_compiler_bytecode.tape`.
  Depends on the product-source work in `TASKS.md`.
  Rust Alpha emission is not a dependency: the reference compiler may report
  that selected operation as not implemented, per the
  [bootstrap contract](bootstrap/CONTRACT.md#selected-execution-chain).

- **OMEGA-C.** Once the product source and bootstrap compiler are ready, compile
  the exact Omega-written sources rooted at `source/omega/{build.omg,main.omg}`
  with D, then with `omega0`. This is the only self-hosting step.
  Acceptance: `D -> C/omega0 -> C/omega` is deterministic, `omega` recompiles
  C under the same source/target profile, and shared product suites pass.

## P5 - Audited chain closure

- **CHAIN-MANIFEST.** Record the remaining self-hosted compiler tapes, stated
  trust assumptions and kept research certificates in edge-owned records and
  shared `tools/bootstrap/` scripts. Source sets, seed and evaluator
  artifacts, request entries and gate drivers already have exact bindings; keep
  their `*_env.sh` materializers and `tests/bootstrap/*-identity.sh` refusal
  controls instead of adding another binding scheme.

  Missing: the `omega0` and `omega` tapes from OMEGA-C, and construction
  records as each variant closes. Each binding must tie together the
  exact source set, artifact, semantics, resource and observation profile,
  construction evidence and stated assumptions before the consuming gate runs.
  Identity-only tests are not execution or proof evidence.

  Acceptance: a reviewer can follow each dependency to the audited root without
  taking hashes, successful runs or producer assertions as proofs. Keep only
  plumbing that has an exact consumer under
  [whole-chain minimization](bootstrap/MINIMIZATION.md#retention-test).
  OFFLINE-REBUILD owns complete blank-host reconstruction.

- **OFFLINE-REBUILD.** Close `tests/bootstrap/` reconstruction from the audited
  Alpha seed and repository-owned bytes on supported hosts. On this route,
  Rust/Python development compilers and network downloads are not semantic
  stages. Accelerated variants state those dependencies separately and do not
  claim this cold rebuild. State all assumptions and leave out retired rungs.

  Reuse the existing identity, source-set and hygiene gates and seed execution
  routes. Linux x86-64 has a selected native seed and runs the lower rungs.
  Windows PE runs under Wine do not validate a Windows host.

  Remaining:
  - Execute Gamma, Delta, Epsilon and `omega-*` customer gates on Windows x64
    through the documented Git Bash route, recording exact observations and
    resource prerequisites. Reuse `tests/bootstrap/alpha-beta-edge.sh` for the
    Alpha/Beta prerequisite; it does not close this higher-rung roster.
    Byte-only identity checks do not replace execution.
  - Complete the chain beyond interpreted D once OMEGA-D, OMEGA-C and their
    executable construction obligations close, then reconstruct/check it on a blank supported
    host without unavailable-host success or a hidden substitute authority.
  - Complete [Delta's cumulative pair containment](bootstrap/interpreted/3_delta/implementation/boundary/execution_storage.md#remaining-obligation)
    for the selected Gamma profile. Scaled allocation projections and individual
    full-extent runs do not cover every admitted composition. Derive the full bound,
    reprovision consistently if needed, and validate the actual Epsilon sources
    and resource-boundary controls without passing off a raw evaluator failure as
    the compiler's result. Capacity tuning is not owner-blocked.

  Acceptance is the complete offline self-host chain, not another passing
  subset or a mix of incompatible source and profile pins. Formal proof
  certificates are not prerequisites for this construction milestone.

## Optional proof research: first complete encoding proof

This research is outside current construction acceptance; its proof claims
hold only once the checks below pass. Never schedule it as a prerequisite for a
runnable bootstrap chain.

- **GAMMA-DERIVATION-CHECKER.** Close the first artifact-specific encoding proof
  with the ordinary-Gamma [checker](bootstrap/proofs/checker/CHECKING.md) and
  [Beta definitions](bootstrap/proofs/beta_encoding/README.md). Follow the
  [full acceptance](bootstrap/proofs/beta_encoding/ACCEPTANCE.md), not a smaller
  batch: the subject is the whole historical Gamma evaluator's raw Beta source
  and stored Alpha tape at
  `d19361fe4deae73e1b488a940a872fec1ed885cb`, with independently reconstructed proposition
  `encode_Beta(S, 0x4000000, 0xfffffc) = Success(T)`.
  It neither certifies the current evaluator nor gates bootstrap construction.

  Status: the full untrusted derivation and source-owned theory exist; full
  checking and mutation acceptance do not. The
  2026-09-27 macOS arm64 evaluator run hit the 43200 s host timeout without
  `Checked`; see `wiki/drafts/measurements/gamma_beta_encoding_check.md`.
  The 135,485,028-byte request fits the selected 136,314,880-byte admission
  bound; `implementation/comparison/session.gamma` uses the selected 2^26
  shared work bound. Exercise these limits; they are not new blockers.
  Reuse the joint evaluator and storage limits in
  [CHECKING.md](bootstrap/proofs/checker/CHECKING.md#complete-generic-execution-provision)
  and [PROFILE.md](bootstrap/proofs/beta_encoding/PROFILE.md); do not revive the
  retired small request limit or add a second work allowance.

  Remaining:
  - Produce the complete certificate through the selected evaluator chain from
    the source-owned definitions. `tests/gamma/beta-encoding-theory/run.sh
    --full-subject` only reproduces and measures host-stepper output.
  - Run `tests/gamma/beta-encoding-check/run.sh` against the current pinned
    checker on supported native seeds. Require the exact `Checked` observation
    for 3,182,974 rows within the selected provision, record actual time/storage,
    then execute the full-subject mutation controls. A timeout, reference-VM
    result or partial run does not count.
  - Preserve the retained-role census and justify every remaining checker rule,
    definition and admission against this subject. Remove unjustified helpers;
    totality cases and owner-fixed rules need not occur in this derivation. Reuse
    the existing census, not a new inventory task.

  The chosen route to admission is more native backing, not a new composition
  rule. The [encoder candidate continuation condition](bootstrap/proofs/beta_encoding/ENCODER_CANDIDATE.md#continuation-condition)
  still governs that separate experiment. Use the provisioned macOS arm64
  seed's 128 GiB extent; other supported seeds are unaffected.

  Acceptance: full evidence checks under the exact
  [result/resource profile](bootstrap/proofs/checker/FORMAT.md), with measured
  bytes, storage, depth and time. Malformed, cyclic, missing-premise,
  wrong-subject, wrong-rule and exhausted requests must not be accepted. State
  the limit: encoding equality does not prove evaluator semantics.
  No proof search, producer-selected root, trusted assembler primitive or
  general-purpose checker extension.
