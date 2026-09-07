# Candidate: Named proof-formula syntax

> **Needs porting.** This document has not been consolidated or vetted for the
> current documentation structure. See the [migration index](../README.md).

Status: unproven ergonomic augmentation, not accepted syntax or an implementation
prerequisite. The governing direction is [Mathematical Proofs](mathematical_proofs.md).

## The question

Machines establish contracts; traits and named conformances bundle mathematical
operations, witnesses, and checked laws. General logic in contracts must express
quantification, existence, and arbitrary mathematical functions and predicates.
Neither candidate below is required merely to provide those capabilities.

The candidates are alternative ways to give a reusable logical expression a name:

```text
proposition equivalent(left: Program, right: Program) = <logical expression>;

machine equivalent(left: Program, right: Program) -> Prop
{
    <logical expression>
}
```

These are illustrative, not compilable replacement examples. Either would name
a statement, not establish it, compute a Boolean decision, or supply evidence of
every application. A theorem about equivalence would still need a definition or
assumptions sufficient to prove its contract.

## What must earn inclusion

Compare a substantial proof using ordinary contracts and a named trait bundle
against the same proof with each candidate. Demonstrate a concrete reduction in
repetition, substitution mistakes, or evidence bookkeeping. Include higher-order
relations and nonconstructive existence, not only a short alias for an inequality.
Measure the added declaration, elaboration, checking, and diagnostic machinery.
LLM authorship does not make witness plumbing free, but it also does not justify
a syntax category without an actual improvement.

The previous surface attached an `evidence Interface;` clause to a nominal
`proposition` and projected hidden witnesses through named `requires`/`ensures`
lanes, with separate `;` call inputs and selected proof outputs. That is not the
baseline design. Moving the same mandatory wrapper under `-> Prop` or a renamed
proof type would not simplify it. Any proposal to restore such packaging must
beat ordinary named trait/conformance bundles on a concrete proof.

## Independent requirements

General predicate abstraction, logical binders, proof-only noncomputable values,
and explicitly tracked selectable axioms remain required independently of this
candidate. The precise source forms and underlying universe/equality rules need
design and validation. Neither candidate establishes Lean-level expressivity by
itself, and removing either does not establish that the current compiler already
has a complete replacement.

This is not an execution-board feature request. Revisit only when a worked proof
demonstrates the ergonomic gap; migration of the current implementation is tracked
separately in `PROOF-CONTRACT-MIGRATION` in [TASKS.md](../../../TASKS.md).
