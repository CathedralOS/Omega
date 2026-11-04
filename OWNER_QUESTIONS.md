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

### 4. Must a lent local's domain qualification hold again when the borrow returns?

A local binding can carry a domain qualification, for example
`let mut bytes: [u8; 2] in Ascii = "ab";`. When it is lent through
`&mut` to a callee whose formal carries the same qualification, the contracts
do not say whether the local's qualification is a stored invariant, which the
callee must restore before returning, or an input qualification, which may
expire.

`tests/omega/fail/collections/fixed_array_write_rejects_element_domain` assumes
it is a stored invariant. Its `corrupt(bytes: &mut [u8; 2] in Ascii, index: u64)`
stores 255 into the lent array and is expected to reject with
`cannot prove default-domain field requirement`. It compiles today. That fixture
is left unchanged pending this decision.

The product requirement is soundness of encoding domains on byte storage. If
the local's qualification may expire, the caller's binding still has type
`[u8; 2] in Ascii`, yet it may hold a non-ASCII byte after the call. Callers
that later rely on the declared type would then rely on a fact that no longer
holds, unless the qualification expires at the lend as well.

Option A, stored invariant: a local's own domain qualification is owed at every
consumption point, like a declared field domain. A callee that receives it
through `&mut` must re-establish it before returning. A formal's additional
input qualification still expires as it does today. Only the lent storage's own
declaration is owed at return.

Option B, input qualification: lending a qualified local makes its
qualification an input to the callee, which may retire it as it may retire any
additional input domain. The caller's fact about the local ends at the lend and
must be re-established before the next use that requires it. The fixture
migrates: it should expect acceptance, or move the rejection to a later use of
`bytes` that needs `Ascii`.

Contracts checked, and why they do not decide it:
- [Mutable arguments and declared field domains](wiki/spec/language/dependent_values.md#mutable-receivers-and-declared-field-domains)
  (lines 143-146) say an ordinary `&mut T` argument carries "the declared
  field/default-domain obligations of `T`", which the callee restores at return.
  Lines 149-151 say "An additional input domain need not survive a call unless
  the ordinary contract guarantees it." Neither says which of the two a local's
  own qualification is.
- [Byte containers and encoding domains](wiki/spec/language/domains.md#byte-containers-and-encoding-domains)
  (line 684) says "Mutation must preserve or re-establish the facts on affected
  places." Read alone, this favors Option A. Read together with the lines above,
  it can mean only the facts a place still owes, which favors Option B.
- The ratified witnesses cover the two neighbors but not this case:
  - `tests/omega/pass/domains/qualified_mutable_scalar_input_domain_expires`
    ("An additional input qualification is not an implicit postcondition")
    breaks a formal's `&mut [u16; 4] in Terminated` and is accepted.
  - The fact row `samples/cli/facts/additional_input_domain_mutation` writes 73
    through `&write Cell in Before` into an unqualified local and is accepted.
  - `tests/omega/fail/domains/write_only_declared_field_domain_requires_restoration`
    protects a stored field `Holder.cell: Cell in Before`, which must be
    restored at return.
