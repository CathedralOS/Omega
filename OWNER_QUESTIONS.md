# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the specification and language guide; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Five owner-level decisions remain. Only the affected behavior below awaits a
ruling; other work on the same task may proceed under existing contracts.

| Named decision | Affected work |
| --- | --- |
| `interpreted-inline-assembly` | Assembly-dependent INTERPRETED-CATHEDRAL acceptance |
| `unmanaged-root-package-identity` | Portable publication in MODULE-NAMESPACE-RESOLUTION |
| `borrowed-service-suspension-carrier` | Suspension/transfer semantics in RC-BUILD-AND-PACKAGES; refresh the witness first |
| `linear-carrier-vacuous-qualification` | Qualification/custody boundary for BUMP-ALLOCATOR-CANARY and placed access |
| `mixed-integer-comparison` | Mixed typed-integer guards in the five text samples named below |

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

Proposed solutions below are recommendations, not owner rulings. Remove an
entry when the contract answers it; retain any unfinished implementation on
its owning task.

### Q1 - How should interpreted components execute checked inline assembly?

Named decision: `interpreted-inline-assembly`.

**Context:** [Embedding](wiki/spec/build/embedding.md) includes interpreted
Cathedral startup and replacement. [Checked assembly](wiki/spec/language/assembly.md)
remains legitimate source; moving all device operations behind new interfaces
has not been ratified.

**Problem:** The contracts do not select live hardware versus modeled target
state, or the correspondence evidence needed for port I/O, idle, and external
entry. Only the assembly-dependent part of **INTERPRETED-CATHEDRAL** is blocked.

[Address exposure](wiki/spec/language/counts_and_addresses.md#address-exposure)
is settled separately: machine/reference casts to `addr`, executable-entry
demand, and noncolliding interpreted code/storage identities. Those IDs do not
establish native byte geometry or execute instructions. This does not select an
answer to the assembly/environment question.

**Proposed solution:** Admit a bounded set of instruction contracts against an
explicit target-state/environment model. Preserve authority, memory, registers,
flags, ordering, control, and faults; reject unsupported instructions explicitly.
Validate one real device sequence and one idle/external-entry sequence.

**Alternatives:**
- Bind checked alternate realizations to the same instruction contracts, with
  correspondence evidence. This can avoid emulating each instruction.
- Define a limited interpreter profile that rejects assembly-bearing components;
  honest scope, but it does not deliver assembly-dependent Cathedral startup.
- Tempting but wrong: replace hardware idle with interpreter pause, redefine
  instructions through provider names, or move OS policy into the compiler.

### Q2 - What portable identity should a root without a package declaration have?

Named decision: `unmanaged-root-package-identity`.

**Context:** [Package identity](wiki/spec/build/declarations.md) uses declared
name and source lineage. Neither it nor [package sources](wiki/spec/packages/sources.md)
defines an undeclared root's identity. [Domain collision rules](wiki/spec/language/domains.md)
assume distinct declaration owners already have identities.

**Problem:** An unpackaged root and injected standard-library declarations can
both carry `package_identity: None`, collapsing separate owners onto keys such
as `[u8; N]::Utf8`. **MODULE-NAMESPACE-RESOLUTION** must not erase independent
collision checking to work around this. Separating local and toolchain declaration
owners is already required and is not blocked here; this decision concerns the
portable identity of exports from an unpackaged root.

**Proposed solution:** Require an explicit portable owner key when an unpackaged
root needs exported nominal identity; a package declaration supplies the existing
route. Keep ordinary local compilation separate from portable publication.

**Alternatives:**
- Derive identity from canonical content. This avoids an authored key but changes
  owner identity on edits and needs exact canonicalization and dependency rules.
- Keep such declarations forbidden without a package. Simpler, but less convenient.
- Tempting but wrong: use host paths or import order, exempt toolchain declarations,
  or assume distinct owner identities make competing visible names unambiguous.

### Q3 - How does a state machine holding service borrows perform a suspending operation?

Named decision: `borrowed-service-suspension-carrier`.

**Context:** [Effects](wiki/spec/language/effects.md#call-site-acknowledgements)
permits a suspending call only as a complete statement, simple `let` right-hand
side, transition subject, or terminal expression, and states that carry policy
may reject a crossing with particular live values. The transition ARM TARGET is
not among the permitted positions, and the transition grammar in
`01_tokens-to-syntax-trees/src/bodies/transitions/` has no place to spell a
`suspend`/`block` acknowledgement on a named transfer.

**Problem:** A previously reported fixture had parameters `clock: &mut Clock`
and `storage: &mut Storage` with a published `suspends; blocks;` contract and
could not reach a suspending operation by either route. As a transition arm target
the call is rejected with "acknowledges neither suspension nor blocking" and the
grammar cannot express the marker. As a terminal expression it is rejected with
"call to `entry` may suspend while `clock` remains live, but its effective policy
is `carry(suspension: forbidden, ...)`; consume the value before the call or use
a suspension-safe carrier". The cited `compiler::service_operational_contracts`
module and shared CONTRACT_PROGRAM are absent from the current tracked tree.
RC-BUILD-AND-PACKAGES must recover a source-level witness before relying on
those diagnostics as a current blocker.

[Carry](wiki/spec/resources/carry.md) already separates permission to suspend
from preservation of a live value, including loan provenance and runtime
preservation. A machine's suspension ceiling grants no suspension-safe loan.
Audit the witness against that existing mechanism first; missing carry evidence
is implementation or caller-contract work, not automatically a new carrier.

**Proposed solution:** Use existing carry and loan evidence for borrowed service
bindings; do not grant suspension safety merely from a boundary-trait type or
callee envelope. Decide the acknowledgement rule for a named transition transfer
if the recovered witness confirms the grammar/contract gap. Escalate a new carrier
only if ordinary carry and preservation contracts cannot express the requirement.

**Alternatives:**
- Admit a named transition transfer as a fifth acknowledgement position and give
  the grammar the marker. A tail transfer does not resume the transferring
  activation, so there may be nothing for it to acknowledge; that reading would
  instead exempt transfers from the check rather than extend the syntax.
- Rule that services reach a suspending body only as `Binding<Service>` fields
  rather than `&mut` parameters, and restate the synchronous-invocation
  (`invokes`) axis for that shape.
- Tempting but wrong: relax the carry policy for boundary-trait borrows without
  saying what keeps a live borrow sound across a park. The rejection is the only
  thing currently preventing a borrow from crossing a suspension.

### Q4 - Does a vacuous qualification on a linear carrier need establishment?

Named decision: `linear-carrier-vacuous-qualification`.

**Context:** [domains](wiki/spec/language/domains.md) states the obligation for a
predicate-free, route-free qualification as "None beyond carrier compatibility",
and illustrates it with `5 as i32::Km`: "a valid bare `i32` may be explicitly
qualified as `Km` without an owner grant. Predicate-free does not mean
uninhabited." The table row carries no carrier condition.

**Problem:** Three checked-in cases disagree about whether that row is
unconditional, and no two of them are reconciled by the spec text.
`fail/memory/bump_allocator_cast_minted_vacant` and `_resident` require the
refusal on core's `[linear] Extent`, reasoning that "a qualification on a
move-only carrier is custody state, not a value tag" -- minting `Vacant` with
`as` would let fabricated custody satisfy a provider view's parameter.
`index_compatibility::tests::restating_let_keeps_a_cast_mint_on_an_all_vacuous_domain_carrier`
requires the opposite on `[linear] Region`, whose only domains are the vacuous
`Left` and `Right`: a retag there is an ordinary linear move. The spec's own
`i32::Km` case requires admission on an unrestricted carrier.

`facts::index_compatibility::domain_target_is_custody_marked` now refuses only
when the carrier is linear **and** some domain over it carries a predicate or an
`established by` route, which is the one rule satisfying all three. It reached
that shape by adding the linearity conjunct: the route-management scan alone
decided `5 as i32::Km` by whether an unrelated `domain i32::Positive` was
declared elsewhere in the same program, which rejected
`pass/domains/explicit_domain_erasure`.

**Problem with the current rule:** it is still non-compositional inside the
linear case. Declaring a new predicate-bearing domain over an existing linear
carrier retroactively makes every vacuous `as` qualification of that carrier
illegal, in source that did not change and does not mention the new domain.
`Region` and `Extent` differ only by whether some *other* domain was declared.

[Placed access](wiki/spec/resources/placed_access.md#establishment-and-retirement)
already gives `Resident` exact custody and introduction routes; core's current
`Vacant`/`Resident` declarations leave placement/custody enforcement unfinished.
Reconcile those rules and declarations with ordinary qualification before adding
a new carrier property. Linearity alone is not authority, and the sibling-domain
scan is not a ratified rule.

**Proposed solution:** Make the carrier's own declaration carry the answer,
rather than inferring custody from its domain set. A linear carrier that manages
custody says so once -- at the data declaration or on the domain family -- and
every vacuous qualification of it then needs establishment whatever its siblings
declare. `Extent` would be marked; `Region` would not.

**Alternatives:**
- Rule that a linear carrier's qualification is always custody. This refuses the
  `Region` retag and deletes that test's premise; linear values would lose
  ordinary vacuous tagging entirely.
- Rule that the spec's row is genuinely unconditional and admit all three. This
  deletes the two bump-allocator refusals, and with them the only check stopping
  a cast from fabricating the custody a provider view reads as held authority.
- Tempting but wrong: keep the sibling scan and call it settled. It makes the
  legality of one module's `as` depend on an unrelated declaration in another,
  which no other qualification rule does.

### Q5 - How does a comparison order two integer operands of different types?

Named decision: `mixed-integer-comparison`.

**Context:** [numeric values](wiki/spec/language/numeric_values.md) lands an
anonymous operand at its typed partner and says a binary result uses the
selected operator's result type; it is silent on two already-typed operands of
different integer types. [Expressions](wiki/spec/language/expressions.md)
mentions comparison only for operator discovery. Typing and checking accept
`self.i < self.text.len` with `i: i32` and the `u64` length. The checked
interpreter compares both operands as unsigned whenever either is `u64`
(`checked-interpreter` `expressions_and_value_calls.rs`, `unsigned_operands`),
so `-1 < len` is false there. Terminal integer comparisons take one scalar type,
and `construct_integer_comparison` (typed-trees-to-checked-trees
`boolean_lowering.rs`) refuses operands of unequal types.

**Problem:** Five text samples (`longest_run`, `parse_int`, `roman_numeral`,
`string_hash`, `substring_search`) guard a loop with an `i32` cursor against a
byte field's `u64` length. They pass checking and then fail native compilation:
the unit plan is omitted at "state graph: terminator: conditional successors:
guard expression". A lowering has to pick one meaning, and the interpreter's
unsigned reading and exact integer order disagree for a negative left operand.

**Proposed solution:** Compare exact integer values, as range facts and exact
arithmetic already do. Lowering then emits `x < 0 || (x as u64) < n` for a
signed `x` and unsigned `n` (and the mirrored forms), and the interpreter drops
its unsigned promotion.

**Alternatives:**
- Refuse comparisons between typed integer operands of different types at
  checking, and require an explicit cast. Each comparison stays single-typed;
  the five samples declare `u64` cursors or cast after proving non-negativity.
- Adopt the interpreter's promotion to unsigned. This is C's rule and silently
  answers wrongly for negative operands.

Settled mathematical binding and proof rules live in the
[mathematical source contract](wiki/spec/proofs/mathematical_bindings.md) and
[foundation](wiki/spec/proofs/foundation.md). Their implementation and required
proofs remain on `TASKS.md`; genuinely new semantic or trust choices belong here.
