# Invocation contracts and live facts

These are source-language obligations, not a list of patterns recognized by the
Rust compiler. [Mathematical proof contracts](../proofs/contracts.md) owns theorem
meaning, assumptions, bundles, and evidence; [Terminal fact validity](../terminal-psi/fact_validity.md)
governs which premises the emitted product's obligations may use.

## Requirement clauses

Named decision: `requires-clause-unification`, settled 2026-10-28.

`where` is retired from Omega source syntax. `requires` introduces predicates
on data declarations, named domains, generic declarations and machines, and
introduces structural or nominal static-callable requirements. Assembly bodies
carry their contract clauses directly after `asm`, without a grouping keyword.
There is no compatibility spelling or second proof system for the retired word.

The declaration determines the subject, scope and checking boundary:

| Context | Requirement |
| --- | --- |
| Data declaration | The complete value's default-domain validity obligations; construction, consumption and invariant windows retain their existing rules. |
| Named domain | Membership obligations for the exact qualified subject, without changing establishment or access routes. |
| Generic declaration | Obligations on the admitted argument tuple, including static type equations and complete callable contracts. |
| Machine | Entry obligations established at every invocation under exact actual-argument substitution. |
| Inline assembly or local assembly machine | Incoming-edge assertions over the exact modeled machine state. Callable assembly machines retain the ordinary machine contract. |

Data and domain predicates retain their logical `self` subject. This does not
introduce a machine receiver, implicit argument, borrow or authority. Generic
constraints retain binder kinds, static/runtime distinctions, type-equation
inference and specialization rules; an entry predicate does not make an unknown
value static. Repeated predicate clauses in one scope conjoin their obligations.
`ensures` remains the distinct normal-completion guarantee and proof obligation.

A static callable parameter states its contract as its type:
`const Step: machine(...)` is structural and
`const Handler: Trait::requirement` is nominal under
[typed static parameters](generics.md#typed-static-parameters). Both are
originating contracts. A `const Step: machine` parameter may instead
[have its exact type inferred](generics.md#inferred-callable-parameter-types);
neither form guesses a contract from an invocation's shape.

Assembly removes the grouping word rather than replacing it with another
`requires`: `asm requires ready ...` has one entry requirement, and
`asm clobbers none { ... }` needs no predicate clause. Body-level `requires`,
`ensures` and `clobbers` retain their checking boundaries. Nested `asm machine`
blocks carry entry/exit assertions under [checked assembly](assembly.md#contracts-and-clobbers);
an enclosing callable machine's postcondition still applies at normal completion.
No assertion becomes an admission or overrides the instruction catalog.

## Machine header delimiters

Named decision: `machine-clause-delimiters`, confirmed 2026-10-30.

Machine header clauses have no trailing semicolons. This applies uniformly to
`requires`, `ensures`, `reaches`, `invokes`, `suspends`, `blocks`, `crashes`,
`terminates` (including `terminates by`), and other machine header clauses.
The next clause keyword or the implementation body delimits a clause; a newline
does not change its owner. A checked machine proceeds directly from its last
clause to its body. A semicolon before that body does not terminate a clause and
then resume the same declaration.

A bodyless machine declaration or requirement ends with one semicolon after its
complete header. That terminator belongs to the declaration, not whichever
clause happens to come last.

A callable parameter's clauses sit inside its type, so an executable machine
header has no nested declaration to terminate:

```omega
machine invoke<const Step: machine() -> u64 terminates>() -> u64
crashes Trap
{
    Step()
}
```

Here `terminates` belongs to the type of `Step`, and `crashes Trap` belongs to
`invoke`. To put a crash ceiling on `Step`, write that clause in its type. A
callable type with several operational axes writes them together:

```omega
const Target: machine() -> u64
    suspends
    blocks
    terminates
```

A machine-shaped `requires machine` hypothesis in a
[mathematical declaration](../proofs/mathematical_bindings.md#logical-hypotheses-and-machine-use)
is a nested bodyless declaration: its semicolon closes the nested contract and
subsequent clauses belong to the enclosing declaration. No extra semicolon is
needed after its last clause, and an additional same-level terminator rejects;
it is not a compatibility spelling.
[Nested callable families](generics.md#nested-contracts-and-proof-families) still
have one terminator per bodyless declaration: closing an inner requirement and
then its enclosing bodyless requirement legitimately needs two declaration
terminators. Neither is a clause terminator. Statement, data-field, and other
declaration punctuation is unchanged by this rule.

## Bindings and arrivals

A machine enters its own executable body; there is no separately authored entry
member. Trailing nested declarations scope helper names, not runtime data.
[Nested machines](machines.md#nested-machines-and-identity) defines visibility,
declaration order and flattening. Both ordinary and terminal calls invoke the
selected machine with ordinary explicit parameters. Context records and their
borrows are supplied like other arguments; no receiver or enclosing activation
is retained or rebound implicitly.
An ordinary terminal value, explicit return, or permitted implicit Unit
completion ends the invocation and owes its declared result/return contract.

A transition explicitly supplies the target machine's arguments. Executable uses
name the exact invocation's parameters or preceding local bindings; arrival
contracts use the target's parameters. Enclosing and sibling runtime bindings cannot be
recovered by matching spelling. Nominal member and case selection retains the
declared owner, including payload projections rooted at an explicit context argument.

Machine `requires` applies at every invocation, including terminal and recursive
calls. Every arrival establishes the target's requirements and declared domains
under actual argument substitution. Nesting does not assume the parent's entry
requirements again, import its return guarantees, or supply facts invalidated
by mutation. Live caller facts can discharge a nested helper's requirements;
body inference must justify any inferred private requirements on every arrival,
not grant them from lexical ancestry. Every invocation retains both sides of
its own contract. A recursive contract is not its own proof; induction follows
the well-founded obligations in [termination](termination.md).

Arrival substitution is simultaneous and identity-based. Scalar arguments retain
their evaluation snapshots; facts about referenced storage must survive writes
from later arguments. A same-named formal cannot alias an unrelated caller value.
Every reachable predecessor participates in a join. Unknown input cannot be
omitted to preserve a convenient invariant. A join merges facts; predecessors
supplying one value agree on its exact declared domain set under
[literals and expected types](domains.md#literals-and-expected-types). A minimum collection length is not
an exact extent, and declaration initializers are not recurring loop invariants.

## Unreachable branches

Named decision: `scoped-contradiction-elimination`, settled 2026-11-03.

A checked contradiction between valid premises can establish that a particular
arrival is impossible. Obligations on that arrival may close by constructive
[contradiction elimination](../proofs/classicality.md#contradiction-elimination).
The contradiction must retain exact subjects, storage versions, scope and premise
dependencies. Failure to prove an obligation is not evidence that its arrival
is impossible; a dead branch cannot supply facts to a reachable predecessor.
Source and operand well-formedness remain required.

By default, issue an unreachable-branch warning when an authored branch
contradicts its machine's entry contract or established source facts independent
of specialization. The diagnostic identifies the branch condition and the
conflicting premises, with their source locations where available. Do not warn
merely because ordinary generic specialization or compile-time selection removes
an alternative; a generic branch that already contradicts its own contract is
not exempt. Proof-internal impossible cases do not each create source warnings.

This is an advisory diagnostic, not an inherent language error. Proving the path
impossible, pruning its emitted code and issuing a warning are separate decisions.
Proof soundness does not depend on pruning or on warning emission.

## Terminal calls in transition arms

Resolve every called target as a machine, nested or otherwise. A terminal call
is an invocation whose normal result completes the current machine, including Unit completion.
Argument admission, result compatibility, machine postconditions, cleanup,
crash and no-normal-return behavior are the ordinary call-and-completion rules.
Only the selected arm executes, with ordinary once-only operand evaluation.

Terminal machine calls accept the same operational acknowledgements as
other calls:

```omega
// Assuming Storage::read permits both suspension and blocking:
transition {
    _ -> suspend block Storage::read(storage)
}
```

This invokes `Storage::read` in the current activation and forwards its normal
result; it does not start or detach a task. If the call parks, completion of the
enclosing machine remains pending. No return, cleanup or storage reclamation
occurs merely because the call is syntactically final or becomes suspended.
Ordinary cleanup-bearing completion edges still discharge their obligations.
Tail-call lowering may remove unnecessary control state only while preserving
all live backing, loans, custody and completion obligations. Recursive runtime
calls still obey the no-accumulated-frames rule.

Nested machine calls have no marker exemption. Their checked call envelope
determines the required `suspend` and `block` acknowledgements, including in
terminal position, even when lowering later implements the call with a jump.
Markers do not redirect name resolution. Missing or partial acknowledgements
reject; redundant markers remain permitted under
[effects](effects.md#call-site-acknowledgements). The same
[borrow/carry requirements](../resources/carry.md#borrows-across-suspension)
apply in a terminal arm as at any other suspending call.

## Mutation and subject identity

Facts apply to their exact subjects and program points. Assignments first
[capture their destination place](expressions.md#evaluation-schedule), including
its selectors and bounds at that program point. They then evaluate the
right-hand side and its effects, capture transferable evidence, and finally
invalidate overlapping destination facts and establish the new value at the
captured place. Right-hand-side mutations invalidate their own overlapping facts
when they occur; the captured selector is not a fact that the selector's live
binding still has its earlier value. Uninitialized storage
has no assumed zero. Copies transport facts about the captured value, not source
expressions to be reevaluated after mutation.

A reference identifies live storage, not its former contents. Replacing a local
reference binding changes that binding without redirecting aliases made from its
previous value. A write through the reference changes the referent, not the
binding. Stored reference leaves retain their captured origins until replacement
or exposure invalidates that relation. Type or lifetime correspondence alone
does not establish a returned reference's storage origin.

Caller-visible mutation summaries include operand evaluation and callee writes.
A complete empty summary preserves facts; an unknown summary proves no
non-interference. Fact invalidation uses storage footprints, while borrow
authorization uses the access route. Neither may-write summaries nor origin
recovery grant loans, permit moves, or discharge linear claims.

### Restoring a saved scalar value

Named decision: `restored-scalar-value-facts`, ratified 2026-11-02.

Saving an integer from a valid ordinary storage place captures its current value.
A later overwrite invalidates facts about that storage, not facts about the saved
copy. Writing the saved value back establishes a new equality between the current
contents of that exact destination and the saved value. A subsequent guard on the
copy may therefore justify a requirement on the restored contents. This is new
evidence from the restoring write, not resurrection of invalidated storage facts.

Checking must establish the captured place, index identity, saved value and
absence of intervening invalidating effects at the use. A different saved value,
changed selector, possible aliasing write or unknown call effect supplies no
unproved equality. Entry storage may supply an initialized reaching value without
an earlier store in the body; no zero value or initialization is inferred for
uninitialized storage. The [Terminal fact rules](../terminal-psi/fact_validity.md#restored-scalar-values)
carry the same distinction into lowered proofs.

Analysis may bound its value/storage tracking and conservatively lose a relation;
it then needs fresh evidence for the obligation. This decision neither requires
an unlimited write history nor permits weakening a proved relation solely because
a write preceded the restoration.

## Returns and proof use

Normal returns owe applicable postconditions for both value and Unit results.
An implicit return occurs after the executable portion's final statement, not
after executing trailing declarations. A selected returning arm uses that arm's
facts. A terminal call owes both the callee's result and the caller's completion
contract; lexical ancestry adds no further return targets. Crash edges are not
normal returns. Synthetic `result` denotes the exact returned value; output-reference
parameters retain their actual reference origins.

Exit proof consumes live facts, not the conclusion being proved. Reference-subject
transport across calls or loop edges does not transport the truth of an expired
entry assumption. Declaration contexts cannot receive a sibling invocation's facts.
Outcome-specific guarantees follow the matching nominal result case and remain
separate from unconditional guarantees, as specified in
[proof availability](../proofs/contracts.md#identity-availability-and-erasure).

For [contract-based repair](dependent_values.md#contract-based-repair), a
successor's checked `ensures` may establish the enclosing machine's completion
promise on the exact borrowed subject. Its `requires` and parameter/default
domains must hold before entry; its postconditions become usable only for the
matching completed outcome. The lender's qualification and restoration duty
remain attached to the original storage through intermediate transfers.
Neither weakening the displayed view nor composing a future guarantee makes
the qualification true during repair. Existing well-founded recursion rules
govern cyclic compositions; a promise cannot prove itself.

A call used as a total predicate term requires termination and absence of crash
routes. Equality conditional on normal runtime return does not establish total
denotation. Repeated calls need effect- and observation-free semantics plus exact
targets and inputs; an empty write summary alone does not prove repeatability.

Source proof search may be incomplete. Failure to infer an invariant, normalize
a term, or prove a supported implication grants no fact and is not a refutation.
