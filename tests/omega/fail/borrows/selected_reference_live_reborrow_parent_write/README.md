The explicit output lifetime retains both immediate reborrow parents, even
when the callee only returns its first input. The other declared parent cannot
be mutated while the selected child is live.

Run `python3 tools/corpus_gate.py --filter
borrows/selected_reference_live_reborrow_parent_write` from the repository root.
