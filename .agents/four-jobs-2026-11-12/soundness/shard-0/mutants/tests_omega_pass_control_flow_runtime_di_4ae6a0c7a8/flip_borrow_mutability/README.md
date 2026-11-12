Each authored transition block is total independently of adjacent dispatches
or crashes. Scalar, generic, case, Boolean-tuple and computed-subject paths complete
with their selected result. The following effectful dispatch must not mutate
the caller before the preceding block returns. A reference return retains the
caller backing while its other edge crosses two internal states to a crash.
A valid dead result after that crash must not become a normal reference origin. Empty streams and exit zero are
required natively on Linux x86-64 and for each declared Terminal target.

Run source admission with `python tools/corpus_gate.py --filter dispatch_boundary`.
Run execution with `python tools/corpus_gate.py --both --filter dispatch_boundary,calls/runtime_guarded_reference_return`.
The matching `fail/control_flow/dispatch_boundary_*` fixtures require coverage
rejection with crashes, separated statements, complementary partial blocks,
a wildcard block, ordinary results, other states and generic/computed subjects.

A dead suffix with an invalid type still rejects, as does a potentially normal
sibling returning activation-local storage; execution projection creates no
source-admission or reference-origin exception.
