# Captured build sources and outputs

Start at [staged_output_tree.rs](src/staged_output_tree.rs) for the kept output
tree, its sealed entries and its canonical commitment.
[capture](src/capture.rs) builds the tree from authorized filesystem access;
[materialization](src/materialization.rs) writes and verifies it.
[captured_source](src/captured_source/mod.rs) keeps immutable source snapshots,
and [tree_from_entries.rs](src/tree_from_entries.rs) validates explicit entries.

Compilation and package commands share these custody records. This independent
package does not depend on either application, run build code, or grant
filesystem authority. Captures use the existing `FilesystemSponsor` and
canonical metadata vocabulary; moving the owner here did not replace that
protocol or make captured bytes admitted source.
