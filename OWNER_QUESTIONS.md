# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the specification and language guide; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Before a proposed surface becomes an owner question, audit whether it is
implemented, whether any authored source uses it, and whether ordinary Omega
already expresses the customer. An unimplemented, unused spelling that adds no
capability beyond existing checked machines is retired rather than redesigned.
Hypothetical future utility does not by itself preserve syntax; a concrete
customer requiring a distinct capability may propose a new surface later.

Every `OWNER-BLOCKED` escalation must name an independently motivated product
requirement or credible external use case. Existing corpus use is not required.
A test, experiment, benchmark, or implementation task cannot be the sole
motivation, and machinery introduced only to support such work is removed or
kept non-authoritative rather than promoted into an owner decision.

Apply the same test to security machinery. Omega owns only claims it can
enforce at its actual compiler, package, and artifact boundaries. A proposal
that merely restates host operating-system, credential, transport, or operator
trust must be deleted or delegated to that owner rather than dressed as an
Omega guarantee. If the boundary or enforceable claim is genuinely ambiguous,
promote that narrow ambiguity here before adding machinery.

Bootstrap design exploration is delegated engineering work, not an owner
question merely because it compares a different implementation or language
facility. Apply [whole-chain minimization](bootstrap/MINIMIZATION.md): identify
the next compiler/checker customer and compare complete audit cost. Experiments
remain non-authoritative; changes to the trust boundary or required assurances
must be surfaced before relying on them.

## Open questions

1. **Board hygiene: items the spec does not ask for, or that duplicate
   another item.** A read-only pass on 2026-09-17 traced every TASKS.md
   item to `wiki/spec`; 64 of 75 rest on a normative clause and twelve are
   named by the spec itself. The remainder need an owner decision because
   removing or merging a board item is a scope choice, not engineering:
   - **CANARY-CORPUS** and **SAMPLE-CORPUS**: two triage umbrellas over the
     same "fix what the suite reports" activity, the first listing the
     second as a dependency; neither defines a corpus the spec names beyond
     `wiki/drafts/rust_compiler_completion.md` RC-REPRESENTATIVE-PROGRAMS.
     Recommend one corpus row with that command as its acceptance.
   - **FFIVAL**: a one-line host-gated canary run that restates
     REGISTERED-CALLBACK-LIFETIME's remaining acceptance and the
     platform-gated verification bullet. Recommend removal.
   - **MACOS-APPLICATION-PUBLICATION**: its own first line says the
     [contract](wiki/spec/build/macos_application.md) is landed; what
     remains is host- and dependency-gated acceptance already listed under
     platform-gated verification. Recommend demoting to that bullet.
   - **BOUNDED-INSTALLATION-REACH-ROWS** (only open work is the carrier
     COMPONENT-SUBSTRATE must supply) into **COMPONENT-SUBSTRATE**;
     **FILESYSTEM-RELEASE-CONTRACT** (same
     [permissions](wiki/spec/build/permissions.md) clause and the same
     `FilesystemOrdinaryReleaseContract` evidence row) into
     **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW**;
     **CLEANUP-HOOK-SELECTION-AND-ERASED-OWNERSHIP** split into **CML4**
     (cleanup hook) and **PROOF-RELEVANCE-MIGRATION** (erased ownership).
     Recommend the merges.
   - **SYMBOLIC-MATERIALIZATION**: the mandate
     ([plans](wiki/spec/layouts/plans.md#derived-consumers)) stands, but its
     notes accreted recursive direct-sum and nested sum-array layout work no
     spec text asks for, and the only stated remaining acceptance is a
     host-gated Linux aarch64 rerun. Recommend trimming to the mandate plus
     one platform-gated line.
   - **EXTERNAL-ENTRY-STACK-EPOCHS** and **TR3-TR8** both own stack leases
     and epochs ([entry stacks](wiki/spec/resources/entry_stacks.md) versus
     [storage](wiki/spec/resources/storage.md)) with no stated boundary;
     either could absorb the other's work. Recommend one boundary line or a
     merge.
   - **OMEGA-PRODUCT-COMPILER-SOURCE**: the spec assumes an Omega-written
     compiler ([compiler request](wiki/spec/build/compiler_request.md)) and
     AGENTS.md names `source/psi` and `source/omega`, but no spec clause
     prescribes the two-sibling-package split or the `build.omg`/`main.omg`
     entrypoints. Keep the item; decide whether that split is spec (add the
     clause) or an owner architecture decision recorded here.

2. **Which route admits the complete Beta-encoding certificate, or does the
    P1 obligation change?** (named decision:
    `beta-encoding-certificate-admission`). GAMMA-DERIVATION-CHECKER's
    acceptance requires the produced
    `encode_Beta(S, 0x4000000, 0xfffffc) = Success(T)` derivation — `S` the
    selected evaluator's complete 47,748-byte Beta source, `T` its 8,575-byte
    persisted tape — to check under the selected
    [result/resource profile](bootstrap/proofs/checker/FORMAT.md). The
    complete 18-sort/361-constructor/108-function theory, the independently
    reconstructed owner root, and the full untrusted derivation now exist,
    and the measurement reproduces exactly from the checked-in stepper
    (24.2s on macOS arm64 at `6a1751fe08`): 3,182,484 proof rows over
    2,130,039 witness terms, 134,800,268 certificate bytes, a 135,451,492-byte
    request — 16.1 times the 8,388,608-byte request provision — projecting
    ~45-52M checker work against the 655,360-unit provision and the
    ~675,017-work ceiling the allocation ledger admits under the selected
    40,265,318-pair arena, which that provision already fills to ~97%. The
    same gap blocks production through the selected chain: a Gamma producer
    cannot emit 135,451,492 bytes under the evaluator's 16,777,212-byte
    buffered-output limit. The
    [cost review](wiki/drafts/bootstrap_cost_review.md) measured the named
    reduction levers (count out of state, concat-collapse flatten, denser
    row sharing): even a tenfold reduction leaves ~13.5MB and ~5M work, so
    the shortfall is structural, not a constants problem. Motivating
    requirement: this is the first artifact-specific proof in the audited
    chain; until a certificate is admitted, the evaluator's raw
    source-to-tape encoding remains a disclosed trust assumption rather
    than a checked edge. Options:

    - (a) Checked closed-lemma composition: a checker rule letting one
      request cite the checked conclusion of a separately admitted bounded
      request, so the certificate splits into on the order of 80 bounded
      sub-requests plus a small root. Extends the trusted calculus; the
      cost review names it as needing owner escalation. Recommended
      default: it preserves the small fixed provisions and the 1.75GiB
      realization, and each sub-request remains independently checkable.
    - (b) More native backing: a larger pair arena and request provision in
      the selected realization — a fixed zeroed startup allocation compared
      against the static image constraints — covering both the producer's
      buffered output and checker admission. At the measured density this
      is roughly 16 times the request and ~55-62 times the pair arena
      (~90-100GB at 40 bytes per pair). An owner-level realization decision
      already named in the review.
    - (c) Restate or retire the obligation: accept a weaker root or a
      different evidence shape for the encoding edge — itself an assurance
      change — or drop the artifact-specific proof and keep the evaluator's
      encoding permanently under the disclosed trust assumption.

    Until answered, GAMMA-DERIVATION-CHECKER remains open with the produced
    certificate measured but inadmissible, and the chain retains its
    explicit assumption that the selected evaluator implements Gamma.

3. **Does a transported contract instantiate its `FloatMeaning` projections
    per use site?** (named decision: `float-meaning-use-site-source-identity`).
    [Terminal source identity](wiki/spec/terminal-psi/mathematical_values.md#source-identity)
    ratifies two classes with no producer: "Non-call operation result | Owner,
    unique producing non-call operation, exact declared scalar result, and
    format." and "Call result | Owner, producing scalar-result call, exact
    declared result, and format." Terminal carries both
    (`DirectOperationFloatResult`, `DirectCallFloatResult`), the codec encodes
    them as tags 6 and 8, and the verifier rejoins them independently
    (`verify_direct_operation_float_result`, `verify_direct_call_float_result`).
    No checked source class can name either one. Projections are collected only
    from `TypedTrees::proof_facts` — signature, state, operator, domain and data
    contracts; body `requires`/`ensures` statements are checked separately and
    register no proof fact — and within such a contract the only float-valued
    names are parameters, the reserved `result`, and literals, which is exactly
    the five signature-relative classes that already exist. A call in proof
    position cannot supply the missing one: "Import its `ensures` under exact
    operand substitution; erase the call if fact-only"
    ([proof contracts](wiki/spec/proofs/contracts.md)), so a fact-only
    invocation emits no artifact operation. The only artifact carrier matching
    the two open classes is the use site of a transported contract — a boundary
    operator's or callee's `ensures Float::meaning32(result) == ...` where that
    use lowers to a scalar float call (`OperationKind::BoundaryCall`, `Call`) or
    to the one non-call float producer,
    `OperationKind::NearestIeeeFloatFusedMultiplyAdd`. Use-site checked rows and
    exact emitted-operation joins both already exist
    (`ContractProofFactOwner::OperatorUse`, `ContractCallFact` carrying caller
    machine/state/statement/call ordinal; `source_call_occurrences`,
    `selected_ieee_float_fma_occurrences`). What does not exist is per-use proof
    value identity: `float_meaning_projections` canonicalizes one row per
    (source key, operation, contract) with a plan-local `CheckedProofValueId`,
    and `float_meaning_equalities` key on one shared authored `source_expression`
    that `ProofFacts::direct_result_float_meaning_reflexivity` requires to be
    unique per owner. Measured at `a2c6676c37` (macOS ARM64): for a caller whose
    body is `helper(value)` where both machines carry the reflexive `ensures`,
    the checked table holds 2 projections and 2 equalities, both
    `DirectMachineResult`, one per owning machine — no call-site row, and the
    caller's `result` classifies as its own machine result rather than as the
    call result that produces it. Motivating requirement: FLOAT-PROVIDERS' open
    clause asks for these two producers, and a proof source that cannot be tied
    to an exact operation or call must not be synthesized. Options:

    - (a) A transported contract instantiates per use, so
      `Float::meaning32(result)` in a callee or boundary operator denotes a
      distinct canonical `FloatMeaning` value at every use site. This adds a
      use-site coordinate to the checked source key, multiplies projection and
      equality rows per site, and changes the reflexivity rejoin from
      (owner, expression) to (owner, expression, use site).
    - (b) Canonical projection identity stays per declaration. The two Terminal
      classes then have no producer from any authored form now in the language,
      and either await a new surface that names an operation or call result in
      proof position, or are retired from the Terminal source vocabulary until
      such a customer exists.

    Until answered, FLOAT-PROVIDERS' artifact-aware proof-source clause stays
    open with the Terminal identities, codec tags and verifier rejoins landed
    and unreachable from any producer.

4. **May the compiler-owned build vocabulary offer a constrained
   filesystem open/query/close chain?** The two-axis review's remaining
   acceptance is a witness that an ordinary compile earns the evidence-bound
   explicit-empty release row. No authored source can produce the retained
   occurrence that witness needs. `BUILD_PRELUDE`
   (`source-files-to-assembled-syntax/src/source_assembly.rs`) declares only
   `BuildSource::{resolve, open, read, close}` and the `BuildOutput` and
   `BuildLog` facets, and `build_facet_filesystem_operation`
   (`checked-interpreter/.../evaluator/build_paths.rs`) maps exactly those
   names onto open, read, close, create and write, none of which is the
   constrained acquisition
   (`build-evaluation/src/evidence/replay_eligibility.rs` wants an
   `open_path_handle` attempt). Reaching the raw boundary instead is refused
   by a settled rule in `build-evaluation/src/admitted_build_program.rs`:
   "build.omg may not reach runtime boundary services; use the
   compiler-owned Build facets." Witnessed at 7be8d50205 (macOS ARM64):
   a compile running the widest lifecycle a build machine can request
   retains three receipted attempts and a verified replay record, and
   `filesystem_native_handle_query_release_contracts` still derives zero
   contracts (`compiler/tests/terminal_authority.rs`). Decision needed:
   (a) add one compiler-owned Source facet that issues the constrained
   chain over a `BuildPath`, the shape std already composes for Windows
   canonicalization (`open_path_handle`, `final_path_name_by_handle`,
   `close_handle`), which widens the authored build surface that
   [permissions](wiki/spec/build/permissions.md) and
   [declarations](wiki/spec/build/declarations.md) currently fix; or (b)
   the constrained row is earned only by program-side filesystem release
   and never by a build, in which case the two-axis acceptance should name
   a program customer instead, and the broad `Filesystem` summary retires
   on that route. Until answered, the summary stays, the settlement wiring
   stands unused, and **TWO-AXIS-TERMINAL-AUTHORITY-REVIEW** and
   **FILESYSTEM-RELEASE-CONTRACT** cannot close.

5. **May a provider's selected plan resolve an installation-bound row it
   owns, or does that wait on receiver-bearing selection?**
   [Interrupt obligations](wiki/spec/build/interrupt_obligations.md#completion-reach-and-lifetime)
   settles what the completion row should be without ambiguity:
   acknowledgement "has a provider-neutral bounded abstract row beneath
   `MachineControl + PortIo`", and the language guide already prints that
   declaration. `source/library/core/interrupt.omg` is the laggard, still
   publishing an unbounded `reaches PortIo`, so no owner input is needed on
   the row itself. What is blocked is landing it. Migrating the declaration
   is a verified one-line change that takes `calling_policy_plans` from 60
   of 60 to 52 of 59: every realization of the installation-bound
   `InterruptEntry::enter` must settle the linear acknowledgement and so
   retains the nested bounded row, and the selected-row rejection that
   **BOUNDED-INSTALLATION-REACH-ROWS** tells us to keep then fires on the
   shipped route. No satisfier for `complete` can be authored today, because
   a checked body cannot discharge the linear receiver, which is the gap
   **TOP-LEVEL-BOUNDARY-REQUIREMENTS** already records. Decision needed:
   (a) installation resolves a nested bounded row against the same provider
   plan that owns it, which the item currently forbids by saying "no nested
   substitution step exists, so keep that rejection", and for which
   `RealizedMachineContractEnvelope::concrete_service_reach` looks like the
   representation; or (b) the whole bullet waits on receiver-bearing
   requirement selection, and the library declaration stays unbounded and
   knowingly stale until then. Until answered, the completion route cannot
   migrate. The rejection itself is now driven from authored source by
   `calling_policy_plans/opaque_boundaries.rs::
   selected_realization_with_an_unresolved_installation_bound_row_rejects`,
   so whichever route is chosen, losing that fence is a red test.

Settled mathematical binding and proof rules live in the
[mathematical source contract](wiki/spec/proofs/mathematical_bindings.md) and
[foundation](wiki/spec/proofs/foundation.md). Their implementation and required
proofs remain on `TASKS.md`; genuinely new semantic or trust choices belong here.
