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
wrappers. The later serializer does not need Delta constructor, pattern,
arithmetic, or lexical-environment knowledge.

## Remaining boundaries

The complete plan enables a later normalization phase; no such normalization
or lifting is implied by this separation. A stack-safe compiler traversal can
still produce a body exceeding Gamma's 255-list profile. Allocated plan and
continuation pairs also consume the evaluator's finite immutable arena.
Resource/internal DCOUT closure and full generated-profile admission remain
open. Exact checking and execution receipts must remain byte-identical while
this structural dependency is introduced.
