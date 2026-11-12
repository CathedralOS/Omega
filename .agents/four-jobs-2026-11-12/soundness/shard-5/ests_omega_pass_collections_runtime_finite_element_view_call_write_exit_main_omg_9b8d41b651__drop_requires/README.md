Run `omega run --both tests/omega/pass/collections/runtime_finite_element_view_call_write_exit/main.omg` from the repository root; native and checked execution must exit 0.

For the shared Terminal engine, run `python3 tools/corpus_gate.py --both --golden tests/omega/corpus_outcomes.txt --filter runtime_finite_element_view_call_write_exit`.
Runtime facts differ from the source-only golden; both engines must nevertheless
meet the authored exit-0 oracle.

The destination index must be captured before the result-producing call changes
the cursor. The helper's finite-result contract supplies the receiving field's
value fact; it does not change the float's storage carrier or create a separate
element-store path. The neighboring cell and sibling field stay unchanged, and
the result-producing call executes once.
