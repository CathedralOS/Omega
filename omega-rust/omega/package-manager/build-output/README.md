# Captured build sources and outputs

Start at [staged_output_tree.rs](src/staged_output_tree.rs) for the retained output
tree, sealed entries and canonical commitment.
[capture](src/capture/mod.rs) constructs it from authorized filesystem access;
[materialization](src/materialization/mod.rs) writes and verifies it.
[captured_source](src/captured_source/mod.rs) retains immutable source snapshots,
while [tree_from_entries.rs](src/tree_from_entries.rs) validates explicit entries.

Compilation and package commands share these custody records. This independent
package does not depend on either application, execute build code, or grant
filesystem authority. Captures use the checked interpreter's existing filesystem
sponsor and canonical metadata vocabulary; moving the owner does not replace
that protocol or turn captured bytes into source admission.
