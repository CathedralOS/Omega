# Borrowed field domain after an indexed sibling write

Borrowed results retain their checked field domains and selected element, so inspection reads that element.

Owning task: CANONICALIZE-SCALAR-RANGE-CONTRACTS, field propositions through
nonliteral/indexed writes with current storage versions. This is another
production in the existing borrowed-result fact, preserving its Ascii[4],
two-cell bound and inspect operation. A nonliteral index now selects one of
two unqualified sibling stamps before inspect consumes the storage predicate.
The indexing operation must carry the completed collection's entire possible
storage footprint rather than lose it when exact referents are unavailable.

[Local positive](../local/indexed_sibling_write/main.omg) and
[twin](../local/indexed_sibling_write/invalid/main.omg) are authored before the
baseline run. The twin adds only the overlapping storage-byte write of255.
Main invokes both indices, checks all four stamp cells (including every
untouched cell after the first call), both returned bytes and all eight payload
bytes. Missing, wrong-index, wrong-element and overly wide stores must trap.

Fixed base:861910090c171e544f99a7a0605c34bd49f50dad. Baseline, native/Terminal
execution, remaining places and pins are pending; this file claims no acceptance.
