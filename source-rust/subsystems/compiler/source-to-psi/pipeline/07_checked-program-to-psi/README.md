# 07 - Checked program to Psi

Public contract: [Terminal Psi product](../../../../../../wiki/spec/terminal-psi/product.md).

The [source driver](../../driver/judge_lowered_source.rs) lowers the frozen
source, then passes that input to the lowered-form checker. The
[accepted-source factory](src/production/accepted.rs) receives the verdicts and source
correspondence of that one compilation. It publishes the module and its evidence, then binds the emitted Build request
rows and source-record correspondence to that code owner.
Its structural reading refuses unserved selection, query, native-formation and
filesystem constructs before publication loses their meaning.

This factory does not supply missing source preparation or Build evaluation.
Nonempty stage 04 preparation and other unserved authority requests still
refuse with `no product route yet`. Publishing a separately served typed
frontier gives no default-Build or native capability beyond that product.

A Product can select `SparseConditionalConstantPropagation` through its ordinary
Build. The pass joins constants at executable predecessors and folds pure
Boolean negations and Boolean/integer comparisons. The lowered-form checker
judges the original whole program and the transformed one independently.
The portable artifact names the executed selection and both semantic identities.
An unimplemented selection refuses publication.

The old checked-source and Build-provider production request interfaces and the
checked-provider inventory reexport are removed, as are their lowering stage
and the Lowered Psi, optimization and publication wrappers. Missing
capabilities belong on the lowered-form route; no old checker supplies a
fallback artifact.

## Code map

[src/lib.rs](src/lib.rs) remains the public entrance. Its reexports keep callers
independent of these private implementation groups:

| Group | Responsibility |
| --- | --- |
| [Production](src/production/mod.rs) | Receive the checked source judgment, publish its code and evidence, retain custody, and authenticate compile-time invocation sites. Start at [accepted.rs](src/production/accepted.rs). |
| [Build](src/build/mod.rs) | Publish companion rows for accepted Build calls and identify the checked host receiver. [bindings.rs](src/build/bindings.rs) defines the retained records; [companions.rs](src/build/companions.rs) fills them. |
| [Source](src/source/mod.rs) | Retain source selections, publish one shared source artifact, issue entry metadata, and record the source's numeric review requirements. |
| [Provider supply](src/provider_supply/mod.rs) | Close and publish the checked demand for a deployment target using the existing [provider choice inputs](src/provider_choice_inputs.rs). |

Selection checking and publication use the same source occurrences from the accepted judgment.
The factory groups folded calls by emitted machine while preserving their order within each machine.
It also orders machine selections by operation so publication can look up the exact source expression.

For native compilation, stage 07 publishes the checked provider demand and completed Build bindings for the exact Psi manifest and deployment target. Native stage 00 verifies both before consuming the record. [Published target providers](../../representations/published-target-providers/publication.rs)

Source publication binds no application roots and issues no entry receipt.
Native preparation [attaches application roots](../../../psi/artifact/codec/src/entry_receipt/application.rs)
and checks the [entry receipt](../../../psi/artifact/codec/src/entry_receipt.rs)
against its own execution view.
