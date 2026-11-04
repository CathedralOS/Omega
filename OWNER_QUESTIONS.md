# Owner Questions

Only the owner answers, closes, removes or rewrites an entry here, including
one that an agent believes the specification already answers. Agents add
questions and may research existing ones, but they report their findings and
citations to the user and leave the entry, the specification and the boards
unchanged; they never edit the specification or a task to settle a question.

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the specification and language guide; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Psi representation and encoding design is delegated; human review is deferred
under [the current policy](omega-rust/pipeline.md#psi-implementation-and-deferred-human-audit).
Missing IR operations or a schema change needed for accepted behavior are
implementation work, not owner questions.

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

Application ports, including Squalr, are implementation customers, not a separate
class of owner questions. Port shape, API adaptation, missing library operations,
and compiler rejections stay with the implementer under the existing contracts.
Escalate only a specific unresolved language, architecture, or trust rule, citing
the contracts that leave it open. The application may motivate that question;
its name or a workaround does not establish one. Do not ask the owner to approve
ordinary port choices or treat a workaround as permission to reduce acceptance.

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

Name the contracts checked and found silent. A question that cannot cite them
has not shown its decision is open. Specifications are organized by mechanism
while questions arrive by symptom, so the answering clause is routinely in a
document whose subject is not the question's: a containment rule sits with
activation inputs rather than with the vocabulary it confines, and an
instantiation rule sits with contract import rather than with the value class
it instantiates. A reviewer verifies those citations before the framing.

## Open questions

### 1. Which input accepts an independent component's assumptions, and how is an accepted assumption spelled?

A verified component description can bind assumptions about its environment,
such as an immediate port-space write, by digest. The specification requires
the consuming side to tell verification which of those assumptions it accepts
(cited below), so that a component is not admitted on its producer's word. That
requirement, not the fixtures, motivates this question. No customer outside
fixtures and samples uses the current surface, but retiring it under the
surface-retirement rule would leave the requirement with no input at all, which
is why the question is not settled as Option B by default.

The repository has a Build call for this, `Build::accept_component_assumption(builder, digest)`.
Its prelude comment (`omega-rust/omega/packages/build-declarations/src/prelude.rs:306-315`)
says the build "accepts one exact digest per call, authored as the same
lowercase hex spelling verification diagnostics report" and that the
declaration "is harvested statically". The last sentence is stale. Today the
call does nothing:
- the checked Build evaluator evaluates the digest and discards it;
- `EvaluatedBuild.accepted_component_assumptions` has no producer;
- provider settlement still passes that always-empty set to component
  verification, so any description that binds an assumption is refused as
  unaccepted whatever the build declares;
- any spelling, including a malformed or short one, is accepted silently.

#1156 removed the static harvester and its digest decoder as unused. That decoder
required 64 hex characters and accepted either case.

The programs that depend on the answer:
- `tests/omega/fail/build/component_assumption_{malformed,short}` expect a
  spelling refusal ("64 hexadecimal characters") but compile.
- `tests/omega/pass/build/component_assumption_{acceptance,policy_accepted,policy_changed,reborrow}`
  make the Build call. `component_assumption_reborrow` spells its digest in
  uppercase. `component_assumption_policy_missing` omits the call;
  `component_assumption_mechanism` is the dependency that publishes the port
  assumption. Together the three `policy_*` fixtures are meant to distinguish a
  missing, a changed and a correct acceptance, which nothing distinguishes today.
  `component_assumption_user_declaration` calls an unrelated
  `Policy::accept_component_assumption`.
- `samples/cli/independent/component_assumption_recording` and
  `samples/cli/probes/build_component_assumptions` (with its
  `controls/invalid_assumption_one_arg`) make the call. The probe's README says
  the call "records the exact assumption in the selected build", which is
  currently false.
- `tests/omega/fail/build/component_assumption_{other_builder,swapped,wrong_arity}`
  pin the call's shape (Build activation, argument types, arity). They reject
  the same way under Options A and C.

Option A, the Build call is the acceptance input: Build evaluation records each
digest, with its machine and span, into the evaluated Build result, and the
verifier receives exactly that set. It refuses a spelling that is not 64 hex
characters. The owner also fixes whether case is significant: with
case-insensitive digests `component_assumption_reborrow` keeps its spelling;
with lowercase only, it migrates. The two fail fixtures become real reject
twins, and the `policy_*` fixtures regain their distinction. No source
re-reading is reintroduced.

Option B, acceptance belongs to package acceptance policy: assumptions are
accepted through the lock and acceptance comparison, like other trust-bearing
decisions, keyed by assumption meaning rather than a spelling. The Build call is
retired. So are the five `fail/build/component_assumption_*` fixtures, the probe control and the Build calls in
the four pass fixtures and two samples. The `policy_*` distinction moves to
lock-acceptance fixtures. The verifier receives the set the lock accepted.

Option C, both: the Build call states the build's intent, and the lock records
the reviewed acceptance as a risk-bearing row, as it does for assumed binding
relationships. Verification requires the digest in both, so the spelling rule
of Option A applies to the call.

A `TopologyRequest` (topology.md, owner intent) also lists accepted assumptions,
but it is a package-defined plan input for instance graphs, not a compiler
input for single-component verification, so it is not offered as a separate
option.

Contracts checked, and why they do not decide it:
- [Verified component descriptions](wiki/spec/build/component_publication.md#verified-component-descriptions)
  (lines 91-96, 130-135): verification returns a `VerifiedComponent` "with
  accepted assumptions", the expected subject and profile "come from independent
  caller policy", "unaccepted assumptions" is a distinct failure, and
  assumptions "are inseparable from claim consumption". This requires accepted
  assumptions to reach verification but names no input that carries them.
- [Topology](wiki/spec/packages/topology.md), verified-description consumer
  (line 228) and owner intent (lines 376-382): the consumer uses "independently
  selected subject, verification profile, and accepted assumptions", and a
  `TopologyRequest` lists accepted assumptions. Neither says whether Build or
  the lock supplies them to compilation.
- [Package acceptance](wiki/spec/packages/acceptance.md) (line 96): "explicit
  trust-bearing assumptions require acceptance". Line 249: "Acceptance binds
  permission and assumption meaning, not diagnostic wording". This leans
  toward Option B, and against keying acceptance on a diagnostic-reported
  spelling, but does not mention component assumptions or the Build call.
- [Locks](wiki/spec/packages/locks.md) (lines 36-43) and
  [assumed binding contracts](wiki/spec/build/assumed_binding_contracts.md):
  assumed binding relationships are risk-bearing rows the lock retains. This is a
  precedent for Option C's shape, but it is stated for binding relationships only.
- [Compiler request](wiki/spec/build/compiler_request.md) (line 52): "External
  admissions cover the assumptions and authority required by the requested
  compilation". This bears on Option B without naming component assumptions.
- No clause in `wiki/spec/build` defines `Build::accept_component_assumption`
  or a digest spelling rule.
