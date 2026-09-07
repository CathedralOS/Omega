# Checked template substitution

[Comparison](COMPARISON.md) | [Inner format](FORMAT.md) | [Calculus](../../../wiki/architecture/bootstrap_chain/derivation_calculus.md)

`compare_unfolded_terms(session, left, right, clause, left_coordinate,
right_coordinate, clause_coordinate)` checks one stated definition unfolding.
It returns the existing Compared Boolean and latest session, or an owned
Rejected/Incomplete outcome. This component does not check proof rows or accept
a certificate. Coordinates belong to the source-owned caller's request fields.

## Admission and bindings

Check the left global ground reference, then the right reference, exactly as
ground comparison does; invalid references reject with code 9 at their supplied
coordinate before any resource charge. The left must be a function application;
otherwise reject with code 10 `unfolding` at the left coordinate. Check the
one-based clause ordinal against that function's clause count before scanning;
an invalid ordinal rejects with code 10 at the clause coordinate.

Reserve `clause` work units before walking to that local clause. This is an
ordinal, not a constructor identity. The first clause begins immediately after
the function's clause-count field. In mode 1, the selected actual argument must
be an explicit constructor application with the clause's constructor identity.
Check its tag before its symbol. A mismatch rejects with code 10 at the clause
coordinate. Do not evaluate a function application to select a case.

Mode 0 binds each parameter slot to the corresponding left argument. Mode 1
preserves the other parameter slots, leaves the selected parent unbound, and
binds slots starting at the function arity to the matched constructor's immediate
children. Formation has checked every variable occurrence and template child;
the substitution environment is derived from this checked syntax, never supplied
as an unchecked map. A fresh invocation cannot reuse another binding environment.

Before building this clause's template index, reserve `T+1` units, where `T` is
its nonempty template count. Read the checked body reference at the clause's
last word. Compare that template under the derived bindings against `right`.
Different heads or children return Compared false; they are not malformed terms.

## Explicit traversal and separate memo

Use depth-first pending frames and tail-called visits/resumptions. A template
variable resolves its binding and calls structural ground comparison. Continue
with the returned session, preserving its work and ground memo even on false.
This is one bounded suspension: ground comparison never calls substitution.
Logical template depth must not accumulate native call frames or expand trees.

Application comparison checks tag, symbol, then children in order. No function
is evaluated. Template and ground identities are different spaces: equal scalar
row numbers are never an identity shortcut. Use a separate completed-equal memo
scoped to exactly this invocation, hence this clause and binding environment.
With ground count `N`, keys are `(template-1)*N+(ground-1)` in `[0,T*N)`.
The admitted extents give `T < 2^20`, `N < 2^19`, and at most 39 tree levels.
Reuse the shared Empty/Present constants but never the ground memo root.

Insert only after a variable's structural comparison or an application's whole
subtree succeeds. Interrupted or differing parents cannot enter the memo. Drop
this local memo on return. In particular, successful unfolding must not insert
the pair `(left,right)` into the structural ground memo: a definitional equality
is not structural identity.

Formation has already established a well-sorted body using only these bound
slots. Substitution preserves those sorts because each binding comes from the
checked application or matched constructor signature. Induction over strictly
backward template children establishes that a successful visit compares exactly
the bound variable or the same application over successful children. The local
memo reuses only completed instances of that same claim under one fixed
environment. Therefore true establishes this single admitted defining equation;
it supplies neither normalization nor an additional equality axiom.

## Cumulative work and storage

`comparison_reserve(session, amount, coordinate)` is the source-owned bulk
preflight. Require `1 <= amount <= 2147483647`; otherwise reject with code 11
`work_request` at the coordinate. Check against `limit-used` before allocation
or the requested operation. Success returns Compared true with unchanged
context/ground memo and consumed work `used+amount`. Exhaustion returns resource
4 with the existing limit and exact requested `used+amount`, without a session.
Invalid amounts precede exhaustion. Neither amounts nor sessions are untrusted
wire state. Zero reservations cannot create uncharged result carriers.

The existing 262,144-unit provision now covers both ground comparisons and
substitution work in the same session. Resource 4 is `checking_work`; the numeric
code and `comparison_steps` accessor remain unchanged. Each template visit and
resume, including the terminal empty resume, reserves one unit before work.
Variable-leaf ground comparisons charge their own transitions in that same
session. The original unfolding left coordinate owns these work refusals;
clause-scan reservation uses the clause coordinate. No reset or old-session reuse
is allowed between calls, even after Compared false. Stop after owned failure.

Every charged unit permits at most 96 cumulative Gamma pairs. A bulk reservation
uses four result/session pairs. The `T+1` reservation covers its result, the
`3T-1` index pairs, and bounded invocation context. Template memo insertion uses
at most 78 pairs; frame/session/result carriers must fit the remaining 18 pairs
of that transition. Ground comparison charges separately. The fixed 128-pair
allowance is once per request, not once per unfolding. The combined upper bound
remains `7,864,346 + 262,144*96 + 128 = 33,030,298` pairs, below the selected
40,265,318-pair arena, including unreachable allocations.

The implementation's clause walk is scalar-only; a case-mismatch rejection adds
four pairs to its four-pair reservation. Index setup uses exactly `3T+7` pairs:
four reservation carriers, `3T-1` index pairs, and four context pairs. A template
transition uses four reservation carriers followed by at most 78 memo pairs,
four pending-frame pairs, or four final-result pairs; those branches are
exclusive. Thus its maximum is 82 pairs. Variable ground-comparison allocations
belong to the ground transitions; its successful local insertion belongs to
the suspended template visit. Bindings and all other projections allocate none.

These are adjustable implementation provisions, not language or calculus laws.
If the real certificate needs more, change the work/storage profile together
and rerun boundary controls; an outer evaluator trap is never a checker result.
The full Beta certificate has not yet established that these provisions fit it.
[Proof checking](CHECKING.md) joins its indexing, rule coordination, and
final-root enforcement to this same cumulative accounting. Its generic Checked
result does not establish the full Beta artifact's authority or acceptance.
