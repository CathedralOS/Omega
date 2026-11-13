# Retired typed preparation

The old typed-source preparation algorithms and their source-checker connections
have been removed. [preparation.rs](src/preparation.rs) preserves the public
request shapes while refusing nonempty source preparation and all Product
operand preparation with `no product route yet`. No checking algorithm moved
to another owner. These interfaces publish no checked facts or proof receipts.

`PreparationRequest` and `PreparationAdmission` retain only actual requested
pending-input policy, assembly grants, deferred layout applications, opaque
representation selections and wire demands. `TypedPreparation` is a compatibility
name for that input metadata, with no judgment extraction API. Assembly grants
and opaque selections use the surviving language-core and typed-tree data owners.

A source input equal to the default semantic program, with no requested layout,
opaque selection or wire work, can move unchanged into `FrozenTypedTrees`. This
is a no-work result, not acceptance; the original program identity is preserved.
Retained data outside the root lists also makes an input nonempty. Constant and
execution preparation use the same restriction. Copied-source restoration is
only a no-op when the actual expression list is empty.

Programs that need specialization, source proof or generic checking, constant
evaluation, conformance elaboration, operator binding, assembly normalization,
record-pattern completion or Product formation now require a genuine producer
in the [structural route](../structure-elaboration/README.md). Until that work
is served, ordinary source and Build compilation may be unavailable. There is
no fallback through the retired source checker.
