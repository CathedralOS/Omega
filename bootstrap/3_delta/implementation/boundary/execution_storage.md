# Canonical compiler execution storage

This source-level audit covers the **Gamma program implementing Delta**, not
the Gamma programs it emits. It narrows the remaining
[resource obligation](README.md#resource-ownership-in-the-selected-producer);
it is not a checked refinement certificate or a cumulative pair-allocation bound.

The subject is the canonical [`delta_compiler.gamma`](../../delta_compiler.gamma)
prefix plus the ordered [`implementation.gamma.sources`](../implementation.gamma.sources)
closure: 155,440 bytes, SHA-256
`65e23e66c57885382a90c5b910a9064d8c829d62ffd1f028091d22e32a8eca84`.
It executes under the exact source/tape and provisions in the
[Gamma evaluator profile](../../../2_gamma/EVALUATOR_PROFILE.md).
Changes to either executable subject require rechecking the corresponding
argument; a new source manifest digest alone does not preserve this evidence.

Fixed runtime emission branches once between counting and publication. Balanced
addition sequences bounded byte sums in publication order without per-chunk
local bindings or linear syntax depth. The three bodies have one local each and
heights 12, 13, and 9. Their shared eight-byte writer has no locals or user calls.
The fixed-source maxima below need no larger provision.

Sequential binding groups replace 94 nested-let chains containing 309 bindings.
Expanding each group to nested lets recovers the preceding compiler's expression
trees exactly: initializer order, binding scope, tail positions, and user-call
edges are unchanged. The purpose is to expose the binding sequence and decision
body without a wrapper per local. This does not remove pair-layout conventions
or the explicit typing and lowering continuations. No preprocessor is retained;
the selected Gamma evaluator validates and executes the grouped syntax directly.

## Fixed source and call inventory

The compiler contains 366 Gamma definitions. Inspecting every body gives these
maxima, including bodies not reached from the canonical `main`:

| Fixed-source quantity | Maximum | Owning body |
| --- | ---: | --- |
| Formal parameters | 11 | `typing_match_bindings` |
| Nested authored lists, including binding groups | 17 | `resolve_collected_constructors` |
| All authored local bindings in one body | 12 | `publish_compiler_failure` |
| Formal parameters plus all body bindings | 16 | `lowering_match_arms` |
| Pending user calls within one body's expression syntax | 5 | `lowering_projection_definition`, `typing_match_body` |

These are compiler-source counts, independent of Delta input depth, width,
identifier length, or generated-helper count. The fixed 17-list maximum fits
Gamma's 255-list validation provision. Its validator visits bodies separately,
resets the lexical environment at each definition, and never executes callees.

For execution, a call's arguments are non-tail. An `if` condition and a `let`
initializer are non-tail; their branches/body inherit the containing tail
position. Other primitive operands are non-tail. A user call temporarily owns
one context while evaluating its arguments, even when the eventual call is tail.
The evaluator releases that tail context before reusing the current activation.

Weight each caller-to-callee edge by the pending enclosing user calls plus one
for a non-tail callee activation. Include each call's temporary argument context
as a local peak. The complete inventory has 65 recursive components. Only four
call sites on recursive cycles have positive weight: two in `capture_sort` and
one in each of `name_children_replace` and `emit_decimal`.

| Non-tail recursive owner | Decreasing quantity | Conservative additional contexts |
| --- | --- | ---: |
| [`capture_sort`](../normalization/capture/sets.gamma) | For `count >= 2`, both children are at most `ceil(count / 2)` and run sequentially. Any positive signed 64-bit count reaches the base case in at most 63 halvings. | 63 |
| [`name_children_replace`](../checking/names.gamma) | Each step consumes one distinct sibling edge. Insertions use admitted identifier bytes: 26 uppercase, 26 lowercase, ten digits, and underscore. New edges prepend only after absence; replacement preserves identity/order. | 63 |
| [`emit_decimal`](../emission/text.gamma) | Recursive calls divide a value at least ten by ten. A positive signed 64-bit value has at most 19 decimal digits. | 18 |

The sort allowance does not depend on a capture-allocation estimate. Values
below two do not recurse. Trie descent across name bytes and ancestor rebuilding
are tail-driven; name length does not multiply the sibling-rebuild allowance.
The remaining parser, grammar, typing, lowering, normalization, capture, emission,
list, and cursor cycles have only zero-weight tail edges.

Remove those four bounded edges, collapse zero-weight recursive components,
and propagate maximum weighted demands from local peaks through the resulting
acyclic graph. The previous 18-context fixed overhead from `main` remains a
conservative bound: the fixed-emitter rewrite removes counted-writer calls and
replaces the recursive unpacking writer with one call containing only primitive
writes and additions, not additional nested user calls. Reserving 64 for
that overhead and adding all three recursion allowances, even though their
deepest paths do not coexist, gives:

```text
live call contexts <= 64 + 63 + 63 + 18 = 208 < 256
live function frames <= one main frame + live contexts = 209
```

This inventory was checked with disposable source inspection and direct review
of the recursive routines and evaluator. The method above, not a retained host
analyzer or its output, defines the audit. It does not execute Delta input,
manufacture a certificate, or grant a new checker rule.

## Lexical and temporary storage

In [`gamma_evaluator.beta`](../../../2_gamma/gamma_evaluator.beta),
`enter_function` installs parameters above the caller's environment;
`bind_let_initializer` adds a binding only after its initializer returns.
Ordinary scope/function completion restores the saved environment count, and
`enter_tail_restart` resets it to the current activation's base. An activation
therefore retains at most its parameters plus all its body's local bindings,
including every binding in a grouped `let`:

```text
live lexical rows <= 209 * 16 = 3,344 < 131,072
```

Every expression-list level retains at most eleven temporary-value entries:
an ordinary call holds at most its eleven arguments; primitive/`let` handling
uses at most three entries for saved operands, operator, environment count, or
binder coordinates. Validation's `let` handling also uses three. Nested child
evaluation is charged to the next expression level, not hidden in that count.
`prepare_call` records the argument base; parameter installation resets the
value pointer to it, ordinary calls restore it on return, and tail replacement
does not retain previous argument blocks. Allow one extra result entry per
activation for `enter_function`'s return handling:

```text
temporary entries <= 209 * (17 * 11 + 1) = 39,292 < 524,288
```

The validator executes no Gamma bodies, so its one-body requirements are smaller
than these execution bounds. Gamma's existing hidden-Alpha-stack containment
argument continues to apply; no native stack or memory partition is changed.

## Remaining obligation

The canonical compiler does not need more call contexts, lexical rows, or
temporary-value storage for source depth or width. Explicit worklists and
immutable plans still allocate **pairs cumulatively**; returning from a call or
dropping a worklist does not reclaim them. Their whole-producer bound, including
checking, lowering, generic capture-batch merges, and serialization, remains
open. This audit neither supplies a DCOUT heap refusal nor converts an outer
Gamma failure into one.

Generated Delta applications are different programs. Their recursion and live
storage can still exhaust the selected evaluator or diverge. The compiler's
fixed-source bounds above must not be applied to those executions, the Epsilon
evaluator, or an arbitrary Gamma source.
