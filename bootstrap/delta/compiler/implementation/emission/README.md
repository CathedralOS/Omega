# Expanded Gamma serialization

Start at [program.gamma](program.gamma). It serializes the counted, ordered
definitions in a completed [Gamma plan](../representation/README.md), emits
their separators, and selects the existing profile adapter at program end.
[declarations.gamma](declarations.gamma) writes each definition's name,
parameters, and body. [expressions.gamma](expressions.gamma) prints the plan's
ordinary Gamma calls and lets; [atoms.gamma](atoms.gamma) prints its atoms.

The pipeline finishes the complete frontend, selected-profile schema check,
and [lowering](../lowering/README.md) and
[normalization](../normalization/README.md) of every authored body before
writing the first receipt byte. A count-only serialization then measures the
complete payload using cached expression extents. Emission does not classify Delta expressions,
resolve locals,
expand constructors or patterns, or choose checked-arithmetic guards. Those
decisions are already explicit Gamma structure.

## Atom and expression custody

Source atoms retain exact admitted source spans through retained-node
accessors. Serialization may copy those bytes without interpreting their
Delta expression structure. Source binding references reuse the established
binding atom; generated names retain their marker and coordinate. Function
atoms receive the existing injective naming treatment, and fixed words and
integer atoms have dedicated textual encodings.

Expression serialization uses explicit continuations for pending sibling
arguments and let initializers. Final arguments and let bodies tail-enter with
a scalar pending-close count; unary projection chains allocate no continuation
per projection. This traversal is over Gamma plan nodes, not retained
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

Emission entrypoints receive a count/publication flag and running byte count.
The expression entrance chooses cached counting or publication once; recursive
publication does not carry or recheck that flag. Byte loops only write, and
their callers advance the count once for each completely written extent.
[extents.gamma](extents.gamma) supplies the canonical expression-size summary
retained by each Gamma node constructor. Its count-only helpers share atom
spellings and call/let prefixes with publication, adding cached child extents
instead of unfolding shared children. Every rebuilt node recomputes its summary;
reused immutable nodes keep theirs. Count mode adds span and packed-text lengths
without reading or writing their bytes; it does not build a byte rope.
The coordinator includes the entry-owned final LF in both totals. If the full
count exceeds 16,777,212, it returns DCOUT `Incomplete` resource 12 in payload
coordinate space 2, at byte 16,777,212, with the limit and exact complete count.
No application marker or partial program precedes that refusal. Admitted
publication checks its returned count against the preflight count; disagreement
remains an internal invariant failure, not a resource refusal.

Preflight accumulation and node summaries use checked nonnegative addition, never
saturation. An arithmetic contradiction fails before any wrapped count can be
admitted or published; it does not invent a resource-12 requested witness.
Canonical internal-failure publication remains separate unfinished work.
The cache costs one additional immutable pair per Gamma node. This trades
bounded retained metadata for repeated traversal of shared projection tails;
full Epsilon recompilation remains a storage regression check.

No source-dependent lowering template remains in this directory. Calls,
constructors, bindings, arithmetic, and matches belong under `lowering/`;
durable Gamma plan nodes belong under `representation/`.

## Validation and remaining boundaries

The staged gate compares exact receipts, executes generated programs, and
exercises nested expressions and wide payloads. Exact Epsilon checking and
execution receipts remain separate full-customer reconstruction gates. A
changed receipt requires explanation, not a relaxed expectation.

The [lowering-plan gate](../../../../../tests/delta/lowering-plan/README.md)
checks authored expectations against the pre-normalization plan's expanded
body heights. The separate normalizer handles the
[selected Gamma profile](../../../../gamma/EVALUATOR_PROFILE.md#exact-capacities)
limit of 255 nested expression lists per generated function body. Serialization
does not make extraction or capture decisions and does not alter those budgets.

Plan and continuation pairs consume the selected evaluator's finite immutable
arena. Stack-safe compiler traversal, complete-before-write planning, and exact
receipt preservation do not close compiler-owned resource/internal outcomes,
generated-profile admission, or the full Delta bootstrap edge.
