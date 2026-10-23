# Arena metadata research — 2026-10-22

The layer-0 inventory and frozen forest census were consulted before searching. The census found 6,595 nonempty generation and occupancy vectors and 265,213 rows of each in the successful original-corpus forests, all uniform and occupied. Logical lengths are not allocator capacities or RSS.

Query: `site.github.com generational arena homogeneous generation lazy metadata append only arena slots`.

Read the actual implementations:
- [id-arena](https://raw.githubusercontent.com/fitzgen/id-arena/master/src/lib.rs): deletion-free arenas use arena identity plus a vector index. This supports the uniform-state hypothesis, but cannot replace Omega's retirement behavior.
- [generational-arena](https://raw.githubusercontent.com/fitzgen/generational-arena/master/src/lib.rs): reusable slots retain generation to prevent stale indices selecting new occupants. That semantic distinction is required after deletion.

No direct adaptive-metadata implementation was found. Sparse/bitset designs address holes; the measured syntax forests have none. The experiment therefore retains uniform metadata at the owner until retirement, rather than adopting a sparse container.

The measured alternatives keep inline metadata headers or move them to a cold owner. First-retirement latency, complete histories, clone/unwind behavior, exact parser/custody observations and actual layout are compared in [the result](README.md). The cold-owner clear-loop refinement follows a demonstrated regression. Raw draft chronology and rejected/stale captures remain in the experiment archive.
