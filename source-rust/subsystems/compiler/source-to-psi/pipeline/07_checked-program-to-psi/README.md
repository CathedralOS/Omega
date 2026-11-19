# 07 - Checked program to Psi

Public contract: [Terminal Psi product](../../../../wiki/spec/terminal-psi/product.md).

The [source driver](../../product/source_judgment.rs) lowers the frozen
source, then passes that input to the lowered-form checker. The
[accepted-source factory](src/accepted.rs) receives the verdicts and source
correspondence of that one compilation. It publishes the module and its evidence, then binds the emitted Build request
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

The [entry receipt](../../../psi/codec/src/entry_receipt.rs) binds the selected
Unit declaration to the canonical artifact. Both source publication and native
preparation [attach application roots](../../../psi/codec/src/entry_receipt/application.rs)
to the current execution view. [Source publication](src/application.rs) records
the receipt check as a separate compilation stage when an entry was requested.
