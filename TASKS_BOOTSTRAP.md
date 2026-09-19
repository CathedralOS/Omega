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

<a id="gamma-product-checking"></a>

- **GAMMA-PRODUCT-COMPARISON.** Decide how the seven-field continuation in
  [Delta argument checking](bootstrap/3_delta/implementation/checking/types/calls.gamma)
  should be represented: ordinary nested pairs, a dynamically checked named
  product, or static nominal typing. `typing_arguments` builds the payload and
  `typing_resume_argument` decodes it; the two agree on the nested-pair layout
  by hand, and grouped bindings expose the decoding without checking it. This
  is exploratory engineering, not owner-blocked work or accepted syntax.

  The [product comparison experiment](tests/gamma/product-comparison-experiment/README.md)
  now covers every candidate on that payload: pairs, a source-emulated product
  with identity and arity words, the kind word the checker's frame already
  carries, and nominal data compiled by the canonical Delta compiler. It
  checks construction order, scope, tail behavior, arity, forged identities,
  and failure mapping, and counts without building the evaluator, contract,
  test, and proof cost of an evaluator-minted product and of nominal typing
  inside Gamma. Its finding retains plain pairs with the existing
  kind-boundary status: source-level checks remove no layout obligation, only
  nominal typing does, and nothing measured justifies that cost in the
  trusted evaluator. Same-typed field order stays unchecked under every
  candidate.

  Remaining work:

  - Measure seeded-defect localization on the customer under each candidate,
    or state in the README why the decision does not need it. No record shows
    how often the manual layout agreement has failed.
  - Record the decision in `checking/types/README.md`, keep only the evidence
    that decision cites, and delete the experiment directory together with its
    README check in `tests/bootstrap/delta-identity.sh`, per the
    [retention row](tests/gamma/README.md).

  Acceptance: the recorded decision compares Beta implementation,
  representation, validation, resources, tests, and proof obligations against
  the customer code and manual layout obligations removed. Experiments stay
  separate from the selected language; adoption requires reconciled contracts
  and evidence. Do not repeat per-field accessor wrappers: they grew
  `calls.gamma` from four to twelve definitions with no layout guarantee.

  Flag: three experiment slices (`1c2f473404`, `ee7fef49c8`, `a7aa9c4006`)
  each ended at "retain plain pairs", and the directory is now 23 files with a
  gate that compiles nine Delta fixtures through the canonical compiler. The
  chain identity gate also depends on it: `tests/bootstrap/delta-identity.sh`
  fails when the experiment README lacks the packed-closure size. The original
  acceptance named no exit for a negative result, so non-authoritative
  evidence is accumulating as permanent validation.

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
  The subject is the entire selected Gamma evaluator's raw Beta source and
  persisted Alpha tape. The complete theory emits from Gamma source, the owner
  proposition `encode_Beta(S, 0x4000000, 0xfffffc) = Success(T)` is
  independently reconstructed, and a complete untrusted derivation has been
  produced. None of that is admission. The derivation is a 135,451,492-byte
  request: 16.1 times the request provision, about 70-80 times the work
  provision and physical pair ceiling, and about 8 times the evaluator's
  buffered-output limit, so the selected chain can neither check nor produce
  it. [PROFILE.md](bootstrap/proofs/beta_encoding/PROFILE.md) holds the
  measurements.

  Blocked on owner decision `beta-encoding-certificate-admission` in
  [OWNER_QUESTIONS.md](OWNER_QUESTIONS.md). The
  [cost review](wiki/drafts/bootstrap_cost_review.md) shows the shortfall is
  structural: a tenfold reduction still exceeds both provisions. Until the
  decision is answered, keep the
  [encoder candidate](bootstrap/proofs/beta_encoding/ENCODER_CANDIDATE.md)'s
  pause: no further isolated helper families, checker rules, or provision
  changes. Independent bootstrap work continues.

  Remaining work once a route is selected:

  - Produce the certificate through the selected chain from source-owned
    definitions. Today the full-subject derivation comes from the host-side
    [stepper](tests/gamma/beta-encoding-theory/stepper.py), a diagnostic
    producer with no gate mode; `tests/gamma/beta-encoding-theory/run.sh`
    covers finite equations, `--subject-shape`, and `--counter-cost` only.
  - Check the full certificate, and the full-subject mutations the acceptance
    document lists, under the exact profile.
  - Show that each retained checker rule and encoding helper has a role in
    that certificate, and remove the rest.

  Acceptance: the full certificate checks under the exact
  [result/resource profile](bootstrap/proofs/checker/FORMAT.md), with
  measured bytes, storage, depth, and time and a reviewable account of the
  definitions and trusted assumptions. Malformed, cyclic, missing-premise,
  wrong-subject, wrong-rule, and exhausted requests cannot accept. Rule
  tests, finite equation batches, partial proofs, and assembler agreement do
  not close this task. Encoding equality does not prove that the evaluator
  implements Gamma; that trust assumption stays explicit. No proof search,
  producer-selected root, trusted assembler primitive, or general-purpose
  extension.

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
  [Epsilon contract](bootstrap/4_epsilon/LANGUAGE.md). Justify retained
  features by the Epsilon-written D source and use concrete existing D slices
  for intermediate acceptance. An unspecified missing construct does not
  authorize another implementation layer or a speculative language facility
  while D is incomplete.

  Every grammar-level execution form executes in the staging evaluator,
  checking covers every closed rejection reason, and the
  [evaluator edge profile](bootstrap/4_epsilon/EVALUATOR_PROFILE.md) derives
  one resource/request/observation profile with exact/adjacent refusals and
  the large-sparse and repeated-update array workloads. That profile covers
  the private diagnostic adapter only. It is not the
  [section 11](bootstrap/4_epsilon/LANGUAGE.md#11-evaluator-application-and-observation-boundary)
  envelope, and its refusals are raw lower-chain statuses with empty stdout,
  not [section 10](bootstrap/4_epsilon/LANGUAGE.md#10-resource-classification)'s
  outer `Incomplete(resource, limit, requested, coordinate?)`.

  Remaining work:

  - The final request/observation envelope and evaluator `main`: bind the
    evaluator artifact, exact source closure, sealed stdin, resource profile,
    and complete `RunEpsilon` observation without host parsing.
  - Carry lower-chain refusals (Gamma statuses 250, 252, 253, 254) to the
    outer `Incomplete`. Add an evaluator-internal budget only if the envelope
    cannot classify them, and no hypothetical dense-storage sizing pass. The
    profile records one pending pin, a direct evaluator-level status-252
    witness.
  - Complete D composition: check and execute the whole D closure through the
    selected lower chain. The six
    [whole-member D customers](tests/epsilon/interpreted-omega-experiment/README.md)
    (`run.sh --customer '<name>'`) and the complete-source parser regression
    `sh tests/bootstrap/omega-parser/run.sh` cover concrete D dependencies,
    not final checking, resource/entry conformance, or whole-D compilation.
    Do not replace those requirements with more emitter-only or parser-only
    controls.
  - Independent `RunEpsilon` refinement with source, stdin, profile, and
    observation mutations, per
    [section 12](bootstrap/4_epsilon/LANGUAGE.md#12-conformance-and-change-control).
  - Fix checking/runtime conformance defects when a D slice or contract
    control witnesses one; none is recorded now.

  Acceptance depends on complete D: that exact source executes through the
  selected lower chain and refines `RunEpsilon`, with no Epsilon-owned Alpha
  backend or hidden host implementation. Sparse diagnostic success is not
  final-profile admission; do not restore the retired Epsilon Alpha backend.

  Flag: the only recorded whole-D allocation measurement
  ([parser comparison](tests/bootstrap/omega-parser/README.md#field-identity-comparison))
  checked the then 470,766-byte customer and made twelve short `parse_view`
  calls in 2,951 seconds, using 80.05% of the 40,265,318-pair arena, which
  Gamma never reclaims. D is now 509,267 bytes and compiles only nullary
  literal-returning machines. No record separates checking from execution
  allocation or projects a complete D compiling the C closure against that
  arena. Measure that before adding evaluator or D machinery; an overrun is
  the first [owner-escalation](bootstrap/MINIMIZATION.md#owner-escalation)
  finding, not an optimization task.

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
  certificate, and disclosed admission. Every edge through Omega D binds
  today: `require_bound_identity` (`tools/bootstrap/alpha/seed_env.sh`) checks
  a subject's exact size and digest against the audit record that states them,
  each edge's `*_env.sh` wraps it per subject — seed container, flat-edge
  source and tape, request entry, manifests, members, composed record, driver,
  receipt — and every test gate consuming a canonical closure reaches it
  through those materializers. The `tests/bootstrap/*-identity.sh` gates cover
  identity, refusal, and agreement with every other repository record that
  pins a bound subject. These are byte identities, not evidence that a rung
  ran: the gates bind without executing, and seed execution needs macOS arm64
  or Windows x64, the only two audited seed containers in `bootstrap/0_alpha/`.

  Remaining work:

  - D's OCREQ request entry, still framed per gate rather than bound.
  - The gate-local prefixes packed on top of bound member bytes: every
    gate-local driver except the shared Epsilon slice driver, and D's
    gate-local entries.
  - The `omega0` and `omega` compiler tapes, which **OMEGA-C** has yet to
    produce.
  - The certificates and disclosed admission records, as the edges producing
    them land. Bind each on arrival; a subject bound after the fact cannot
    show that the artifact a gate consumed was the audited one.

  Acceptance: a reviewer can follow every dependency back to the audited root
  without treating a digest, a successful execution, or a producer assertion as
  a proof. Each binding names the exact customer that requires it, per the
  [retention test](bootstrap/MINIMIZATION.md#retention-test); this orchestration
  is permanent host tooling inside the audited surface, so added plumbing
  counts against the same budget it protects.

  **OFFLINE-REBUILD** owns reconstruction of the whole chain on a blank host.

- **OFFLINE-REBUILD.** Close `tests/bootstrap/` reconstruction across all
  completed edges. Acceptance: a blank supported host reconstructs and checks
  the entire chain from the audited Alpha seed and repository-owned bytes;
  Rust, Python, networking, and package managers are never semantic stages.
  The manifest contains no retired rung or undisclosed authority substitute.
