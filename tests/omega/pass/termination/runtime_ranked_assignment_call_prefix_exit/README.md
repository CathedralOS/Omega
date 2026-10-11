# Ranked prefixes use complete operand frames

The countdown writes the caller's scratch fields using nested selected calls,
evaluates a call in an indexed store target, explicitly discards a call result,
and calls through a short-circuit guard. Each complete write frame misses the
ranked `remaining` parameter. Execution must update only the selected scratch
element, observe the final zero, and return normally with exit 0.

The prior compiler admitted value-producing calls in local initializers, but
moving the same readonly call into an assignment rejected the rank.
One prefix frame check now covers direct writes and all operand evaluations;
only call-free evaluations contribute arithmetic context. A call preserving a
fact does not establish a fact about its result.

`fail/termination/ranked_assignment_call_prefix_write` resets the rank inside a
nested assignment operand. `ranked_index_call_prefix_write` hides the same write
inside the store's index. Both must reject the ranking even though the final
store targets unrelated memory. The selected-requirement prefix controls retain
their ordinary call-site contract and opaque-write protections.

```sh
python3 tools/corpus_gate.py --filter termination/runtime_ranked_assignment_call_prefix_exit,termination/ranked_assignment_call_prefix_write,termination/ranked_index_call_prefix_write,termination/selected_requirement_rank_,termination/runtime_selected_requirement_rank_
python3 tools/corpus_gate.py --both --filter termination/runtime_ranked_assignment_call_prefix_exit
omega run tests/omega/pass/termination/runtime_ranked_assignment_call_prefix_exit/main.omg
```
