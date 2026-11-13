# Unbounded integer computation and exact mathematical division

Ratified 2026-11-11. Integer algorithms can use `i` and `u` without choosing a
finite width, while `embed` enters exact mathematical arithmetic. Mathematical
division returns a rational; integer quotient remains explicitly available.
The rules live in [numeric values](../../spec/language/numeric_values.md#unbounded-computational-integers)
and the [proof correspondence](../../spec/proofs/contracts.md#total-arithmetic).

Named decisions: `unbounded-computational-integers`,
`mathematical-exact-division`, and `explicit-landing-proof-int-peer`.
This document records the remaining migration and acceptance for
**OPERATOR-MACHINE-SUPPLY** in [TASKS.md](../../../TASKS.md). It does not claim
compiler support, fresh fixture results or unrestricted runtime big integers.
Examples below specify required behavior and have not been executed.

## Arithmetic choices

`i` contains every signed integer; `u` contains every nonnegative integer.
Their `/` computes integer quotient and `%` its corresponding remainder.
These are concrete types with exact integer operations, not finite-width
variables awaiting inference. Same-carrier arithmetic and explicit conversions
keep the computational choice stable. An algorithm that should work over a
selected generic carrier uses anonymous literals rather than choosing `7u`.

The three transitions serve different purposes:

```text
(7i32 as i) / 2i           = 3i     remove the finite range restriction
embed(7i32) / 2            = 7/2    request exact mathematical division
embed(7i / 2i)             = 3      project the quotient already computed
```

No mandatory intermediate `i`/`u` step is introduced before embedding. Unsigned
embedding also returns mathematical `Int`, with nonnegativity retained as a
fact, so subsequent mathematical subtraction can produce a negative integer.
Mathematical `/` accepts integer and rational operands through checked exact
inclusions, and anonymous operands as exact rationals. It always returns `Rat`.

```text
embed(7i) / 0.5            : Rat, value 14
embed(6i) / 2             : Rat, value 3
embed(7i) / (1 / 2 * 4)   : Rat, value 7/2
```

Typed rationals never become anonymous during simplification. Their exact
narrowing for an integer-only use must be explicit, with integrality established.
This decision does not approve automatic promotion for every mathematical
operator, function argument, record field or branch join.

## Landing and conversion

Computational `as i`/`as u` conversions preserve numerical values and check
nonnegativity or finite destination bounds where required. They do not execute
quotient, truncation or policy overflow behavior. The established
[whole-expression acknowledgment](explicit_integer_landing.md) applies when an
anonymous calculation has fractional intermediates and lands in a computational
integer, including unbounded `i` and `u`.

```text
(5 / 2) * 2 at an implicit i/u destination       reject: acknowledgment required
((5 / 2) * 2) as i                             accept 5i
(5i / 2i) * 2i                                accept 4i
(5 / 2) as i                                  reject: nonintegral final value
(5 / 2) * 2 at mathematical Int/Nat            accept 5 without acknowledgment
0u - 1u                                       reject: negative u result
(0u as i) - (1u as i)                          accept -1i
7i + 2u                                       reject without explicit conversion
```

Arithmetic facts can discharge conversion conditions but cannot silently change
an already typed source value's carrier. Computational comparisons may compare
exact integer values across widths and signedness; arithmetic still needs an
explicit common typed carrier. Computational/mathematical crossings use `embed`,
including comparisons. No general mathematical-to-computational conversion is
approved by this settlement.

## Proof migration

The computational division bridge uses named integer quotient:

```text
embed(a / b) == integer::quotient(embed(a), embed(b))
```

It does not equate that result with rational `embed(a) / embed(b)`. Retain the
source primitive's formation and representability obligations and apply its
selected bounded overflow policy to the integer denotation. For signed quotient,
`-7` by `2` gives `-3`, with remainder `-1`. Named quotient/remainder are checked
mathematical operations, not trusted facts inferred from helper names.

Existing `tests/omega/pass/proofs/proof_integer_quotient_remainder` supplies
the primary migration witness. Preserve the integer theorem's intent by selecting
named quotient, rather than weakening a bound or accepting a false rational
statement. In particular:

- The old assertion `embed(7i32) / (1 / 2 * 4) == 3` becomes false under the
  new division meaning. Rational division gives `7/2`; named quotient gives `3`.
- The bound `embed(value) / 2 <= 4` for `0 <= value <= 9` needs quotient;
  rational division is `9/2` at the upper endpoint.
- The existing large quotient of `2*A + 1` by `2` must select quotient to
  retain result `A`; exact division gives `A + 1/2`.
- `%` remains integer remainder. Its anonymous integral operand, even one
  with fractional history, needs no mathematical acknowledgment.

Folding, source checking, semantic evaluation, lowering and artifact receipt
must preserve selected operation and result type. Old quotient encodings cannot
silently become rational division. Retire/rebuild incompatible artifacts under
existing admission rules; no legacy-support campaign is introduced.

## Delivery checks

- Static constants, symbolic inputs and generic consumers use the same semantics.
  A literal-only evaluator shortcut does not implement mathematical division.
- `i`/`u` remain concrete carriers through fields, parameters, results, constants,
  joins and specialization; expected types do not reinterpret typed suffixes.
- Same-carrier arithmetic, exact cross-computational comparisons and explicit
  conversions cover signedness, negative-to-unsigned refusal, finite narrowing,
  unbounded values beyond 128 bits, and unsigned subtraction/negation obligations.
- Mathematical division by integer or fractional anonymous operands accepts
  when defined and always returns `Rat`, even for integral results.
- Integer-only uses reject implicit typed-rational narrowing. Mathematical
  landing uses final integrality; computational landing retains fractional-history
  acknowledgment, with existing fixed-carrier controls preserved.
- Zero divisors reject before cancellation, multiplication by zero or a cast can
  hide them. No conclusion or unfinished constraint justifies its own formation.
- Quotient/remainder sign controls and the source-policy proof bridge remain
  valid for Exact, Wrapping, Saturating and normal-return Trapping cases.
- Checked rational inclusions and source-operation correspondence survive
  publication without trusting a type name, internal payload or unchecked
  `IntPair` conversion. Source witnesses and their negative twins carry through
  Psi and native execution wherever an admitted executable realization exists.
- Unsupported runtime or word-width-dependent uses reject rather than choosing
  a hidden finite width, modulus, floating approximation or resource behavior.

## Separate decisions

This settlement does not choose runtime big-integer/rational storage, allocation,
copying, cleanup, failure behavior or ABI. Executable uses require an admitted
realization and resource/effect contract. An implementation with proved finite
input bounds may use finite storage without changing the source type's domain.

[Arbitrary finite widths and carrier-family constraints](../../spec/language/numeric_values.md#width-indexed-integers)
were separately ratified on 2026-11-13; their delivery is tracked under
`WIDTH-INDEXED-INTEGERS` and `INTEGER-CARRIER-FAMILY-CONSTRAINTS`.
Unbounded bitwise/shift operations, broad mathematical promotion, and the full
rational representation and conversion API remain separately reviewed work.
Unsupported combinations reject until their contracts are specified. The
rational requirement is an exact, source-visible, checked `Rat` interface, not
approval of either a canonical-record or quotient representation by implication.
