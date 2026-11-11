# Arbitrary-width integer carriers

Status: proposed, not accepted. No arbitrary-width implementation or source
spelling is ratified by this document. The immediate domain-carrier checking
defect is independent of this proposal.

Unbounded computational `i`/`u` were separately ratified on 2026-11-11 under
[numeric values](../spec/language/numeric_values.md#unbounded-computational-integers).
They have no finite numerical width. That decision does not approve arbitrary
finite widths, and choosing `i200` would restrict values rather than merely
choose a layout for `i`. Runtime realization remains separately required.

Affected owners: [numeric values](../spec/language/numeric_values.md),
[generics](../spec/language/generics.md),
[domains](../spec/language/domains.md),
[layout plans](../spec/layouts/plans.md), and
[calling plans](../spec/build/calling_plans.md).

## Problem and customer

Omega has a fixed set of bounded integer carriers, independently of the
unbounded computational carriers. A program needing arithmetic with another
finite width must use a larger bounded carrier with explicit range/masking work,
or an ordinary multi-limb library representation. The question is whether a
width-indexed integer carrier would make a concrete program clearer, safer, or
more efficient than those alternatives.

Potential customers include arithmetic with protocol-defined bit widths and
fixed-width multi-limb calculations. Merely storing a packed field is not enough:
an explicit layout or codec can already describe a stored width without adding
another arithmetic type. No customer requiring arbitrary-width language
integers was demonstrated in the discussion that opened this proposal.

This document also retains the distinct idea of reusable domains over every
integer carrier, or only signed or unsigned carriers. That idea can serve the
existing fixed widths without depending on arbitrary-width implementation.

## Proposed direction

Explore integer carriers parameterized by a positive, proof-static bit width
and signedness. Candidate surface forms include extending the existing `uN` and
`iN` spellings, such as `u13` or `i257`, or ordinary generic type applications
whose width is explicit. Neither spelling is selected here.

For width `Bits`, the candidate mathematical ranges are:

$$
0 \leq value < 2^{Bits}
$$

for unsigned integers, and

$$
-2^{Bits-1} \leq value < 2^{Bits-1}
$$

for signed integers. Thus a proposed `u1` has values zero and one; `i1` has
values minus one and zero. Whether these widths are admitted is an open
decision, and neither carrier would silently become `bool`.

"Arbitrary-width" means a width chosen by the program within an admitted
implementation/resource bound, not infinite compiler resources or a
runtime-changing inline representation. A runtime-sized integer remains a
different library representation. Proof `Int` and `Nat` are not new runtime
carriers merely because they support mathematical reasoning.

## Carrier families and domains

A type-generic library may need to express one of these restrictions:

- Any admitted integer carrier.
- Any admitted signed integer carrier.
- Any admitted unsigned integer carrier.
- An integer carrier with a width satisfying an explicit relationship.

Investigate ordinary generic constraints and compiler-authoritative numeric
metadata before adding a new keyword, ambient implementation search, or one
trait implementation per width. Data and code remain independent: a carrier
restriction identifies types; explicitly selected named implementations supply
operations and their checked laws.

Omega already has carrier-generic domain families. A reusable "small integer"
domain could be instantiated separately for `u8`, `u16`, or a proposed `u13`.
Those applications share a definition, not carrier identity. A signed-only
domain family would reject an unsigned carrier argument rather than change its
meaning or reinterpret the bits.

This is not permission for a domain declared over concrete `u16` to classify a
`u8` implicitly. Membership must be formed against the resolved applied carrier
before substituting a mathematical value into the predicate. Explicit exact
carrier conversion precedes membership in the other carrier's domain. Numeric
equality across widths does not establish type or domain identity.

The representation should retain exact resolved declaration/type identities and
generic substitutions across this boundary. Predicate expansion must not use a
same-spelled symbol or an erased numeric value as a substitute for checking
the subject against the domain's carrier. Type-qualified arithmetic policy and
other semantic obligations remain independent of representability.

One possible customer is a numeric library that publishes the same useful
classification for several widths without duplicating declarations. Before
adding integer-family constraint syntax, demonstrate that customer and show why
existing generic carriers and explicit operation requirements are insufficient.

## Arithmetic and proof

Preserve the existing Exact, Wrapping, Saturating, and Trapping distinctions at
each operation's declared width. Do not silently widen intermediates or repair
an invalid operand merely because the final result fits.

The proposal must account for arithmetic, division and remainder edge cases,
shift-count rules, comparisons, exact conversions, literal landing, extrema,
bit operations, and integer embedding. Exact cross-carrier integer comparison
keeps its existing mathematical-value meaning without implicit arithmetic
conversion. Arithmetic policy and signedness do not grant algebraic laws by
name; selected implementations still prove the laws a normalizer consumes.

Identity includes the integer carrier's signedness and normalized static width.
Equivalent closed width expressions must identify the same application under
the ordinary index rules; a matching width does not merge unrelated nominal
data. Proof formation and executable behavior must agree on every admitted
width and policy.

## Representation and targets

Define the mathematical carrier independently of its target realization.
Investigate storage size/alignment, padding bits and canonical values, explicit
layouts/codecs, extension and truncation, register versus multi-limb placement,
and foreign ABI boundaries. A target lacking a single matching instruction
cannot silently change the width's semantics.

Psi owns type formation, arithmetic meaning, and target-neutral evidence.
Omega owns realization and calling plans; backends own unavoidable ISA/ABI
detail. Do not encode host integer limits into source type identity or imply
that a foreign ABI accepts every new width. Existing layout stored-width
facilities are not an arbitrary-width arithmetic implementation.

## Alternatives

- Keep the fixed carrier set and use larger carriers with explicit checked
  range/masking operations. This is adequate when only a few boundaries need
  unusual widths.
- Use ordinary fixed-limb or dynamic large-integer library data. Compare its
  explicit representation and operation contracts with the proposed carrier;
  do not assume syntax alone justifies compiler ownership.
- Add carrier-family constraints and reusable domains for existing integers
  first, if a concrete customer needs them. This does not accept arbitrary
  widths or cross-carrier membership.
- Add a finite selection of motivated widths instead of an open width family.
  Measure whether those widths expose a general need or merely repeated
  implementation work.

Implicitly applying a concrete domain to another carrier is not an alternative
implementation of a domain family: it changes the subject type and leaves
predicate operation and authority meaning unresolved.

## Open decisions

1. Which actual program needs nonstandard-width arithmetic, rather than only
   packed storage, and why is an ordinary library carrier insufficient?
2. Which width range is admitted, including one-bit and zero-bit cases, and how
   are compile-time resource limits accounted for?
3. What is the width-indexed type surface, and which existing widths are aliases
   or canonical applications of that family?
4. How do generic constraints describe integer/signed/unsigned carrier families
   without attaching code implementations to data or adding implicit selection?
5. What are the storage, calling-plan, foreign-binding, and target realization
   contracts for widths outside native register lanes?

## Evidence before acceptance

Start with a runnable customer and the existing-library alternative. Compare
authored complexity, checked arithmetic and formation, emitted behavior,
layout/ABI boundaries, and measured compile/runtime costs. A spelling demo or
a matrix of widths is not a customer.

Any accepted subset needs an explicit owner decision, updated numeric/generic/
domain/layout contracts, and bounded end-to-end implementation acceptance.
Selected mechanisms need positive and rejecting source-to-native witnesses,
including carrier-exact domain membership, policy/overflow boundaries, and
representation or ABI lane changes. This unaccepted proposal is not an
execution-board prerequisite and authorizes no compiler or bootstrap campaign.
