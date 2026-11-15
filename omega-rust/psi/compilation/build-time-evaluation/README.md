# Build-time artifact execution

This service runs requested scalar applications in real canonical Psi
artifacts and passes arguments and results through the interpreter's
structural catalog. It does not check source bodies
or build a checked program. Omega owns Build configuration, package
acquisition and host effects.

Start at [scalar_applications.rs](src/scalar_applications.rs) for running
artifacts and [terminal_values.rs](src/terminal_values.rs) for passing values.
A canonical scalar receipt records its artifact, target, observations and
actual work used. Its wire coordinate is unchanged. A receipt with the old
tree-evaluation coordinate is rejected rather than read as scalar evidence.

The old source evaluator, invocation admission, source proof queries,
layout/access-policy execution, checked snapshots and tree replay are deleted.
[source_frontier.rs](src/source_frontier.rs) keeps real empty inputs and the
source authority metadata. A request to normalize or pre-check nonempty source
is refused with `no product route yet` until something on the lowered-form
route produces it. The refusal does not say the source program is wrong.

A program activated from a retained base answers only the requests of the
declarations it keeps; the base's other sources stay with the base.
Declaring a constant is not a request. A reference to a constant from another
declaration is a request only when no stage recorded the constant's value; the
compiler refuses that reference by the constant's name and never reads its
initializer in place of the value.
[source_jobs.rs](src/source_jobs.rs) keeps the real request configuration and
refuses unavailable jobs explicitly. No interpreter default replaces a value.

[product.rs](src/product.rs) keeps the real provider, native-binding and test
registration records. It refuses nonempty source selection and admission
requests; the old collect, rederive and checking algorithms were not moved to
another crate. This crate no longer depends on the deleted `validation` crate.
It still depends on the source frontend crates for their input types.
