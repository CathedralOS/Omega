# Checked Delta to expanded Gamma

Start at [program.gamma](program.gamma). It consumes the complete checked
program and resolved catalogs after selected-profile schema validation, skips
data declarations, and builds every function definition in authored order.
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

[bindings.gamma](bindings.gamma) owns the counted lexical binding spine. A let
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
receipts remain unchanged when the original bodies already fit the profile.
