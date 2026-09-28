# Package compilation inputs

This independent package is shared by compilation, build evaluation, realization,
and package commands. It owns their input/custody contract, not install/update
or compiler orchestration. Neither its sibling input packages nor this package
depend on the `package-manager` application or the `compiler` crate.

Start at [package_compilation.rs](src/package_compilation.rs): it constructs the
package graph, validates source roots and dependency bindings, and separates the
shared source projection from target-specific attachments.

[source_snapshot.rs](src/source_snapshot.rs) owns complete physical-root capture:
metadata rows, canonical modes, bounded traversal and byte-content commitments.
Input construction and final compiler evidence both revalidate captured metadata
by recapturing the root. A matching file length is not a matching snapshot.

[source_consumption.rs](src/source_consumption.rs) owns the exact source units
actually consumed by compilation and their package commitment, including generated
source custody. This is distinct from the complete root visible to build evaluation.
[semantic_bindings.rs](src/semantic_bindings.rs) owns accepted nominal identities
for receiving-policy services.

[build_execution.rs](src/build_execution.rs) owns build filesystem custody:
preparing immutable source and named-input roots, projecting interpreter
observations, settling sealed-output obligations, and selecting generated source.
Capture precedes validation of the returned Build value; output settlement
follows it. No partially settled observation is published, and the evaluator
does not reconstruct the evidence fields. The subordinate filesystem scope owns
root-overlap checks, snapshot materialization and release; observations retain
their canonical identity independently of their compiler consumer.
The [completed output owner](src/build_execution/completed_outputs.rs) selects
only settled required files, commits their manifest, and publishes or verifies
their exact content-addressed set. Publication custody is recorded only after
the files and manifest succeed; compiler reports cannot assert a directory on
the output owner's behalf. Scratch files and generated source are not products.
See [observation custody](observation_custody.md) for the retained contract.
