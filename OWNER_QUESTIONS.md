# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the specification and language guide; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Three owner-level decisions remain. Only the affected behavior below awaits a
ruling; other work on the same task may proceed under existing contracts.

| Named decision | Affected work |
| --- | --- |
| `linear-carrier-vacuous-qualification` | Qualification/custody boundary for BUMP-ALLOCATOR-CANARY and placed access |
| `mixed-integer-comparison` | Mixed typed-integer guards in the five text samples named below |
| `assumed-binding-refutable-claim` | Whether ASSUMED-BINDING-CONTRACTS may override a checker-refutable semantic claim |

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

### Q1 - Does a vacuous qualification on a linear carrier need establishment?

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

### Q2 - How does a comparison order two integer operands of different types?

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

### Q3 - May an assumed runtime binding override a checker-refutable claim?

Named decision: `assumed-binding-refutable-claim`.

**Context:** The root may now
[assume a binding contract](wiki/spec/build/assumed_binding_contracts.md) across
compatible signatures without proving semantic refinement. Both bodies remain
checked, audit classifies the relationship as extremely high risk, and build-time
execution/use rejects. This permits deliberately false external claims about
backing and write authority. [Contracts](wiki/spec/proofs/contracts.md#axioms-and-receiving-policy)
separately require rejection when the checker can refute an admitted claim.

**Problem:** The discussion identified that veto but did not choose an exception.
A concrete distinction is linking a supplied `value() -> u32` guaranteeing
`result == 0` to a caller-facing requirement guaranteeing `result == 1`.
The runtime representation matches, but the semantic contradiction is known.
An unknown claim about physical backing need not be refutable in that way.
The low-level memory-editor/patching customer motivates deliberate trust; this
decision determines whether exposing more checked implementation facts can
prevent a root's intended runtime-only assumption.

**Proposed solution:** Permit the contradiction only at an explicitly accepted
`assume_binding_contract` relationship. Preserve both statements and the
contradiction in its exact risk/trust record. Structural representation, ABI,
ownership mapping and the build-time ban remain checked; the exception supplies
no checked theorem or guarantee after its false runtime premise is used.

**Alternative:** Keep the veto even here. Roots may accept statements whose truth
is unknown, including false external claims, but may not override a demonstrated
contradiction. That is the current fail-closed rule until a ruling. Do not remove
the veto globally or silently treat an ordinary failed proof as refutation.

Settled mathematical binding and proof rules live in the
[mathematical source contract](wiki/spec/proofs/mathematical_bindings.md) and
[foundation](wiki/spec/proofs/foundation.md). Their implementation and required
proofs remain on `TASKS.md`; genuinely new semantic or trust choices belong here.
