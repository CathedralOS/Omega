# Canonical compiler execution storage

This source-level audit covers the **Gamma program implementing Delta**, not
the Gamma programs it emits. It narrows the remaining
[resource obligation](README.md#resource-ownership-in-the-selected-producer);
it is not a checked refinement certificate or a cumulative pair-allocation bound.

The subject is the canonical [`delta_compiler.gamma`](../../delta_compiler.gamma)
prefix plus the ordered [`implementation.gamma.sources`](../implementation.gamma.sources)
closure: 147,840 bytes, SHA-256
`fbcb9e17b7ce0c75849136086bc5a4b6df4264054be72b5aae6d70325f9d0929`.
It executes under the exact source/tape and provisions in the
[Gamma evaluator profile](../../../2_gamma/EVALUATOR_PROFILE.md).
Changes to either executable subject require rechecking the corresponding
argument; a new source manifest digest alone does not preserve this evidence.

Runtime emission is support-member custody, not fixed text encoded in the
emitter. [`support.gamma`](../emission/support.gamma) validates the sealed
input's trailing 2,998-byte support section with one tail-recursive wrapping
byte-sum pass per member before any Delta-source phase, then copies each
member verbatim during publication through a tail-recursive `write`/`read`
walk while count mode advances by the same bound extent. The three support
loops carry at most three locals each and no non-tail user calls; the
fixed-source maxima below need no larger provision.

Sequential binding groups replace 94 nested-let chains containing 309 bindings.
Expanding each group to nested lets recovers the preceding compiler's expression
trees exactly: initializer order, binding scope, tail positions, and user-call
edges are unchanged. The purpose is to expose the binding sequence and decision
body without a wrapper per local. This does not remove pair-layout conventions
or the explicit typing and lowering continuations. No preprocessor is retained;
the selected Gamma evaluator validates and executes the grouped syntax directly.

## Fixed source and call inventory

The compiler contains 377 Gamma definitions. Inspecting every body gives these
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
as a local peak. The complete inventory has 67 recursive components. Only three
call sites on recursive cycles have positive weight: two in `capture_sort` and
one in `emit_decimal`. The capture merge passes (`capture_merge_all`,
`capture_merge_pass`) and the support-member loops (`support_sum_region`,
`support_check_members`, `emit_support_bytes`) are zero-weight tail edges.

| Non-tail recursive owner | Decreasing quantity | Conservative additional contexts |
| --- | --- | ---: |
| [`capture_sort`](../normalization/capture/sets.gamma) | For `count >= 2`, both children are at most `ceil(count / 2)` and run sequentially. Any positive signed 64-bit count reaches the base case in at most 63 halvings. | 63 |
| [`emit_decimal`](../emission/text.gamma) | Recursive calls divide a value at least ten by ten. A positive signed 64-bit value has at most 19 decimal digits. | 18 |

The sort allowance does not depend on a capture-allocation estimate. Values
below two do not recurse. Trie descent across name bytes, sibling-row scans,
and ancestor rebuilding are tail-driven; name length does not multiply the
sibling-rebuild allowance. The remaining parser, grammar, typing, lowering,
normalization, capture, emission,
list, and cursor cycles have only zero-weight tail edges.

Remove those three bounded edges, collapse zero-weight recursive components,
and propagate maximum weighted demands from local peaks through the resulting
acyclic graph. The previous 18-context fixed overhead from `main` remains a
conservative bound: the support-member rewrite removes the counted fixed-text
writers and adds tail-recursive sum and copy loops whose calls sit in tail
position, not additional nested user calls. Reserving 64 for
that overhead and adding both recursion allowances, even though their
deepest paths do not coexist, gives:

```text
live call contexts <= 64 + 63 + 18 = 145 < 256
live function frames <= one main frame + live contexts = 146
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
live lexical rows <= 146 * 16 = 2,336 < 131,072
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
temporary entries <= 146 * (17 * 11 + 1) = 27,448 < 524,288
```

The validator executes no Gamma bodies, so its one-body requirements are smaller
than these execution bounds. Gamma's existing hidden-Alpha-stack containment
argument continues to apply; no native stack or memory partition is changed.

## Remaining obligation

The canonical compiler does not need more call contexts, lexical rows, or
temporary-value storage for source depth or width. Explicit worklists and
immutable plans still allocate **pairs cumulatively**; returning from a call or
dropping a worklist does not reclaim them. Every producer phase now carries a
per-occurrence charge derived in the same style:

- [Serialization's own traversal](../emission/README.md#publication-traversal-pairs)
  is bounded below the pair arena by the admitted payload extent.
- The [normalizer's frames and rebuilt nodes](../normalization/README.md#traversal-and-rebuild-pairs)
  are charged per plan-node occurrence with capture merges bounded per
  collection by `(2T + 2) * ceil(log2(k + 1)) + 2 * (k + 1) + 1`.
- The [checking audit](../checking/README.md#traversal-and-rebuild-pairs)
  charges census metadata, resolution rows, typing continuations, and
  environment binds per source occurrence — at most `39*S + 36` site pairs —
  and the [lowering audit](../lowering/README.md#traversal-and-rebuild-pairs)
  charges continuation frames, plan construction, and the produced `G` at
  `<= 200*S + 91` with `G <= 40*S + 15`.
- The shared [name-trie and cursor audit](../checking/names/README.md#pair-accounting)
  charges lookups, descents, fresh suffixes, and immutable rebuilds; departed
  ancestor levels amortize to descended name bytes through the cursor zipper
  identity, and each rebuilt branch level now prepends one fresh row at five
  pairs rather than copying up to 63 sibling rows.

What remains open is whether these per-occurrence products stay below the
40,265,318-pair arena for every admitted shape: the coarse checking,
lowering, and shared-name envelope is at most `295*S + 34*N + 149` pairs
— before the normalizer's own `45*G + 7*F + 1` term — no longer dominated by
sibling-row copies in name rebuilds, though it can still exceed the arena at
maximum source extents.
The capture merge term is now logarithmic in batch count per collection; its
aggregate `sum(T)` over nested helper captures carries the same open status.
This audit
neither supplies a DCOUT heap refusal nor converts an outer Gamma failure
into one.

Generated Delta applications are different programs. Their recursion and live
storage can still exhaust the selected evaluator or diverge. The compiler's
fixed-source bounds above must not be applied to those executions, the Epsilon
evaluator, or an arbitrary Gamma source.
