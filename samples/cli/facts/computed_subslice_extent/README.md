# Computed subslice extent from a predicate

Given `can_window(items, low) == true`, the captured view
`items[low + 0 .. low + 2]` has extent 2, and reading offset 1 observes original
backing `items[low + 1]`.

`can_window` is an ordinary pure total predicate whose checked body states
`items.len == 4 && low <= 2`. Its name alone grants nothing. Its exact
application supplies the premises for the checked endpoint arithmetic and the
independent ordered range-formation obligation. The subsequent offset-1 read
still owes its own bound against the resulting descriptor. The target-family
head retains the whole predicate requirement: publishing interval observations
requires checked equivalence in both directions, rather than only necessary
consequences. Disconnected truth sets cannot be replaced by their interval hull.

The nine places are `local`, `field`, `parameter`, `call_result`, `match_join`,
`const_name`, `other_state`, `other_machine`, and `target_body`. Each retains
that same fact, bound and read. Every runnable entry checks the view's measured
extent and reads from `[11, 23, 37, 53]` at both `low = 1` and the allowed
endpoint `low = 2`, expecting 37 and 53 respectively. The match place executes
both alternatives at both values. All four target bodies are checked; native
execution observes Linux x86-64 on this host.

Each place's `controls/insufficient_bound` changes only the predicate's bound
from `low <= 2` to `low <= 3`. The original range and read remain intact. The
twin must reject the range call's `end <= items.len` obligation, because the
weaker premise permits an endpoint of 5 in backing of extent 4.

Current-main baseline `166de40fb0` rejects the frozen predicate positive at
`low + 2` as an unproved exact-u64 addition. Direct conjunct preconditions
already pass and execute in both native and Terminal engines. This fact covers
the callee-predicate premise gap under **BORROW-PROOF-CONVERGENCE**; other
predicate, runtime-projection and theorem-premise requirements remain open.

```sh
python3 tools/sample_native_gate.py --filter cli/facts/computed_subslice_extent
python3 tools/corpus_gate.py --both --filter computed_subslice_extent
```

Authority comes from [state contracts](../../../../wiki/spec/language/state_contracts.md),
[checked predicate consequences](../../../../wiki/spec/language/dependent_values.md#collection-invariants),
[exact subslice descriptors](../../../../wiki/spec/terminal-psi/byte_views.md), and
[live fact identity](../../../../wiki/spec/terminal-psi/fact_validity.md).
