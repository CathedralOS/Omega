# Indexed assignment retains its selected destination

The signed index is checked before an RHS call changes its source. The store
must use the captured coordinate, and its implicit count conversion must consume
the exact checked index decision rather than searching again after the write.
Bounds evidence does not authorize an element value that violates a domain.

`omega run --both tests/omega/pass/collections/runtime_indexed_assignment_selector_snapshot_exit/main.omg`
must execute with exit 0 in native and checked interpretation.

`python3 tools/corpus_gate.py --both --filter collections/runtime_indexed_assignment_selector_snapshot_exit`
must execute with exit 0 in native and Terminal interpretation.
