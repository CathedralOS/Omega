# Reference observation controls

These are compositions inside the parent fact, not another nine-place matrix.

| Program | Required behavior |
| --- | --- |
| [Descriptor snapshot](descriptor_snapshot/main.omg) | Read a bounded scalar, replace only its local reference binding, and retain the saved scalar's bound. Check the replacement is observed at runtime. |
| [Descriptor rebound](descriptor_rebound/main.omg) | Replace the binding before the read; the unconstrained replacement cannot inherit the old field predicate. Reject the exact addition. |
| [Wrong field](wrong_field/main.omg) | Borrow the unrestricted marker beside the bounded reference field. Reject the exact addition. |
| [Disjoint local write](disjoint_local_write/main.omg) | Perform a provably disjoint borrowed write inside the consuming body, then read the bounded field. Check the marker became 41. |
| [Unbounded match arm](unbounded_match_arm/main.omg) | Select between the bounded field and an unrestricted reference. Reject the exact addition despite the bounded alternative. |

All positives retain the 60/-17 inputs and 130/53 oracles. The public bodies
cannot rely on those entry literals. Source copies are pinned in the corpus.
