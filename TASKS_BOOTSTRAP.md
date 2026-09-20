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

The `P`-numbered groups below are task-group labels, not owner-question
numbers or a mandatory serial schedule. P1 is the Gamma checker's proof that
the entire selected evaluator Beta source encodes to its exact persisted
Alpha tape; it is not a proof that the evaluator implements Gamma. An empty
`OWNER_QUESTIONS.md` means there is no unanswered owner decision, not that
these implementation tasks pass.
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

  Owner decision `beta-encoding-certificate-admission` selects more native
  backing over a checker composition rule, recorded at the
  [checking ledger](bootstrap/proofs/checker/CHECKING.md#complete-generic-execution-provision).
  The [cost review](wiki/drafts/bootstrap_cost_review.md) established that no
  reduction closes the gap, so the provisions grow instead and the five-rule
  calculus stays. The
  [encoder candidate](bootstrap/proofs/beta_encoding/ENCODER_CANDIDATE.md)'s
  pause is unaffected: the decision settles which route admission takes, not
  when provisions are selected, and that
  [continuation condition](bootstrap/proofs/beta_encoding/ENCODER_CANDIDATE.md#continuation-condition)
  still requires one complete definition package and integrated recipe to pin
  the extrapolated coefficients first.

  Remaining work:

  - Done: the ground, index, memo and allocation bounds were rederived at the
    measured extent and the three coupled provisions selected, recorded in the
    [checking ledger](bootstrap/proofs/checker/CHECKING.md#complete-generic-execution-provision)
    with the second-platform reproduction of every measured figure in
    [PROFILE.md](bootstrap/proofs/beta_encoding/PROFILE.md). Selected:
    136,314,880-byte request extent (130 MiB) inside a 137,363,456-byte
    (131 MiB) evaluator frame, a 67,108,864-unit (2^26) work counter, and a
    3,387,293,850-pair arena — the deeper memo key spaces moved the amortized
    ledger constant to 50 pairs per unit.
  - Done: the Alpha realization supplies the extent. Both audited seeds now
    obtain `M` at startup — `VirtualAlloc` on Windows x64, anonymous
    `mmap` via `svc #0x80` on macOS arm64 — over `MEMSIZE = 0x2000000000`
    (128 GiB) in AlphaBootstrapV5. `M` stays a flat zeroed array, no opcode
    transition moved, and execution remains a function of tape and input.
    The evaluator frames requests at `0x10000000..0x18300000`, buffers
    135,266,304 output bytes at `0x18300000..0x20400000`, and admits
    3,422,453,760 pair nodes at `0x20400000..0x2000000000`; the ledger's
    3,387,293,850-pair and 134,800,268-byte certificate provisions hold.
    Native acceptance on Windows/macOS/QEMU is still required and is the
    remaining validation for the derived containers.

  - Produce the certificate through the selected chain from source-owned
    definitions. The host-side
    [stepper](tests/gamma/beta-encoding-theory/stepper.py) production now
    has a gate mode:
    `tests/gamma/beta-encoding-theory/run.sh --full-subject` reproduces the
    complete derivation on any host with python3, pins the theory
    reconstruction, both subject identities, and every emitted figure, and
    emits the retained-role census; it remains diagnostic production rather
    than the selected chain, which still needs a native host to evaluate and
    check.
  - Check the full certificate, and the full-subject mutations the acceptance
    document lists, under the exact profile.
  - Show that each retained checker rule and encoding helper has a role in
    that certificate, and remove the rest. The gate's census supplies the
    certificate-level evidence: every rule except symmetry appears, 107 of
    108 theory functions unfold (function 62 never does), 359 of 361
    constructors appear in terms (S_EMPTY and A_EXHAUSTED do not), and 1,911
    of the 2,813 declared clauses appear as unfolding premises; which of the
    unexercised members are retained for generality versus removed is the
    open decision.

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
  Complete D execution belongs to P4; its absence does not justify extra
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
  Depends on the product-source work in `TASKS.md`.
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
