# Terminal production

Public contract: [Terminal Psi product](../../../../wiki/spec/terminal-psi/product.md).
This crate sequences production; it does not own another executable IR.

Enter [production.rs](src/production.rs). The sequence is
`checked-trees-to-lowered-psi` -> `lowered-psi-to-lowered-psi` ->
`lowered-psi-to-terminal-psi`. `LoweredPsi` belongs to representations.
Publication consumes the validated optimization-stage result, not an unoptimized
producer-private shortcut.

The product keeps checked boundary-operator application scope and selected
floating-point occurrences beside the canonical artifact. Callback custody is
an opaque owned sidecar: carrying it does not interpret placement or grant
registration, invocation, address, or lifetime authority. Native realization
must rejoin the exact source/build receipts under its own authority.

## Boundary source custody at the compiler handoff

Compiler Terminal handoffs restore boundary requirement calls through
`CheckedCompilation::terminal_production_trees`. The existing inverse-edit guards
validate selected adapter identities and operand graphs before restoration;
checked plans remain unchanged. Adapter edits are retained separately from
operator/FMA settlement because those transformations carry their own checked
plans. Full package source queries undo both batches in reverse settlement order.
This is a transitional handoff for the selected-dispatch tree, not permission to
serialize a selected provider as a source call or to weaken call-source custody.

## Source byte-view lowering

The [byte-view contract](../../../../wiki/spec/terminal-psi/byte_views.md)
is independent of the producer's indexing scheme. The source lowering keeps:

- Whole-parameter indexed reads as an empty projection path, distinct from a
  nominal field's nonempty path.
- Call-range endpoint facts keyed by state, statement, call ordinal, and dense
  structural argument ordinal.
- State-edge endpoint facts keyed by state, transition statement, and authored
  target argument position, not a dense structural ordinal.
- Length observations scoped to the selected path, restoring the incoming set
  before lowering a sibling.

The emitter must rejoin those coordinates to the exact expression and builtin
range before producing obligations. Helpers and trust-source identities must
follow that reconstruction; an observed length is not a checked bound.

## Service receipts

An erased `Service<R> in Bound` parameter is not an ordinary empty record.
Its source receipt binds typed parameter symbol and authored position, normalized
carrier and qualification, exact requirement, and selected-plan digest. The
raw producer checks typed custody; final compiler admission additionally rejoins
selection provenance and an ordered call/checked-operation bijection.

Attached and free helpers keep their actual attachment shape. Free helpers do
not invent an empty receiver. Whole-Service forwarding must rejoin both endpoint
receipts rather than infer authority from zero payload. Source support for
particular forwarding/control shapes is an implementation limit, not a new
meaning for Service or an alternate Terminal namespace.
