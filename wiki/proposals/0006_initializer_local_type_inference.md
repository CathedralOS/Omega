# 0006: Initializer-based local type inference

Status: draft proposal, not accepted syntax or an implementation prerequisite.
The owner requested this proposal separately from the accepted explicit mutable
range rule. All annotation-free examples below are proposed, not current guide
examples. Delete this proposal on acceptance after moving its rules to their
specification owners, or on rejection.

Affected owners: [expressions](../spec/language/expressions.md),
[numeric landing](../spec/language/numeric_values.md#literal-landing-and-destinations),
[quoted bytes](../spec/language/data_and_literals.md#quoted-bytes),
[generic inference](../spec/language/generics.md), and guide chapters 1 and 5.

## Problem and scope

Local declarations currently spell `let [mut] name: Type [= value];`; the
product parser requires the type. Generic argument inference and a machine's
substituted result type do not currently authorize omitting that annotation.
Often the initializer already determines the complete type, so the local
annotation repeats information without resolving any choice.

Propose optional local annotations when an initializer determines one complete
type. Apply uniformly in machine and state bodies. Do not extend this to fields,
parameters, machine result declarations, uninitialized locals, named constants,
or top-level mathematical definitions. Those surfaces retain their contracts.

## Proposed rule

```omega
let message = "hello";        // &[u8], not implicitly Utf8-qualified.
let count = 5u64;             // u64; the suffix chooses its carrier.
let middle = &mut view[2..4];  // &mut [u8], given view: &mut [u8].
let mut next = 0u64;          // Mutable local, fixed u64 type.
```

For a call, infer the selected application's declared, substituted result. For
example, if an ordinary machine `make_packet` returns `Packet`, then
`let packet = make_packet();` binds `Packet`. The spelling `Utf8::From("hello")`
would follow the same rule if that machine exists; this proposal introduces
neither that API nor a text conversion.

Type the initializer using existing expression, operator, generic-selection
and admission rules, without an expected local type. It must determine one
complete type, including generic arguments, qualifications and reference
access. Bind that type once; later uses and assignments neither select nor
change it. An ambiguous or underdetermined initializer requests an annotation
or explicit arguments rather than choosing a candidate or searching later code.

Preserve the initializer's type and its ordinary custody: no implicit cast,
domain erasure, ownership duplication, lifetime extension or new borrow is
introduced to make inference succeed. `mut` still controls whether the local
may be reassigned, independently of whether its value is a mutable reference.
Flow-derived propositions remain facts, not newly authored qualification types.
Explicitly typed declarations retain their existing landing and admission rules.

This is not a new closed list of inferable initializer shapes. Calls, literals,
borrows, constructions and compound expressions all use the same criterion:
does ordinary typing determine one complete type without the local annotation?

## Deliberate limits

```omega
let count = 5;                // Proposed rejection: no selected carrier.
let ratio = 7 / 2;            // Proposed rejection: still anonymous numeric.
let storage;                  // Rejection: neither type nor initializer.
let mutable: &mut [u8] = view[2..4]; // Rejection under the accepted range rule.
```

No default integer width or float format is introduced. Unsuffixed numbers can
still land inside an initializer when an existing typed operand, cast, selected
parameter or construction field supplies the destination. Thus a call returning
`u64` can determine the local's type even if one of its arguments is `5`.
An entirely anonymous expression does not become a stored rational or acquire
a carrier from its later uses. Use a suffix or explicit local annotation.

The accepted [borrowed range access](../spec/language/expressions.md#borrowed-range-access)
rule remains independent: bare core ranges produce shared views when received
as references; `&mut` explicitly creates a mutable view. A local annotation
cannot request that borrow implicitly. This proposal does not decide a new
standalone value type for an unborrowed range place; use an explicit borrow or
the existing typed shared-view destination when a reference is intended.

## Alternatives and trade-offs

- Keep every local type explicit. No new inference surface, but repetitive
  annotations remain even when the initializer names the type exactly.
- Infer from later uses or introduce numeric defaults. More short declarations
  work, but moving a use or changing surrounding code can change the local's
  representation or inferred meaning. Reject for this bounded proposal.
- Infer borrow intent from the destination. Not part of type inference and
  contrary to the accepted explicit mutable-range decision; do not reopen it.

Initializer inference makes code shorter but may require following a machine's
signature to learn a local's type. Explicit annotations remain available for
readability or to enforce a particular receiving contract. This is an ergonomics
proposal, not additional expressive power.

## Ratification and acceptance

The unresolved owner choice is whether to accept this initializer-only scope,
including refusal of standalone anonymous numbers. No execution-board work is
blocked on it, and no implementation mandate exists before acceptance.

If accepted, demonstrate equivalent checking and native/Terminal behavior for
annotated and inferred byte literals, suffixed numbers, resolved call results,
generic constructions, qualified values and explicit borrows. Include a call
with anonymous numeric arguments landing at its declared parameters, a mutable
local whose type stays fixed, and a reference whose original backing loan stays
live. Reject unresolved generic results, standalone anonymous numbers,
uninitialized untyped locals, changed-type assignments, invalid qualification
weakening, implicit mutable range formation and escaped local backing.

Normalize inferred locals to the same typed representation as annotated locals
before freezing. Downstream checking and lowering must not gain an inferred-local
mode or a parallel representation. Migrate a small real machine as evidence;
do not mechanically remove annotations across the repository.
