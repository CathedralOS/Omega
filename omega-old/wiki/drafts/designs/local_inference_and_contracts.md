# Local inference and published contracts

Ratified 2026-11-11. Routine helper extraction should preserve useful local
facts, while independently checked clients rely on promises the API author
intended to publish. A written precondition constrains the implementation;
inference must not tighten it to make an unsafe body pass.

Named decisions: `callee-result-body-summary` and `nested-helper-caller-facts`.
The rules live in [invocation contracts](../../spec/language/state_contracts.md#local-call-summaries).
Implementation and migration remain under **NESTED-MACHINE-UNIFICATION** in
[TASKS.md](../../../TASKS.md), with the
[lowered checker](check_on_lowered_form.md) using the same policy.
This document claims no compiler implementation, fixture migration or fresh
execution evidence. The examples describe required behavior, not compiled programs.

## Calls and summaries

| Call boundary | Permitted semantic facts |
| --- | --- |
| Exact checked call inside the fixed verification unit | Written contract plus checked body-derived facts. |
| Ordinary call outside the unit | Published contract; additional semantic guarantees are authored. |
| Abstract requirement, unknown callable or replaceable boundary | Its interface contract, even if a possible implementation is nearby. |
| Generic callback | Its originating contract and explicitly published callback dependencies. |

The unit is fixed before proof checking, with deterministic membership and a
retained fingerprint and boundary map in the checked artifact. Importing source,
sharing a process, optimization and inlining do not silently change membership.
Use of a local body fact records the exact body/summary dependency so an edit
invalidates the affected local proofs. It does not publish an incidental
constant result as a new API guarantee.

Reusable summaries are over formal arguments, entry-state storage, outcomes
and results, with exact paths, versions and alias/custody conditions. Derive and
check the summary once against its body, then substitute each call's evaluated
arguments and establish its entry requirements. A fact for one actual argument
cannot become an unconditional guarantee over all arguments. Cyclic summaries
need justified invariants or induction; repeated assumptions are not proof.

Published inferred reach, write frames, ownership, borrow origins and validity
retain their existing rules. Authored callback-result forwarding remains
available through [result relations](../../spec/language/effects.md#callback-result-relations).
Explicit compile-time evaluation and mathematical definitions retain their own
evaluation rules. No keyword, inference toggle or runtime-check fallback is added.

## Entry conditions and intent

A private checked helper with omitted `requires` may infer simple entry
conditions. Every ordinary, terminal and recursive arrival proves them, and a
callable conversion must not hide them. A public or required machine cannot
infer narrower inputs than its published or originating contract admits.

An authored `requires` is complete even on a private helper. For example:

```text
private previous(n: u32), no requires, body n - 1
    inferred entry: n > 0
    normal result: result == n - 1
    guarded call with n > 0: accepted
    call with zero: rejected

private previous_small(n: u32), requires n <= 7, body n - 1
    rejected: the written contract admits zero
    repair: explicitly require n > 0 and n <= 7

private helper, requires true, body n - 1
    rejected: every otherwise valid u32 input must be handled
```

Writing part of an intended precondition does not ask the compiler to complete
the rest. Types, domains and other declared entry obligations remain in force.
A written `ensures` is a minimum guarantee checked against every applicable
normal exit; it does not prevent stronger local result precision.

For `score` promising a result above 100 and returning 1000, exact local callers
can know 1000 while an independent client knows only the published bound.
Changing the body to 200 preserves that promise; changing it to 50 violates it.
Only local proofs depending on the former constant acquire that body dependency.

## Separately checked traversal

Use an ordinary library predicate `all<P>(items)` and a separately verified
traversal to test that published contracts are expressive enough for reuse.
The defining relationship, in mathematical notation rather than new syntax, is:

```text
all<P>(empty)       = true
all<P>(head + tail) = P(head) && all<P>(tail)
```

The library implements the predicate using ordinary checked machines and proves
the decomposition and reassembly facts. Neither the name `all` nor the traversal
receives a compiler-known collection-proof rule.

The traversal takes a static predicate and a callback whose contract establishes
that predicate for the visited `u32`. Its published postcondition establishes
`all<P>` for the completed slice. An independently checked client supplies a
zero predicate and a callback promising to write zero; after the call it proves
the whole slice is zero without reading the traversal body, reopening the
callback body or writing a client-specific adapter.

The proof must establish that later iterations cannot invalidate earlier
elements: the callback establishes the head property, traversal establishes the
tail property, and checked disjointness/frames preserve the head while the tail
is processed. Scalar elements and no additional mutable context provide a
focused first witness. General reference-bearing elements or context require
their corresponding alias/noninterference conditions, not an assumption that
`reaches none` prevents every write. A weaker traversal promising only safe
invocation remains useful without this collection postcondition.

If the example needs a workaround, repair the general contract-composition gap
rather than expose arbitrary library implementation facts across the boundary.

## Acceptance

- When both branches of
  `choose` return false, exact same-unit callers obtain `result == false`
  without adding `ensures`. A cross-unit client cannot obtain that extra result
  fact until the published contract promises it.
- Preserve helper-extraction precision for constants, copied values, a simple
  arithmetic relation such as predecessor, branch/case outcomes, and facts
  unaffected by provably disjoint writes. Nonconstant selected paths must not
  be generalized from one convenient invocation.
- A private predecessor with omitted requirements accepts a guarded positive
  call, rejects zero, and rejects conversion to a callable accepting every
  `u32`. An additional unguarded arrival cannot disappear from the check.
- A written private `requires n <= 7` does not gain positivity from the body
  or a single guarded caller. `requires true`, public omission and requirement
  satisfaction likewise cannot hide narrower input needs.
- A private stored-reference rank with no ambient helper bound
  remains a rejection: its authored `<= 7` admits zero. Retain the input and
  operation; do not make that negative pass by silently strengthening its header.
  Qualify the explicit-positive-contract and omitted-private-contract variants
  separately, reusing existing controls where possible.
- Omitted private requirements may infer; authored ones remain complete.
- Verify the traversal and client in different recorded units. The client uses
  only the published relation and callback contract; a callback with insufficient
  guarantees or a later write invalidating prior elements must not establish
  the final collection property.
- Optimization, source availability and inlining do not change source acceptance
  by changing unit membership. A changed body invalidates dependent local facts;
  an implementation edit preserving the published semantic contract does not
  export a different incidental result fact to independent clients.
- Recursive summary checking cannot bootstrap its own proof. Inference need
  not discover arbitrary recursive invariants or ranking measures; authored
  contracts/proofs and existing bounded-work outcomes remain available.
- Preserve callback dependency, ownership, write-frame and evaluation controls
  across both producers and the full Psi/native path for executable witnesses.
  Missing precision never becomes permission, an assumed fact or guarded native
  execution.
