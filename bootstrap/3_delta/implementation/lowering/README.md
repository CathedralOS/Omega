# Checked Delta to expanded Gamma

Start at [program.gamma](program.gamma). It consumes the complete checked
program and resolved catalogs after selected-profile schema validation and
builds every function definition in authored order. Data declarations select
whether one shared projection definition is needed; they emit no Delta types.
No lowering helper publishes receipt bytes. Its result is the complete
[expanded Gamma program](../representation/README.md).

[expressions.gamma](expressions.gamma) owns the shared visit/resume machine.
Concept-owned helpers handle bindings, applications, constructors, checked
arithmetic, and matches. They consume completed grammar, name, arity, and type
judgments rather than deciding those judgments again.

## Continuations and binding custody

The common operations are:

```text
lowering_visit(node, locals, depth, frames, globals)
lowering_resume(value, depth, frames, globals)
frame = (pair kind (pair payload previous))
```

The resumed value is an expanded Gamma expression. Frame depth counts pending
compiler work, not source depth or generated expression height. Resume pops
exactly one frame before dispatch; handlers receive the decreased depth and
previous stack. Payload owners retain completed child values and the lexical
environment needed for the next child. Counts govern pair-spine projections.

[bindings.gamma](bindings.gamma) reuses the existing exact-name trie, storing
original Gamma binding atoms rather than types. Immutable snapshots preserve
lexical scopes; only parameters retain a separate declaration-order spine. A let
initializer is lowered in its outer environment; only the body receives the
new source binding atom. Parameter and pattern references likewise reuse their
established atoms. Looking up that custody does not introduce a second conflict
or type policy after checking.

## Concept ownership

Application arguments and constructor fields are lowered left to right before
their enclosing Gamma expression is built. Checked arithmetic binds each
operand once before constructing its existing overflow guard; generated marker
and coordinate identities remain shared by the guard's references.

[matches.gamma](matches.gamma) coordinates the subject and arm bodies.
[matches/arms.gamma](matches/arms.gamma) owns authored arm order, tag selection,
and the final exhaustive fallback.
[matches/bindings.gamma](matches/bindings.gamma) owns pattern binding and
payload projections. The completed subject appears once under its generated
binding; arm bodies use the common continuation machine. Constructor metadata
supplies checked representation and tags.

### Wide payload projections

Patterns with at most 256 fields keep their existing inline projection chains;
each individual chain then has at most 255 Gamma expression lists. Wider
patterns call one ordinary generated Gamma function, `$dp(payload, index)`,
which tail-recursively takes `second` until the index reaches zero. Nonfinal
fields take `first` of that tail; the final field is the unwrapped tail itself.
Indices are generated in `0..arity-1`, so each reached `second` has a pair
under the already-checked product representation. The helper neither allocates
product nodes nor changes field order, provenance, or authored trap behavior.
Its recursive call is proper-tail, but each initializer's initial call consumes
one ordinary runtime context; this is not a promise of unchanged exhaustion
thresholds for arbitrary application stacks.

The checked data-before-functions order lets `program.gamma` install `$dp` once
when the first constructor wider than 256 fields is encountered. During that
prefix the definition count is zero or one. Later wide constructors reuse it,
even across nominal owners; a wide declaration without a matching use retains
the same small definition instead of requiring another usage-analysis pass.
The helper's two parameters and height-three body are ordinary Gamma structure.
Its reserved name cannot collide with renamed authored functions or generated
`$hN` extraction names. It is not a new primitive, adapter, or language rung.

This avoids rebuilding and printing quadratic projection chains for wide
patterns. It does not change the product layout or make evaluation of all `k`
fields subquadratic: the runtime still walks each requested prefix. Enclosing
binding chains and wide constructor products still undergo normal height
normalization. Programs whose constructors all fit 256 fields retain their
previous plan and receipt bytes.

Calls, lets, products, and guards are ordinary Gamma plan nodes. Their
constructors compute expanded expression-list heights, including generated
wrappers, and cache exact serialization extents through the serializer's
count-only formatting helpers. The later serializer does not need Delta constructor, pattern,
arithmetic, or lexical-environment knowledge.

## Remaining boundaries

This phase deliberately returns the expanded, pre-normalization plan. The
private lowering-height diagnostic observes those original heights, including
heights above 255. Normal compilation passes the plan to
[normalization](../normalization/README.md) before publication; that separate
owner handles body nesting without changing Delta lowering rules.

Allocated plan and continuation pairs consume the evaluator's finite immutable
arena. Resource/internal DCOUT closure and full generated-profile admission
remain open. Body-height normalization does not by itself bound helper count,
live runtime contexts, or cumulative storage. Exact checking and execution
receipts remain explicit regression obligations.
