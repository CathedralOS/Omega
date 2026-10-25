# Package compilation inputs

This independent package is shared by compilation, build evaluation, realization,
and package commands. It owns their input/custody contract, not install/update
or compiler orchestration. Neither its sibling input packages nor this package
depend on the `package-manager` application or the `compiler` crate.

Start at [package_compilation.rs](src/package_compilation.rs): it constructs the
package graph, validates source roots and dependency bindings, and separates the
shared source projection from target-specific attachments.

[source_assembly.rs](src/source_assembly.rs) prepares the immutable source/parse
checkpoint and assembles one source forest for resolution. It admits a child's
source projection before attaching generated dependencies, resolves deferred
imports, and loads entry contracts and the Build prelude through Psi 00/01.
Physical discovery and exact-input continuation share one ordered loading loop.
This preparation never executes a Build machine or checks a program. Compiler
and prefix consumers use it directly, without depending on Build evaluation.
An activation's `GeneratedSourceExtension::parse` uses the same loading and
parsing stages after source admission. It rejoins the exact base before parsing
and preserves one forest per generated unit; the compiler never reopens bytes
or chooses source-instance coordinates between Build execution and resolution.

[build_execution/build_admission.rs](src/build_execution/build_admission.rs) selects
the exact source-owned Build machine, rejects runtime-service reach, and admits
its initial value and filesystem mode from the captured package scope. The compiler
sequences Psi preparation/reach inference before this decision and retains the
prepared program with its exact entry token; this owner does not execute it.

[build_execution/build_value.rs](src/build_execution/build_value.rs) owns the
Build-value protocol: admit the exact vocabulary, initialize the activation's
facets, and decode normalized settings after interpretation. `initial` owns
private facet construction, `vocabulary` the exact declaration checks,
`optimization` counter admission/decoding, and `configuration` the resulting
application, deployment, optimization and privileged-service settings.
The compiler retains this result whole; provider/root/trust receipts remain
separate, and interpreter execution does not move into this package.

[source_imports.rs](src/source_imports.rs) discovers imports in authored order
and rejoins them to exact parsed source instances. Its `package_selection`
module owns requester-local aliases, build/product dependency scopes and
physical/generated collisions; `paths` owns candidate paths and containment;
`bindings` joins loaded declarations; `frontier` retains package and toolchain
ownership. Source assembly asks these owners for the next frontier and passes
it through Psi's loading, lexing and parsing stages.

[Entry-contract seeds](src/source_imports/entry_contract_seed.rs) select every
target profile's contract source from exact package suppliers or the bundled
fallback. Source assembly schedules loading and parsing; package provenance and
accepted supplier selection stay here.

[Build prelude preparation](src/source_imports/build_prelude.rs) selects the
toolchain Build source and its scoped aliases. An authored Build in the selected
build source suppresses injection; an unrelated product-local Build keeps its
own identity. The declarations come from `build-declarations`; source assembly
feeds them through the same loading and parsing stages.

[toolchain_library.rs](src/toolchain_library.rs) selects the installed or
development library and verifies release-library digests. This package's
`build.rs` records the development root or release digest for that owner.
Package import discovery never substitutes requester-local content for those
bundled toolchain sources.

[source_snapshot.rs](src/source_snapshot.rs) owns complete physical-root capture
and adapts retained Build inventories to Psi's neutral snapshot-read protocol:
metadata rows, canonical modes, bounded traversal and byte-content commitments.
Retained acquisition storage supplies package source bytes without reopening the
root. A physical-custody adapter captures sealed roots when no retained tree is
available. A matching file length is not a matching snapshot.

[source_consumption.rs](src/source_consumption.rs) owns the consumed-source package
commitment and generated-source custody. Exact source-unit identity and canonical
encoding live in the shared [source foundation](../../../psi/foundation/source/src/source_identity.rs),
so early Psi jobs and package review use the same protocol without a dependency
from Psi to package management. This is distinct from the complete root visible
to build evaluation.
[BuildSourceCustody](src/source_consumption/checked_sources.rs) admits the current
activation's generated extension: package context, exact root and source coordinates,
checked source counts, and retained generated-output custody. Empty output needs no
package context; rejection leaves prior custody unchanged. The compiler then schedules
parsing, resolution and typing and returns the completed program for source sealing.
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
physical output root-overlap checks and memory input-root issuance; observations retain
their canonical identity independently of their compiler consumer.
The [completed output owner](src/build_execution/completed_outputs.rs) selects
only settled required files, commits their manifest, and publishes or verifies
their exact content-addressed set. Publication custody is recorded only after
the files and manifest succeed; compiler reports cannot assert a directory on
the output owner's behalf. Scratch files and generated source are not products.
See [observation custody](observation_custody.md) for the retained contract.
