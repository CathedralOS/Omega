# Build-time artifact execution

This service evaluates demanded scalar applications in actual canonical Psi
artifacts and marshals real arguments/results through the interpreter's structural
catalog. It does not check source bodies or manufacture a checked program.
Build configuration, package acquisition, and host effects remain Omega-owned.

Start at [scalar_applications.rs](src/scalar_applications.rs) for artifact
execution and [terminal_values.rs](src/terminal_values.rs) for value transport.
Canonical scalar receipts retain their artifact, target, observations and actual
work usage. Their wire coordinate remains unchanged. The old tree-evaluation
receipt coordinate is rejected instead of being interpreted as scalar evidence.

The former source evaluator, invocation admission, source proof queries,
layout/access-policy execution, checked snapshots and tree replay are deleted.
[source_frontier.rs](src/source_frontier.rs) preserves actual empty inputs and
source authority metadata; nonempty source normalization/pre-check work is
refused with `no product route yet` until a producer through the lowered-form
route serves it. Refusal does not claim that the source program is wrong.
[source_jobs.rs](src/source_jobs.rs) retains real request configuration and
explicit refusals for unavailable jobs. No interpreter default replaces a value.

[product.rs](src/product.rs) retains genuine provider, native-binding and test
registration records. Nonempty source selection/admission requests are refused;
old collect, rederive and checking algorithms are not moved into another crate.
BTE no longer directly depends on `validation` or `checked-trees`. Other source
frontend crates remain dependencies for their real input types, so this is not
a claim that the complete compiler is free of the retired checker graph yet.
