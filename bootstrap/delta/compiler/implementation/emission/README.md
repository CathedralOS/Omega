# Expanded Gamma serialization

Start at [program.gamma](program.gamma). It serializes the counted, ordered
definitions in a completed [Gamma plan](../representation/README.md), emits
their separators, and selects the existing profile adapter at program end.
[declarations.gamma](declarations.gamma) writes each definition's name,
parameters, and body. [expressions.gamma](expressions.gamma) prints the plan's
ordinary Gamma calls and lets; [atoms.gamma](atoms.gamma) prints its atoms.

The pipeline finishes the complete frontend, selected-profile schema check,
and [lowering](../lowering/README.md) of every authored body before writing the first
receipt byte. Emission does not classify Delta expressions, resolve locals,
expand constructors or patterns, or choose checked-arithmetic guards. Those
decisions are already explicit Gamma structure.

## Atom and expression custody

Source atoms retain exact admitted source spans through retained-node
accessors. Serialization may copy those bytes without interpreting their
Delta expression structure. Source binding references reuse the established
binding atom; generated names retain their marker and coordinate. Function
atoms receive the existing injective naming treatment, and fixed words and
integer atoms have dedicated textual encodings.

Expression serialization uses explicit continuations for pending arguments,
let bodies, and closes. This traversal is over Gamma plan nodes, not retained
Delta expression children. Counts govern list projections; continuation depth
is neither source expression depth nor generated Gamma expression-list height.
The plan's height summaries do not change serialization or silently select a
different representation.

## Fixed publication text

[text.gamma](text.gamma) owns textual primitives. [bytes.gamma](bytes.gamma)
retains the existing fixed byte-runtime and application-adapter text. The
pipeline's existing runtime selection remains separate from generic expression
serialization. These helpers, definition order, whitespace, hygienic spellings,
and the final publication byte retain the established receipt format.

No source-dependent lowering template remains in this directory. Calls,
constructors, bindings, arithmetic, and matches belong under `lowering/`;
durable Gamma plan nodes belong under `representation/`.

## Validation and remaining boundaries

The staged gate compares exact receipts, executes generated programs, and
exercises nested expressions and wide payloads. Exact Epsilon checking and
execution receipts remain separate full-customer reconstruction gates. A
changed receipt requires explanation, not a relaxed expectation.

The [lowering-plan gate](../../../../../tests/delta/lowering-plan/README.md)
checks authored expectations against the plan's expanded body heights before
serialization. Recording those heights does not normalize or lift over-height
bodies. The [selected Gamma profile](../../../../gamma/EVALUATOR_PROFILE.md#exact-capacities)
still admits at most 255 nested expression lists per generated function body.
Generated wrappers can exceed that bound within Delta's admitted 1,024-level
expression profile.

Plan and continuation pairs consume the selected evaluator's finite immutable
arena. Stack-safe compiler traversal, complete-before-write planning, and exact
receipt preservation do not close compiler-owned resource/internal outcomes,
generated-profile admission, or the full Delta bootstrap edge.
