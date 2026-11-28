# Dijkstra over a binary min-heap

Single-source shortest paths on a fixed 12-node weighted digraph held in
CSR form (`off`/`dst`/`wgt`), using a generic fixed-capacity binary
min-heap parameterized by an ordering machine.

`Heap<T>` stores sixteen cells. `push_heap<T, Less>` and
`pop_heap<T, Less>` take the ordering as a compile-time parameter,
`const Less: HeapOrder<T>`. Both operations sift through state loops.
Entries pack `dist * 256 + node` into a `u64`, so `ent_less` is ordinary
comparison ordered distance-major with node as tiebreak. `pop_heap`
delivers the minimum through a `Popped<T>` out-record: a bare generic
element borrowed out of storage has no lowered loan carrier yet, so the
result leaves through a concrete record field instead.

The driver seeds the source entry, then pops and relaxes until the heap
drains; stale entries die on the `nd <= dist[node]` guard. The oracle
recomputes the distance sum (150) and walks the `prev` chain from node 11
(`11 <- 4 <- 3 <- 1 <- 0`, four hops), exiting 154 on agreement.

`../invalid_dijkstra_binary_heap/` calls `pop_heap` on a heap that was never proven non-empty —
the `requires h.length >= 1` contract must reject it.
