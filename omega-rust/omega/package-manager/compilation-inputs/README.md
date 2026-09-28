# Package compilation inputs

This independent package is shared by compilation, build evaluation, realization,
and package commands. It owns their input/custody contract, not install/update
or compiler orchestration. Neither its sibling input packages nor this package
depend on the `package-manager` application or the `compiler` crate.

Start at [package_compilation.rs](src/package_compilation.rs): it constructs the
package graph, validates source roots and dependency bindings, and separates the
shared source projection from target-specific attachments.

[build_execution/build_value.rs](src/build_execution/build_value.rs) owns the
Build-value protocol: admit the exact vocabulary, initialize the activation's
facets, and decode normalized settings after interpretation. `initial` owns
private facet construction, `vocabulary` the exact declaration checks,
`optimization` counter admission/decoding, and `configuration` the resulting
application, deployment, optimization, PCC and privileged-service settings.
The compiler retains this result whole; provider/root/trust receipts remain
separate, and interpreter execution does not move into this package.

[source_imports.rs](src/source_imports.rs) discovers imports in authored order
and rejoins them to exact parsed source instances. Its `package_selection`
module owns requester-local aliases, build/product dependency scopes and
physical/generated collisions; `paths` owns candidate paths and containment;
`bindings` joins loaded declarations; `frontier` retains package and toolchain
ownership. It does not schedule parsing or checking. The compiler asks these
owners for the next source frontier and passes it through Psi's loading,
lexing and parsing stages.

[Build prelude preparation](src/source_imports/build_prelude.rs) selects the
toolchain Build source and its scoped aliases. An authored Build in the selected
build source suppresses injection; an unrelated product-local Build keeps its
own identity. The declarations come from `build-declarations`, and loading and
parsing remain explicit compiler stage calls.

[toolchain_library.rs](src/toolchain_library.rs) selects the installed or
development library and verifies release-library digests. This package's
`build.rs` records the development root or release digest for that owner.
Package import discovery never substitutes requester-local content for those
bundled toolchain sources.

[source_snapshot.rs](src/source_snapshot.rs) owns complete physical-root capture:
metadata rows, canonical modes, bounded traversal and byte-content commitments.
Input construction and final compiler evidence both revalidate captured metadata
by recapturing the root. A matching file length is not a matching snapshot.

[source_consumption.rs](src/source_consumption.rs) owns the exact source units
actually consumed by compilation and their package commitment, including generated
source custody. This is distinct from the complete root visible to build evaluation.
[production_subject.rs](src/production_subject.rs) joins that source subject to the
selected Build identity, validated execution accounting, exact observation, and
target. It owns their canonical byte order; compiler reports bind the result to
an artifact under their versioned manifest domain. No artifact producer or
compiler report type enters this package.
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
