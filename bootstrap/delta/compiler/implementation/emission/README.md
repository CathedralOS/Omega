# Retained-node Gamma emission

Start at [program.gamma](program.gamma). It walks checked declarations in
authored order, skips data declarations, emits function separators, and selects
the existing profile adapter at program end.
[declarations.gamma](declarations.gamma) emits each signature from retained
parameter nodes and enters [expressions.gamma](expressions.gamma) for its body.
The compiler pipeline calls this entrance only after grammar, global and local
resolution, complete body typing, and selected-profile schema validation.

Emission consumes those judgments. It does not repeat syntax, annotation,
arity, or type checking. Retained children supply structure; exact source spans
still supply authored names, admitted numeric spellings, and hygienic generated
name coordinates. Constructor metadata supplies nominal representation and
declaration tags. Output order and lowering rules preserve the existing Gamma
receipts rather than selecting a new runtime representation.

## Expression control

The expression dispatcher owns two mutually tail-calling operations:

```text
emission_visit(node, depth, frames, globals)
emission_resume(depth, frames, globals)
frame = (pair kind (pair payload previous))
```

Depth is the count of live emission continuations, not source expression depth
and not generated Gamma nesting. A function body starts with depth zero and
frames zero. Visiting a child with pending work pushes one frame and increases
depth by one. Completion resumes that stack; depth zero completes the body.
Counts govern every projection, never the provenance or numeric value of a
pair reference.

`emission_resume` pops exactly one frame before dispatch. Every handler receives
the decreased depth and previous stack, not the frame it is handling. The
handler must not pop again. It may push a new continuation for subsequent
children or resume the already-popped stack. This contract keeps nested matches,
arithmetic, constructors, and bindings on the same expression machine.

| Kind | Payload | Owner and remaining work |
| --- | --- | --- |
| 1 | `(pair remaining-count nodes)` | [calls.gamma](calls.gamma): emit remaining arguments and the call close |
| 2 | closing-delimiter count | [text.gamma](text.gamma): emit pending closes, then resume |
| 3 | body node | [bindings.gamma](bindings.gamma): separate initializer/body, visit body, then close the let |
| 4 | arithmetic node | [arithmetic.gamma](arithmetic.gamma): separate operands, open the right binding, and visit the right operand |
| 5 | arithmetic node | [arithmetic.gamma](arithmetic.gamma): emit the checked operation, overflow guard, and wrappers |
| 6 | match node | [matches.gamma](matches.gamma): unpack the completed subject and begin authored-order arms |
| 7 | `(pair remaining-count (pair arms match-start))` | [matches/arms.gamma](matches/arms.gamma): separate the completed arm from remaining arms |
| 8 | `(pair remaining-count nodes)` | [constructors.gamma](constructors.gamma): separate the completed field and emit remaining product fields |

Calls and constructor fields advance left to right. A constructor's product
closes share a bottom closing frame instead of accumulating recursive emitter
calls. Checked arithmetic binds each operand once before its guard. Ordinary
`if` retains its Gamma form; let bodies and final exhaustive match arms remain
in their established generated tail positions.

## Match ownership

[matches.gamma](matches.gamma) coordinates subject completion and arm entry.
Its subordinate files separate:

- [wrappers.gamma](matches/wrappers.gamma): hygienic subject, tag, and payload
  bindings and conditional prefixes;
- [arms.gamma](matches/arms.gamma): authored arm order, tag comparisons, the
  final exhaustive fallback, and continuation payloads;
- [payloads.gamma](matches/payloads.gamma): retained pattern binders and ordered
  product projections before each arm body.

The subject is emitted once. One closing frame owns the match wrappers and
remaining conditional closes; pattern-binding closes have their own counted
frame. Arm bodies enter the shared expression machine, so a nested match cannot
overwrite an outer arm continuation. Helpers consume already-checked
constructor identity and payload counts, not a new semantic decision.

## Validation and remaining boundaries

The staged Delta gate compares existing source/receipt pairs, executes generated
programs, and exercises former compiler-stack failures with nested expressions
and wide payloads. Exact Epsilon checking and execution receipts are separate
full-customer reconstruction gates. A changed receipt must be explained, not
accepted merely because a traversal was reorganized.

The emitter no longer needs one Gamma call context per source expression level.
Its continuation pairs still consume the selected evaluator's finite immutable
arena. This does not establish every compiler resource preflight or canonical
`InternalFailure` publication.

Compiler traversal and generated-program acceptance are different bounds. The
[selected Gamma profile](../../../../gamma/EVALUATOR_PROFILE.md#exact-capacities)
admits at most 255 nested expression lists in each emitted function body.
Inline checked arithmetic, nested lets, constructor products, and match
wrappers can increase generated nesting. D30's 1,024-level Delta `parse_depth`
profile therefore does not establish successful generated Gamma admission or
execution throughout that depth. Stack-safe compiler traversal does not close
that lowering/profile obligation or the full Delta bootstrap edge.
