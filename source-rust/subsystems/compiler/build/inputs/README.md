# Package compilation inputs

This package decides which inputs a compilation uses and records where each
came from. Compilation, build evaluation, realization and the package commands
share it; it does not install packages or run the compiler. Neither this package nor its sibling input
packages depend on the `package-manager` application or the `compiler` crate.

Start at [package.rs](package.rs): it builds the
package graph, checks source roots and dependency bindings, and separates
shared source from target-specific attachments.

[Source assembly](source/assembly/mod.rs) builds one parsed forest from the captured files, scoped imports and generated sources. [The checkpoint](source/assembly/checkpoint.rs) keeps Product roots deferred until the Build settles. [Resumed assembly](source/assembly/resumed.rs) reselects those same captured sources for a Build continuation or a declaration query. The query returns the source needed to describe Product declarations. The [fixed Build imports migration](../../../../../wiki/spec/build/scoped_execution.md#checked-build-stages) retires resumed assembly for extending an executing Build's imports.

[build_execution/build_admission.rs](build_execution/build_admission.rs)
selects the Build machine the source declares, rejects reach into runtime
services, and accepts its initial value and filesystem mode from the
captured package scope. The compiler runs Psi preparation and reach inference
first and keeps the prepared program with its entry token; this module does
not run it.

[build_execution/build_value.rs](build_execution/build_value.rs) handles
the Build value: it checks the vocabulary, initializes the activation's
facets, and decodes normalized settings after interpretation. `initial`
builds the private facets, `vocabulary` checks declarations exactly,
`optimization` accepts and decodes counters, and `configuration` produces the
application, deployment, optimization and privileged-service settings. The
compiler keeps this result whole, apart from provider, root and trust
receipts; the interpreter runs elsewhere.

[source_imports.rs](source/source_imports.rs) resolves imports by logical
declaration and module identity. `package_selection` checks requester-local
aliases and Build/Product dependency permissions. The [inventory](source/source_imports/logical.rs)
groups each package’s captured files by their parent directory relative to
the package root. Files directly under that root share the root namespace.
See [directory namespaces](../../../../../wiki/spec/language/modules.md#directory-namespaces).

Discovery runs the ordinary parser on each unit and retains its syntax page.
When admission selects the same bytes, it adopts that page under the admitted
source identity. Admission assigns the owner, scope and bindings. Discovery
excludes the Build entry and directories owned by other packages.

A selected namespace pulls in its module groups, including descendants used
by qualified references. Ordinary Product sources also pull in siblings from
their own module. Build sources discover namespaces through explicit imports.
The inventory refreshes when the captured
filesystem or excluded roots change. Discovery currently parses every
owned source unit in full before selecting groups.

`bindings` joins each authored import to one unambiguous checked-package
anchor. Psi then resolves the declaration or namespace in that package, enforcing visibility, temporal scope, overload ownership and
file-local exposure. Pulling in a module group does not import all its names.
An authored `use` never falls back to a filename; direct source inputs the
compiler issues are tracked separately.

[Entry-contract seeds](source/source_imports/entry_contract_seed.rs) choose
each target profile's contract source from exact package suppliers or the
bundled fallback. Ordinary suppliers keep their identity. The fallback
captures its root once, before source discovery, and reuses that snapshot
across Product joins and later seeds. It keeps a distinct
closed-contract source role, also recorded in source identities and stored
review/provider records, and pulls in its logical Product siblings; it does
not give core ambient visibility of std declarations. Source assembly schedules
loading and parsing; package provenance and the choice of accepted supplier
stay here.

[Build prelude preparation](source/source_imports/build_prelude.rs) selects
the toolchain Build source and its scoped aliases. An authored Build in the
selected build source turns off injection; an unrelated product-local Build
keeps its own identity. The declarations come from `build-declarations`;
source assembly feeds them through the same loading and parsing stages.

[toolchain_library.rs](source/toolchain_library.rs) selects the installed
or development library and verifies release-library digests, which this
package's `build.rs` records along with the development root. Import discovery
never replaces those bundled toolchain sources with requester-local
content.

[source_snapshot.rs](source/source_snapshot.rs) captures complete physical
roots and serves retained Build inventories through Psi's neutral
snapshot-read protocol: metadata rows, canonical modes, bounded traversal and
byte-content commitments. Retained acquisition storage supplies package
source bytes without reopening the root; otherwise a physical adapter captures
sealed roots. A matching file length is not a
matching snapshot.

[source_consumption.rs](source/source_consumption.rs) records which
package sources were consumed and tracks generated source. Exact source-unit
identity and canonical encoding live in the shared
[source foundation](../../foundation/source-provenance/source_provenance.rs), so
early Psi jobs and package review use the same protocol without Psi depending
on package management. This is not the complete root build evaluation sees.
[BuildSourceCustody](source/source_consumption_checked_sources.rs) accepts
the current activation's generated extension: package context, exact root and
source positions, checked source counts, and the record of generated output.
Empty output needs no package context; a rejection changes nothing. The
compiler then schedules parsing, resolution and typing and
returns the program for source sealing.
[Source-consumption accounting](source/source_consumption.rs) joins that source record to
the selected Build identity, validated execution accounting, exact observation,
and target. It owns their canonical byte order; compiler reports bind the
result to an artifact under their versioned manifest domain. No artifact
producer or compiler report type enters this package.
[Provider acceptance](../providers/acceptance/acceptance.rs) holds accepted nominal
identities for receiving-policy services.

[Build execution](build_execution.rs) prepares immutable input roots, records
filesystem observations and settles required files after the Build returns.
Today `Build::include_source` records the supplied filename and UTF-8 bytes
when the call runs. A dependency wait preserves those bytes. Product imports use
the recorded filename and never reopen an output file to obtain the source.
[Product source views](../../../../../wiki/spec/build/execution.md#generated-source-boundary)
replace this implementation. The migration is tracked in
[STAGED-BUILD-CAPABILITIES](../../../../../TASKS.md#immediate-product-closure).
The filesystem scope checks physical output roots for overlap.
The [completed output module](build_execution/completed_outputs.rs)
selects only settled required files, commits their manifest, and publishes or
verifies their content-addressed set. Publication is recorded only once the
files and manifest succeed; compiler reports cannot claim a directory for
it. Scratch files and generated source are not products.
See [observation custody](observation_custody.md) for the retained contract.

[Native Build output](package/native_bindings.rs) extends the existing
generated-source bundle's record of package, purpose, target, execution
profile, consumed sources and dependency closure. It carries checked source
formations and the completed native intent, including an empty roster.
Product imports consume only their relative Product outputs; a Build-role
instance cannot supply native bindings to Product. The final checked Product
resolves exact requirements once, including private ones in the supplier's
scope. These outputs are inert choices: they neither prove
foreign behavior nor grant permission to execute.
