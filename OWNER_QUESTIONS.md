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
   - **CRASH-GUARD-COST**: no spec text asks for checker throughput work;
     the mandated experiment ran and was inconclusive (1.5%), the prototype
     was discarded, and the cache-dedup slice landed. Recommend removal.
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
   - **BOUNDARY-ISSUANCE** (four-line stub sequenced "after conservation
     closes", same spec section) into **CONSERVATION-CONTRACT**;
     **BOUNDED-INSTALLATION-REACH-ROWS** (only open work is the carrier
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

2. **Is a token-bound proof-machine `requires` a formation obligation at
   the selecting use?** (named decision: `proof-operator-requires-formation`).
   `core/nat.omg` states that subtraction is partial at formation with its
   premise carried by the operator contract, and
   `fail/proofs/nat_exact_subtraction_requires_order` pins that a bare `-`
   on `Nat` without a prior `used <= total` fact rejects ("cannot prove
   `used <= total` -- the `requires` of `Nat::subtract`"). That premise is
   enforced today only by the separate operator-contract prover on the
   `operator` declaration form. The
   [executable-supply contract](wiki/spec/language/expressions.md#executable-supply)
   retires that form: `Nat::subtract` and `Nat::less_or_equal` have no
   compiler catalog identity, so they become declaration-owned bodies
   (`machine - Nat::subtract(left: Nat, right: Nat) -> Nat requires right
   <= left; { saturating_sub(left, right) }`), and a token call then "retains
   the same declaration/body association" as the named call. But
   `typed-trees-to-checked-trees/src/checks/contracts.rs` deliberately
   exempts proof-machine-to-proof-machine calls from the `requires` prover
   ("a call between proof machines denotes a mathematical application whose
   value does not depend on the callee's requires"; keeping the prover on
   such calls refuses sound requires-bearing induction), and the named call
   `Nat::subtract(total, used)` is accepted today without the order fact.
   [Mathematical bindings](wiki/spec/proofs/mathematical_bindings.md#logical-hypotheses-and-machine-use)
   and [citation and induction](wiki/spec/proofs/contracts.md#citation-and-induction)
   settle recursive citation but not whether a proof application's declared
   `requires` is a formation-time obligation at the use site. Product
   requirement: a prototype migration (reverted) compiles both Nat canaries
   including the fail fixture, so the partial-subtraction premise would be
   silently dropped for every core `Nat` consumer. Options:

   - (a) Formation obligation: a proof-to-proof application proves the
     callee's `requires` at the site whenever its facts are decidable
     there, keeping the induction exemption only for the recursive
     component's own contract under its proved ranking edge. Preserves the
     `nat.omg` rule and the fail fixture; implementation in
     `checks/contracts.rs` plus the migrated bodies. Recommended default.
   - (b) Exempt, like every named proof-to-proof call: migrate the pair,
     delete the fail fixture's rule and the harness assertion, and restate
     `nat.omg` so partiality is documented rather than enforced.
   - (c) Keep only these two declarations on a retained bodyless form
     with the operator-contract prover, contradicting the one-supply rule.

   Until answered, the `core/nat.omg` satisfier pairs stay on `operator`
   and OPERATOR-MACHINE-SUPPLY's map marks them design-blocked.

3. **Which explicit binder selects an indexed domain's index operation
   contract?** (named decision: `open-index-operation-selection`). The
   [executable-supply contract](wiki/spec/language/expressions.md#executable-supply)
   retires bodyless root `operator` slots and states that "any required
   conformance/provider selection is explicit, never an implicit search
   among visible satisfying machines"; [licensed normalization](wiki/spec/proofs/contracts.md)
   requires "an explicitly selected conformance with checked operation and
   law slots", and [domains](wiki/spec/language/domains.md) require "the
   exact selected checked algebra". Today the PDI3 open-index operation is
   the bodyless root slot `operator + IndexAlgebra::plus(left: u64, right:
   u64) -> u64;` supplied by an implicit unique-satisfier search
   (`validation/src/value_custody/type_references/open_index_expressions.rs`
   collects every machine whose conformances name `IndexAlgebra::plus` and
   requires exactly one, then `structural_judgment.rs` demands its proved
   AC algebra), and `fail/generics/open_index_unlicensed_algebra` pins
   "requires one exact proved associative/commutative algebra instance,
   but 0 were found". `IndexAlgebra` is a bare path prefix, not a declared
   type or domain, and the operand tuple is bare `u64`, so the operator
   families rule gives the declaration no semantic home and the machine
   form rejects it as a compiler-owned-family injection. Product
   requirement: `pass/generics/{open_computed_quantity_result,
   open_index_local_fact}` and their two fail controls, and every
   `domain<T, const I: u64> T::Indexed<I>` computed index. Options:

   - (a) A token-bearing trait requirement (`trait IndexAlgebra { machine +
     plus(left: Self, right: Self) -> Self; }` with `u64 satisfies
     IndexAlgebra` realized by the provider) selected by an explicit
     conformance named on the indexed domain declaration, for example
     `domain<T, const I: u64> T::Indexed<I> using IndexAlgebra;`, with the
     AC law slots on the same trait. Fits the trait row of the supply table
     and keeps selection at the declaration. Recommended default.
   - (b) The same trait selected at each use site through an explicit
     proof-static binder on the computed index expression. Finer-grained
     but repeats the selection at every index.
   - (c) Keep these slots on a retained bodyless root form with the
     implicit search, contradicting the explicit-selection rule.

   Until answered, the four `IndexAlgebra::plus` fixtures stay on
   `operator` and OPERATOR-MACHINE-SUPPLY's map marks them design-blocked;
   this is distinct from the proof-machine `requires` question, which concerns
   formation-time `requires` rather than the selection binder.

4. **Which boot protocol issues the AP startup vector, and who owns it?**
   (named decision: `ap-startup-protocol-ownership`). The [executable
   installation contract](wiki/spec/build/executable_installation.md) says AP
   startup "installs a compiler-produced low-memory trampoline and invokes a
   target boot protocol" but names no protocol; no spec, board, or source text
   mentions INIT/SIPI, a startup IPI, the local APIC ICR, or
   `EFI_MP_SERVICES_PROTOCOL`, and the xAPIC/x2APIC register facts are recorded
   as Cathedral's `local_apic` package (15221af38f), which the firewall keeps
   package-owned. `external-roots` already owns the trampoline placement
   ledger, the start edge and the quiescence edge (344063c651), whose 4 KiB
   vector geometry and real-mode arrival regime are INIT/SIPI-shaped but only
   by inference. Decision needed for x86-64: (a) the vector is issued by an
   INIT/SIPI sequence through the local APIC ICR, which makes those register
   facts a compiler-owned `target`/`program-entry-plan` leg like the UEFI Boot
   Services rows; (b) it is issued through firmware
   `EFI_MP_SERVICES_PROTOCOL.StartupThisAP` while Boot Services are live, which
   needs a located-protocol row the entry plan does not carry; or (c) a
   Cathedral-owned provider consumes the ledger's
   `SecondaryProcessorStartupInvocation` carrier and mints
   `SecondaryProcessorStartupReceipt`, the compiler owns no protocol edge, and
   the remaining compiler work is sealing that receipt's issuance (today
   `SecondaryProcessorStartupReceipt::from_provider` is public). Motivating
   customer: Cathedral's multiprocessor boot, whose secondary processors cannot
   start until one of these edges exists; AP-BRINGUP owns the implementation
   either way.

5. **Is `alpha_bootstrap` an ordinary target profile of the differential
   compiler, and what happens to a root row owned by a profile the comparator
   does not catalogue?** (named decision: `alpha-bootstrap-target-profile`).
   The [bootstrap contract](bootstrap/CONTRACT.md#selected-execution-chain)
   says interpreted D compiles the Omega compiler source closure C "given the
   package-resolved Omega compiler source closure C and ordinary target
   `alpha_bootstrap`", so `source/omega/build.omg` binds
   `builder.roots.bind(alpha_bootstrap::ProgramEntry, Main::main)` beside the
   four native rows, and the
   [compiler request](wiki/spec/build/compiler_request.md) already tags an
   `alpha_bootstrap_tape` product. But no std target package declares
   `alpha_bootstrap::ProgramEntry`, the Rust comparator's profile catalog
   (`omega/representations/target/src/lib.rs`) knows only the native, UEFI,
   `cross_platform_cli` and `local_unchecked` profiles, and the
   [entry-roots contract](wiki/spec/build/entry_roots.md) says "only rows
   owned by the selected profile enter the durable child projection" while
   listing as rejections only a row "misattributing its slot to another
   profile", duplicates and missing required slots; it does not say whether a
   row owned by a profile the toolchain does not catalogue is merely
   unselected or rejects. Today `build-evaluation/src/admission/selection.rs`
   rejects it (`root slot alpha_bootstrap::ProgramEntry belongs to unknown
   target profile alpha_bootstrap`), which is the next stop of
   `omega --check source/omega/main.omg` after the std calling-policy
   admission bug (OMEGA-PRODUCT-COMPILER-SOURCE at 18a391f660). Options:

   - (a) Catalogue `alpha_bootstrap` as an ordinary profile with a std target
     package (`std/targets/alpha_bootstrap/entry.omg` declaring
     `ProgramEntry` and its calling policy) whose only realization is the
     `alpha_bootstrap_tape` product; native realization of that profile
     rejects explicitly. Unknown-profile rows keep rejecting. This keeps the
     contract's "ordinary target" wording literal and the fail-closed row
     rule intact. Recommended default.
   - (b) Keep the comparator's catalog as is and make a row owned by an
     uncatalogued profile unselected rather than rejected (the spec sentence
     "only rows owned by the selected profile enter" read permissively). Cheap,
     but a misspelled profile would then pass silently, which the entry-roots
     rejection list otherwise guards against.
   - (c) Remove the `alpha_bootstrap` row from `source/omega/build.omg` until
     the bootstrap boards need it, and let the bootstrap toolchain supply the
     row by its own invocation. Unblocks the product check immediately but
     contradicts the bootstrap contract's "same C for the same target"
     requirement.

   Until answered, the product check stops on this row once the calling-policy
   admission bug is fixed; everything before that stop is engineering.

6. **Which route admits the complete Beta-encoding certificate, or does the
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

7. **Does a transported contract instantiate its `FloatMeaning` projections
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

8. **May the compiler-owned build vocabulary offer a constrained
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

Settled mathematical binding and proof rules live in the
[mathematical source contract](wiki/spec/proofs/mathematical_bindings.md) and
[foundation](wiki/spec/proofs/foundation.md). Their implementation and required
proofs remain on `TASKS.md`; genuinely new semantic or trust choices belong here.
