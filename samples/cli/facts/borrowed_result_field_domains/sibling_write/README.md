# Returned field domain after a disjoint sibling write

Borrowed results retain their checked field domains and selected element, so inspection reads that element.

Parent: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, preservation of field propositions
through current-storage writes; this is another production of the existing
borrowed-result fact, whose original parent is **NESTED-MACHINE-UNIFICATION**.
The fixed baseline is c0f864f5127636dcd5b80e7f63a2aab4ea6ded78.

The caller invokes the same unknown-index choose/relay chain for both elements.
A write to selected.stamp is disjoint from selected.storage's standing Ascii
predicate. inspect consumes that field after the store, with no reconstruction
or equality guard. The twin adds only an overlapping storage-byte write of255
before inspect; its exact authored diagnostic will be pinned from observation.

The runtime oracle reads both selected bytes, the first selected stamp and its
untouched neighbor before the second call, then both final stamps and all eight
unchanged storage bytes. Omitted, misplaced and wrong-index stores and wrong
selection must trap. Both four-byte payloads keep the existing bounds and fact.

[Local](../local/sibling_write/main.omg) and
[twin](../local/sibling_write/invalid/main.omg) are frozen before baseline.
Current source/native outcomes, other places and pins remain unverified.
