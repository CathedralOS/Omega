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

### 1. Is the expanded sum of products the canonical form of an open index under a declared distributivity law?

[Licensed normalization](wiki/spec/proofs/contracts.md#licensed-normalization)
says distributivity requires its law, that changing the canonical form is an
explicit language compatibility event, and that index normalizer version 3
retains the exact selected provider. Version 3 sorted operands under
commutativity and flattened under associativity, and ignored a declared
distributivity law. The compiler now applies a declared left-distributivity law
by expanding `x * (y + z)` to `x * y + x * z` once per summand, flattening and
sorting the result under the addition's own laws, and expanding a nested sum in
the same way. It keeps `normalizer_version: 3` and caps an expansion at 64
summands, keeping the unexpanded form beyond. A trait that declares
distributivity in both directions (a distributive lattice) expands in neither.
Which expressions are identified depends only on the laws a conformance
licenses; this question settles the representative text, the version label and
the cap.

Option A, expansion is canonical and the version advances to 4: amend the
sentence naming version 3 and record the compatibility event.

Option B, expansion is canonical and the version stays 3: no recorded form
changes, because an expression's form moves only when its trait declares a
distributivity law, which no normalizer consulted until now, and the library's
only such trait (`CommutativeSemiring`) has no index customer.

Option C, another representative (factoring, or a polynomial with explicit
coefficients): every form the implementation produces under the law changes.

Contracts checked: [licensed normalization](wiki/spec/proofs/contracts.md#licensed-normalization)
fixes the licensing and the version provenance but names no representative.
`source/library/core/algebra.omg` describes the proof engine's tier-2
normalizer as expanding a capped natural-coefficient polynomial form; that is
commentary on the proof side, not a rule for index identity. [Domains](wiki/spec/language/domains.md)
says computed open indices retain a normalized expression and that exact
operation and law identities survive generic substitution and interface
publication, without defining the expression's form.
`samples/cli/facts/open_index_distributivity` pins the expansion with its reject
twins.

### 2. Is a layout used only inside unselected library code a Psi obligation under every target?

A fixed-representation use (`LayoutStorage`, `CLayout`) inside a body that no
Build row selects and no entry reaches is laid out today by Psi's build-time
admission under every selected target that carries representation facts. Since
std moved its POSIX native bindings to Build rows (#1617, #1622, #1625), portable
std code such as `posix_canonicalize` and `FilesystemPosixProvider`, and the
Darwin C-layout stat and timespec records their closures use, is no longer
reached through any target's provider defaults. It is therefore unscoped, and it
is laid out under every target with facts (#1653 stopped also demanding it from
targets without facts). A console-only program pays for laying out filesystem
code it never calls, once per target.

The product requirement is compile latency of ordinary std-dependent programs.
`tests/omega/pass/host/console_exit_process_exit` is recorded as
`checked 11753ms` in `tests/omega/corpus_outcomes.txt` (`@ccdb99ca2214`, check
target macos_arm64, host unrecorded). On main `de9648a198` plus #1653
(`a151ce613a`), one `omega --check` of it took 148.5 s on a linux_x86_64 cloud
container (the author's measurement), and 95.6 s on main `91b6625545` after
#1656. Three smoke fixtures
(`control_flow/borrowed_state_argument`,
`control_flow/runtime_owned_state_parallel_swap_exit`,
`write_only/state_forwarded_scalar_store_exit`) exceed the 120 s corpus cap. For
the first of them, PR #1656 reports 33.9 s per check in fixed-layout evaluation
across 85 layout applications in 6 target scopes at `5209eac451`. After its
repair of the proof judge's quadratic scans, about 79 s of extra checking
remains with no single hot frame. On `91b6625545` the console check spends 52 s
in stage-05 checking of the newly demanded std code and only a few seconds in
layout formation. Option A below would remove the layout share only; both
options still resolve, type and check that code, so neither recovers the rest.

A Build row's target cannot scope the code: a source declaration acquires no
target tag, and the row demands only a body-less boundary requirement, not its
portable callers.

Option A, demand-limited layout: Psi lays out a fixed-representation
application only under each target whose Build rows select, or whose entry
reaches, a closure containing it. That includes rows for targets outside the
requested output, so
`tests/omega/fail/layouts/unselected_target_{helper,callback}_layout_underflow`
and `unselected_target_layout_contract` keep rejecting. A use reached by no row
and no entry is resolved, typed and checked, but not laid out under target facts.
Its layout applicability is left to Omega's validation of whichever closure a
later row selects. "Every supplied implementation is checked" then excludes
target-fact layout formation for code no row or entry reaches.

Option B, mandatory layout of all supplied code: every supplied
fixed-representation application is laid out under every target with facts,
whether or not anything reaches it. Layout formation is part of the ordinary
source check of every supplied implementation, and the cost scales with a
library's target-specific code times the number of targets, for every program
that depends on it.

Contracts checked, and why they do not decide it:
- [Layout plans](wiki/spec/layouts/plans.md#compile-time-journey-and-build)
  (lines 301-305) say "the using representation application selects the exact
  implementation" and the compiler "supplies its authorized schema and target
  facts"; lines 313-314 say "merely importing a package does not instantiate its
  layouts". Neither says whether a use inside a checked but unselected body is a
  using application or a mere import.
- [Terminal Psi product](wiki/spec/terminal-psi/product.md) (lines 28-30) calls
  "target-selected layout" a later realization fact; lines 216-219 say "selected
  ... layout ... applicability is a realization obligation". These place
  *selected* layout in Omega but say nothing of unselected uses.
- [Multi-target compilation](wiki/spec/build/configuration.md#multi-target-compilation)
  (lines 166-173), [scoped execution](wiki/spec/build/scoped_execution.md)
  (lines 164-167, "including unselected target implementations") and
  [provider selection](wiki/spec/build/provider_selection.md#build-owned-target-selection)
  (line 106) require ordinary source checks of every supplied implementation.
  They do not say whether target-fact layout formation is one.
- [Behavior exclusions](wiki/spec/build/behavior_exclusions.md) (lines 126-131)
  already draw the same line for another obligation: "An unused unselected
  implementation ... need not satisfy a product exclusion, but remains subject to
  ordinary source checks."
- [Machines](wiki/spec/language/machines.md) (lines 186-190),
  [provider selection](wiki/spec/build/provider_selection.md#build-owned-target-selection)
  (lines 91-93, 107) and [Build-owned native binding](wiki/spec/build/foreign_bindings.md#build-owned-native-binding)
  (lines 36-39) keep target keys on Build rows, which is why a row cannot scope
  the code.
- The layout implementation note (`omega-rust/psi/compilation/build-time-evaluation/layouts.md`,
  not a contract) leans both ways. Lines 43-45 say Build supplies "selected
  providers" and that a scope with no application needs no facts. Lines 96-98 and
  110-113 follow provider bodies "under each applicable target" and reject a bad
  target-family member "even when deployment requests another target".

### 3. Which input accepts an independent component's assumptions, and how is an accepted assumption spelled?

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
