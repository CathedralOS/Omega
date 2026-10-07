The authored result is exit 0 in native and Terminal execution:
`python3 tools/corpus_gate.py --both --filter runtime_nested_claim_bearing_temporary_cleanup_exit`.

The constructor moves two linear receipts and an affine stamp through nested
owners. Receipt claims retain their identities and acquire the destination's
typed paths; the stamp remains the disjoint affine complement. The final
borrow observes all three values, then normal cleanup retires their original
obligations. The stamp hook's prerequisite must already be established from
its constructor, not bypassed by native lowering.

Both branches run: the false branch must create no temporary or obligation;
the true branch must borrow and finish the complete owner. This exercises the
shared constructor transaction, not a fixture-specific lowering path.
