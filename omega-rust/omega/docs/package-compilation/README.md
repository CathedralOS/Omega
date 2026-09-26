# Package compilation inputs

Start at [package_compilation.rs](../../src/package_compilation/package_compilation.rs): it constructs the
package graph, validates source roots and dependency bindings, and separates the
shared source projection from target-specific attachments.

[source_snapshot.rs](../../src/package_compilation/source_snapshot.rs) owns complete physical-root capture:
metadata rows, canonical modes, bounded traversal and byte-content commitments.
Input construction and final compiler evidence both revalidate captured metadata
by recapturing the root. A matching file length is not a matching snapshot.

[source_consumption.rs](../../src/package_compilation/source_consumption.rs) owns the exact source units
actually consumed by compilation and their package commitment, including generated
source custody. This is distinct from the complete root visible to build evaluation.
[semantic_bindings.rs](../../src/package_compilation/semantic_bindings.rs) owns accepted nominal identities
for receiving-policy services.
