A nested child does not detach either possible parent of its selected source.
Writing the unselected parent while the nested child remains live must reject.

Run `python3 tools/corpus_gate.py --filter
borrows/selected_reference_live_nested_parent_write`.
