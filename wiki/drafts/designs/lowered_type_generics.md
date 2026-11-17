# Type and value parameters on the lowered form

A generic declaration must be valid for everything its requirements admit, not
only the applications currently used. This note records the representation
constraints for implementing that rule on the lowered form. It is not a second
checker, a fixed list of supported types or a report of current coverage.

[Generics](../../spec/language/generics.md),
[validation subjects](../../spec/build/validation_subjects.md) and
[the lowered-form design](check_on_lowered_form.md#the-gate) govern acceptance.
The existing checker and generic tasks in [TASKS.md](../../../TASKS.md) own
implementation. Delete this note once those owners carry its useful constraints.

## What the specification asks

Check a generic subject declaration under its complete authored requirements,
including when no caller specializes it. A dependency that is not demanded does
not become a validation subject just because it is bundled.

A type equation establishes which operations and values are available in its
scope. It is not permission to choose type identity using arbitrary proof search.
Contracts retain ordinary guarded formation, carrier, ownership, loan and
progress requirements. An abstract callable is checked through its requirement,
not today's selected implementation.

Specialization retains exact binder-to-argument correspondence, selected
operations and evidence. A consumer's smaller proof-discovery budget must not
force rediscovery of an already established derivation. Publication and separate
consumption additionally follow [producer admission](../../spec/terminal-psi/product.md#publication-and-separate-consumption),
not an unconditional receiver-side replay rule.

## Three kinds of parameter

| Parameter | Required treatment |
| --- | --- |
| Value, such as `const N: u64` | An exact symbolic value with its declared carrier and requirements. Specialization substitutes the selected value, not its display spelling. |
| Opaque type `T` | A rigid binder identity with only its declared structural properties and selected operation contracts. No assumed integer width, layout or comparison. |
| Explicit finite family, such as `T == u32 || T == u64` | Check every admitted alternative, used or not. Static equations refine operations and guarded contracts in their scope. |

A runtime-capable value binder is not automatically a request for a separate
specialization. Follow its existing source contract and ordinary argument
semantics. Value-dependent array extents and generic data must preserve their
binder relationships rather than being reduced to whichever literal one caller
happens to supply.

### Opaque types

Moving, storing, returning and lending an opaque `T` preserve its exact type and
custody. Copying needs the declared copy property. Cleanup needs the actual
permitted disposition, and a loan needs its backing and lifetime relationship.
An open application such as `Pair<T>` retains its selected declaration and
argument tuple; it does not become a record identified by field spellings.

Proof rules state which properties they require of their subjects. A rule
requiring integer width cannot act on an unconstrained rigid type. Boolean
reasoning, contract citation or identity transport may apply only under their
actual typed premises. A concrete stand-in must not leak extra operations,
ranges, zero inhabitants or layout facts into the generic proof.

### Finite families

[Static type equality](../../spec/language/generics.md#static-type-equality)
permits checking an explicitly finite family once per declared alternative,
independently of consumer demand. It does not require an additional check-only
branch instruction. An implementation may share the common work while retaining
coverage of every alternative.

This does not generalize to checking a symbolic integer family by enumerating
a few widths. A body over that family needs the properties common to all
admitted carriers. Unsupported symbolic reasoning refuses explicitly.

## What a carried certificate must preserve

Substitute exact types, values, lifetimes and selected operation evidence into
the derivation's subjects and premises. Check that the resulting derivation
establishes the receiving obligation. Static branch elimination retains the
correspondence between the selected alternative and its checked body. Removing
a dead arm cannot erase an obligation of a surviving arm.

This local transformation is different from admitting an artifact produced
elsewhere. The receiving host's trust in an exact artifact/producer permits
reuse under the product contract; simply possessing a certificate or finding a
matching identity grants no such trust. Neither route may use an unrelated
same-spelled binder or an implementation's private guarantees as a substitute.

## The alternative to reject

Checking only demanded specializations can miss an invalid unused alternative
or a generic subject declaration with no calls. It can also repeat proof search
for every consumer rather than preserve the declaration's established evidence.
Those are not acceptable shortcuts to supporting the generic source contract.

## Recommendation and order

Keep value parameters symbolic at their actual carriers, opaque types rigid
under their declared properties, and finite-family checks complete. Reuse the
ordinary lowered operations and proof judgments instead of another generic
source interpreter. Do not prescribe a duplicate representation merely to keep
an old experiment running.

A focused end-to-end witness exercises one declared family and its selected
applications, with an invalid undemanded alternative or weakened requirement as
its rejection control. Preserve calls, returned values, contained loans and
publication where the customer uses them. An earlier unsupported error does not
establish the intended rejection.

Follow current task priority. Historical experiment steps, stand-in encodings,
fixture pass counts and superseded proposal alternatives are in Git, not current
implementation instructions. No timing or performance result is asserted here.
