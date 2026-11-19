# Terminal production

Public contract: [Terminal Psi product](../../../../wiki/spec/terminal-psi/product.md).

The [accepted-source factory](src/accepted.rs) keeps the exact frozen typed
owner before the lowered-form checker judges the whole compilation. It
publishes that module and its evidence, then binds the emitted Build request
rows, source-record correspondence and application roots to that code owner.
Its structural reading refuses unserved selection, query, native-formation and
filesystem constructs before publication loses their meaning.

This factory does not supply missing source preparation or Build evaluation.
Nonempty stage 04 preparation and other unserved authority requests still
refuse with `no product route yet`. Publishing a separately served typed
frontier gives no default-Build or native capability beyond that product.

The old checked-source and Build-provider production request interfaces and the
checked-provider inventory reexport are removed, as are their lowering stage
and the Lowered Psi, optimization and publication wrappers. Missing
capabilities belong on the lowered-form route; no old checker supplies a
fallback artifact.

[src/lib.rs](src/lib.rs) exposes the production and application contracts.
[custody.rs](src/custody.rs) keeps code, evidence, companions and source
correspondence for portable reuse and invocation-local production.
Companion records are in [build_bindings.rs](src/build_bindings.rs).

The [codec entry-receipt owner](../representations/terminal-psi/codec/src/entry_receipt.rs)
checks a published artifact's unique Unit entry and binds its exact semantic
identity. Application attachment and later reuse use the
same receipt without reopening source trees.
