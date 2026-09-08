# State contracts and live facts

These are source-language obligations, not a list of patterns recognized by the
Rust compiler. [Mathematical proof contracts](../proofs/contracts.md) owns theorem
meaning, assumptions, bundles, and evidence; [Terminal verification](../terminal-psi/verification.md)
independently checks the emitted product.

## Bindings and arrivals

A transition explicitly supplies the target state's values. Executable uses name
the exact current-state declarations or preceding local bindings; state arrival
contracts use that state's parameters. Entry and sibling bindings cannot be
recovered by matching spelling. Nominal member and case selection retains the
declared owner, including payload projections rooted at `self`.

Machine `requires` applies at machine entry. Internal named-state jumps establish
the target state's requirements without importing machine return guarantees or
assuming machine-entry requirements again. A `self` backedge to entry must
re-establish machine preconditions and declared field domains. Actual invocations
retain both sides of the invoked contract.

Arrival substitution is simultaneous and identity-based. Scalar arguments retain
their evaluation snapshots; facts about referenced storage must survive writes
from later arguments. A same-named formal cannot alias an unrelated caller value.
Every reachable predecessor participates in a join. Unknown input cannot be
omitted to preserve a convenient invariant. A minimum collection length is not
an exact extent, and declaration initializers are not recurring state invariants.

## Mutation and subject identity

Facts apply to their exact subjects and program points. Assignments first evaluate
the right-hand side and its effects, capture transferable evidence, then invalidate
overlapping destination facts and establish the new value. Uninitialized storage
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

## Returns and proof use

Normal returns owe applicable postconditions for both value and Unit results.
An implicit return occurs after the final statement; a selected returning arm
uses that arm's facts. Named jumps are transfers, and crash edges are not normal
returns. Synthetic `result` denotes the exact returned value; output-reference
parameters retain their actual reference origins.

Exit proof consumes live facts, not the conclusion being proved. Reference-subject
transport across states does not transport the truth of an expired entry
assumption. Global declaration contexts cannot receive a sibling state's facts.
Outcome-specific guarantees follow the matching nominal result case and remain
separate from unconditional guarantees, as specified in
[proof availability](../proofs/contracts.md#identity-availability-and-erasure).

A call used as a total predicate term requires termination and absence of crash
routes. Equality conditional on normal runtime return does not establish total
denotation. Repeated calls need effect- and observation-free semantics plus exact
targets and inputs; an empty write summary alone does not prove repeatability.

Source proof search may be incomplete. Failure to infer an invariant, normalize
a term, or prove a supported implication grants no fact and is not a refutation.
The checked program is not a substitute for an independently verified Terminal
certificate. Current algorithms and support limits belong beside
[checking](../../../omega-rust/psi/pipeline/typed-trees-to-checked-trees/README.md)
and [validation](../../../omega-rust/psi/semantics/validation/README.md).
