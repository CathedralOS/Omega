Two pure identity calls with the same unknown scalar input construct equal
Pair fields. The source requirement is valid; both execution engines must
finish with exit 0.

After package preparation, run:

```text
omega run --both tests/omega/pending/calls/runtime_constructor_child_pure_calls_exit/main.omg
```

The child-capture candidate accepts source and the checked interpreter exits 0.
Native production now retains the owned-field requirement but cannot produce its
CallRequires proof: both fields link to their exact call results, and neither
result carries a published equality to the common input.

Terminal call composition imports only the callee's explicit ensures; identity
has none. Preserve a general checked return summary or validated pure-call
congruence through the existing contract/proof boundary. A source eligibility
check alone is not a Terminal certificate. Do not add an ensures clause to this
fixture, assume equality from repeated syntax, or increase the proof budget.
