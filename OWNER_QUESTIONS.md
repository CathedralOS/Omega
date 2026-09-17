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

1. **Bare boundary-trait instance fields on a program-entry receiver.** The
   language guide ratifies `console: Console;` as an instance binding
   (chapter 1 Hello World, chapter 19 service bindings), and 136 maintained
   samples spell their Console field that way against 4 using
   `console: Service<Console> in Bound`. The [entry contract](wiki/spec/build/entry_roots.md)
   defines occurrence evidence only for `Service<R> in Bound` receiver
   fields, so the hosted receiver bridge has no establishment row for a bare
   instance field and rejects the entry ("macOS hosted receiver bridge lost
   exact contract, storage, or entry custody"); TASKS.md forbids admitting an
   erased field without a row. Witness (macOS ARM64, ab5f28700d): unchanged
   `samples/cli/basics/number_guess` fails there, and the same program with
   only that field respelled `Service<Console> in Bound` compiles and exits
   70. Decision needed: (a) a bare instance field on the entry receiver is
   establishable, which requires Psi eligibility to record provider-backed
   erased fields as a positive witness that settlement and the bridge check
   (no erased-field fallback); or (b) `Service<R> in Bound` is the only
   establishable receiver spelling, in which case chapter 1's Hello World and
   the 136 samples migrate (precedent: cli_mvp 70f1ebe6cd, generic_counters
   838a868432) and the guide's instance-binding examples stay valid only for
   non-entry data. Motivating customers: SAMPLE-CORPUS `number_guess` and
   the `cli/basics` cohort; ENTRY-CONTENT-ROOTS owns the implementation
   either way.

2. **Do fixed-token operator operands auto-borrow a place into a reference
   parameter?** [Expressions](wiki/spec/language/expressions.md#operators)
   makes operator selection operand-directed over normalized operand shapes,
   gives an attached receiver position zero "with its exact
   ownership/access mode", and otherwise puts the first ordinary parameter
   there; [indexing](wiki/spec/language/expressions.md#indexing-and-ranges)
   says `[]` selects an ordinary operator. Neither says whether a record
   place operand (`self.buffer: Buffer`) matches an ordinary
   `items: &Buffer` parameter, the way a `&self` receiver is taken on a
   place. Today only collection shells adapt (`[T; N]`, `&[T; N]`,
   `&mut [T]` into `&[T]`, per
   `typed-trees/src/typed_trees/declarations/operator/indexing.rs`); a
   `Buffer` place never matches `&Buffer`, and the checking test
   `indexed_operand_access_preserves_shared_collection_and_owned_index`
   (wiki/drafts/known_baseline_failures.md) fails only because f1f9f898e2
   stopped a wildcard re-seed that had hidden the mismatch. Decision
   needed: (a) operand position zero of a fixed-token use takes a shared
   loan of a place when the declared parameter is `&T` and the place is
   `T` (an implicit `&` limited to that position, matching attached
   receivers), or (b) operand shapes are exact and the author spells
   `(&self.buffer)[index]`, in which case the fixture is respelled and no
   typing change is needed. Recommendation: (b), since every other operand
   position and ordinary call already requires the explicit borrow and the
   spec names no implicit loan outside attached receivers.

Settled mathematical binding and proof rules live in the
[mathematical source contract](wiki/spec/proofs/mathematical_bindings.md) and
[foundation](wiki/spec/proofs/foundation.md). Their implementation and required
proofs remain on `TASKS.md`; genuinely new semantic or trust choices belong here.
